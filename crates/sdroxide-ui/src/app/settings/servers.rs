//! The Servers tab: the three interfaces this app offers to other software.
//!
//! Hamlib rigctld and the TCI server are control surfaces third-party clients
//! connect to; the WSJT-X UDP broadcast is one-way, feeding decodes, status
//! and logged QSOs to GridTracker, JTAlert, N1MM+ and Log4OM. All three are
//! bound by the engine, which is also what persists their configuration and
//! reports back whether the bind succeeded.
//!
//! That makes these three settings the *station's*, not the screen's: the
//! engine announces what it has, and this tab edits that. `seeded` is whether
//! the announcement has arrived yet — a client that has not been told cannot
//! be allowed to apply defaults over the real thing.

use crate::chrome::StyledCombo;
use eframe::egui::{self, Color32, ComboBox, RichText};

/// Live status of the built-in TCI server, from `RadioEvent::TciServerStatus`.
#[derive(Clone, PartialEq)]
pub(in crate::app) struct TciServerStatus {
    pub(in crate::app) running: bool,
    pub(in crate::app) addr: String,
    pub(in crate::app) clients: usize,
    pub(in crate::app) error: Option<String>,
}

pub(in crate::app) fn settings_wsjtx_tab(
    ui: &mut egui::Ui,
    cfg: &mut sdroxide_types::WsjtxConfig,
    seeded: bool,
    apply: &mut bool,
) {
    ui.label(RichText::new(crate::language_plugin::text("settings.servers.wsjt_x_udp_broadcast", "WSJT-X UDP broadcast")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(4.0);
    if !seeded {
        ui.label(RichText::new(crate::language_plugin::text("settings.servers.waiting_for_the_station_s_wsjt_x_broadcast_configuration", "Waiting for the station's WSJT-X broadcast configuration…")).weak());
        return;
    }
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.servers.sends_decodes_station_status_and_logged_qsos_the_way", "Sends decodes, station status and logged QSOs the way WSJT-X does, so GridTracker, \
             JTAlert, N1MM+ and Log4OM work with sdroxide unchanged. Output only — nothing on \
             this socket can touch the radio."),
        )
        .weak(),
    );
    ui.add_space(6.0);
    crate::chrome::checkbox(ui, &mut cfg.enabled, crate::language_plugin::text("settings.servers.enable", "Enable"));
    ui.add_space(6.0);
    ui.add_enabled_ui(cfg.enabled, |ui| {
        egui::Grid::new("wsjtx-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("settings.servers.send_to", "Send to"));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut cfg.host)
                    .desired_width(160.0)
                    .hint_text("127.0.0.1"),
            )
            .on_hover_text(
                crate::language_plugin::text("settings.servers.127_0_0_1_reaches_clients_on_this_machine", "127.0.0.1 reaches clients on this machine; a LAN address or a multicast \
                 group (224.0.0.1) reaches others"),
            );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.port", "Port"));
            ui.add(egui::DragValue::new(&mut cfg.port).range(1..=65535))
                .on_hover_text(crate::language_plugin::text("settings.servers.2237_is_the_port_every_client_defaults_to", "2237 is the port every client defaults to"));
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.identify_as", "Identify as"));
            crate::chrome::field(ui, egui::TextEdit::singleline(&mut cfg.id).desired_width(160.0))
                .on_hover_text(
                    crate::language_plugin::text("settings.servers.the_name_clients_see_some_loggers_only_accept_traffic", "The name clients see. Some loggers only accept traffic identifying itself \
                 as WSJT-X."),
                );
            ui.end_row();
        });
    });

    // ── N1MM+, the other dialect ──
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(
        RichText::new(crate::language_plugin::text("settings.servers.n1mm_contactinfo_broadcast", "N1MM+ contactinfo broadcast"))
            .size(13.0)
            .strong()
            .color(crate::theme::CYAN()),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.servers.the_same_news_in_n1mm_s_words_one_xml", "The same news in N1MM's words: one XML datagram per logged contact, for the loggers \
             that listen for N1MM rather than for WSJT-X — the World Radio League's bridge takes \
             either. Contacts only; there is no decode stream in this protocol. Its own switch \
             and its own port, because a station may want both."),
        )
        .weak(),
    );
    ui.add_space(6.0);
    crate::chrome::checkbox(ui, &mut cfg.n1mm.enabled, crate::language_plugin::text("settings.servers.enable", "Enable"));
    ui.add_space(6.0);
    ui.add_enabled_ui(cfg.n1mm.enabled, |ui| {
        egui::Grid::new("n1mm-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("settings.servers.send_to", "Send to"));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut cfg.n1mm.host)
                    .desired_width(160.0)
                    .hint_text("127.0.0.1"),
            )
            .on_hover_text(
                crate::language_plugin::text("settings.servers.127_0_0_1_reaches_clients_on_this_machine_112", "127.0.0.1 reaches clients on this machine. N1MM's own advice for a contest \
                 network is this subnet's broadcast address — 192.168.1.255 for a 192.168.1.n \
                 network — which reaches every position at once."),
            );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.port", "Port"));
            ui.add(egui::DragValue::new(&mut cfg.n1mm.port).range(1..=65535))
                .on_hover_text(crate::language_plugin::text("settings.servers.12060_is_the_port_n1mm_s_documentation_recommends", "12060 is the port N1MM's documentation recommends"));
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.station_name", "Station name"));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut cfg.n1mm.station).desired_width(160.0),
            )
            .on_hover_text(
                crate::language_plugin::text("settings.servers.what_n1mm_calls_the_stationname_the_name_of_the", "What N1MM calls the StationName: the name of the computer that sent the \
                 packet. Loggers show it to tell one operating position from another."),
            );
            ui.end_row();
        });
    });

    ui.add_space(8.0);
    if crate::chrome::chip_accent(
        ui,
        false,
        RichText::new(crate::language_plugin::text("settings.servers.apply", " APPLY ")).strong(),
        crate::theme::GREEN(),
        crate::theme::INK_ON_CYAN(),
    )
    .on_hover_text(crate::language_plugin::text("settings.servers.persist_and_re_open_the_broadcast_sockets", "Persist and (re)open the broadcast sockets"))
    .clicked()
    {
        *apply = true;
    }
}

pub(in crate::app) fn settings_rigctld_tab(
    ui: &mut egui::Ui,
    cfg: &mut sdroxide_types::RigctldConfig,
    seeded: bool,
    status: &Option<TciServerStatus>,
    apply: &mut bool,
) {
    use sdroxide_types::RigctldConfig;

    ui.label(
        RichText::new(crate::language_plugin::text("settings.servers.hamlib_rigctld_server", "Hamlib rigctld server")).size(14.0).strong().color(crate::theme::CYAN()),
    );
    ui.add_space(4.0);
    if !seeded {
        ui.label(RichText::new(crate::language_plugin::text("settings.servers.waiting_for_the_station_s_rigctld_configuration", "Waiting for the station's rigctld configuration…")).weak());
        return;
    }
    ui.add_space(2.0);
    crate::chrome::checkbox(ui, &mut cfg.enabled, crate::language_plugin::text("settings.servers.enable", "Enable"));
    ui.add_space(6.0);

    ui.add_enabled_ui(cfg.enabled, |ui| {
        egui::Grid::new("rigctld-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("settings.servers.listen_on", "Listen on"));
            ComboBox::from_id_salt("rigctld_bind").selected_text(&cfg.bind).show_styled(ui, |ui| {
                for b in RigctldConfig::BINDS {
                    let label = if b == "127.0.0.1" {
                        crate::language_plugin::text("settings.servers.127_0_0_1_this_machine", "127.0.0.1 (this machine)")
                    } else {
                        crate::language_plugin::text("settings.servers.0_0_0_0_whole_network", "0.0.0.0 (whole network)")
                    };
                    if ui.selectable_label(cfg.bind == b, label).clicked() {
                        cfg.bind = b.to_string();
                    }
                }
            });
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.port", "Port"));
            ui.add(egui::DragValue::new(&mut cfg.port).range(1..=65535))
                .on_hover_text(crate::language_plugin::text("settings.servers.4532_is_the_port_every_rigctld_client_assumes", "4532 is the port every rigctld client assumes"));
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.rig_name", "Rig name"));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut cfg.rig_name)
                    .desired_width(160.0)
                    .hint_text(crate::language_plugin::text("settings.servers.reported_to_clients", "reported to clients")),
            );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.max_clients", "Max clients"));
            ui.add(egui::DragValue::new(&mut cfg.max_clients).range(1..=32));
            ui.end_row();
        });
        ui.add_space(4.0);
        crate::chrome::checkbox(ui, &mut cfg.allow_tx, crate::language_plugin::text("settings.servers.allow_clients_to_transmit", "Allow clients to transmit")).on_hover_text(
            crate::language_plugin::text("settings.servers.off_refuses_every_key_request_and_stops_advertising_a", "Off refuses every key request and stops advertising a transmit range, so Hamlib \
             itself declines to key."),
        );
    });

    // Same hazard as TCI: the protocol has no authentication at all.
    if cfg.enabled && cfg.is_open_to_network() {
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                crate::language_plugin::text("settings.servers.rigctld_has_no_authentication_on_0_0_0_0", "⚠ rigctld has no authentication. On 0.0.0.0, anyone who can reach this port can \
                 tune and key your transmitter."),
            )
            .color(Color32::from_rgb(230, 170, 60)),
        );
    }

    ui.add_space(8.0);
    match status {
        Some(s) if s.running => {
            let clients = match s.clients {
                1 => crate::language_plugin::text("settings.servers.1_client", "1 client").to_string(),
                n => crate::language_plugin::format("settings.servers.n_clients", "{n} clients", &[format!("{n}")]),
            };
            ui.label(
                RichText::new({ let __lp_arg_0 = &(s.addr); crate::language_plugin::format("settings.servers.listening_on_clients", "● Listening on {} — {clients}", &[format!("{}", __lp_arg_0), format!("{clients}")]) })
                    .color(Color32::from_rgb(90, 200, 110)),
            );
        }
        Some(s) => match &s.error {
            Some(e) => {
                ui.label(
                    RichText::new(crate::language_plugin::format("settings.servers.failed_e", "Failed: {e}", &[format!("{e}")])).color(Color32::from_rgb(230, 90, 80)),
                );
            }
            None => {
                ui.label(RichText::new(crate::language_plugin::text("settings.servers.not_running", "Not running.")).weak());
            }
        },
        None => {
            ui.label(RichText::new(crate::language_plugin::text("settings.servers.status_unknown_press_apply", "Status unknown — press APPLY.")).weak());
        }
    }

    ui.add_space(8.0);
    if crate::chrome::chip_accent(
        ui,
        false,
        RichText::new(crate::language_plugin::text("settings.servers.apply", " APPLY ")).strong(),
        crate::theme::GREEN(),
        crate::theme::INK_ON_CYAN(),
    )
    .on_hover_text(crate::language_plugin::text("settings.servers.persist_and_re_bind_the_server", "Persist and (re)bind the server"))
    .clicked()
    {
        *apply = true;
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.servers.lets_any_hamlib_capable_program_drive_this_radio_frequency", "Lets any Hamlib-capable program drive this radio: frequency, mode, PTT, split, RIT \
             and power. In WSJT-X, fldigi or CQRLOG choose the rig \u{201c}Hamlib NET rigctl\u{201d} \
             (model 2) and point it at this address; in GPredict and N1MM enter the host and port \
             directly. Unlike the TCI server it carries control only \u{2014} no audio, no IQ."),
        )
        .weak(),
    );
}

/// The rotctld *client* — the one interface on this tab where sdroxide dials
/// out instead of listening. Driven by the satellite lock's az/el.
pub(in crate::app) fn settings_rotator_tab(
    ui: &mut egui::Ui,
    cfg: &mut sdroxide_types::RotatorConfig,
    seeded: bool,
    status: &Option<(bool, f64, f64, Option<String>)>,
    apply: &mut bool,
) {
    ui.label(
        RichText::new(crate::language_plugin::text("settings.servers.antenna_rotator_rotctld_client", "Antenna rotator (rotctld client)"))
            .size(14.0)
            .strong()
            .color(crate::theme::CYAN()),
    );
    ui.add_space(4.0);
    if !seeded {
        ui.label(RichText::new(crate::language_plugin::text("settings.servers.waiting_for_the_station_s_rotator_configuration", "Waiting for the station's rotator configuration…")).weak());
        return;
    }
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.servers.points_a_motorized_antenna_at_the_satellite_you_are", "Points a motorized antenna at the satellite you are locked onto, through a Hamlib \
             rotctld daemon (run e.g. \u{201c}rotctld -m 603 -r /dev/ttyUSB0\u{201d} for a GS-232B \
             rotator, or \u{201c}rotctld -m 1\u{201d} to try it without hardware). One daemon covers \
             every rotator Hamlib drives — EasyComm, GS-232, SPID and the rest."),
        )
        .weak(),
    );
    ui.add_space(6.0);
    crate::chrome::checkbox(ui, &mut cfg.enabled, crate::language_plugin::text("settings.servers.enable", "Enable"));
    ui.add_space(6.0);
    ui.add_enabled_ui(cfg.enabled, |ui| {
        egui::Grid::new("rotator-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("settings.servers.host", "Host"));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut cfg.host)
                    .desired_width(160.0)
                    .hint_text("127.0.0.1"),
            );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.port", "Port"));
            ui.add(egui::DragValue::new(&mut cfg.port).range(1..=65535))
                .on_hover_text(crate::language_plugin::text("settings.servers.4533_is_rotctld_s_default", "4533 is rotctld's default"));
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.min_elevation", "Min elevation"));
            ui.add(egui::DragValue::new(&mut cfg.min_el_deg).range(0.0..=45.0).suffix("°"))
                .on_hover_text(
                    crate::language_plugin::text("settings.servers.below_this_the_rotator_parks_instead_of_tracking_set", "Below this the rotator parks instead of tracking — set it to your local \
                     horizon or roofline"),
                );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.azimuth_offset", "Azimuth offset"));
            ui.add(egui::DragValue::new(&mut cfg.az_offset_deg).range(-180.0..=180.0).suffix("°"))
                .on_hover_text(crate::language_plugin::text("settings.servers.added_to_every_command_for_a_rotator_whose_north", "Added to every command, for a rotator whose north is off"));
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.min_movement", "Min movement"));
            ui.add(egui::DragValue::new(&mut cfg.min_move_deg).range(0.1..=10.0).suffix("°"))
                .on_hover_text(
                    crate::language_plugin::text("settings.servers.steps_smaller_than_this_are_not_sent_chasing_tenths", "Steps smaller than this are not sent — chasing tenths of a degree wears \
                     motors for nothing"),
                );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.park_position", "Park position"));
            let mut has_park = cfg.park.is_some();
            if ui
                .checkbox(&mut has_park, crate::language_plugin::text("settings.servers.drive_to_a_position_when_idle", "drive to a position when idle"))
                .on_hover_text(crate::language_plugin::text("settings.servers.off_leaves_the_antenna_wherever_the_last_pass_ended", "Off leaves the antenna wherever the last pass ended"))
                .changed()
            {
                cfg.park = has_park.then_some((0.0, 0.0));
            }
            ui.end_row();

            if let Some((az, el)) = cfg.park.as_mut() {
                ui.label("");
                ui.horizontal(|ui| {
                    ui.label(crate::language_plugin::text("common.az", "az"));
                    ui.add(egui::DragValue::new(az).range(0.0..=360.0).suffix("°"));
                    ui.label(crate::language_plugin::text("common.el", "el"));
                    ui.add(egui::DragValue::new(el).range(0.0..=90.0).suffix("°"));
                });
                ui.end_row();
            }
        });
    });

    ui.add_space(8.0);
    match status {
        Some((true, az, el, _)) => {
            ui.label(
                RichText::new(crate::language_plugin::format("settings.servers.connected_antenna_at_az_0_az_el_0_el", "● Connected — antenna at {az:.0}° az, {el:.0}° el", &[format!("{az:.0}"), format!("{el:.0}")]))
                    .color(Color32::from_rgb(90, 200, 110)),
            );
        }
        Some((false, _, _, Some(e))) => {
            ui.label(RichText::new(crate::language_plugin::format("settings.servers.failed_e", "Failed: {e}", &[format!("{e}")])).color(Color32::from_rgb(230, 90, 80)));
        }
        Some((false, _, _, None)) => {
            ui.label(RichText::new(crate::language_plugin::text("settings.servers.not_connected", "Not connected.")).weak());
        }
        None if cfg.enabled => {
            ui.label(RichText::new(crate::language_plugin::text("settings.servers.status_unknown_press_apply", "Status unknown — press APPLY.")).weak());
        }
        None => {}
    }

    ui.add_space(8.0);
    if crate::chrome::chip_accent(
        ui,
        false,
        RichText::new(crate::language_plugin::text("settings.servers.apply", " APPLY ")).strong(),
        crate::theme::GREEN(),
        crate::theme::INK_ON_CYAN(),
    )
    .on_hover_text(crate::language_plugin::text("settings.servers.persist_and_re_connect_to_the_daemon", "Persist and (re)connect to the daemon"))
    .clicked()
    {
        *apply = true;
    }
}

pub(in crate::app) fn settings_tci_server_tab(
    ui: &mut egui::Ui,
    cfg: &mut sdroxide_types::TciServerConfig,
    seeded: bool,
    status: &Option<TciServerStatus>,
    apply: &mut bool,
) {
    use sdroxide_types::TciServerConfig;

    if !seeded {
        ui.label(RichText::new(crate::language_plugin::text("settings.servers.waiting_for_the_station_s_tci_server_configuration", "Waiting for the station's TCI server configuration…")).weak());
        return;
    }

    ui.label(RichText::new(crate::language_plugin::text("settings.servers.built_in_tci_server", "Built-in TCI server")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(6.0);
    crate::chrome::checkbox(ui, &mut cfg.enabled, crate::language_plugin::text("settings.servers.enable", "Enable"));
    ui.add_space(6.0);

    ui.add_enabled_ui(cfg.enabled, |ui| {
        egui::Grid::new("tci-srv-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(crate::language_plugin::text("settings.servers.listen_on", "Listen on"));
            ComboBox::from_id_salt("tci_srv_bind").selected_text(&cfg.bind).show_styled(ui, |ui| {
                for b in TciServerConfig::BINDS {
                    let label = if b == "127.0.0.1" {
                        crate::language_plugin::text("settings.servers.127_0_0_1_this_machine", "127.0.0.1 (this machine)")
                    } else {
                        crate::language_plugin::text("settings.servers.0_0_0_0_whole_network", "0.0.0.0 (whole network)")
                    };
                    if ui.selectable_label(cfg.bind == b, label).clicked() {
                        cfg.bind = b.to_string();
                    }
                }
            });
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.port", "Port"));
            ui.add(egui::DragValue::new(&mut cfg.port).range(1..=65535));
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.device_name", "Device name"));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut cfg.device_name)
                    .desired_width(160.0)
                    .hint_text(crate::language_plugin::text("settings.servers.reported_to_clients", "reported to clients")),
            );
            ui.end_row();

            ui.label(crate::language_plugin::text("settings.servers.max_clients", "Max clients"));
            ui.add(egui::DragValue::new(&mut cfg.max_clients).range(1..=32));
            ui.end_row();
        });
        ui.add_space(4.0);
        crate::chrome::checkbox(ui, &mut cfg.allow_tx, crate::language_plugin::text("settings.servers.allow_clients_to_transmit", "Allow clients to transmit")).on_hover_text(
            crate::language_plugin::text("settings.servers.off_leaves_control_and_the_receive_streams_working_but", "Off leaves control and the receive streams working, but every key request \
             is refused."),
        );
    });

    // Security: TCI has no authentication at all, so binding wide open hands
    // the transmitter to anyone who can reach the port.
    if cfg.enabled && cfg.is_open_to_network() {
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                crate::language_plugin::text("settings.servers.tci_has_no_authentication_on_0_0_0_0", "⚠ TCI has no authentication. On 0.0.0.0, anyone who can reach this port can \
                 tune and key your transmitter."),
            )
            .color(Color32::from_rgb(230, 170, 60)),
        );
    }

    ui.add_space(8.0);
    match status {
        Some(s) if s.running => {
            let clients = match s.clients {
                1 => crate::language_plugin::text("settings.servers.1_client", "1 client").to_string(),
                n => crate::language_plugin::format("settings.servers.n_clients", "{n} clients", &[format!("{n}")]),
            };
            ui.label(
                RichText::new({ let __lp_arg_0 = &(s.addr); crate::language_plugin::format("settings.servers.listening_on_clients", "● Listening on {} — {clients}", &[format!("{}", __lp_arg_0), format!("{clients}")]) })
                    .color(Color32::from_rgb(90, 200, 110)),
            );
        }
        Some(s) => match &s.error {
            Some(e) => {
                let msg = RichText::new(crate::language_plugin::format("settings.servers.failed_e", "Failed: {e}", &[format!("{e}")]));
                ui.label(msg.color(Color32::from_rgb(230, 90, 80)));
            }
            None => {
                ui.label(RichText::new(crate::language_plugin::text("settings.servers.not_running", "Not running.")).weak());
            }
        },
        None => {
            ui.label(RichText::new(crate::language_plugin::text("settings.servers.status_unknown_press_apply", "Status unknown — press APPLY.")).weak());
        }
    }

    ui.add_space(8.0);
    if crate::chrome::chip_accent(
        ui,
        false,
        RichText::new(crate::language_plugin::text("settings.servers.apply", " APPLY ")).strong(),
        crate::theme::GREEN(),
        crate::theme::INK_ON_CYAN(),
    )
    .on_hover_text(crate::language_plugin::text("settings.servers.persist_and_re_bind_the_server", "Persist and (re)bind the server"))
    .clicked()
    {
        *apply = true;
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.servers.lets_tci_capable_programs_drive_this_radio_frequency_and", "Lets TCI-capable programs drive this radio: frequency and mode control, wideband IQ, \
             receive audio, and transmit audio. In WSJT-X choose the SunSDR (TCI) rig type, point \
             it at this address, and set PTT to TCI."),
        )
        .weak(),
    );
}
