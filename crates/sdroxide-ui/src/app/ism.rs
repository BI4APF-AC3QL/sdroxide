//! The ISM window: what the licence-exempt bands around you are saying.
//!
//! One row per device rather than per transmission — see
//! [`sdroxide_types::IsmReport`] for why — with the channels the decoder is
//! listening on above them, because an empty list has two very different causes
//! and only one of them is "there is nothing here".

use eframe::egui::{self, RichText};
use sdroxide_types::{Command, IsmFamily, IsmReport, IsmSettings, Mode, Vfo};

use crate::app::SdroxideApp;
use crate::app::panels::widgets::{row_cell, sort_head_cell};
use crate::app::util::fmt_age;
use crate::theme::ThemedScroll;

/// How the device list is ordered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) enum IsmSort {
    /// Most recently heard first. What you want while watching a band.
    #[default]
    Heard,
    /// Most frequently heard first. What you want when identifying what is
    /// permanently nearby as against what walked past once.
    Count,
    Signal,
    Frequency,
}

/// Least tall a device row may be. Rows grow past it: the readings column wraps,
/// and a weather station with eight readings needs two or three lines at any
/// window width somebody would actually use.
const ROW_H: f32 = 19.0;

/// Column widths, in the order they are drawn.
///
/// Every one of these is a hard clip — the labels are built with `.truncate()`,
/// so a long model name ends in an ellipsis instead of being painted across the
/// device id next to it.
const W_AGE: f32 = 30.0;
const W_FREQ: f32 = 62.0;
const W_KIND: f32 = 132.0;
const W_DEVICE: f32 = 78.0;
const W_SNR: f32 = 36.0;
const W_COUNT: f32 = 32.0;
const COL_GAP: f32 = 5.0;

/// Below this the signal columns are dropped rather than squeezing the readings.
const NARROW_W: f32 = 560.0;

impl SdroxideApp {
    pub(in crate::app) fn ism_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Command>) {
        if !self.show_ism {
            return;
        }
        let mut open = self.show_ism;
        // Edited in place and sent whole on any change, as the skimmer's and the
        // scanner's are; the engine persists it and echoes it back, so there is
        // no apply step to get wrong.
        let mut cfg = self.state.ism;
        let resp = egui::Window::new(crate::language_plugin::text("window.ism.text_60_ccf31e", "ISM DEVICES")).id(egui::Id::new("ISM DEVICES"))
            .id(crate::layout::salted_id(ctx, "Ism"))
            .open(&mut open)
            .frame(crate::chrome::window_frame())
            .resizable(true)
            // Wide enough that a full weather station's readings fold onto two
            // lines rather than five. It is resizable either way.
            .default_width(crate::layout::window_w(ctx, 760.0))
            .show(ctx, |ui| {
                crate::chrome::window_body_bg(ui);
                self.ism_body(ui, &mut cfg, cmds)
            });
        if let Some(r) = &resp {
            crate::chrome::paint_window_border(ctx, &r.response);
        }
        if cfg != self.state.ism {
            cmds.push(Command::SetIsmConfig(cfg));
        }
        self.show_ism = open;
    }

    fn ism_body(&mut self, ui: &mut egui::Ui, cfg: &mut IsmSettings, cmds: &mut Vec<Command>) {
        // Matching the skimmer popup: a radio whose capabilities have not
        // arrived yet is assumed wideband, so the controls are live from the
        // first frame rather than greyed until the first state message.
        let wideband = self.caps.as_ref().is_none_or(|c| !c.audio_mode);

        ui.horizontal(|ui| {
            let run = crate::chrome::chip_enabled(
                ui,
                wideband,
                cfg.enabled,
                if cfg.enabled { crate::language_plugin::text("window.ism.text_92_ce8133", "DECODING") } else { crate::language_plugin::text("window.ism.text_92_38cca6", "OFF") },
            );
            if run.clicked() {
                cfg.enabled = !cfg.enabled;
            }
            if !wideband {
                run.on_hover_text(crate::language_plugin::text("window.ism.text_98_3abbc8", "Needs a wideband IQ source; a CAT rig sends only audio"));
            }

            ui.add_space(8.0);
            ui.label(RichText::new(crate::language_plugin::text("window.ism.text_102_4ec7c5", "sql")).size(10.0).color(crate::theme::CYAN_DIM()));
            ui.add_enabled(
                wideband,
                egui::DragValue::new(&mut cfg.threshold_db).range(6..=30).suffix(" dB").speed(0.2),
            )
            .on_hover_text(
                crate::language_plugin::text("window.ism.text_108_249393", "How far a burst must stand above the channel's own noise floor \
                 before it is decoded"),
            );
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for f in IsmFamily::ALL {
                let usable = wideband && f.implemented();
                let chip =
                    crate::chrome::chip_enabled(ui, usable, cfg.family_enabled(f), f.label());
                if chip.clicked() {
                    let on = cfg.family_enabled(f);
                    cfg.set_family(f, !on);
                }
                // A family with nothing behind it yet says so rather than
                // looking like a switch that does not work. The devices are on
                // the air either way, and an operator who knows meters are "not
                // yet" is better informed than one who sees no mention of them.
                chip.on_hover_text(if f.implemented() {
                    f.hint().to_string()
                } else {
                    { let __lp_arg_0 = &(f.hint()); crate::language_plugin::format("window.ism.text_130_3afa88", "{} — not decoded yet", &[format!("{}", __lp_arg_0)]) }
                });
            }
        });

        ui.add_space(6.0);
        self.ism_bands(ui, cfg, cmds, wideband);

        ui.add_space(6.0);
        self.ism_channels(ui);
        self.ism_rtl433(ui, cmds);

        ui.add_space(6.0);
        ui.separator();
        self.ism_count(ui);
        ui.add_space(2.0);
        self.ism_list(ui, cmds);
    }

    /// Which band the decoder is on, and how wide a window it gets. One button
    /// each, and pressing a band goes there.
    ///
    /// This is the *only* set of tuning buttons in the window. There used to be
    /// two — a "TUNE 868.881 MHz" for the native channel plan and a separate
    /// TUNE beside each rtl_433 band — which is two answers to one question. The
    /// bands are hundreds of megahertz apart, so choosing one is choosing where
    /// the radio points; both sets of decoders then work on whatever that window
    /// contains.
    fn ism_bands(
        &self,
        ui: &mut egui::Ui,
        cfg: &mut IsmSettings,
        cmds: &mut Vec<Command>,
        wideband: bool,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(crate::language_plugin::text("window.ism.text_166_65f15c", "band")).size(10.0).color(crate::theme::CYAN_DIM()));
            for (bit, label, center_hz) in sdroxide_types::RTL433_BAND_LABELS {
                let selected = cfg.rtl433.band_enabled(bit);
                let chip = crate::chrome::chip_enabled(ui, wideband, selected, label);
                if chip.clicked() {
                    cfg.rtl433.bands = bit;
                    // Selecting a band the receiver cannot hear would only swap
                    // one "outside the window" row for another, so go there —
                    // and go there by the *window*, not by the dial. On a
                    // zero-IF front end the LO sits a quarter-span above the
                    // dial and the window follows the LO, so a dial on the band
                    // centre puts the window a quarter-span above it (#310).
                    cmds.push(Command::TuneWidebandTo(center_hz));
                }
                chip.on_hover_text(crate::language_plugin::format("window.ism.text_181_b9f5f8", "Listen on {label} — tunes the radio there. One band at a time: they are \
                     too far apart for any receiver to cover two at once.", &[format!("{label}")]));
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(crate::language_plugin::text("window.ism.text_188_ab02ee", "bw")).size(10.0).color(crate::theme::CYAN_DIM()));
            for (hz, label) in sdroxide_types::RTL433_BANDWIDTHS {
                let selected = cfg.rtl433.bandwidth_hz == hz;
                let chip = crate::chrome::chip_enabled(ui, wideband, selected, label);
                if chip.clicked() {
                    cfg.rtl433.bandwidth_hz = hz;
                }
                chip.on_hover_text(if hz == sdroxide_types::RTL433_BANDWIDTH_AUTO {
                    crate::language_plugin::text("window.ism.text_196_a8e6a2", "Give each band the width it normally needs — a quarter of a megahertz \
                     for the OOK bands at 315, 345 and 433 MHz, a full one for 868 and 915.")
                        .to_string()
                } else {
                    crate::language_plugin::format("window.ism.text_201_280fae", "Watch {label}Hz around the band centre, whatever the band would have \
                         asked for. Narrower fits a receiver that cannot deliver the band's \
                         own width; wider reaches devices sitting further off centre. Your \
                         receiver has to hand over about a third more than this.", &[format!("{label}")])
                });
            }
        });
    }

    /// What the embedded rtl_433 is doing, as one row under the channel list.
    ///
    /// No section and no switch of its own: it is compiled in or it is not, and
    /// when it is it decodes whatever band the buttons above chose. Anything it
    /// needs to say about *where* it is listening is already said by those
    /// buttons and by the channel rows, so all that is left here is how it is
    /// getting on.
    fn ism_rtl433(&self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        let Some(rt) = self.ism_status.as_ref().and_then(|s| s.rtl433.as_ref()) else { return };

        // Its own lamp row, in the same shape as the channels above it, so the
        // two lanes read as one list of "who is listening where".
        ui.horizontal(|ui| {
            Self::ism_lamp(ui, rt.running);
            let live = rt.bands.iter().find(|b| b.live);
            let freq =
                live.map(|b| format!("{:.3}", b.freq_hz / 1e6)).unwrap_or_else(|| "—".into());
            ui.label(RichText::new(freq).size(11.0).monospace().color(if rt.running {
                crate::theme::TEXT()
            } else {
                crate::theme::CYAN_DIM()
            }));
            let tail = rtl433_status_tail(rt.unavailable.as_deref(), live.map(|b| b.label.as_str()), rt.rate_hz);
            ui.label(RichText::new(tail).size(10.5).color(crate::theme::CYAN_DIM())).on_hover_text(
                crate::language_plugin::text("window.ism.text_243_fe234a", "Several hundred more device decoders, from the rtl_433 project, built in. \
                     Reads the 433 MHz OOK devices sdroxide's own decoders do not reach at all. \
                     Where both can read a device, rtl_433 knows far more variants and takes \
                     over."),
            );
        });

        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if rt.running {
                let flex = if rt.flex == 1 {
                    crate::language_plugin::text("window.ism.text_254_c156d9", "1 of your own").to_string()
                } else {
                    { let __lp_arg_0 = &(rt.flex); crate::language_plugin::format("window.ism.text_256_baa432", "{} of your own", &[format!("{}", __lp_arg_0)]) }
                };
                ui.label(
                    RichText::new({ let __lp_arg_0 = &(rt.decoders); let __lp_arg_1 = &(rt.decodes); crate::language_plugin::format("window.ism.text_260_465aff", "{} decoders ({flex}), {} decoded", &[format!("{}", __lp_arg_0), format!("{flex}"), format!("{}", __lp_arg_1)]) })
                    .size(10.0)
                    .color(crate::theme::CYAN_DIM()),
                )
                .on_hover_text(
                    crate::language_plugin::text("window.ism.text_267_f8418a", "Device decoders registered, how many came from your rtl433_flex.conf, \
                     and how many transmissions have been decoded since it started."),
                );
                ui.add_space(6.0);
            }
            // The operator's decoder file is edited outside sdroxide, so nothing
            // notices it changed until asked — the same reason the band plan has
            // a reload button.
            if crate::chrome::chip(ui, false, crate::language_plugin::text("window.ism.text_275_280ede", "RELOAD DECODERS"))
                .on_hover_text(
                    crate::language_plugin::text("window.ism.text_277_601b15", "Re-read rtl433_flex.conf after editing it. The devices already \
                     heard stay in the list."),
                )
                .clicked()
            {
                cmds.push(Command::ReloadIsmDecoders);
            }
        });

        // Which of sdroxide's own decoders have stood down, so a Bresser row
        // changing its protocol name is explained rather than mysterious.
        if !rt.superseded.is_empty() {
            let names: Vec<&str> = rt.superseded.iter().map(|p| p.label()).collect();
            ui.label(
                RichText::new({ let __lp_arg_0 = &(names.join(", ")); crate::language_plugin::format("window.ism.text_291_d276d7", "handled by rtl_433: {}", &[format!("{}", __lp_arg_0)]) })
                    .size(10.0)
                    .color(crate::theme::CYAN_DIM()),
            )
            .on_hover_text(
                crate::language_plugin::text("window.ism.text_296_a65f9a", "rtl_433 knows more variants of these than sdroxide's own decoders do, so it \
                 reads them while it is listening here. Select another band and sdroxide's \
                 own take over again."),
            );
        }

        for e in &rt.errors {
            ui.label(RichText::new(crate::language_plugin::format("window.ism.text_303_2f201d", "flex: {e}", &[format!("{e}")])).size(10.0).color(crate::theme::YELLOW()))
                .on_hover_text(
                    crate::language_plugin::text("window.ism.text_305_8ae82c", "A decoder in your rtl433_flex.conf was refused. The others still loaded. \
                     Fix the line, then press RELOAD DECODERS."),
                );
        }
    }

    /// The filled or hollow lamp that starts a status row.
    ///
    /// Painted rather than written: a filled-circle glyph is not in the bundled
    /// font's fallback chain, so `●` comes out as an empty box — and next to the
    /// hollow `○` of an idle row that reads as the wrong state rather than as a
    /// missing glyph.
    fn ism_lamp(ui: &mut egui::Ui, live: bool) {
        let colour = if live { crate::theme::GREEN() } else { crate::theme::CYAN_DIM() };
        let (lamp, _) = ui.allocate_exact_size(egui::vec2(11.0, 11.0), egui::Sense::hover());
        if live {
            ui.painter().circle_filled(lamp.center(), 3.5, colour);
        } else {
            ui.painter().circle_stroke(lamp.center(), 3.5, egui::Stroke::new(1.0, colour));
        }
    }

    /// Where the decoder is listening, and why it is not where it is not.
    fn ism_channels(&self, ui: &mut egui::Ui) {
        let Some(st) = &self.ism_status else {
            ui.label(
                RichText::new(crate::language_plugin::text("window.ism.text_331_317c5e", "waiting for the decoder…"))
                    .size(11.0)
                    .color(crate::theme::CYAN_DIM()),
            );
            return;
        };

        if let Some(why) = &st.unavailable {
            ui.label(RichText::new(crate::language_plugin::backend_status(why)).size(11.0).color(crate::theme::YELLOW()));
            // Which window, exactly. The decoder's window sits on the *hardware
            // centre*, not the VFO, and on a wide front end those are routinely
            // far apart — so "nothing is inside the window" reads as plainly
            // wrong next to a dial showing 868.88 MHz until the window it means
            // is on screen too.
            if st.window_rate_hz > 0.0 {
                ui.label(
                    RichText::new({ let __lp_arg_0 = &(st.window_center_hz / 1e6); let __lp_arg_1 = &(st.window_rate_hz / 1e6); crate::language_plugin::format("window.ism.text_348_ae1f04", "window {:.3} MHz, {:.3} MHz wide", &[format!("{:.3}", __lp_arg_0), format!("{:.3}", __lp_arg_1)]) })
                    .size(10.5)
                    .color(crate::theme::CYAN_DIM()),
                )
                .on_hover_text(
                    crate::language_plugin::text("window.ism.text_356_2b30c1", "Where the decoder is actually listening. This follows the receiver's \
                     hardware centre, not the VFO — on a wide front end the two can be a long \
                     way apart, and it is the window that decides which channels are reachable."),
                );
            }
            // No TUNE button here. The band buttons above are the one place
            // this window offers to move the radio, and a second control for
            // the same job — differing only in which decoder lane it suited —
            // is two answers to one question.
            ui.add_space(4.0);
        }

        for c in &st.channels {
            ui.horizontal(|ui| {
                Self::ism_lamp(ui, c.live);
                ui.label(
                    RichText::new(format!("{:.3}", c.freq_hz / 1e6)).size(11.0).monospace().color(
                        if c.live { crate::theme::TEXT() } else { crate::theme::CYAN_DIM() },
                    ),
                );
                let tail = match &c.reason {
                    Some(r) => format!("{}  —  {r}", c.label),
                    None => c.label.clone(),
                };
                ui.label(RichText::new(tail).size(10.5).color(crate::theme::CYAN_DIM()));
            });
        }

        // What the gate is seeing, which is the difference between "quiet band"
        // and "busy band full of things we cannot read yet" — and, when it is
        // zero, between those and "not actually listening". Shown whenever a
        // channel is live, including at zero: hiding it then makes a decoder that
        // has heard nothing look the same as one that is not running.
        if st.channels.iter().any(|c| c.live) {
            ui.add_space(3.0);
            ui.label(
                RichText::new({ let __lp_arg_0 = &(st.bursts); let __lp_arg_1 = &(st.decodes); crate::language_plugin::format("window.ism.text_392_a939b0", "{} bursts, {} decoded", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) })
                    .size(10.0)
                    .color(crate::theme::CYAN_DIM()),
            )
            .on_hover_text(
                crate::language_plugin::text("window.ism.text_397_dc1c72", "Transmissions the gate opened on, and how many produced a valid frame. \
                 Many bursts and no decodes means the band is busy with devices \
                 sdroxide cannot read yet. None at all means nothing is reaching the \
                 threshold — lower it, or check that the dial is on the band."),
            );
        }
    }

    /// How many devices are in the list. The ordering used to live here as a
    /// row of chips and is now on the column headings themselves, where a table
    /// is sorted everywhere else.
    fn ism_count(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            crate::chrome::row_tail(ui, |ui| {
                ui.label(
                    RichText::new({ let __lp_arg_0 = &(self.ism_reports.len()); crate::language_plugin::format("window.ism.text_412_b46511", "{} devices", &[format!("{}", __lp_arg_0)]) })
                        .size(10.0)
                        .color(crate::theme::CYAN_DIM()),
                );
            });
        });
    }

    fn ism_list(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        if self.ism_reports.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(crate::language_plugin::text("window.ism.text_423_63ee8f", "nothing heard yet")).size(11.0).color(crate::theme::CYAN_DIM()));
            return;
        }

        let mut order: Vec<&IsmReport> = self.ism_reports.iter().collect();
        let desc = self.ism_sort_desc;
        match self.ism_sort {
            IsmSort::Heard => order.sort_by_key(|r| r.last_at),
            IsmSort::Count => order.sort_by_key(|r| r.count),
            IsmSort::Signal => order.sort_by_key(|r| r.snr_db),
            IsmSort::Frequency => order.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz)),
        }
        if desc {
            order.reverse();
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Staged out of the closure: it borrows `self` and cannot also push a
        // tune command through `cmds`.
        let mut tune_to: Option<f64> = None;
        let narrow = ui.available_width() < NARROW_W;
        ism_header(ui, narrow, &mut self.ism_sort, &mut self.ism_sort_desc);
        egui::ScrollArea::vertical()
            .id_salt("ism-devices")
            .max_height(ui.available_height())
            .auto_shrink([false, false])
            .show_themed(ui, |ui| {
                for r in order {
                    if ism_row(ui, r, now, narrow).clicked() {
                        tune_to = Some(r.freq_hz);
                    }
                }
            });

        if let Some(hz) = tune_to {
            // These are wideband FSK bursts, not something to listen to: tuning
            // to one puts it under the cursor so the operator can watch it on
            // the waterfall. NFM is the widest ordinary mode, which is as close
            // as the receiver gets to the right shape.
            cmds.push(Command::SetVfo { vfo: Vfo::A, hz });
            cmds.push(Command::SetMode { rx: sdroxide_types::RxId::Main, mode: Mode::Nfm });
        }
    }
}

fn rtl433_status_tail(unavailable: Option<&str>, live_label: Option<&str>, rate_hz: f64) -> String {
    match (unavailable, live_label) {
        (Some(why), _) => format!("rtl_433  —  {}", crate::language_plugin::backend_status(why)),
        // The width as well as the place: the operator chooses it now, and
        // what the downconverter settles on is a whole-number division of the
        // receiver's stream rather than the round figure that was asked for.
        (None, Some(label)) => format!("rtl_433 {}  ·  {:.0} kHz", label, rate_hz / 1e3),
        (None, None) => "rtl_433".to_string(),
    }
}

/// The column headings, so the numbers in a row say what they are — and what
/// re-orders the list.
fn ism_header(ui: &mut egui::Ui, narrow: bool, sort: &mut IsmSort, desc: &mut bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = COL_GAP;
        // The frame's left margin, so the headings sit over their columns.
        ui.add_space(6.0);
        let mut head = |ui: &mut egui::Ui, w: f32, right: bool, text: &str, key| {
            sort_head_cell(ui, w, right, text, key, sort, desc);
        };
        head(ui, W_AGE, true, &crate::language_plugin::text("controls.app.ism.text_482_013f54", "age"), Some(IsmSort::Heard));
        head(ui, W_FREQ, true, "MHz", Some(IsmSort::Frequency));
        // The model name and the device id are labels, not measurements: an
        // alphabetical sky of thermometers answers no question this window is
        // for.
        head(ui, W_KIND, false, &crate::language_plugin::text("controls.app.ism.text_487_263a4d", "device"), None);
        head(ui, W_DEVICE, false, &crate::language_plugin::text("controls.app.ism.text_488_a56145", "id"), None);
        if !narrow {
            head(ui, W_SNR, true, &crate::language_plugin::text("controls.app.ism.text_490_a54399", "sig"), Some(IsmSort::Signal));
            head(ui, W_COUNT, true, &crate::language_plugin::text("controls.app.ism.text_491_1b16b1", "n"), Some(IsmSort::Count));
        }
        head(ui, ui.available_width().max(40.0), false, &crate::language_plugin::text("controls.app.ism.text_493_303d03", "readings"), None);
    });
}

/// One device. Reads the way the question is asked: when, what, which one, and
/// what it said.
///
/// The fixed columns truncate and the readings column wraps, which between them
/// are what keep a row inside the window: a long model name used to be painted
/// straight over the device id beside it, and a weather station's eight readings
/// used to force the whole window wider than the screen.
fn ism_row(ui: &mut egui::Ui, r: &IsmReport, now: i64, narrow: bool) -> egui::Response {
    let inner = egui::Frame::new()
        .fill(crate::theme::ROW_BG())
        .inner_margin(egui::Margin { left: 6, right: 6, top: 3, bottom: 3 })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // Top-aligned, not centred: the readings wrap to two or three lines
            // and the columns beside them should stay level with the first.
            ui.horizontal_top(|ui| {
                ui.set_min_height(ROW_H);
                ui.spacing_mut().item_spacing.x = COL_GAP;

                // Age, not a clock: "how long since this spoke" is the question,
                // and a device that has gone quiet is the thing worth noticing.
                row_cell(
                    ui,
                    W_AGE,
                    ROW_H,
                    true,
                    egui::Label::new(
                        RichText::new(fmt_age(now - r.last_at))
                            .size(10.5)
                            .color(crate::theme::CYAN_DIM()),
                    )
                    .truncate(),
                );
                row_cell(
                    ui,
                    W_FREQ,
                    ROW_H,
                    true,
                    egui::Label::new(
                        RichText::new(format!("{:.4}", r.freq_hz / 1e6))
                            .size(11.0)
                            .monospace()
                            .color(crate::theme::TEXT()),
                    )
                    .truncate(),
                );
                row_cell(
                    ui,
                    W_KIND,
                    ROW_H,
                    false,
                    egui::Label::new(
                        RichText::new(r.fmt_kind()).size(11.0).color(crate::theme::CYAN()),
                    )
                    .truncate(),
                );
                row_cell(
                    ui,
                    W_DEVICE,
                    ROW_H,
                    false,
                    egui::Label::new(RichText::new(&r.device).size(11.0).monospace().strong())
                        .truncate(),
                );
                if !narrow {
                    row_cell(
                        ui,
                        W_SNR,
                        ROW_H,
                        true,
                        egui::Label::new(
                            RichText::new(format!("{} dB", r.snr_db))
                                .size(10.5)
                                .color(crate::theme::CYAN_DIM()),
                        )
                        .truncate(),
                    );
                    // How many times it has been heard: one frame is a CRC that
                    // happened to pass, fifty is a device that is really there.
                    row_cell(
                        ui,
                        W_COUNT,
                        ROW_H,
                        true,
                        egui::Label::new(
                            RichText::new(format!("×{}", r.count))
                                .size(10.5)
                                .color(crate::theme::CYAN_DIM()),
                        )
                        .truncate(),
                    );
                }
                // The readings take whatever is left and wrap inside it, so a
                // narrow pane drops the signal columns and then folds the values
                // rather than pushing the window wider than the screen.
                let colour =
                    if r.encrypted { crate::theme::YELLOW() } else { crate::theme::GREEN() };
                ui.add(
                    egui::Label::new(RichText::new(r.fmt_readings()).size(11.0).color(colour))
                        .wrap(),
                );
            });
        });

    let resp = inner.response.interact(egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut hover = { let __lp_arg_0 = &(r.protocol.label()); let __lp_arg_1 = &(r.device); let __lp_arg_2 = &(r.freq_hz / 1e6); let __lp_arg_3 = &(r.snr_db); let __lp_arg_4 = &(r.count); let __lp_arg_5 = &(fmt_age(now - r.first_at)); crate::language_plugin::format("window.ism.text_606_9d4279", "{}  id {}\n{:.4} MHz   {} dB   heard {}×\nfirst heard {} ago", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1), format!("{:.4}", __lp_arg_2), format!("{}", __lp_arg_3), format!("{}", __lp_arg_4), format!("{}", __lp_arg_5)]) };
    if !r.raw_hex.is_empty() {
        hover.push_str(&format!("\n\n{}", r.raw_hex));
    }
    hover.push_str(&crate::language_plugin::text("window.ism.text_617_524ec2", "\n\nClick to tune the receiver to it"));
    resp.on_hover_text(hover)
}

#[cfg(test)]
mod language_ism_header_tests {
    use super::*;

    #[test]
    fn rtl433_header_status_translates_and_falls_back_without_radio_activity() {
        for enabled in [true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let reason = "rtl_433 could not start";
            let shown = rtl433_status_tail(Some(reason), None, 0.0);
            assert_eq!(shown, format!("rtl_433  —  {}", if enabled { "rtl_433 无法启动" } else { reason }));
            assert_eq!(rtl433_status_tail(None, Some("433 MHz"), 250_000.0), "rtl_433 433 MHz  ·  250 kHz");
            assert_eq!(rtl433_status_tail(None, None, 0.0), "rtl_433");
        }
    }
    #[test]
    fn sensor_headers_render_without_changing_sort_key_or_direction() {
        for enabled in [true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (width,narrow) in [(360.0,true),(600.0,false),(1000.0,false)] {
                let ctx=egui::Context::default();let mut fonts=egui::FontDefinitions::default();
                crate::language_plugin::add_fonts(&mut fonts);ctx.set_fonts(fonts);
                let mut sort=IsmSort::Heard;let mut desc=true;
                let output=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width,100.0))),..Default::default()},|ui|ism_header(ui,narrow,&mut sort,&mut desc));
                let rendered:Vec<_>=output.shapes.iter().filter_map(|s|match &s.shape {egui::epaint::Shape::Text(t)=>Some(t.galley.job.text.clone()),_=>None}).collect();output.drop_without_applying_deltas();
                for (raw,zh) in [("age","时龄"),("device","设备"),("id","标识"),("readings","读数")] {
                    assert!(rendered.iter().any(|t|t.starts_with(if enabled {zh} else {raw})),"{rendered:?}");
                }
                assert_eq!(sort,IsmSort::Heard);assert!(desc);
            }
        }
    }
}
