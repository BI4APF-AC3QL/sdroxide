//! The Controls tab: keyboard, mouse and MIDI bindings.
//!
//! A rebind has no APPLY step — it takes effect on the next frame and is
//! written straight out, because a binding the operator cannot see saved is
//! one they will make again after the next restart. MIDI learn works the same
//! way: arm a row, move the control, and the row captures whatever arrives.

use eframe::egui::{self, Color32, ComboBox, RichText};
use sdroxide_types::MemoryChannel;

use crate::app::settings::SettingsIo;
use crate::chrome::StyledCombo;

/// The built-in TCI *server*: this app acting as a TCI rig for third-party
/// clients (WSJT-X's TCI rig type, JTDX, MSHV, skimmers). Distinct from the TCI
/// *client* section on the Radio tab, which connects sdroxide to another rig.

/// A dropdown over every bindable [`Action`], grouped by section. `memories`
/// contributes one recall entry per stored channel, since those are the only
/// actions whose parameter comes from the operator's own data.
fn action_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    action: &mut sdroxide_types::Action,
    memories: &[MemoryChannel],
) -> bool {
    use sdroxide_types::Action;
    let mut changed = false;
    ComboBox::from_id_salt(id).width(210.0).selected_text(crate::language_plugin::action_text(*action)).show_styled(ui, |ui| {
        let mut group = "";
        let all =
            Action::all().into_iter().chain(memories.iter().map(|m| Action::MemoryRecall(m.id)));
        for a in all {
            if a.group() != group {
                group = a.group();
                ui.add_space(4.0);
                ui.label(RichText::new(crate::language_plugin::display_label(group)).small().weak());
            }
            if ui.selectable_label(*action == a, crate::language_plugin::action_text(a)).clicked() {
                *action = a;
                changed = true;
            }
        }
    });
    changed
}

/// Keyboard chords, panadapter mouse behaviour and mouse-button bindings.
///
/// Edits here are live and self-persisting; there is no APPLY chip, because a
/// binding is only useful once it is already in effect.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn settings_controls_tab(
    ui: &mut egui::Ui,
    io: &mut SettingsIo,
    memories: &[MemoryChannel],
    midi_in: &[(String, String)],
    midi_out: &[(String, String)],
    midi_status: &crate::input::MidiStatusView,
    last_midi: Option<(sdroxide_types::MidiMsg, u8)>,
) {
    use sdroxide_types::{
        Action, ActionKind, ButtonMode, KeyBinding, MouseButton, MouseButtonBinding, WheelAction,
        WheelSettings,
    };
    let cfg = &mut *io.input_edit;
    let key_capture = &mut *io.key_capture;

    ui.label(RichText::new(crate::language_plugin::text("common.keyboard", "Keyboard")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.controls.text_73", "Click a shortcut to rebind it, then press the key combination (Esc cancels). \
             Bindings are ignored while you are typing in a text field."),
        )
        .weak(),
    );
    ui.add_space(6.0);

    let mut remove: Option<usize> = None;
    egui::Grid::new("keys-grid").num_columns(6).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
        ui.label(RichText::new(crate::language_plugin::text("common.shortcut", "Shortcut")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.does", "Does")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.step_mode", "Step / mode")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.accel", "Accel")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.on", "On")).small().weak());
        ui.label("");
        ui.end_row();

        for (i, b) in cfg.keys.iter_mut().enumerate() {
            let capturing = *key_capture == Some(i);
            let label = if capturing { crate::language_plugin::text("panel23.app_settings_controls.text_92_a05892", "press a key…").to_string() } else { crate::language_plugin::key_chord_label(&b.chord) };
            if crate::chrome::chip(ui, capturing, RichText::new(label).monospace()).clicked() {
                *key_capture = if capturing { None } else { Some(i) };
            }

            if action_combo(ui, ("keyact", i), &mut b.action, memories) {
                b.tuning.step = b.action.default_step();
            }

            match b.action.kind() {
                ActionKind::Continuous => {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut b.tuning.step)
                                .speed(1.0)
                                .range(0.0001..=1_000_000.0),
                        );
                        // The sign of `value` is the direction, so one
                        // action can have an up key and a down key.
                        let mut down = b.value < 0.0;
                        if crate::chrome::checkbox(ui, &mut down, crate::language_plugin::text("settings.controls.text_112", "down")).changed() {
                            b.value = if down { -1.0 } else { 1.0 };
                        }
                    });
                    ui.add(egui::DragValue::new(&mut b.tuning.accel).speed(0.05).range(0.0..=4.0));
                }
                ActionKind::Momentary => {
                    ui.horizontal(|ui| {
                        for m in ButtonMode::ALL {
                            if crate::chrome::chip(ui, b.button == m, m.label()).clicked() {
                                b.button = m;
                            }
                        }
                    });
                    ui.label("");
                }
            }

            crate::chrome::checkbox(ui, &mut b.enabled, "");
            if ui.small_button("✕").on_hover_text(crate::language_plugin::text("common.remove_this_binding", "Remove this binding")).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        cfg.keys.remove(i);
        if *key_capture == Some(i) {
            *key_capture = None;
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if crate::chrome::chip(ui, false, crate::language_plugin::text("common.add_shortcut", "+ Add shortcut")).clicked() {
            cfg.keys.push(KeyBinding::default());
            *key_capture = Some(cfg.keys.len() - 1);
        }
        if crate::chrome::chip(ui, false, crate::language_plugin::text("common.restore_defaults", "Restore defaults")).clicked() {
            cfg.keys = KeyBinding::defaults();
            *key_capture = None;
        }
        // PTT ships unbound on purpose; this is the one-click opt-in.
        let has_ptt = cfg.keys.iter().any(|b| b.action == Action::Ptt);
        if !has_ptt
            && crate::chrome::chip(ui, false, crate::language_plugin::text("common.bind_hold_to_talk_to_space", "Bind hold-to-talk to Space"))
                .on_hover_text(
                    crate::language_plugin::text("settings.controls.text_159", "Hold Space to transmit; releasing it — or losing window focus — unkeys"),
                )
                .clicked()
        {
            cfg.keys.push(KeyBinding {
                chord: sdroxide_types::KeyChord::plain("Space"),
                action: Action::Ptt,
                button: ButtonMode::Momentary,
                ..KeyBinding::default()
            });
        }
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(crate::language_plugin::text("common.unkey_a_held_ptt_after", "Unkey a held PTT after"));
        ui.add(
            egui::DragValue::new(&mut cfg.ptt_hold_timeout_s)
                .speed(5.0)
                .range(0.0..=3600.0)
                .suffix(" s"),
        );
    })
    .response
    .on_hover_text(
        crate::language_plugin::text("settings.controls.text_184", "Backstop against a stuck key or a controller that stops reporting. 0 disables."),
    );

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(RichText::new(crate::language_plugin::text("common.panadapter_mouse", "Panadapter mouse")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(6.0);

    let w = &mut cfg.wheel;
    egui::Grid::new("mouse-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(crate::language_plugin::text("common.wheel", "Wheel"));
        wheel_action_combo(ui, "wheel-plain", &mut w.wheel);
        ui.end_row();

        ui.label(crate::language_plugin::text("common.wheel_shift", "Wheel + Shift"));
        wheel_action_combo(ui, "wheel-shift", &mut w.wheel_shift);
        ui.end_row();

        ui.label(crate::language_plugin::text("common.tune_step", "Tune step"));
        ui.add(
            egui::DragValue::new(&mut w.tune_step_hz).speed(10.0).range(1.0..=1e6).suffix(" Hz"),
        );
        ui.end_row();

        ui.label(crate::language_plugin::text("common.zoom_rate", "Zoom rate"));
        ui.add(egui::DragValue::new(&mut w.zoom_rate).speed(0.05).range(0.1..=5.0));
        ui.end_row();

        ui.label(crate::language_plugin::text("common.click_tune_rounding", "Click-tune rounding"));
        ui.add(
            egui::DragValue::new(&mut w.click_tune_step_hz)
                .speed(1.0)
                .range(1.0..=10_000.0)
                .suffix(" Hz"),
        );
        ui.end_row();
    });
    ui.add_space(4.0);
    crate::chrome::checkbox(ui, &mut w.invert, crate::language_plugin::text("settings.controls.text_223", "Invert wheel direction"));
    crate::chrome::checkbox(ui, &mut w.drag_tunes, crate::language_plugin::text("settings.controls.text_224", "Left-drag tunes as well as pans"))
        .on_hover_text(crate::language_plugin::text("settings.controls.text_225", "Off makes left-drag pan the view only, like right-drag."));
    crate::chrome::checkbox(
        ui,
        &mut w.digit_wheel,
        crate::language_plugin::text("settings.controls.text_229", "Scroll a digit on the frequency readout to tune it"),
    );
    if w.wheel == WheelAction::Tune && w.wheel_shift == WheelAction::Tune {
        ui.label(
            RichText::new(crate::language_plugin::text("settings.controls.text_233", "Both wheel actions are Tune — there is no way left to zoom."))
                .color(Color32::from_rgb(230, 170, 60)),
        );
    }
    ui.add_space(6.0);
    if crate::chrome::chip(ui, false, crate::language_plugin::text("common.restore_mouse_defaults", "Restore mouse defaults")).clicked() {
        cfg.wheel = WheelSettings::default();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(RichText::new(crate::language_plugin::text("common.mouse_buttons", "Mouse buttons")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.controls.text_249", "The left and right buttons are reserved for tuning and panning; the middle and \
             extra buttons are free. A side button held for PTT behaves like a footswitch."),
        )
        .weak(),
    );
    ui.add_space(6.0);

    let mut remove: Option<usize> = None;
    egui::Grid::new("mousebtn-grid").num_columns(5).spacing([10.0, 6.0]).striped(true).show(
        ui,
        |ui| {
            for (i, b) in cfg.mouse_buttons.iter_mut().enumerate() {
                ComboBox::from_id_salt(("mb", i))
                    .width(130.0)
                    .selected_text(crate::language_plugin::display_label(b.button.label()))
                    .show_styled(ui, |ui| {
                        for m in MouseButton::ALL {
                            if ui.selectable_label(b.button == m, crate::language_plugin::display_label(m.label())).clicked() {
                                b.button = m;
                            }
                        }
                    });
                action_combo(ui, ("mbact", i), &mut b.action, memories);
                ui.horizontal(|ui| {
                    for m in ButtonMode::ALL {
                        if crate::chrome::chip(ui, b.button_mode == m, crate::language_plugin::display_label(m.label())).clicked() {
                            b.button_mode = m;
                        }
                    }
                });
                crate::chrome::checkbox(ui, &mut b.enabled, "");
                if ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        },
    );
    if let Some(i) = remove {
        cfg.mouse_buttons.remove(i);
    }
    ui.add_space(6.0);
    if crate::chrome::chip(ui, false, crate::language_plugin::text("common.add_mouse_button", "+ Add mouse button")).clicked() {
        cfg.mouse_buttons.push(MouseButtonBinding::default());
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(crate::language_plugin::text("settings.controls.text_297", "F1 always opens this manual, even while typing, so it is not rebindable."))
            .weak(),
    );

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    settings_midi_section(
        ui,
        cfg,
        io.midi_learn,
        io.midi_rescan,
        memories,
        midi_in,
        midi_out,
        midi_status,
        last_midi,
    );
}

/// MIDI control surfaces: port selection, a live message readout, and the
/// binding table with its LEARN capture.
#[allow(clippy::too_many_arguments)]
fn settings_midi_section(
    ui: &mut egui::Ui,
    cfg: &mut sdroxide_types::InputSettings,
    learn: &mut Option<crate::input::MidiLearn>,
    rescan: &mut bool,
    memories: &[MemoryChannel],
    midi_in: &[(String, String)],
    midi_out: &[(String, String)],
    status: &crate::input::MidiStatusView,
    last_midi: Option<(sdroxide_types::MidiMsg, u8)>,
) {
    use sdroxide_types::{ActionKind, ButtonMode, MidiBinding, RelativeMode};

    ui.label(RichText::new(crate::language_plugin::text("common.midi_controller", "MIDI controller")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(4.0);
    if !status.supported {
        ui.label(
            RichText::new(
                crate::language_plugin::text("settings.controls.text_338", "MIDI controllers need the native app — the browser client has no MIDI access."),
            )
            .weak(),
        );
        return;
    }
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.controls.text_346", "Any class-compliant MIDI surface works: a DJ controller's jog wheel makes a fine              VFO knob, its pads make PTT and band buttons, its faders make gain controls."),
        )
        .weak(),
    );
    ui.add_space(6.0);
    crate::chrome::checkbox(ui, &mut cfg.midi.enabled, crate::language_plugin::text("common.enable", "Enable"));
    ui.add_space(6.0);

    ui.add_enabled_ui(cfg.midi.enabled, |ui| {
        egui::Grid::new("midi-ports").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("common.controller", "Controller"));
            midi_port_combo(
                ui,
                "midi-in",
                midi_in,
                &mut cfg.midi.in_port_id,
                &mut cfg.midi.in_port_name,
            );
            ui.end_row();

            ui.label(crate::language_plugin::text("common.feedback_to", "Feedback to"));
            midi_port_combo(
                ui,
                "midi-out",
                midi_out,
                &mut cfg.midi.out_port_id,
                &mut cfg.midi.out_port_name,
            );
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if crate::chrome::chip(ui, false, crate::language_plugin::text("common.rescan_ports", "Rescan ports")).clicked() {
                *rescan = true;
            }
            if status.connected {
                ui.label(
                    RichText::new(format!("● {}", status.port))
                        .color(Color32::from_rgb(90, 200, 110)),
                );
            } else if let Some(e) = &status.error {
                ui.label(RichText::new(e).color(Color32::from_rgb(230, 90, 80)));
            } else {
                ui.label(RichText::new(crate::language_plugin::text("common.not_connected", "Not connected.")).weak());
            }
        });
        ui.add_space(4.0);
        // Naming the control that just moved is what makes an unlabelled
        // surface bindable at all.
        match last_midi {
            Some((msg, v)) => {
                ui.label(RichText::new({ let __lp_arg_0 = &(msg.label()); crate::language_plugin::format("common.last_message_value_v", "Last message: {}  value {v}", &[format!("{}", __lp_arg_0), format!("{v}")]) }).weak())
            }
            None => ui.label(RichText::new(crate::language_plugin::text("common.move_a_control_to_see_it_here", "Move a control to see it here.")).weak()),
        };
    });

    ui.add_space(8.0);
    let mut remove: Option<usize> = None;
    egui::Grid::new("midi-grid").num_columns(7).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
        ui.label(RichText::new(crate::language_plugin::text("common.control", "Control")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.does", "Does")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.reads_as", "Reads as")).small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.step_mode", "Step / mode")).small().weak());
        ui.label(RichText::new("LED").small().weak());
        ui.label(RichText::new(crate::language_plugin::text("common.on", "On")).small().weak());
        ui.label("");
        ui.end_row();

        for (i, b) in cfg.midi.bindings.iter_mut().enumerate() {
            let learning = learn.map(|l| l.row) == Some(i);
            let label = if learning { crate::language_plugin::text("panel23.app_settings_controls.text_417_c742e4", "move it…").to_string() } else { b.msg.label() };
            if crate::chrome::chip(ui, learning, RichText::new(label).monospace())
                .on_hover_text(crate::language_plugin::text("common.click_then_move_the_control_you_want_to_bind", "Click, then move the control you want to bind"))
                .clicked()
            {
                *learn = if learning { None } else { Some(crate::input::MidiLearn { row: i }) };
            }

            if action_combo(ui, ("midiact", i), &mut b.action, memories) {
                b.tuning.step = b.action.default_step();
            }

            match b.action.kind() {
                ActionKind::Continuous => {
                    ComboBox::from_id_salt(("midirel", i))
                        .width(170.0)
                        .selected_text(crate::language_plugin::display_label(b.relative.label()))
                        .show_styled(ui, |ui| {
                            for m in RelativeMode::ALL {
                                if ui.selectable_label(b.relative == m, crate::language_plugin::display_label(m.label())).clicked() {
                                    b.relative = m;
                                }
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut b.tuning.step)
                                .speed(1.0)
                                .range(0.0001..=1_000_000.0),
                        );
                        ui.add(
                            egui::DragValue::new(&mut b.tuning.accel)
                                .speed(0.05)
                                .range(0.0..=4.0)
                                .prefix("×"),
                        )
                        .on_hover_text(crate::language_plugin::text("common.speed_sensitivity_spin_faster_to_tune_faster", "Speed sensitivity: spin faster to tune faster"));
                        // Sign/magnitude and 64-centred encoders are
                        // indistinguishable from small movements, so a wrong
                        // guess shows up as a knob that turns the wrong way.
                        crate::chrome::checkbox(ui, &mut b.tuning.invert, crate::language_plugin::text("settings.controls.text_457", "rev"));
                    });
                }
                ActionKind::Momentary => {
                    ui.label("");
                    ui.horizontal(|ui| {
                        for m in ButtonMode::ALL {
                            if crate::chrome::chip(ui, b.button_mode == m, crate::language_plugin::display_label(m.label())).clicked() {
                                b.button_mode = m;
                            }
                        }
                    });
                }
            }

            crate::chrome::checkbox(ui, &mut b.feedback, "")
                .on_hover_text(crate::language_plugin::text("common.send_the_current_value_back_to_light_an_led_or_move_a", "Send the current value back, to light an LED or move a fader"));
            crate::chrome::checkbox(ui, &mut b.enabled, "");
            if ui.small_button("✕").clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        cfg.midi.bindings.remove(i);
        if learn.map(|l| l.row) == Some(i) {
            *learn = None;
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if crate::chrome::chip(ui, false, crate::language_plugin::text("common.add_midi_control", "+ Add MIDI control")).clicked() {
            cfg.midi.bindings.push(MidiBinding::default());
            *learn = Some(crate::input::MidiLearn { row: cfg.midi.bindings.len() - 1 });
        }
        if !cfg.midi.bindings.is_empty() && crate::chrome::chip(ui, false, crate::language_plugin::text("common.clear_all", "Clear all")).clicked() {
            cfg.midi.bindings.clear();
            *learn = None;
        }
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.controls.text_503", "Endless (jog) encoders send a relative step rather than a position, in one of three              encodings that look alike from small movements. LEARN guesses from a clockwise turn;              if the knob then tunes the wrong way, tick \u{201c}rev\u{201d}."),
        )
        .weak(),
    );
}

/// Port dropdown that keeps both the stable id and the human name — the id
/// reconnects across a replug, the name is the label and the fallback.
fn midi_port_combo(
    ui: &mut egui::Ui,
    id: &str,
    ports: &[(String, String)],
    sel_id: &mut String,
    sel_name: &mut String,
) {
    let shown = if sel_name.is_empty() { crate::language_plugin::text("panel23.app_settings_controls.text_518_13915b", "— none —") } else { sel_name.clone() };
    ComboBox::from_id_salt(id).width(280.0).selected_text(shown).show_styled(ui, |ui| {
        if ui.selectable_label(sel_name.is_empty(), crate::language_plugin::text("common.none", "— none —")).clicked() {
            sel_id.clear();
            sel_name.clear();
        }
        for (pid, name) in ports {
            if ui.selectable_label(sel_name == name, name).clicked() {
                *sel_id = pid.clone();
                *sel_name = name.clone();
            }
        }
    });
}

/// Dropdown over [`WheelAction`].
fn wheel_action_combo(ui: &mut egui::Ui, id: &str, act: &mut sdroxide_types::WheelAction) {
    ComboBox::from_id_salt(id).width(130.0).selected_text(crate::language_plugin::display_label(act.label())).show_styled(ui, |ui| {
        for a in sdroxide_types::WheelAction::ALL {
            if ui.selectable_label(*act == a,crate::language_plugin::display_label(a.label())).clicked() {
                *act = a;
            }
        }
    });
}

#[cfg(test)]
mod language_midi_port23_tests {
 use super::*;
 #[test]
 fn midi_empty_choice_is_localized_but_named_ports_and_configuration_stay_raw() {
  for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
   for name in ["", "— none —", "USB MIDI {id} 中文"] {
    let ctx=egui::Context::default();let mut fonts=egui::FontDefinitions::default();crate::language_plugin::add_fonts(&mut fonts);ctx.set_fonts(fonts);
    let mut id="external port id".to_owned();let mut selected=name.to_owned();let before=(id.clone(),selected.clone());
    let output=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(600.0,200.0))),..Default::default()},|ui|midi_port_combo(ui,"offline-midi",&[(id.clone(),selected.clone())],&mut id,&mut selected));
    let texts:Vec<_>=output.shapes.iter().filter_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape{Some(t.galley.job.text.clone())}else{None}).collect();
    let expected=if name.is_empty(){if enabled{"— 无 —"}else{"— none —"}}else{name};assert!(texts.contains(&expected.to_owned()),"{texts:?}");
    assert_eq!((id,selected),before);output.drop_without_applying_deltas();
   }
  }
 }
}
