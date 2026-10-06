//! The Remote tab: connect this screen to an sdroxide server somewhere else.
//!
//! The same thing `--connect` does on the command line, without the command
//! line — and, because a station reached this way arrives as a radio tab of its
//! own, without giving up the radios already open here. The engine stays where
//! the antenna is; what crosses the network is the spectrum, the audio and the
//! commands.
//!
//! Native only. A browser client is already attached to the server that served
//! it and has nowhere to put a second connection, so there is no such tab
//! there — see [`crate::app::persist::load_remote_server`].
//!
//! What is edited here is the *screen's* setting, not the station's: it is
//! written to this machine's `config.toml` as it is typed, the way the control
//! bindings are, because there is nothing to apply and a pressed CONNECT is a
//! poor moment to discover the address was never saved.

use eframe::egui::{self, RichText};
use sdroxide_types::RemoteServer;

/// Draw the tab. `connect` is set when the operator presses CONNECT; the dial
/// itself happens in the shell, after the frame — it is the only thing that can
/// add the tab the connection lands in.
///
/// `can_connect` is whether this session can hold another radio at all (false
/// where there is no shell around the app to put one in), and `status` is what
/// the last attempt from this screen did, if there was one.
pub(in crate::app) fn settings_remote_tab(
    ui: &mut egui::Ui,
    cfg: &mut RemoteServer,
    connect: &mut bool,
    can_connect: bool,
    status: Option<&Result<crate::language_plugin::UiNotice, crate::language_plugin::UiNotice>>,
) {
    ui.label(RichText::new(crate::language_plugin::text("settings.remote.connect_to_a_server", "Connect to a server")).size(14.0).strong().color(crate::theme::CYAN()));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.remote.drive_an_sdroxide_running_somewhere_else_the_shack_machine", "Drive an sdroxide running somewhere else — the shack machine, a remote site — over \
             the network. Start it there with --server, and enter its address here. The radio, \
             the decoders and the logbook stay on that machine; this screen gets the waterfall \
             and the audio, and its speakers and microphone are the ones you use."),
        )
        .weak(),
    );
    ui.add_space(8.0);

    // A pasted URL carries its own scheme and its own port (or that scheme's
    // default), so the two boxes below have nothing left to say and grey out
    // rather than sitting there looking as though they still apply.
    let typed_url = cfg.host.contains("://");
    let mut entered = false;
    egui::Grid::new("remote-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new(crate::language_plugin::text("settings.remote.address", "Address")).strong());
        let addr = crate::chrome::field(
            ui,
            egui::TextEdit::singleline(&mut cfg.host)
                .desired_width(240.0)
                .hint_text(crate::language_plugin::text("settings.remote.host_name_or_ip", "host name or IP")),
        );
        let addr = addr.on_hover_text(
            crate::language_plugin::text("settings.remote.the_machine_running_sdroxide_server_a_host_name_an", "The machine running sdroxide --server: a host name, an IPv4 or IPv6 address, or a \
             complete ws:// (or wss://) URL if the server sits behind a reverse proxy — a URL is \
             used exactly as typed and ignores the port and the switch below."),
        );
        // Enter in the address box is the same as pressing CONNECT: an address
        // box is a thing one types into and presses return on.
        entered = addr.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.end_row();

        ui.label(RichText::new(crate::language_plugin::text("settings.remote.port", "Port")).strong());
        ui.add_enabled_ui(!typed_url, |ui| {
            ui.add(egui::DragValue::new(&mut cfg.port).range(1..=65535)).on_hover_text(
                crate::language_plugin::text("settings.remote.the_port_that_server_listens_on_server_port_in", "The port that server listens on — server_port in its config.toml. 4950 unless \
                 it was given --port."),
            );
        });
        ui.end_row();

        // The scheme, as a switch rather than as something to be spelled out in
        // the address box. A browser client reads it off the page it was served
        // from; this one has no page, so the operator has to say — and until
        // they could, a station behind an HTTPS proxy on 443 was reachable only
        // by typing the whole URL out, which then took the port box with it
        // (issue #360).
        ui.label(RichText::new(crate::language_plugin::text("settings.remote.secure", "Secure")).strong());
        ui.add_enabled_ui(!typed_url, |ui| {
            ui.checkbox(&mut cfg.tls, "wss:// (TLS)").on_hover_text(
                crate::language_plugin::text("settings.remote.tick_this_where_the_server_sits_behind_something_that", "Tick this where the server sits behind something that terminates HTTPS for it — \
                 a reverse proxy on port 443, which is the usual way a station is reached across \
                 the open internet. sdroxide's own server speaks plain ws:// and does not want \
                 this."),
            );
        });
        ui.end_row();
    });

    ui.add_space(8.0);
    let ready = !cfg.host.trim().is_empty() && can_connect;
    ui.horizontal(|ui| {
        let button = ui.add_enabled_ui(ready, |ui| {
            crate::chrome::chip_accent(
                ui,
                false,
                RichText::new(crate::language_plugin::text("settings.remote.connect", " CONNECT ")).strong(),
                crate::theme::GREEN(),
                crate::theme::INK_ON_CYAN(),
            )
        });
        if ready
            && (button.inner.on_hover_text(crate::language_plugin::text("settings.remote.open_this_station_as_a_radio_tab_of_its", "Open this station as a radio tab of its own")).clicked()
                || entered)
        {
            *connect = true;
        }
        if !cfg.host.trim().is_empty() {
            ui.add(egui::Label::new(RichText::new(cfg.url()).weak().monospace()).wrap());
        }
    });

    if !can_connect {
        ui.add_space(4.0);
        ui.label(
            RichText::new(crate::language_plugin::text("settings.remote.this_client_cannot_hold_a_second_connection", "This client cannot hold a second connection."))
                .color(crate::theme::ALERT()),
        );
    }

    match status {
        Some(Ok(msg)) => {
            ui.add_space(6.0);
            ui.label(RichText::new(msg.display()).color(crate::theme::GREEN()));
        }
        Some(Err(e)) => {
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(e.display()).color(crate::theme::ALERT())).wrap());
        }
        None => {}
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("settings.remote.the_station_appears_as_a_tab_beside_the_radios", "The station appears as a tab beside the radios already open here, and the tab strip \
             at the top of the window switches between them (⊞ puts two side by side). Close it \
             from the roster at the top of the Radio tab — that hangs up; nothing on the server \
             is changed.\n\nIf the server asks for a username and password, its sign-in screen \
             comes up in the new tab. A plain ws:// link carries none of this encrypted, so \
             across the open internet either put it through a VPN or an SSH tunnel, or terminate \
             HTTPS in front of the server and tick Secure above."),
        )
        .weak(),
    );
}

#[cfg(test)]
mod language_remote_notice_tests {
 use super::*;
 use crate::language_plugin::UiNotice;
 #[test]
 fn remote_settings_redraw_cached_notices_without_connecting_or_changing_configuration() {
  let address="wss://station.example/{id}";
  let status:Result<UiNotice,UiNotice>=Ok(UiNotice::new(format!("Dialling {address}…"),"Dialling {}…",vec![address.into()]));
  let original=status.clone();
  for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
   for width in [360.0,600.0,1000.0] {
    let ctx=egui::Context::default();let mut fonts=egui::FontDefinitions::default();crate::language_plugin::add_fonts(&mut fonts);ctx.set_fonts(fonts);
    let mut cfg=RemoteServer::default();let before=serde_json::to_value(&cfg).unwrap();let mut connect=false;
    let output=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width,1200.0))),..Default::default()},|ui|settings_remote_tab(ui,&mut cfg,&mut connect,true,Some(&status)));
    let texts:Vec<_>=output.shapes.iter().filter_map(|s|if let egui::epaint::Shape::Text(t)=&s.shape {Some(t.galley.job.text.clone())}else{None}).collect();
    let expected=if enabled {format!("正在连接 {address}…")} else {format!("Dialling {address}…")};assert!(texts.contains(&expected),"{texts:?}");
    assert!(!connect);assert_eq!(serde_json::to_value(&cfg).unwrap(),before);assert_eq!(status,original);output.drop_without_applying_deltas();
   }
  }
 }
}
