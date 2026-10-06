//! The General tab's sound-device pickers and the server's sign-in credentials.
//!
//! The operator's own card is always shown; the radio's is only relevant to
//! the CAT / Audio interface, since every other backend carries its audio
//! in-band.

use eframe::egui::{self, Color32, ComboBox, RichText};
use sdroxide_types::{Command, Region, RemoteAccess, SWR_LIMIT_MAX, SWR_LIMIT_MIN, swr_tune_limit};

use crate::app::SdroxideApp;
use crate::app::persist::band_plan_path;
use crate::chrome::StyledCombo;

/// The IARU region dropdown: the number the band plans are published under,
/// with the part of the world it covers next to it.
///
/// Both, because neither alone identifies it for most operators — "Region 2"
/// means nothing until you know it is the Americas, and the number is what
/// every band-plan document and contest rule actually says.
pub(in crate::app) fn region_combo(ui: &mut egui::Ui, region: &mut Region) {
    ComboBox::from_id_salt("iaru-region").width(360.0).selected_text(crate::language_plugin::display_label(region.label())).show_styled(
        ui,
        |ui| {
            for r in Region::ALL {
                if ui.selectable_label(*region == r,crate::language_plugin::display_label(r.label())).clicked() {
                    *region = r;
                }
            }
        },
    );
}

/// A device dropdown ("System default" + names); calls `pick(Some(name)|None)`.
pub(in crate::app) fn device_combo(
    ui: &mut egui::Ui,
    id: &str,
    names: &[String],
    selected: &Option<String>,
    mut pick: impl FnMut(Option<String>),
) {
    let shown = selected.clone().unwrap_or_else(|| crate::language_plugin::text("settings.general.system_default", "System default").into());
    ComboBox::from_id_salt(id).width(300.0).selected_text(shown).show_styled(ui, |ui| {
        if ui.selectable_label(selected.is_none(), crate::language_plugin::text("settings.general.system_default", "System default")).clicked() {
            pick(None);
        }
        for n in names {
            if ui.selectable_label(selected.as_deref() == Some(n), n).clicked() {
                pick(Some(n.clone()));
            }
        }
    });
}

/// Who may connect to this machine's server (`[remote_access]` in
/// `config.toml`).
///
/// Only drawn when the engine is in this process. These are a file on the
/// machine the radio is attached to: a remote client has nothing here to read
/// them from, and offering it a box that writes to its own disk instead would
/// be worse than offering nothing — it would look as though the station's
/// password had been changed when it had not.
///
/// Written as it is typed, like the control bindings, rather than behind an
/// APPLY: the server re-reads the file for every sign-in, so there is no
/// separate step for an APPLY to stand for.
pub(in crate::app) fn remote_access_settings(ui: &mut egui::Ui, access: &mut RemoteAccess) {
    ui.label(RichText::new(crate::language_plugin::text("settings.general.remote_access", "Remote access")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.general.what_a_remote_client_the_browser_page_or_another", "What a remote client — the browser page, or another sdroxide started with \
             --connect — has to give before this station will let it operate. Applies in server \
             mode (--server); the next sign-in picks up a change, with no restart."),
        )
        .size(11.5)
        .weak(),
    );
    ui.add_space(8.0);
    egui::Grid::new("remote-access-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(crate::language_plugin::text("settings.general.username", "Username"));
        crate::chrome::field(
            ui,
            egui::TextEdit::singleline(&mut access.username).desired_width(200.0),
        );
        ui.end_row();
        ui.label(crate::language_plugin::text("settings.general.password", "Password"));
        crate::chrome::field(
            ui,
            egui::TextEdit::singleline(&mut access.password).password(true).desired_width(200.0),
        );
        ui.end_row();
    });
    ui.add_space(6.0);
    // The state of the door, said plainly. "Both boxes empty means anyone may
    // key my transmitter" is not something an operator should have to infer.
    if access.is_enforced() {
        if access.username.is_empty() {
            ui.label(
                RichText::new(
                    crate::language_plugin::text("settings.general.clients_must_give_the_password_leaving_the_username_empty", "Clients must give the password. Leaving the username empty is fine."),
                )
                .size(11.5)
                .color(crate::theme::GREEN()),
            );
        } else {
            ui.label(
                RichText::new(crate::language_plugin::text("settings.general.clients_must_sign_in", "Clients must sign in.")).size(11.5).color(crate::theme::GREEN()),
            );
        }
    } else {
        ui.label(
            RichText::new(
                crate::language_plugin::text("settings.general.empty_anyone_who_can_reach_the_server_s_port", "⚠ Empty: anyone who can reach the server's port can operate this radio, on \
                 your callsign. Set a password before forwarding the port."),
            )
            .size(11.5)
            .color(crate::theme::YELLOW()),
        );
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.general.stored_in_the_clear_in_config_toml_like_the", "Stored in the clear in config.toml, like the other passwords sdroxide keeps."),
        )
        .size(10.5)
        .color(crate::theme::gray(140)),
    );
}

/// The fixed trim on this radio's receive audio.
///
/// Lives beside the sound-card pickers because that is what it is for: the AF
/// rail on the strip tops out at unity, which can turn a radio down and never
/// up, and a transceiver whose USB codec puts out a quiet signal is then quiet
/// at full volume in sdroxide *and* in the operating system (issue #315). It is
/// per radio, because what it corrects is that radio's interface rather than
/// how loudly anyone wants to listen.
///
/// Edited straight into `radio_edit`, which the settings window sends and saves
/// on any change — so it takes effect while the operator is listening, with no
/// Apply and no reopen.
pub(in crate::app) fn settings_rx_audio_gain(
    ui: &mut egui::Ui,
    cfg: &mut sdroxide_types::RadioConfig,
) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(crate::language_plugin::text("settings.general.receive_audio_gain", "Receive audio gain")).strong());
        ui.add(
            egui::DragValue::new(&mut cfg.rx_audio_gain_db)
                .speed(0.5)
                .range(-20.0..=30.0)
                .fixed_decimals(1)
                .suffix(" dB"),
        );
        if cfg.rx_audio_gain_db != 0.0 && ui.button("0 dB").clicked() {
            cfg.rx_audio_gain_db = 0.0;
        }
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.general.extra_gain_on_everything_this_radio_sends_to_the", "Extra gain on everything this radio sends to the speakers, on top of the volume \
             control. Leave it at 0 dB unless the radio is quiet at full volume: the volume \
             rail's top is the audio as it arrives, so it can turn a radio down but never up, \
             and some transceivers' USB sound output sits well below full scale.\n\n\
             Go up 6 dB at a time. Too much clips — the audio is limited at full scale rather \
             than allowed to wrap round, so overdoing it sounds harsh rather than loud. \
             Recordings are taken ahead of this and are not affected."),
        )
        .size(10.5)
        .color(crate::theme::gray(140)),
    );
}

impl SdroxideApp {
    /// The band-plan file: where it is, whether the station is on it, and the
    /// button that re-reads it.
    ///
    /// No editor here on purpose. A band plan is forty rows of numbers that
    /// exist to be pasted from a published table and compared against one; a
    /// text editor does that far better than any grid this dialog could hold,
    /// and the file is written to be readable. What the dialog owes the
    /// operator is the path, and a way to apply an edit without restarting.
    pub(in crate::app) fn settings_band_plan_file(
        &self,
        ui: &mut egui::Ui,
        cmds: &mut Vec<sdroxide_types::Command>,
    ) {
        let custom = !sdroxide_types::band_plan().is_default();
        ui.horizontal_wrapped(|ui| {
            if crate::chrome::chip(ui, false, RichText::new(crate::language_plugin::text("settings.general.reload_band_plan", "RELOAD BAND PLAN")).size(10.5))
                .on_hover_text(
                    crate::language_plugin::text("settings.general.re_read_bandplan_json_on_the_machine_the_radio", "Re-read bandplan.json on the machine the radio is attached to and apply \
                     it — band edges, sub-segments and skimmer windows — without restarting."),
                )
                .clicked()
            {
                cmds.push(sdroxide_types::Command::ReloadBandPlan);
            }
            ui.add(
                egui::Label::new(
                    RichText::new(if custom {
                        crate::language_plugin::text("settings.general.running_on_the_station_s_own_band_plan", "Running on the station's own band plan.")
                    } else {
                        crate::language_plugin::text("settings.general.running_on_the_built_in_iaru_tables", "Running on the built-in IARU tables.")
                    })
                    .size(10.5)
                    .weak(),
                )
                .wrap(),
            );
        });
        ui.add_space(4.0);
        // Only where the file is actually on this machine. A remote client
        // showing its own config path would be pointing at the wrong computer.
        let path = (!self.ctrl.engine_is_remote())
            .then(band_plan_path)
            .flatten()
            .map(|p| p.display().to_string());
        ui.label(
            RichText::new(match &path {
                Some(p) => crate::language_plugin::format("settings.general.every_band_edge_and_sub_segment_comes_from_p", "Every band edge and sub-segment comes from {p}, written from the built-in \
                     IARU tables the first time and yours to edit after that — narrow a band to \
                     your licence and sdroxide will refuse to transmit outside it. Frequencies \
                     are in MHz; delete the file for a fresh copy of the defaults. This is the \
                     regional allocation, not your licence: your own conditions may be narrower, \
                     and national plans differ inside a region.", &[format!("{p}")]),
                None => crate::language_plugin::text("settings.general.every_band_edge_and_sub_segment_comes_from_bandplan", "Every band edge and sub-segment comes from bandplan.json on the machine \
                         the radio is attached to. This is the regional allocation, not your \
                         licence: your own conditions may be narrower, and national plans differ \
                         inside a region.")
                    .to_string(),
            })
            .size(10.5)
            .color(crate::theme::gray(140)),
        );
    }

    /// Take this station's settings away as one file, and put one back.
    ///
    /// The answer to "how do I copy all this to my other machine" being
    /// "screenshots" (issue #356). Native only, and only where the settings are
    /// on *this* machine: a browser client has no filesystem, and a remote one
    /// would be exporting its own laptop's configuration rather than the
    /// station's, which is the opposite of what was asked for.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn settings_transfer(
        &self,
        ui: &mut egui::Ui,
        export: &mut bool,
        import: &mut bool,
    ) {
        ui.label(RichText::new(crate::language_plugin::text("settings.general.settings_file", "Settings file")).strong());
        if self.ctrl.engine_is_remote() {
            ui.label(
                RichText::new(
                    crate::language_plugin::text("settings.general.the_settings_are_on_the_machine_the_radio_is", "The settings are on the machine the radio is attached to. Export them \
                     there."),
                )
                .size(10.5)
                .color(crate::theme::gray(140)),
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if crate::chrome::chip(ui, false, RichText::new(crate::language_plugin::text("settings.general.export", "EXPORT…")).size(10.5))
                .on_hover_text(
                    crate::language_plugin::text("settings.general.write_every_setting_at_this_station_the_radios_the", "Write every setting at this station — the radios, the modes, the servers, \
                     the memories, the band plan — to one file you can carry to another \
                     installation. Your logbook and any saved server password stay here."),
                )
                .clicked()
            {
                *export = true;
            }
            if crate::chrome::chip(ui, false, RichText::new(crate::language_plugin::text("settings.general.import", "IMPORT…")).size(10.5))
                .on_hover_text(
                    crate::language_plugin::text("settings.general.replace_this_station_s_settings_with_the_ones_in", "Replace this station's settings with the ones in a file exported from \
                     another installation. Restart sdroxide afterwards."),
                )
                .clicked()
            {
                *import = true;
            }
        });
        if let Some(note) = &self.settings_transfer_note {
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(RichText::new(note).size(10.5).color(Color32::LIGHT_GREEN)).wrap(),
            );
        }
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                RichText::new(
                    crate::language_plugin::text("settings.general.an_import_overwrites_what_is_here_file_for_file", "An import overwrites what is here, file for file, and takes effect the \
                     next time sdroxide starts — the settings already in memory would \
                     otherwise be written straight back over it. Anything the file does not \
                     mention is left as it is, so a bundle from a one-radio station does not \
                     remove a second radio here. Your logbook is never in the file: export it \
                     as ADIF from the LOG window if you want to move that too."),
                )
                .size(10.5)
                .color(crate::theme::gray(140)),
            )
            .wrap(),
        );
    }

    /// The browser client has no filesystem, and the settings it would export
    /// are on the engine's machine in any case.
    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn settings_transfer(
        &self,
        _ui: &mut egui::Ui,
        _export: &mut bool,
        _import: &mut bool,
    ) {
    }

    /// Carry out what [`Self::settings_transfer`]'s buttons asked for, after
    /// the window closure has given `&mut self` back — see [`SettingsIo`].
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn run_settings_transfer(&mut self, export: bool, import: bool) {
        if export {
            self.settings_transfer_note = Some(match sdroxide_config::transfer::export_json() {
                Ok(json) => {
                    let note = sdroxide_config::transfer::export()
                        .map(|b| crate::language_plugin::bundle_summary(&b))
                        .unwrap_or_else(|_| crate::language_plugin::scope_text("display.config.fallback.", "settings"));
                    crate::download::save("sdroxide-settings.json", json.as_bytes());
                    crate::language_plugin::format("settings.general.exported_note_choose_where_to_save_it", "Exported {note} — choose where to save it.", &[format!("{note}")])
                }
                Err(e) => crate::language_plugin::format("settings.general.export_failed_e", "Export failed: {e}", &[crate::language_plugin::config_error(&e)]),
            });
        }
        if import {
            self.settings_transfer_note = None;
            crate::download::load_text(
                &crate::language_plugin::text("settings.general.sdroxide_settings", "sdroxide settings"),
                &["json"],
                self.settings_import_inbox.clone(),
            );
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn run_settings_transfer(&mut self, _export: bool, _import: bool) {}

    /// Apply a settings bundle the operator picked, once the picker thread has
    /// delivered it. Drained every frame beside the ADIF import.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn poll_settings_import(&mut self) {
        let loaded = self.settings_import_inbox.lock().ok().and_then(|mut g| g.take());
        let Some(loaded) = loaded else { return };
        // Said on screen rather than on stderr: the operator who pressed the
        // button is looking at the window, and on Windows there is no console
        // behind it to print to.
        self.settings_transfer_note = Some(match loaded {
            Err(e) => crate::language_plugin::format("settings.general.import_failed_e", "Import failed: {e}", &[e.display()]),
            Ok(loaded) => match sdroxide_config::transfer::import(&loaded.text) {
                Err(e) => crate::language_plugin::format("settings.general.import_failed_e", "Import failed: {e}", &[crate::language_plugin::config_error(&e)]),
                Ok(report) => {
                    let mut msg = { let __lp_arg_0 = &(crate::language_plugin::import_report_summary(&report)); crate::language_plugin::format("settings.general.restart_sdroxide_to_use_them", "{} — restart sdroxide to use them.", &[format!("{}", __lp_arg_0)]) };
                    for (path, why) in report.skipped.iter().take(4) {
                        msg.push_str(&crate::language_plugin::format("settings.general.skipped_path_why", "\nSkipped {path}: {why}", &[format!("{path}"), crate::language_plugin::transfer_skip_reason(why)]));
                    }
                    msg
                }
            },
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn poll_settings_import(&mut self) {}

    /// The SWR guard: arm it, and set the ratio it stops transmitting at.
    ///
    /// Reads the live values out of the broadcast TX state rather than off
    /// disk, so a remote client shows the radio's setting and not its own
    /// machine's `config.toml`, which would be a different antenna entirely.
    /// The command is sent only on an actual change, since a `DragValue` reports
    /// its value every frame it is dragged and each one would be a config write.
    pub(in crate::app) fn settings_swr_guard(&self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        ui.label(RichText::new(crate::language_plugin::text("settings.general.swr_guard", "SWR guard")).strong());
        ui.add_space(4.0);

        let mut enabled = self.state.tx.swr_guard;
        // The engine clamps this too; matching the range here keeps the widget
        // from offering a value that would come back changed.
        let mut limit = self.state.tx.swr_limit.clamp(SWR_LIMIT_MIN, SWR_LIMIT_MAX);

        ui.horizontal(|ui| {
            if crate::chrome::checkbox(ui, &mut enabled, crate::language_plugin::text("settings.general.stop_transmitting_on_high_swr", "Stop transmitting on high SWR")).changed()
            {
                cmds.push(Command::SetSwrGuard { enabled, limit });
            }
        });
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label(crate::language_plugin::text("settings.general.trip_at", "Trip at"));
                let r = ui.add(
                    egui::DragValue::new(&mut limit)
                        .speed(0.1)
                        .range(SWR_LIMIT_MIN..=SWR_LIMIT_MAX)
                        .fixed_decimals(1)
                        .suffix(":1"),
                );
                // `drag_stopped` and `lost_focus`, not `changed`: one command
                // per settled value rather than one per frame of the drag.
                if (r.drag_stopped() || r.lost_focus()) && limit != self.state.tx.swr_limit {
                    cmds.push(Command::SetSwrGuard { enabled, limit });
                }
                // The tune limit is derived from this one rather than typed, so
                // it is shown here: an operator who has just had a tune-up
                // stopped is told a figure, and this is where they find out
                // where it came from.
                ui.label(
                    RichText::new({ let __lp_arg_0 = &(swr_tune_limit(limit)); crate::language_plugin::format("settings.general.tuning_1_1", "(tuning: {:.1}:1)", &[format!("{:.1}", __lp_arg_0)]) })
                        .size(11.0)
                        .color(crate::theme::gray(140)),
                );
            });
        });

        ui.add_space(6.0);
        ui.label(
            RichText::new(
                crate::language_plugin::text("settings.general.stops_the_transmission_when_the_radio_reports_an_swr", "Stops the transmission when the radio reports an SWR at or above this figure, and \
                 keeps transmit locked out until you acknowledge it. Catches a disconnected \
                 antenna, a failed feeder, or a switch left on the wrong port.\n\n\
                 Tuning is treated differently, because feeding a mismatch is the point of it: an \
                 antenna tuner gets double the limit and about five seconds before the guard \
                 applies at all. A manual tuner that takes longer than that wants the guard \
                 switched off for the session.\n\n\
                 Needs a rig that reports SWR over CAT. Ignores the first fifth of a second of \
                 each transmission, and does not wait for high power."),
            )
            .size(10.5)
            .color(crate::theme::gray(140)),
        );
        if let Some(swr) = self.state.tx.swr_tripped {
            ui.add_space(4.0);
            ui.label(
                RichText::new(crate::language_plugin::format("settings.general.currently_tripped_at_swr_1_1_transmit_is_locked", "⚠ Currently tripped at {swr:.1}:1 — transmit is locked out.", &[format!("{swr:.1}")]))
                .size(11.0)
                .color(Color32::from_rgb(255, 190, 70)),
            );
        }
    }

    /// Forget every mode's remembered settings overrides.
    ///
    /// The **DEFAULTS** chip beside the receiver controls puts one mode back;
    /// this is the way back from a long session of fiddling without walking
    /// every mode. A command rather than a config edit, so it reaches whichever
    /// engine is running — local or remote — and undoes itself.
    pub(in crate::app) fn settings_mode_defaults(
        &self,
        ui: &mut egui::Ui,
        cmds: &mut Vec<sdroxide_types::Command>,
    ) {
        ui.label(RichText::new(crate::language_plugin::text("settings.general.per_mode_settings", "Per-mode settings")).strong());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if crate::chrome::chip(ui, false, RichText::new(crate::language_plugin::text("settings.general.reset_every_mode", "RESET EVERY MODE")).size(10.5))
                .on_hover_text(
                    crate::language_plugin::text("settings.general.forget_every_mode_s_remembered_agc_squelch_noise_reduction", "Forget every mode's remembered AGC, squelch, noise reduction, notch and \
                     stereo switches, and put each mode's own defaults back."),
                )
                .clicked()
            {
                cmds.push(sdroxide_types::Command::ResetModeDefaults { mode: None });
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                crate::language_plugin::text("settings.general.selecting_a_mode_lays_that_mode_s_own_starting", "Selecting a mode lays that mode's own starting values for AGC, squelch, noise \
                 reduction, the notch and the stereo switches on the receiver, and changing one \
                 remembers it for that mode alone. The circular-arrow chip at the end of the \
                 receiver's filter/noise row lists what has been changed and puts the current \
                 mode back."),
            )
            .size(10.5)
            .color(crate::theme::gray(140)),
        );
    }

    /// The user's own speakers / microphone (applied live).
    pub(in crate::app) fn settings_user_audio(
        &self,
        ui: &mut egui::Ui,
        audio_pick: &mut Option<(bool, Option<String>)>,
    ) {
        let Some(devs) = &self.audio_devices else {
            return;
        };
        ui.label(RichText::new(crate::language_plugin::text("settings.general.your_audio_speakers_microphone", "Your audio (speakers / microphone)")).strong());
        egui::Grid::new("user-audio").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("settings.general.output", "Output"));
            device_combo(ui, "u-out", &devs.outputs, &devs.selected_output, |n| {
                *audio_pick = Some((true, n))
            });
            ui.end_row();
            ui.label(crate::language_plugin::text("settings.general.input", "Input"));
            device_combo(ui, "u-in", &devs.inputs, &devs.selected_input, |n| {
                *audio_pick = Some((false, n))
            });
            ui.end_row();
        });
    }
}
