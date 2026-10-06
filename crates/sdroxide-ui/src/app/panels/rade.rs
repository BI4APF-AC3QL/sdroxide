//! The FreeDV / RADE digital-voice panel.
//!
//! There is no text to show and no decode list to draw, so the panel is a
//! status view: sync state, SNR, and the reporting the mode does to FreeDV
//! Reporter on the operator's behalf.

use eframe::egui::{self, RichText};
use sdroxide_types::Command;

use crate::app::{SdroxideApp, tx_gated};

impl SdroxideApp {
    pub(in crate::app) fn rade_panel(
        &mut self,
        ui: &mut egui::Ui,
        cmds: &mut Vec<Command>,
        _panel_h: f32,
    ) {
        let status = self.digi_status.clone();
        let rade = status.as_ref().and_then(|s| s.rade).unwrap_or_default();
        let transmitting = status.as_ref().map(|s| s.transmitting).unwrap_or(false);
        // The last callsign decoded from a remote End-of-Over frame. The
        // controller has always carried it in `dx_call` (and reported it to
        // FreeDV Reporter from the same place) — this is where it reaches the
        // operator. It is `None` while transmitting: our own over is not a
        // station being heard.
        let dx_call = status.as_ref().and_then(|s| s.dx_call.clone());

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("RADE").size(11.0).strong().color(crate::theme::CYAN()));
            ui.label(
                RichText::new(crate::language_plugin::text("panels.rade.text_32_3ae956", "FreeDV V1 digital voice")).size(10.5).color(crate::theme::CYAN_DIM()),
            );
            crate::chrome::row_tail(ui, |ui| {
                if transmitting {
                    ui.label(
                        RichText::new(crate::language_plugin::text("panels.rade.text_37_548c99", "● TX")).size(11.0).strong().color(crate::theme::ALERT()),
                    );
                    ui.add_space(8.0);
                }
                // Silence the raw signal, leaving only decoded speech audible.
                let muted = self.digi_cfg_edit.rade_mute_analog;
                let resp = crate::chrome::chip(ui, muted, RichText::new(crate::language_plugin::text("panels.rade.text_43_a16f1b", "MUTE ANALOG")).size(10.5));
                if resp.clicked() && self.digi_cfg_seeded {
                    self.digi_cfg_edit.rade_mute_analog = !muted;
                    cmds.push(Command::SetDigiConfig(self.digi_cfg_edit.clone()));
                }
                resp.on_hover_text(
                    crate::language_plugin::text("panels.rade.text_49_15a7f5", "Mute the demodulated audio, so only decoded speech is heard. \
                     The raw signal is otherwise passed through whenever the modem \
                     has nothing to play — that hiss is how you find an over before \
                     it syncs, so leave this off while tuning."),
                );
            });
        });
        ui.add_space(8.0);

        // Sync lamp + link readouts.
        ui.horizontal(|ui| {
            let (lamp, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            let lit = rade.sync && !transmitting;
            ui.painter_at(lamp).circle_filled(
                lamp.center(),
                5.5,
                if lit { crate::theme::GREEN() } else { crate::theme::gray(48) },
            );
            ui.label(
                RichText::new(if transmitting {
                    crate::language_plugin::text("panels.rade.text_69_504220", "transmitting")
                } else if rade.sync {
                    crate::language_plugin::text("panels.rade.text_71_7dcad6", "SYNC")
                } else {
                    crate::language_plugin::text("panels.rade.text_73_848333", "searching")
                })
                .size(12.0)
                .strong()
                .color(if lit {
                    crate::theme::GREEN()
                } else {
                    crate::theme::gray(130)
                }),
            );
            ui.add_space(16.0);
            let dim = crate::theme::gray(150);
            if rade.sync && !transmitting {
                ui.label(
                    RichText::new(crate::language_plugin::format("panels.rade.snr_reading", "SNR {:.0} dB", &[format!("{:.0}", rade.snr_db)]))
                        .size(12.0)
                        .color(crate::theme::TEXT_STRONG()),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new({ let __lp_arg_0 = &(rade.freq_offset_hz); crate::language_plugin::format("panels.rade.text_93_7613f4", "offset {:+.0} Hz", &[format!("{:+.0}", __lp_arg_0)]) })
                        .size(11.0)
                        .color(dim),
                )
                .on_hover_text(
                    crate::language_plugin::text("panels.rade.text_98_044ea7", "How far the received signal sits from where the modem expects it. \
                     Large values still decode — the acquisition loop tracks them — but \
                     nudging the dial to bring this near zero gives the best margin."),
                );
            } else {
                ui.label(RichText::new(crate::language_plugin::text("panels.rade.snr_unavailable", "SNR —")).size(12.0).color(dim));
            }
            // Who we last heard, from the End-of-Over frame that closed their
            // over — the only time a RADE station names itself. Held for a
            // minute past the signal, then dropped by the controller.
            if let Some(call) = &dx_call {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(crate::language_plugin::format("panels.rade.text_111_babbb2", "heard {call}", &[format!("{call}")]))
                        .size(12.0)
                        .strong()
                        .color(crate::theme::GREEN()),
                )
                .on_hover_text(
                    crate::language_plugin::text("panels.rade.text_117_171c68", "The callsign the station put in its End-of-Over frame — the only \
                     point in a RADE over at which one is sent. It is also what is \
                     reported to FreeDV Reporter, so a station you hear appears on \
                     qso.freedv.org as heard by you.\n\n\
                     Held for a minute after the signal goes, and dropped when you \
                     start an over of your own: it names the station on frequency, \
                     not every station the receiver has ever heard."),
                );
            }
        });
        ui.add_space(8.0);

        // Decoded-speech level.
        {
            let (bar, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
            let p = ui.painter_at(bar);
            p.rect_filled(bar, 0.0, crate::theme::gray(22));
            let level = rade.rx_level.clamp(0.0, 1.0);
            if level > 0.0 && !transmitting {
                let mut fill = bar;
                fill.set_width(bar.width() * level);
                p.rect_filled(fill, 0.0, crate::theme::CYAN());
            }
        }
        ui.add_space(12.0);

        // Transmit. `DigiTxActive` is the same command the main PTT button ends
        // up sending in this mode, so the two stay in step.
        let tx_ok = self.tx_capable();
        ui.horizontal(|ui| {
            let label = if transmitting { crate::language_plugin::text("panel23.app_panels_rade.text_148_8063a7", "STOP TALKING") } else { crate::language_plugin::text("panel23.app_panels_rade.text_148_5c791d", "TALK (PTT)") };
            let resp = tx_gated(ui, tx_ok, |ui| {
                crate::chrome::chip_accent(
                    ui,
                    transmitting,
                    RichText::new(label).size(13.0).strong(),
                    crate::theme::ALERT(),
                    crate::theme::TEXT_STRONG(),
                )
            });
            if resp.clicked() {
                cmds.push(Command::DigiTxActive(!transmitting));
            }
            resp.on_hover_text(
                crate::language_plugin::text("panels.rade.text_162_6e5a31", "Open or close a RADE over. The modem needs ~120 ms of speech before \
                 the first frame goes out, and sends an end-of-over frame when you stop, \
                 so transmit runs on a little past the button."),
            );
            ui.add_space(10.0);
        });
        ui.add_space(12.0);

        ui.label(
            RichText::new(
                crate::language_plugin::text("panels.rade.text_172_a4d154", "RADE V1 occupies roughly 1060–1880 Hz of the passband — upper sideband, \
                 or lower on 160, 80 and 40 m, where RADE keeps phone practice and \
                 sdroxide switches for you. Put the signal inside the shaded band on the \
                 waterfall; the modem finds it from there."),
            )
            .size(10.5)
            .color(crate::theme::gray(125)),
        );
        if rade.dropped > 0 {
            ui.add_space(6.0);
            ui.label(
                RichText::new({ let __lp_arg_0 = &(rade.dropped); crate::language_plugin::format("panels.rade.text_184_206f9a", "⚠ {} samples dropped between the receiver and the decoder — this \
                     machine is not keeping up with the neural decode.", &[format!("{}", __lp_arg_0)]) })
                .size(10.5)
                .color(crate::theme::YELLOW()),
            );
        }
    }
}
