//! The Profiles tab (issue #197): save the station's setup under a name, and
//! put it back on whole in one click.
//!
//! A profile captures the whole working setup — the dials and VFOs, the mode
//! and filters, the gains, drive and antennas, the callsign and message
//! templates, and the per-band registers — and applying one restores all of
//! it. The hardware (back end, audio devices, converters) is deliberately
//! left out, so a profile is the way the operator works the station rather
//! than the radio it runs on.
//!
//! The engine owns the store and answers every action with the fresh name
//! list, which is all this tab ever holds: the profiles themselves live with
//! everything else the radio remembers, wherever the radio happens to be.

use eframe::egui::{self, RichText};

use sdroxide_types::Command;

use crate::app::settings::SettingsIo;

pub(in crate::app) fn settings_profiles_tab(
    ui: &mut egui::Ui,
    io: &mut SettingsIo,
    cmds: &mut Vec<Command>,
    profiles: &[String],
) {
    use crate::theme;

    ui.label(
        RichText::new(crate::language_plugin::text("settings.profiles.profiles_the_station_s_saved_setups", "Profiles: the station's saved setups"))
            .size(14.0)
            .strong()
            .color(theme::CYAN()),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.profiles.a_profile_captures_the_whole_working_setup_the_dials", "A profile captures the whole working setup — the dials and VFOs, the mode \
             and filters, the gains, drive and antennas, your callsign and message \
             templates, and the per-band registers — and applies it back in one click. \
             The hardware (back end, audio devices, converters) is deliberately left \
             alone: a profile is the way you work the station, not the radio it runs on."),
        )
        .weak(),
    );
    ui.add_space(10.0);

    ui.label(RichText::new(crate::language_plugin::text("settings.profiles.save_the_current_setup", "Save the current setup")).strong());
    ui.horizontal(|ui| {
        crate::chrome::field(
            ui,
            egui::TextEdit::singleline(io.profile_name).hint_text(crate::language_plugin::text("settings.profiles.profile_name", "profile name")),
        );
        if !io.profile_name.trim().is_empty() && ui.button(crate::language_plugin::text("settings.profiles.save", "Save")).clicked() {
            cmds.push(Command::ProfileSave(io.profile_name.trim().to_string()));
            io.profile_name.clear();
        }
    });
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.profiles.applies_to_whatever_sdroxide_is_attached_to_dials_and", "Applies to whatever sdroxide is attached to: dials and VFOs, mode and \
             filters, levels, gains, drive, antennas, the digital identity and the band \
             stacks."),
        )
        .weak(),
    );
    ui.add_space(12.0);

    if profiles.is_empty() {
        ui.label(RichText::new(crate::language_plugin::text("settings.profiles.no_profiles_saved_yet_name_one_above_and_press", "No profiles saved yet — name one above and press Save.")).weak());
        return;
    }

    ui.label(RichText::new(crate::language_plugin::text("settings.profiles.saved_profiles", "Saved profiles")).strong());
    ui.add_space(4.0);
    for name in profiles {
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).strong());
            if ui
                .button(crate::language_plugin::text("settings.profiles.apply", "Apply"))
                .on_hover_text(crate::language_plugin::format("settings.profiles.put_the_station_back_onto_name_dials_vfos_mode", "Put the station back onto \u{201c}{name}\u{201d}: dials, VFOs, mode, filters, \
                 gains, drive, antennas, identity and band stacks.", &[format!("{name}")]))
                .clicked()
            {
                cmds.push(Command::ProfileApply(name.clone()));
                // The engine rewrites the digital identity in place; its answer
                // re-seeds this screen's copy of it, or the next edit here
                // would put the old callsign back.
                *io.digi_reseed = true;
            }
            if ui.button("✕").on_hover_text(crate::language_plugin::format("settings.profiles.delete_name", "Delete \u{201c}{name}\u{201d}", &[format!("{name}")])).clicked() {
                cmds.push(Command::ProfileDelete(name.clone()));
            }
        });
        ui.add_space(2.0);
    }
}
