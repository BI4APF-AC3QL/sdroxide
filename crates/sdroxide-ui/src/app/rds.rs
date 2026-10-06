//! The RDS window: what a broadcast FM station says about itself.
//!
//! Two tabs. **STATION** is the one to read — the name, the programme type, the
//! radio text, and what is playing. **DIAGNOSTICS** is the one to look at when
//! the first is empty or flickering, and answers the only question that matters
//! then: is anything actually arriving, and how much of it survives.
//!
//! Everything here is rendered from raw codes, so the RDS/RBDS selector re-labels
//! what has already been received rather than waiting for the station to send it
//! again. See [`sdroxide_types::RdsStandard`].

use eframe::egui::{self, Color32, RichText};
use sdroxide_types::{RdsData, RdsGroupLog, RdsStandard, pty_name, rt_plus_class};

use crate::app::SdroxideApp;
use crate::chrome::StyledCombo;

/// How many groups the diagnostics log keeps. About a minute at the eleven
/// groups a second the standard runs at — enough to watch a marginal station
/// come and go, short enough to stay scrollable.
pub const RDS_LOG_ROWS: usize = 600;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum RdsTab {
    #[default]
    Station,
    Diagnostics,
}

/// Field labels and anything the reader is not meant to look at first.
fn dim_ink() -> Color32 {
    crate::theme::gray(110)
}

fn dim(s: impl AsRef<str>) -> RichText {
    RichText::new(s.as_ref()).size(9.5).color(dim_ink())
}

impl SdroxideApp {
    /// Fold a snapshot from the engine into the window's state.
    ///
    /// The group log arrives as a delta with an absolute index, so a batch lost
    /// between here and the engine — a slow client, a reconnect — leaves a
    /// visible gap instead of silently closing up and making the log look
    /// continuous when it is not.
    pub(in crate::app) fn on_rds(&mut self, mut data: RdsData) {
        let groups = std::mem::take(&mut data.groups);
        if data.group_seq < self.rds_next_seq {
            // The decoder restarted — a retune, or a mode change. Its counter
            // went backwards, so the old log is somebody else's.
            self.rds_log.clear();
        }
        self.rds_next_seq = data.group_seq + groups.len() as u64;
        for g in groups {
            if self.rds_log.len() >= RDS_LOG_ROWS {
                self.rds_log.pop_front();
            }
            self.rds_log.push_back(g);
        }
        if !data.sync && data.is_empty() {
            // A cleared snapshot: the dial left the station.
            self.rds_log.clear();
            self.rds_next_seq = 0;
        }
        self.rds = Some(data);
    }

    /// Which table this station's codes are read against, with `Auto` settled.
    fn rds_standard(&self) -> RdsStandard {
        let (pi, ecc) = match self.rds.as_ref() {
            Some(d) => (d.pi, d.ecc),
            None => (None, None),
        };
        self.view.rds_standard.resolve(pi, ecc)
    }

    pub(in crate::app) fn rds_window(&mut self, ctx: &egui::Context) {
        if !self.show_rds {
            return;
        }
        let mut open = self.show_rds;
        let resp = egui::Window::new(crate::language_plugin::text("window.rds.text_78_147238", "RDS")).id(egui::Id::new("RDS"))
            .id(crate::layout::salted_id(ctx, "RDS"))
            .open(&mut open)
            .frame(crate::chrome::window_frame())
            .resizable(true)
            .default_width(crate::layout::window_w(ctx, 460.0))
            .default_height(crate::layout::window_h(ctx, 420.0))
            .show(ctx, |ui| {
                crate::chrome::window_body_bg(ui);
                self.rds_body(ui)
            });
        if let Some(r) = &resp {
            crate::chrome::paint_window_border(ctx, &r.response);
        }
        self.show_rds = open;
    }

    fn rds_body(&mut self, ui: &mut egui::Ui) {
        let standard = self.rds_standard();
        // Read before the selector can change it, so the page below and the
        // combo above agree about the same frame.
        let chosen = self.view.rds_standard;

        crate::chrome::tab_bar(ui, |ui, bar| {
            for (tab, label) in [(RdsTab::Station, crate::language_plugin::text("window.rds.text_102_817708", "STATION")), (RdsTab::Diagnostics, crate::language_plugin::text("window.rds.text_102_c34dc2", "DIAGNOSTICS"))]
            {
                if bar.tab(ui, self.rds_tab == tab, label).clicked() {
                    self.rds_tab = tab;
                }
            }
            // The selector is not a tab and the baseline runs on under it.
            bar.end_tabs(ui);

            // The standard selector. A display choice, not a command: the raw
            // codes are already here, so this re-labels everything at once.
            egui::ComboBox::from_id_salt("rds-standard")
                .selected_text(if chosen == RdsStandard::Auto {
                    { let __lp_arg_0 = &(crate::language_plugin::rds_label(standard.label())); crate::language_plugin::format("window.rds.text_115_ac88e5", "Auto ({})", &[format!("{}", __lp_arg_0)]) }
                } else {
                    crate::language_plugin::rds_label(chosen.label())
                })
                .show_styled(ui, |ui| {
                    for s in RdsStandard::ALL {
                        ui.selectable_value(&mut self.view.rds_standard, s, crate::language_plugin::rds_label(s.label()));
                    }
                })
                .response
                .on_hover_text(
                    crate::language_plugin::text("window.rds.text_126_14c106", "Which programme-type table to read this station against. RDS is used \
                     everywhere outside North America; RBDS adds the call sign a US station's \
                     identity code spells out. Auto follows the extended country code the \
                     station sends, and until one arrives it will not spell out a call sign — \
                     the identity ranges it would read are shared with nine countries' RDS \
                     codes, so select RBDS to name a station that sends no country code."),
                );
        });
        // No separator: the strip's own baseline is the line between the tabs
        // and the page they open.
        ui.add_space(8.0);

        match self.rds_tab {
            RdsTab::Station => self.rds_station(ui, standard, chosen),
            RdsTab::Diagnostics => self.rds_diagnostics(ui),
        }
    }

    /// `standard` is the resolved one, which every code is read against.
    /// `chosen` is what the operator actually selected, and is only consulted
    /// about the call sign — see [`RdsStandard::names_from_pi`].
    fn rds_station(&mut self, ui: &mut egui::Ui, standard: RdsStandard, chosen: RdsStandard) {
        let Some(d) = self.rds.as_ref() else {
            ui.label(dim(crate::language_plugin::text("window.rds.text_150_128cfd", "waiting for the receiver…")));
            return;
        };

        if !d.sync && d.is_empty() {
            ui.label(dim(if self.state.rx[0].mode == sdroxide_types::Mode::Wfm {
                crate::language_plugin::text("window.rds.text_156_e7e079", "No RDS on this station. Not every transmitter carries it, and a weak \
                 one may carry it without it arriving.")
            } else {
                crate::language_plugin::text("window.rds.text_159_190263", "RDS is only carried on WFM broadcast stations.")
            }));
            return;
        }

        // The station's name, big, with what it is playing under it.
        ui.horizontal(|ui| {
            let title = d.title(chosen).unwrap_or_else(|| "—".to_string());
            ui.label(RichText::new(title).size(22.0).strong());
            if !d.sync {
                ui.label(RichText::new(crate::language_plugin::text("window.rds.text_169_495879", "HOLDING")).size(9.5).color(crate::theme::ALERT()))
                    .on_hover_text(crate::language_plugin::text("window.rds.text_170_83db8a", "Block sync has dropped — this is the last station read."));
            }
        });

        if let Some(rt) = d.rt_plus.as_ref() {
            let now = [rt.artist.as_deref(), rt.title.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" — ");
            if !now.is_empty() {
                ui.label(RichText::new(now).size(13.0).color(crate::theme::CYAN_DIM()));
            }
        }
        ui.add_space(8.0);

        egui::ScrollArea::vertical().id_salt("rds-station").show(ui, |ui| {
            egui::Grid::new("rds_grid").num_columns(2).spacing([14.0, 4.0]).striped(true).show(
                ui,
                |ui| {
                    let row = |ui: &mut egui::Ui, k: String, v: String| {
                        ui.label(dim(k));
                        ui.label(RichText::new(v).size(11.0));
                        ui.end_row();
                    };

                    if let Some(pi) = d.pi {
                        let mut v = format!("{pi:04X}");
                        if let Some(call) = d.call_sign(chosen) {
                            v = format!("{pi:04X}  ({call})");
                        }
                        row(ui, crate::language_plugin::text("window.rds.text_201_82ec80", "IDENTITY"), v);
                    }
                    if let Some(pty) = d.pty {
                        row(ui, crate::language_plugin::text("window.rds.text_204_e299e2", "PROGRAMME"), format!("{}  ({pty})", crate::language_plugin::rds_label(pty_name(pty, standard))));
                    }
                    if let Some(n) = d.ptyn.as_ref().filter(|n| !n.is_empty()) {
                        row(ui, crate::language_plugin::text("window.rds.text_207_939a2e", "DESCRIBED AS"), n.clone());
                    }
                    if let Some(m) = d.music {
                        row(ui, crate::language_plugin::text("window.rds.text_210_65f23e", "CONTENT"), if m { crate::language_plugin::text("window.rds.text_210_6eb00b", "Music") } else { crate::language_plugin::text("window.rds.text_210_3565fc", "Speech") }.to_string());
                    }
                    row(
                        ui,
                        crate::language_plugin::text("window.rds.text_214_204639", "TRAFFIC"),
                        match (d.tp, d.ta) {
                            (_, true) => crate::language_plugin::text("window.rds.text_216_c3746a", "announcement on air now").to_string(),
                            (true, false) => crate::language_plugin::text("window.rds.text_217_ea4e42", "carried by this station").to_string(),
                            (false, false) => crate::language_plugin::text("window.rds.text_218_9f8521", "not carried").to_string(),
                        },
                    );
                    if let Some(rt) = d.radiotext.as_ref().filter(|t| !t.is_empty()) {
                        ui.label(dim(crate::language_plugin::text("window.rds.text_222_7a753b", "RADIO TEXT")));
                        ui.label(RichText::new(rt.clone()).size(11.0));
                        ui.end_row();
                    }
                    if let Some(c) = d.clock {
                        row(ui, crate::language_plugin::text("window.rds.text_227_b0810b", "STATION CLOCK"), c.label());
                    }
                    if let Some(ecc) = d.ecc {
                        row(ui, crate::language_plugin::text("window.rds.text_230_e3b435", "COUNTRY CODE"), format!("{ecc:02X}"));
                    }
                    if !d.af.is_empty() {
                        let list: Vec<String> =
                            d.af.iter().map(|hz| format!("{:.1}", *hz as f64 / 1e6)).collect();
                        ui.label(dim(crate::language_plugin::text("window.rds.text_235_3afc66", "ALSO ON")));
                        ui.label(RichText::new(format!("{} MHz", list.join(", "))).size(11.0));
                        ui.end_row();
                    }
                },
            );

            // Anything the station tagged beyond artist and title.
            let extra = d.rt_plus.as_ref().map(|r| r.other.clone()).unwrap_or_default();
            if !extra.is_empty() {
                ui.add_space(8.0);
                ui.label(dim(crate::language_plugin::text("window.rds.text_246_640cfc", "ALSO TAGGED")));
                for (class, text) in extra {
                    let name = rt_plus_class(class)
                        .map(crate::language_plugin::rds_label)
                        .unwrap_or_else(|| crate::language_plugin::format("window.rds.text_250_eed507", "type {class}", &[format!("{class}")]));
                    ui.label(RichText::new(format!("{name}: {text}")).size(10.5));
                }
            }
        });
    }

    fn rds_diagnostics(&mut self, ui: &mut egui::Ui) {
        let Some(d) = self.rds.as_ref() else {
            ui.label(dim(crate::language_plugin::text("window.rds.text_260_128cfd", "waiting for the receiver…")));
            return;
        };
        let stats = d.stats;

        ui.horizontal(|ui| {
            let (label, colour) = if d.sync {
                (crate::language_plugin::text("window.rds.text_267_7dcad6", "SYNC"), crate::theme::CYAN_DIM())
            } else {
                (crate::language_plugin::text("window.rds.text_269_621cf7", "NO SYNC"), crate::theme::ALERT())
            };
            ui.label(RichText::new(label).size(11.0).strong().color(colour));
            ui.label(dim(&{ let __lp_arg_0 = &(stats.groups); crate::language_plugin::format("window.rds.text_272_1af17e", "· {} groups", &[format!("{}", __lp_arg_0)]) }));
            let ber = stats.block_error_rate();
            ui.label(dim(&{ let __lp_arg_0 = &(ber * 100.0); crate::language_plugin::format("window.rds.text_274_a857c0", "· {:.1} % of blocks in error", &[format!("{:.1}", __lp_arg_0)]) })).on_hover_text(
                crate::language_plugin::text("window.rds.text_275_b7dacf", "Blocks that failed their check, plus those the error corrector had to \
                     repair. Corrections count as errors on purpose: a rising figure is the \
                     first sign a station is about to stop decoding."),
            );
        });
        ui.label(dim(&{ let __lp_arg_0 = &(stats.blocks_ok); let __lp_arg_1 = &(stats.blocks_corrected); let __lp_arg_2 = &(stats.blocks_bad); crate::language_plugin::format("window.rds.text_281_d078f8", "{} clean · {} corrected · {} lost", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1), format!("{}", __lp_arg_2)]) }));

        // The cause, when it is this one. A block error rate is a symptom, and
        // the symptom on its own sends the operator hunting for a better aerial
        // — which is the wrong direction, because the station is already too
        // strong. Nothing they can hear will tell them: the data subcarrier
        // sits 28-34 dB below peak deviation up at 57 kHz, where FM's noise is
        // some 15 dB worse than down where the programme lives, so it goes
        // while the audio is still clean and the pilot still locked.
        if self.meters.is_some_and(|m| m.adc_overloaded()) {
            ui.add_space(4.0);
            ui.label(
                RichText::new(crate::language_plugin::text("window.rds.text_295_0ae503", "FRONT END OVERLOADING"))
                    .size(10.5)
                    .strong()
                    .color(crate::theme::ALERT()),
            )
            .on_hover_text(
                crate::language_plugin::text("window.rds.text_301_150a26", "The converter is clipping, which puts distortion right across the \
                 multiplex — including on the 57 kHz data subcarrier. Turn the RF gain \
                 down, or switch in an attenuator: RDS goes about a decibel before \
                 anything is audible, so a station that sounds perfect can still be far \
                 too strong for its data to survive."),
            );
        }
        ui.add_space(6.0);

        // Which group types this station actually sends. Empty columns are as
        // informative as full ones — a station with no 2A sends no radio text,
        // and that is the answer to "why is the text blank".
        let seen: Vec<String> = stats
            .group_types
            .iter()
            .enumerate()
            .filter(|&(_, &n)| n > 0)
            .map(|(i, n)| format!("{}{}×{}", i / 2, if i % 2 == 0 { 'A' } else { 'B' }, n))
            .collect();
        ui.label(dim(crate::language_plugin::text("window.rds.text_320_9f5583", "GROUP TYPES")));
        ui.label(
            RichText::new(if seen.is_empty() { "—".to_string() } else { seen.join("  ") })
                .size(10.5)
                .monospace(),
        );
        ui.add_space(6.0);

        ui.label(dim(crate::language_plugin::text("window.rds.text_328_ffb520", "RECENT GROUPS — newest last")));
        egui::ScrollArea::vertical().id_salt("rds-log").stick_to_bottom(true).show(ui, |ui| {
            for g in &self.rds_log {
                ui.label(rds_log_line(g));
            }
            if self.rds_log.is_empty() {
                ui.label(dim(crate::language_plugin::text("window.rds.text_334_480cd8", "nothing decoded yet")));
            }
        });
    }
}

/// One group as a line of hex, with the blocks that did not survive marked.
///
/// A lost block is shown as `····` rather than as the bits that arrived: those
/// bits failed their check, so printing them would put four plausible hex digits
/// on screen that mean nothing. A corrected block is shown with its value and a
/// marker, because that value *is* trustworthy — it just cost a repair.
fn rds_log_line(g: &RdsGroupLog) -> RichText {
    let mut s = String::with_capacity(40);
    for i in 0..4 {
        if g.valid & (1 << i) == 0 {
            s.push_str("····");
        } else {
            s.push_str(&format!("{:04X}", g.blocks[i]));
        }
        s.push(if g.corrected & (1 << i) != 0 { '*' } else { ' ' });
    }
    if let Some(kind) = g.kind() {
        s.push_str(&format!(" {kind}"));
    }
    let colour = if g.valid == 0b1111 && g.corrected == 0 {
        crate::theme::gray(170)
    } else if g.valid == 0b1111 {
        crate::theme::gray(130)
    } else {
        crate::theme::ALERT()
    };
    RichText::new(s).size(10.0).monospace().color(colour)
}

#[cfg(test)]
mod language_tests {
    use super::*;
    #[test]
    fn raw_group_log_preserves_hex_loss_and_correction_markers() {
        let group = RdsGroupLog {
            blocks: [0xABCD, 0x2A3B, 0x1234, 0x4567], valid: 0b1011, corrected: 0b0010,
        };
        crate::language_plugin::test_manual_enabled(true);
        assert_eq!(rds_log_line(&group).text(), "ABCD 2A3B*···· 4567  2B");
        crate::language_plugin::test_manual_enabled(false);
        assert_eq!(rds_log_line(&group).text(), "ABCD 2A3B*···· 4567  2B");
    }
}
