//! The HD Radio window: what the digital sidecar of an FM broadcast says about
//! itself.
//!
//! The sync row is for tuning one in — it shows how far the decoder has got, so
//! a station that is present but not decoding says *where* it stopped. The
//! figures below it are for once that has succeeded: how much margin the
//! sidebands have, and who is broadcasting and what they are playing.
//!
//! HD Radio's digital sidebands sit about 20 dB down from the analog carrier
//! they share, so acquisition needs a listenable signal *and* a quiet one. The
//! MER traces are the honest read on that: they fall well before the audio
//! does, which is what makes them worth watching while a station is being
//! aligned or an antenna turned.

use eframe::egui::{self, Color32, RichText};
use sdroxide_types::{Command, HdRadioStatus, Mode};

use crate::app::SdroxideApp;

/// How many status snapshots the quality history keeps. The engine sends four
/// a second, so this is about a minute — long enough to watch a fade come and
/// go.
const HISTORY_LEN: usize = 240;

/// Field labels and anything the reader is not meant to look at first.
fn dim_ink() -> Color32 {
    crate::theme::gray(110)
}

impl SdroxideApp {
    pub(in crate::app) fn on_hd(&mut self, data: HdRadioStatus) {
        // Only while locked: the figures mean nothing otherwise, and plotting
        // the zeros a dropout leaves would draw a cliff that says "the signal
        // got worse" when what happened is that it went away.
        if data.locked {
            if self.hd_history.len() >= HISTORY_LEN {
                self.hd_history.pop_front();
            }
            self.hd_history.push_back((data.mer_lower_db, data.mer_upper_db));
        } else {
            // Sync gone — start the trace again rather than joining across the
            // gap.
            self.hd_history.clear();
        }
        self.hd = Some(data);
    }

    pub(in crate::app) fn hd_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Command>) {
        if !self.show_hd {
            return;
        }
        let mut open = self.show_hd;
        let resp = egui::Window::new(crate::language_plugin::text("window.hd.title", "HD Radio"))
            .id(crate::layout::salted_id(ctx, "HD Radio"))
            .open(&mut open)
            .frame(crate::chrome::window_frame())
            .resizable(true)
            .default_width(crate::layout::window_w(ctx, 440.0))
            .default_height(crate::layout::window_h(ctx, 360.0))
            .show(ctx, |ui| {
                crate::chrome::window_body_bg(ui);
                self.hd_body(ui)
            });
        if let Some(r) = &resp {
            cmds.extend(r.inner.clone().unwrap_or_default());
        }
        if let Some(r) = &resp {
            crate::chrome::paint_window_border(ctx, &r.response);
        }
        self.show_hd = open;
    }

    fn hd_body(&mut self, ui: &mut egui::Ui) -> Vec<Command> {
        let mut cmds = Vec::new();
        let dim = |s: &str| RichText::new(s).size(9.5).color(dim_ink());

        let Some(d) = self.hd.clone() else {
            ui.label(dim(&crate::language_plugin::text("window.hd.text_78_128cfd", "waiting for the receiver…")));
            return cmds;
        };

        if self.state.rx[0].mode != Mode::HdRadio {
            ui.label(dim(
                &crate::language_plugin::text("window.hd.text_84_6ec9b7", "Not in HD Radio. Set the mode to HD Radio on an FM broadcast carrying the \
                 digital sidecars — the dial goes on the analog carrier's centre, not beside it."),
            ));
            ui.add_space(6.0);
        }

        self.hd_sync_row(ui, &d);
        ui.add_space(8.0);

        if let Some(why) = d.unavailable.as_deref() {
            ui.label(dim(&crate::language_plugin::backend_status(why)));
            return cmds;
        }
        if !d.locked {
            ui.label(dim(
                &crate::language_plugin::text("window.hd.text_99_77d1dc", "No HD Radio lock. The digital sidebands are transmitted about 20 dB below \
                 the analog carrier, so a listenable station can still be too noisy to \
                 decode: the decoder needs a few seconds on a clean one."),
            ));
            return cmds;
        }

        self.hd_signal(ui, &d);
        ui.add_space(8.0);
        self.hd_station(ui, &d, &mut cmds);
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        self.hd_quality_history(ui);
        cmds
    }

    /// How far up the chain the decoder has got, left to right in the order the
    /// stages lock.
    fn hd_sync_row(&self, ui: &mut egui::Ui, d: &HdRadioStatus) {
        let dot = |ui: &mut egui::Ui, on: bool| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
            let ink = if on { Color32::from_rgb(90, 200, 120) } else { crate::theme::gray(90) };
            ui.painter().circle_filled(rect.center(), 3.5, ink);
        };
        let stages = [
            (crate::language_plugin::text("window.hd.text_125_7dcad6", "SYNC"), d.locked, crate::language_plugin::text("window.hd.text_125_28deda", "OFDM frame timing recovered")),
            (crate::language_plugin::text("window.hd.text_126_859e89", "AUDIO"), d.audio, crate::language_plugin::text("window.hd.text_126_6d8b28", "Audio frames decoding")),
        ];
        ui.horizontal_wrapped(|ui| {
            for (label, on, hover) in stages {
                let text = RichText::new(label).size(9.5).color(if on {
                    crate::theme::gray(200)
                } else {
                    dim_ink()
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    dot(ui, on);
                    ui.label(text);
                })
                .response
                .on_hover_text(hover);
                ui.add_space(6.0);
            }
        });
    }

    /// Lower and upper sideband MER over the last minute.
    ///
    /// The two traces are drawn on one scale because the *difference* between
    /// them is the reading: the sidebands are close together in frequency but
    /// at opposite ends of the channel, so a selective fade, a tilted antenna
    /// or a neighbouring station near one edge shows up as one trace falling
    /// while the other holds.
    fn hd_quality_history(&self, ui: &mut egui::Ui) {
        let dim = |s: &str| RichText::new(s).size(9.5).color(dim_ink());
        ui.horizontal(|ui| {
            ui.label(dim(&crate::language_plugin::text("window.hd.text_157_272776", "QUALITY")));
            ui.label(RichText::new(crate::language_plugin::text("window.hd.text_158_86543e", "LOWER")).size(9.0).color(Color32::from_rgb(90, 190, 230)));
            ui.label(RichText::new(crate::language_plugin::text("window.hd.text_159_9cba1c", "UPPER")).size(9.0).color(Color32::from_rgb(150, 210, 120)));
            ui.label(dim(&crate::language_plugin::text("window.hd.text_160_503737", "last minute")));
        });

        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 54.0), egui::Sense::hover());
        if !ui.is_rect_visible(rect) {
            return;
        }
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 2.0, crate::theme::gray(24));

        if self.hd_history.len() < 2 {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                crate::language_plugin::text("window.hd.text_175_052eb4", "waiting for a locked signal"),
                egui::FontId::proportional(10.0),
                dim_ink(),
            );
            return;
        }

        // A fixed 0–30 dB window, widened only if the signal goes past it.
        // Autoscaling to the data would make a steady signal look like it was
        // moving, which is the opposite of what a history is for.
        let top =
            self.hd_history.iter().flat_map(|&(l, u)| [l, u]).fold(30.0f32, f32::max).min(60.0);
        let newest = self.hd_history.len().saturating_sub(1);
        let x_at = |i: usize| {
            rect.right() - rect.width() * ((newest - i) as f32 / (HISTORY_LEN - 1) as f32)
        };
        let y_at = |db: f32| rect.bottom() - rect.height() * (db.clamp(0.0, top) / top);

        // Ten-dB rules, so the height can be read without an axis.
        let mut db = 10.0;
        while db < top {
            let y = y_at(db);
            p.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(1.0, crate::theme::gray(40)),
            );
            p.text(
                egui::pos2(rect.left() + 3.0, y),
                egui::Align2::LEFT_BOTTOM,
                format!("{db:.0}"),
                egui::FontId::proportional(8.0),
                crate::theme::gray(70),
            );
            db += 10.0;
        }

        for (pick, color) in
            [(0usize, Color32::from_rgb(90, 190, 230)), (1usize, Color32::from_rgb(150, 210, 120))]
        {
            let pts: Vec<egui::Pos2> = self
                .hd_history
                .iter()
                .enumerate()
                .map(|(i, &(l, u))| egui::pos2(x_at(i), y_at(if pick == 0 { l } else { u })))
                .collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.2, color)));
        }
    }

    fn hd_signal(&self, ui: &mut egui::Ui, d: &HdRadioStatus) {
        let dim = |s: &str| RichText::new(s).size(9.5).color(dim_ink());
        let val = |s: String| RichText::new(s).size(11.0);

        egui::Grid::new("hd-signal").num_columns(4).spacing([14.0, 3.0]).show(ui, |ui| {
            ui.label(dim(&crate::language_plugin::text("window.hd.text_229_8c217f", "MER L")));
            ui.label(val(format!("{:.1} dB", d.mer_lower_db)))
                .on_hover_text(crate::language_plugin::text("window.hd.text_231_33b64e", "Modulation error ratio of the lower digital sideband."));
            ui.label(dim(&crate::language_plugin::text("window.hd.text_232_8163e9", "MER U")));
            ui.label(val(format!("{:.1} dB", d.mer_upper_db)))
                .on_hover_text(crate::language_plugin::text("window.hd.text_234_57def9", "Modulation error ratio of the upper digital sideband."));
            ui.end_row();

            ui.label(dim("CBER"));
            ui.label(val(format!("{:.2e}", d.cber))).on_hover_text(
                crate::language_plugin::text("window.hd.text_239_ace6bc", "Channel bit-error ratio after the inner code. Below about 1e-3 the outer \
                 code can correct it; above that the audio starts to break up."),
            );
            ui.label(dim(&crate::language_plugin::text("window.hd.text_242_82047f", "OFFSET")));
            ui.label(val(format!("{:+.0} Hz", d.freq_offset_hz))).on_hover_text(
                crate::language_plugin::text("window.hd.text_244_348a79", "Residual carrier frequency offset. Large and steady means the receiver's \
                 reference is off, not the broadcast."),
            );
            ui.end_row();

            if d.psmi > 0 {
                ui.label(dim("PSMI"));
                ui.label(val(format!("MP{}", d.psmi))).on_hover_text(
                    crate::language_plugin::text("window.hd.text_252_b54ada", "Primary Service Mode Indicator: which of the FM hybrid service modes \
                     the station is transmitting."),
                );
                ui.label("");
                ui.label("");
                ui.end_row();
            }
        });
    }

    fn hd_station(&self, ui: &mut egui::Ui, d: &HdRadioStatus, cmds: &mut Vec<Command>) {
        let dim = |s: &str| RichText::new(s).size(9.5).color(dim_ink());

        if !d.station_name.is_empty() {
            ui.label(RichText::new(&d.station_name).size(15.0).strong());
        }
        if !d.station_slogan.is_empty() {
            ui.label(dim(&d.station_slogan));
        }

        // Only worth a control when the multiplex actually carries a choice.
        if d.audio_services.len() > 1 {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(dim(&crate::language_plugin::text("window.hd.text_276_e299e2", "PROGRAMME")));
                for svc in &d.audio_services {
                    let on = svc.program == d.program;
                    let label = format!("HD-{}", svc.program_number());
                    let mut hover = { let __lp_arg_0 = &(label); let __lp_arg_1 = &(svc.codec_label()); crate::language_plugin::format("window.hd.text_280_8f0dda", "Decode {} ({})", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) };
                    if svc.restricted() {
                        hover.push_str(&crate::language_plugin::text("window.hd.text_282_b1917b", " — restricted access"));
                    }
                    if crate::chrome::chip(ui, on, label).on_hover_text(hover).clicked() && !on {
                        cmds.push(Command::SetHdProgram { program: svc.program });
                    }
                }
            });
        }

        if !d.station_message.is_empty() {
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new(&d.station_message).size(11.0));
        }
    }
}
