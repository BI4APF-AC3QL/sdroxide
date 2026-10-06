//! Client-side data language pack, enabled on first run and controlled from Settings / UI.
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily};
use sdroxide_language_pack::{Catalog, HelpManual, Manifest};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
mod transfer_display;
#[cfg(not(target_arch = "wasm32"))]
pub use transfer_display::{bundle_summary, import_report_summary, transfer_skip_reason, config_error};

mod backend_settings_display;
pub use backend_settings_display::{login_test_message, relay_refusal_message, relay_error_message, relay_description, relay_sequence_note, relay_channel_name};

mod radio_reference_display;
pub use radio_reference_display::{wefax_station_name, wefax_where_label, wefax_chart_title, satellite_link_label, satellite_link_note, satellite_link_mode, satellite_link_mode_with_inversion};

mod entity_display;
pub use entity_display::{entity_name, award_builtin_names, award_entity_name};


mod boundary_display;
pub use boundary_display::{bandplan_label, vdl2_channel_role, image_picker_filter, ui_count_plural};



mod notice_display;
pub use notice_display::{UiNotice, chrome_control_text};
pub(crate) use notice_display::{speech_status_note, alert_status_note, satellite_update_notice, alert_output_notice};

#[cfg(test)]
mod shell_notice_tests;

mod panel_state23;
pub use panel_state23::{packet_state_display, sstv_placeholder_help, rifp_sender_display};


mod boundary25;
pub use boundary25::{js8_command_display, hpsdr_header_display, meter_row_display, js8_speed_display};


mod connection27;
pub use connection27::{connection_error_notice, radio_notice_projection};

mod map28;
pub use map28::{aprs_symbol_display, aprs_object_origin_display};
mod settings24;
pub use settings24::{transverter_header_display, pan_receiver_selected, radio_fallback, satellite_tab_label};
mod panel_display;
pub use panel_display::{panel_tab_label, novelty_badge, decode_sender_fallback};

mod key_display;
pub use key_display::key_chord_label;

const STORAGE_KEY: &str = "sdroxide-language-plugin-v1";
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    enabled: bool,
    activation_id: String,
    folder: String,
    imported_catalog: Option<String>,
}
#[derive(Default)]
struct State {
    initialized: bool,
    prefs: Preferences,
    catalog: Option<Catalog>,
    font: Option<Arc<FontData>>,
    version: String,
    partial: bool,
    error: Option<String>,
    font_dirty: bool,
    manual: Option<Arc<HelpManual>>,
    help_error: Option<String>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

pub fn initialize(storage: Option<&dyn eframe::Storage>) {
    let activated = STATE.with(|s| {
        if s.borrow().initialized { return false; }
        let saved_by_eframe: Option<Preferences> = storage.and_then(|s| eframe::get_value(s, STORAGE_KEY));
        let mut prefs = saved_by_eframe.clone().unwrap_or_default();
        // The packaged Chinese executable is an opt-in localization build:
        // enable the bundled pack on first run. A saved setting still wins,
        // so the Settings checkbox can switch back to the original interface.
        #[cfg(not(target_arch = "wasm32"))]
        if preference_path().map(|p| !p.exists()).unwrap_or(false) {
            prefs.enabled = true;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let browser_saved = web_sys::window()
                .and_then(|window| window.local_storage().ok().flatten())
                .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
                .and_then(|raw| serde_json::from_str::<Preferences>(&raw).ok());
            if let Some(saved) = browser_saved { prefs = saved; }
            else if saved_by_eframe.is_none() { prefs.enabled = true; }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let saved = preference_path()
                .ok()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| serde_json::from_slice(&bytes).ok());
            if let Some(saved) = saved { prefs = saved; }
            else { prefs.enabled = true; }
        }
        let mut activated = false;
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(exe) = std::env::current_exe()
            && let Some(parent) = exe.parent()
            && let Ok(id) = std::fs::read_to_string(parent.join("plugins/zh-CN/activation-id.txt"))
            && !id.trim().is_empty() && id.len() < 128
            && prefs.activation_id != id.trim() {
            // An explicit install/repair enables the newly installed pack once.
            // Later checkbox changes remain authoritative until another repair.
            prefs.enabled = true;
            prefs.folder.clear();
            prefs.imported_catalog = None;
            prefs.activation_id = id.trim().to_owned();
            activated = true;
        }
        let mut state = s.borrow_mut();
        state.initialized = true;
        state.prefs = prefs;
        let loaded = load_default(&state.prefs);
        match loaded {
            Ok((catalog, font, version, partial, manual, help_error)) => {
                state.catalog = Some(catalog); state.font = Some(font);
                state.version = version; state.partial = partial;
                state.manual = manual; state.help_error = help_error;
            }
            Err(e) => { state.error = Some(e); state.prefs.enabled = false; activated = false; }
        }
        activated
    });
    if activated { persist(); }
}

#[cfg(not(target_arch = "wasm32"))]
fn preference_path() -> Result<std::path::PathBuf, String> {
    sdroxide_config::config_dir().map(|p| p.join("language-plugin.json")).map_err(|e| e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn read_asset(root: &std::path::Path, path: &str, max: u64) -> Result<Vec<u8>, String> {
    let path = sdroxide_language_pack::asset_path(root, path)?;
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > max { return Err("language pack asset is too large".into()); }
    std::fs::read(path).map_err(|e| e.to_string())
}

type Loaded = (Catalog, Arc<FontData>, String, bool, Option<Arc<HelpManual>>, Option<String>);

fn parse_manual(raw: &str) -> Result<Arc<HelpManual>, String> {
    let manual = HelpManual::parse(raw, crate::help::MANUAL_MD)?;
    crate::help::validate_manual(&manual.markdown)?;
    Ok(Arc::new(manual))
}

pub(crate) fn manual() -> Option<Arc<HelpManual>> {
    STATE.with(|s| {
        let state = s.borrow();
        if state.prefs.enabled { state.manual.clone() } else { None }
    })
}

#[cfg(test)]
pub(crate) fn test_manual_enabled(enabled: bool) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.prefs.enabled = enabled;
        state.manual = Some(parse_manual(include_str!("../../../plugins/zh-CN/manual.zh-CN.json")).unwrap());
    });
}
#[cfg(test)]
pub(crate) fn test_pack_enabled(enabled: bool) {
    STATE.with(|s| {
        *s.borrow_mut() = State {
            initialized: true,
            prefs: Preferences { enabled, ..Default::default() },
            catalog: Some(Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap()),
            font: Some(Arc::new(FontData::from_static(include_bytes!("../../../plugins/zh-CN/fonts/NotoSansSC.ttf")))),
            ..Default::default()
        };
    });
}
#[cfg(not(target_arch = "wasm32"))]
fn load_default(prefs: &Preferences) -> Result<Loaded, String> {
    let root = if prefs.folder.is_empty() {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let beside = exe.parent().ok_or("executable has no parent")?.join("plugins/zh-CN");
        // Development checkout only. Production puts plugins beside the exe.
        if beside.is_dir() { beside } else { std::path::PathBuf::from("plugins/zh-CN") }
    } else { std::path::PathBuf::from(&prefs.folder) };
    let manifest = String::from_utf8(read_asset(&root, "manifest.json", sdroxide_language_pack::MAX_JSON_BYTES as u64)?).map_err(|e| e.to_string())?;
    let m = Manifest::parse(&manifest, env!("CARGO_PKG_VERSION"))?;
    let raw = String::from_utf8(read_asset(&root, &m.catalog, sdroxide_language_pack::MAX_JSON_BYTES as u64)?).map_err(|e| e.to_string())?;
    let catalog = Catalog::parse(&raw)?;
    let asset = m.fonts.first().ok_or("language pack has no Chinese font")?;
    read_asset(&root, &asset.license, 64 * 1024)?;
    let bytes = read_asset(&root, &asset.file, 32 * 1024 * 1024)?;
    validate_font(&bytes)?;
    let help = m.help.as_deref().map(|file| {
        let raw = String::from_utf8(read_asset(&root, file, sdroxide_language_pack::MAX_JSON_BYTES as u64)?)
            .map_err(|e| e.to_string())?;
        parse_manual(&raw)
    }).transpose();
    let (manual, help_error) = match help {
        Ok(manual) => (manual, None),
        Err(error) => (None, Some(error)),
    };
    Ok((catalog, Arc::new(FontData::from_owned(bytes)), m.version, pack_is_partial(&m.completeness, false), manual, help_error))
}

#[cfg(target_arch = "wasm32")]
fn load_default(prefs: &Preferences) -> Result<Loaded, String> {
    // Browser users may replace the catalog with an imported JSON. The bundled
    // plugin font remains available offline and is not fetched from a CDN.
    let m = Manifest::parse(include_str!("../../../plugins/zh-CN/manifest.json"), env!("CARGO_PKG_VERSION"))?;
    let raw = prefs.imported_catalog.as_deref().unwrap_or(include_str!("../../../plugins/zh-CN/translations.zh-CN.json"));
    let c = Catalog::parse(raw)?;
    let bytes = include_bytes!("../../../plugins/zh-CN/fonts/NotoSansSC.ttf");
    validate_font(bytes)?;
    let (manual, help_error) = match parse_manual(include_str!("../../../plugins/zh-CN/manual.zh-CN.json")) {
        Ok(manual) => (Some(manual), None),
        Err(error) => (None, Some(error)),
    };
    Ok((c, Arc::new(FontData::from_static(bytes)), m.version,
        pack_is_partial(&m.completeness, prefs.imported_catalog.is_some()), manual, help_error))
}

fn pack_is_partial(completeness: &str, imported_catalog: bool) -> bool {
    completeness != "complete" || imported_catalog
}

fn validate_font(bytes: &[u8]) -> Result<(), String> {
    use ab_glyph::Font;
    let font = ab_glyph::FontRef::try_from_slice(bytes).map_err(|e| e.to_string())?;
    for ch in "简体中文设置频率发射接收语音：，。".chars() {
        if font.glyph_id(ch).0 == 0 { return Err(format!("font is missing {ch}")); }
    }
    Ok(())
}

pub fn text(key: &str, original: &str) -> String {
    STATE.with(|s| {
        let s = s.borrow();
        if s.prefs.enabled && let Some(c) = &s.catalog { c.text(key, original) }
        else { original.to_owned() }
    })
}

pub fn format(key: &str, original: &str, values: &[String]) -> String {
    let translated = text(key, original);
    sdroxide_language_pack::render(&translated, values)
        .unwrap_or_else(|_| original.to_owned())
}

/// Keep English inflection when the entire sentence falls back to its source.
pub fn plural_suffix(key: &str, original: &str, singular: bool) -> &'static str {
    plural_suffix_with(key, original, singular, "s")
}

pub fn plural_suffix_with(key: &str, original: &str, singular: bool, plural: &'static str) -> &'static str {
    if singular || text(key, original) != original { "" } else { plural }
}

pub fn enum_text(id: &str, original: &str) -> String {
    let namespace = match id {
        "ui-layout" => "display.layout.",
        "ui-theme" => "display.theme.",
        "ui-btn-style" | "ui-win-style" => "display.chrome.",
        "ui-skim-font" | "ui-wf-font" | "ui-menu-font" => "display.font_size.",
        _ => "display.enum.",
    };
    scope_text(namespace, original)
}

pub fn display_label(original: impl AsRef<str>) -> String {
    scope_text("display.enum.", original.as_ref())
}

/// Translate the application's fixed no-radio placeholder while preserving
/// capability labels supplied by actual drivers.
pub fn radio_label(original: &str) -> String {
    let key = match original {
        "No radio" => "display.radio.no_radio",
        "Switched off" => "display.radio.switched_off",
        "Panadapter receiver" => "display.radio.panadapter_receiver",
        "Signal generator" => "display.radio.signal_generator",
        _ => return original.to_owned(),
    };
    text(key, original).to_owned()
}

pub fn repeater_shift_label(shift: sdroxide_types::Shift) -> String {
    scope_text("display.repeater.shift.", shift.label())
}

pub fn repeater_tone_mode_label(mode: sdroxide_types::ToneMode) -> String {
    scope_text("display.repeater.tone.", mode.label())
}

pub fn nr_text(original: &str) -> String {
    scope_text("display.receiver.nr.", original)
}

pub fn agc_text(original: &str) -> String {
    scope_text("display.receiver.agc.", original)
}

pub fn rate_note_text(original: &str) -> String {
    scope_text("display.device.rate.", original)
}

pub fn speed_chip(speed: sdroxide_types::Speed) -> String {
    let original = speed.label();
    let translated = scope_text("display.receiver.speed.", original);
    if translated == original { original.to_uppercase() } else { translated }
}

pub fn drm_summary(status: &sdroxide_types::DrmStatus) -> String {
    let original = status.summary();
    if !status.service.label.is_empty() { original }
    else { scope_text("display.receiver.drm.", &original) }
}

pub fn hd_summary(status: &sdroxide_types::HdRadioStatus) -> String {
    let original = status.summary();
    if status.unavailable.is_none() && !status.station_name.is_empty() { original }
    else { scope_text("display.receiver.hd.", &original) }
}

pub fn action_text(action: sdroxide_types::Action) -> String {
    use sdroxide_types::Action;
    let (original, values) = match action {
        Action::VfoSelect(v) => ("Select VFO {}", vec![if v == sdroxide_types::Vfo::A { "A" } else { "B" }.to_owned()]),
        Action::BandSelect(b) => ("Band {}", vec![b.label().to_owned()]),
        Action::ModeSelect(m) => ("Mode {}", vec![m.label().to_owned()]),
        Action::MemoryRecall(n) => ("Recall memory {n}", vec![n.to_string()]),
        Action::VoicePlay(n) => ("Voice keyer slot {}", vec![(n + 1).to_string()]),
        _ => return display_label(action.label()),
    };
    sdroxide_language_pack::render(&display_label(original), &values)
        .unwrap_or_else(|_| action.label())
}

pub fn scope_text(namespace: &str, original: &str) -> String {
    STATE.with(|s| {
        let s = s.borrow();
        if s.prefs.enabled && let Some(c) = &s.catalog {
            for (key, entry) in c.entries.range::<str, _>((std::ops::Bound::Included(namespace), std::ops::Bound::Unbounded)) {
                if !key.starts_with(namespace) { break; }
                if entry.source == original { return entry.translation.clone(); }
            }
        }
        original.to_owned()
    })
}

pub fn ais_label(original: &str) -> String {
    scope_text("display.ais.", original)
}

pub fn adsb_label(original: &str) -> String {
    scope_text("display.adsb.", original)
}

pub fn rds_label(original: &str) -> String {
    scope_text("display.rds.", original)
}

pub fn compass(az_deg: f64) -> String {
    scope_text("display.compass.", sdroxide_solar::satellites::compass(az_deg))
}

pub fn month_day(month: u32, day: u32, fallback: String) -> String {
    let source = "{month}/{day}";
    let translated = scope_text("display.date.", source);
    if translated == source || !(1..=12).contains(&month) || !(1..=31).contains(&day) { return fallback; }
    sdroxide_language_pack::render(&translated, &[month.to_string(), day.to_string()]).unwrap_or(fallback)
}

pub fn atchat_roster_status(original: &str) -> String {
    scope_text("display.atchat.roster.", original)
}

pub fn wspr_hop_blocked_status(original: &str) -> String {
    scope_text("display.wspr.hop_blocked.", original)
}

/// Translate only the WSPR packer's three known transmit-refusal messages.
/// The reason is deliberately produced by the transmitter code; unknown
/// upstream wording remains visible verbatim.
pub fn wspr_tx_blocked_status(original: &str) -> String {
    const GRID_PREFIX: &str = " is not a Maidenhead locator, so there is nothing to transmit as. Two letters and two digits — JN47 — is what the message carries.";
    const CALL_SUFFIX: &str = " cannot be sent: WSPR's message carries a plain callsign, and a compound one needs a layout this station cannot encode. Receiving is unaffected.";
    if original == "Set your callsign and grid before transmitting." {
        return text("display.wspr.tx_blocked.identity", original);
    }
    if let Some(grid) = original.strip_suffix(GRID_PREFIX) {
        return format("display.wspr.tx_blocked.grid", "{grid} is not a Maidenhead locator, so there is nothing to transmit as. Two letters and two digits — JN47 — is what the message carries.", &[grid.to_owned()]);
    }
    if let Some(call) = original.strip_suffix(CALL_SUFFIX) {
        return format("display.wspr.tx_blocked.callsign", "{call} cannot be sent: WSPR's message carries a plain callsign, and a compound one needs a layout this station cannot encode. Receiving is unaffected.", &[call.to_owned()]);
    }
    original.to_owned()
}

/// Localize the Winlink worker's known activity prefixes without translating
/// a gateway name, server address, or any future upstream status.
pub fn winlink_activity_status(original: &str) -> String {
    if original == "stopping…" {
        return text("display.winlink.activity.stopping", original);
    }
    for (prefix, key, source) in [
        ("connecting to ", "display.winlink.activity.connecting", "connecting to {address}…"),
        ("calling ", "display.winlink.activity.calling", "calling {gateway}…"),
    ] {
        if let Some(value) = original.strip_prefix(prefix).and_then(|s| s.strip_suffix("…")) {
            return format(key, source, &[value.to_owned()]);
        }
    }
    original.to_owned()
}

/// Translate Winlink's fixed session/protocol error wording at the display
/// boundary, preserving callsigns, message IDs, route addresses and OS errors.
pub fn winlink_error_status(original: &str) -> String {
    let fixed = [
        ("peer sent no SID, so it is not speaking B2F", "display.winlink.error.no_sid"),
        ("peer sent a proposal block with a bad checksum", "display.winlink.error.proposal_checksum"),
        ("peer disconnected mid-session", "display.winlink.error.disconnected"),
        ("telnet login: connection closed before a password prompt arrived", "display.winlink.error.no_prompt"),
        ("the Winlink session ended without a result", "display.winlink.error.no_result"),
    ];
    for (source, key) in fixed {
        if original == source {
            return text(key, source);
        }
    }
    if let Some(rest) = original.strip_prefix("peer does not support B2 compressed forwarding (SID codes ")
        && let Some(codes) = rest.strip_suffix(')') {
        return format("display.winlink.error.no_b2", "peer does not support B2 compressed forwarding (SID codes {codes})", &[codes.to_owned()]);
    }
    if let Some(rest) = original.strip_prefix("connecting to ")
        && let Some((address, error)) = rest.split_once(": ") {
        return format("display.winlink.error.connect", "connecting to {address}: {error}", &[address.to_owned(), error.to_owned()]);
    }
    if let Some(error) = original.strip_prefix("telnet login: ") {
        return format("display.winlink.error.telnet", "telnet login: {error}", &[error.to_owned()]);
    }
    if let Some(rest) = original.strip_prefix("decompressing ")
        && let Some((mid, error)) = rest.split_once(": ") {
        return format("display.winlink.error.decompress", "decompressing {mid}: {error}", &[mid.to_owned(), error.to_owned()]);
    }
    if let Some(rest) = original.strip_prefix("parsing ")
        && let Some((mid, error)) = rest.split_once(": ") {
        return format("display.winlink.error.parse", "parsing {mid}: {error}", &[mid.to_owned(), error.to_owned()]);
    }
    original.to_owned()
}

pub fn login_refusal_status(original: &str) -> String {
    scope_text("display.login.refusal.", original)
}

pub fn decode_sort_label(sort: sdroxide_types::DecodeSort) -> String {
    scope_text("display.decode.sort.", sort.label())
}

/// Translate the RIFP receiver's known protocol failure summaries while
/// retaining opaque session IDs and leaving future decoder details untouched.
pub fn rifp_error_status(original: &str) -> String {
    fn session_id(value: &str) -> Option<&str> {
        let id = value.strip_prefix("session ")?.get(..16)?;
        id.bytes().all(|b| b.is_ascii_hexdigit()).then_some(id)
    }
    if let Some(detail) = original.strip_prefix("transmit: ") {
        return format("display.rifp.error.transmit", "transmit: {detail}", &[detail.to_owned()]);
    }
    if let Some(id) = original.strip_prefix("dropped incomplete session ") {
        if id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return format("display.rifp.error.dropped", "dropped incomplete session {id}", &[id.to_owned()]);
        }
    }
    if original.ends_with(" cancelled by the sender") {
        if let Some(id) = original.strip_prefix("session ").and_then(|s| s.strip_suffix(" cancelled by the sender"))
            && id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return format("display.rifp.error.cancelled", "session {id} cancelled by the sender", &[id.to_owned()]);
        }
    }
    let Some((head, detail)) = original.split_once(": ") else { return original.to_owned() };
    let Some(id) = session_id(head) else { return original.to_owned() };
    if head.len() != 24 { return original.to_owned(); }
    if detail == "manifest session ID does not match the frame header" {
        return format("display.rifp.error.manifest_id", "session {id}: manifest session ID does not match the frame header", &[id.to_owned()]);
    }
    if detail == "manifest chunk count does not match the frame header" {
        return format("display.rifp.error.manifest_chunks", "session {id}: manifest chunk count does not match the frame header", &[id.to_owned()]);
    }
    if detail == "manifest digest is not a 64-digit hex string" {
        return format("display.rifp.error.manifest_digest", "session {id}: manifest digest is not a 64-digit hex string", &[id.to_owned()]);
    }
    if detail == "manifest is not valid UTF-8" {
        return format("display.rifp.error.manifest_utf8", "session {id}: manifest is not valid UTF-8", &[id.to_owned()]);
    }
    if detail == "odd-length RLE8 payload" {
        return format("display.rifp.error.rle_odd", "session {id}: odd-length RLE8 payload", &[id.to_owned()]);
    }
    if detail == "zero run length in RLE8 payload" {
        return format("display.rifp.error.rle_zero", "session {id}: zero run length in RLE8 payload", &[id.to_owned()]);
    }
    if detail == "RLE8 payload expands past the declared raster" {
        return format("display.rifp.error.rle_overflow", "session {id}: RLE8 payload expands past the declared raster", &[id.to_owned()]);
    }
    if detail == "Group 3 is receive-only in this build; pick Group 4" {
        return format("display.rifp.error.group3", "session {id}: Group 3 is receive-only in this build; pick Group 4", &[id.to_owned()]);
    }
    if detail == "no content encoding in the manifest" {
        return format("display.rifp.error.no_encoding", "session {id}: no content encoding in the manifest", &[id.to_owned()]);
    }
    if detail == "conflicting manifests, abandoned" {
        return format("display.rifp.error.conflict", "session {id}: conflicting manifests, abandoned", &[id.to_owned()]);
    }
    if detail == "object CRC-32 mismatch" {
        return format("display.rifp.error.crc", "session {id}: object CRC-32 mismatch", &[id.to_owned()]);
    }
    if detail == "object SHA-256 mismatch" {
        return format("display.rifp.error.sha", "session {id}: object SHA-256 mismatch", &[id.to_owned()]);
    }
    if detail == "expired incomplete" {
        return format("display.rifp.error.expired", "session {id}: expired incomplete", &[id.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("reassembled ")
        && let Some((got, tail)) = rest.split_once(" octets, manifest says ")
        && !got.is_empty() && got.bytes().all(|b| b.is_ascii_digit())
        && !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.size", "session {id}: reassembled {got} octets, manifest says {want}", &[id.to_owned(), got.to_owned(), tail.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("truncated raster: ")
        && let Some((got, tail)) = rest.split_once(" of ")
        && let Some(want) = tail.strip_suffix(" octets")
        && !got.is_empty() && got.bytes().all(|b| b.is_ascii_digit())
        && !want.is_empty() && want.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.truncated_raster", "session {id}: truncated raster: {got} of {want} octets", &[id.to_owned(), got.to_owned(), want.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("chunk ")
        && let Some((sequence, suffix)) = rest.split_once(" arrived twice with different contents, abandoned")
        && suffix.is_empty() && !sequence.is_empty() && sequence.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.chunk_conflict", "session {id}: chunk {sequence} arrived twice with different contents, abandoned", &[id.to_owned(), sequence.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("unsupported object type ")
        && let Some((media_type, encoding)) = rest.split_once(" / ") {
        return format("display.rifp.error.object_type", "session {id}: unsupported object type {media_type} / {encoding}", &[id.to_owned(), media_type.to_owned(), encoding.to_owned()]);
    }
    if let Some(depth) = detail.strip_prefix("unsupported grayscale depth ")
        && !depth.is_empty() && depth.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.depth", "session {id}: unsupported grayscale depth {depth}", &[id.to_owned(), depth.to_owned()]);
    }
    if let Some(version) = detail.strip_prefix("unsupported manifest version ") {
        return format("display.rifp.error.manifest_version", "session {id}: unsupported manifest version {version}", &[id.to_owned(), version.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("manifest of ")
        && let Some(size) = rest.strip_suffix(" octets is too large")
        && !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.manifest_size", "session {id}: manifest of {size} octets is too large", &[id.to_owned(), size.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("not a RIFP manifest (protocol ")
        && let Some(protocol) = rest.strip_suffix(')') {
        return format("display.rifp.error.protocol", "session {id}: not a RIFP manifest (protocol {protocol})", &[id.to_owned(), protocol.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("object of ")
        && let Some(size) = rest.strip_suffix(" octets exceeds local limits")
        && !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.object_size", "session {id}: object of {size} octets exceeds local limits", &[id.to_owned(), size.to_owned()]);
    }
    if let Some(size) = detail.strip_prefix("implausible chunk size ")
        && !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.chunk_size", "session {id}: implausible chunk size {size}", &[id.to_owned(), size.to_owned()]);
    }
    if let Some(count) = detail.strip_prefix("implausible chunk count ")
        && !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()) {
        return format("display.rifp.error.chunk_count", "session {id}: implausible chunk count {count}", &[id.to_owned(), count.to_owned()]);
    }
    if let Some(parse_error) = detail.strip_prefix("manifest: ") {
        return format("display.rifp.error.manifest_parse", "session {id}: manifest: {detail}", &[id.to_owned(), parse_error.to_owned()]);
    }
    if let Some(rest) = detail.strip_prefix("image dimensions ")
        && let Some(dims) = rest.strip_suffix(" exceed local limits")
        && let Some((width, height)) = dims.split_once('×') {
        return format("display.rifp.error.dimensions", "session {id}: image dimensions {width}×{height} exceed local limits", &[id.to_owned(), width.to_owned(), height.to_owned()]);
    }
    original.to_owned()
}

pub fn backend_status(original: &str) -> String {
    let active = STATE.with(|s| { let s = s.borrow(); s.prefs.enabled && s.catalog.is_some() });
    if !active { return original.to_owned(); }
    let fixed = scope_text("display.backend.status.", original);
    if fixed != original { return fixed; }
    let Some((key, template, values)) = backend_status_template(original) else { return original.to_owned(); };
    format(key, template, &values)
}

pub fn public_sdr_blocked_status(original: &str) -> String {
    let fixed = scope_text("display.publicsdr.blocked.", original);
    if fixed != original { return fixed; }
    if let Some(device) = original.strip_prefix("no client for ").and_then(|s| s.strip_suffix("'s protocol yet")) {
        return format("display.publicsdr.blocked.device-protocol", "no client for {}'s protocol yet", &[device.to_owned()]);
    }
    if let Some(rest) = original.strip_prefix("full — ") {
        if let Some((users, maximum)) = rest.strip_suffix(" channels in use").and_then(|s| s.split_once(" of ")) {
            return format("display.publicsdr.blocked.capacity", "full — {} of {} channels in use", &[users.to_owned(), maximum.to_owned()]);
        }
    }
    original.to_owned()
}

fn backend_status_template(original: &str) -> Option<(&'static str, &'static str, Vec<String>)> {
    if original == "no enabled band is inside the receiver's window" {
        return Some(("display.backend.status.ism-no-band", "no enabled band is inside the receiver's window", vec![]));
    }
    if let Some(rest) = original.strip_prefix("the receiver's window is too narrow for ") {
        if let Some(bandwidth) = rest.strip_suffix(" kHz — choose a narrower bandwidth, or AUTO") {
            return Some(("display.backend.status.ism-narrow-window", "the receiver's window is too narrow for {} kHz — choose a narrower bandwidth, or AUTO", vec![bandwidth.into()]));
        }
    }
    if original == "The nrsc5 library on this machine was built without its audio decoder (USE_FAAD2=OFF), so it can find HD Radio stations but not play them. Install an nrsc5 built with it — upstream's default, which carries its own patched faad2 — then restart sdroxide." {
        return Some(("display.backend.status.nrsc5-no-audio", "The nrsc5 library on this machine was built without its audio decoder (USE_FAAD2=OFF), so it can find HD Radio stations but not play them. Install an nrsc5 built with it — upstream's default, which carries its own patched faad2 — then restart sdroxide.", vec![]));
    }
    if let Some(rest) = original.strip_prefix("HD Radio needs nrsc5's decoder library (") {
        if let Some(library) = rest.strip_suffix("), which is not installed on this machine. sdroxide loads it at run time rather than building it in: install nrsc5 from your distribution or from github.com/theori-io/nrsc5, then restart sdroxide.") {
            return Some(("display.backend.status.nrsc5-missing-library", "HD Radio needs nrsc5's decoder library ({}), which is not installed on this machine. sdroxide loads it at run time rather than building it in: install nrsc5 from your distribution or from github.com/theori-io/nrsc5, then restart sdroxide.", vec![library.into()]));
        }
    }
    if let Some(rest) = original.strip_prefix("ADS-B needs at least ") {
        if let Some((minimum, rest)) = rest.split_once(" Msps and this stream is ") {
            if let Some(current) = rest.strip_suffix(" Msps — lower the front-end decimation, or raise the device sample rate") {
                return Some(("display.backend.status.adsb-minimum-rate", "ADS-B needs at least {} Msps and this stream is {} Msps — lower the front-end decimation, or raise the device sample rate", vec![minimum.into(), current.into()]));
            }
        }
    }
    if let Some(rest) = original.strip_prefix("1090.000 MHz is outside the receiver's window, which is ") {
        if let Some((width, center)) = rest.split_once(" MHz wide about ").and_then(|(a,b)| b.strip_suffix(" MHz").map(|b|(a,b))) {
            return Some(("display.backend.status.adsb-outside-window", "1090.000 MHz is outside the receiver's window, which is {} MHz wide about {} MHz", vec![width.into(), center.into()]));
        }
    }
    if let Some(rest) = original.strip_prefix("this stream is ") {
        if let Some((current, rest)) = rest.split_once(" Msps and a Mode S chip is half a microsecond, so the signal is barely sampled: the strong aircraft decode and the weak ones are lost. ") {
            if let Some(minimum) = rest.strip_suffix(" Msps or more is what it takes — widen the receiver's window if it has the setting.") {
                return Some(("display.backend.status.adsb-low-sampling", "this stream is {} Msps and a Mode S chip is half a microsecond, so the signal is barely sampled: the strong aircraft decode and the weak ones are lost. {} Msps or more is what it takes — widen the receiver's window if it has the setting.", vec![current.into(), minimum.into()]));
            }
        }
    }
    if let Some(rest) = original.strip_prefix("VDL2 needs at least ") {
        if let Some((minimum, rest)) = rest.split_once(" kHz of stream and this one is ") {
            if let Some(current) = rest.strip_suffix(" kHz — lower the front-end decimation, or raise the device sample rate") {
                return Some(("display.backend.status.vdl2-minimum-rate", "VDL2 needs at least {} kHz of stream and this one is {} kHz — lower the front-end decimation, or raise the device sample rate", vec![minimum.into(), current.into()]));
            }
        }
    }
    if let Some(rest) = original.strip_prefix("no VDL2 channel is inside the receiver's window, which is ") {
        if let Some((width, center)) = rest.split_once(" MHz wide about ").and_then(|(a,b)| b.strip_suffix(" MHz").map(|b|(a,b))) {
            return Some(("display.backend.status.vdl2-outside-window", "no VDL2 channel is inside the receiver's window, which is {} MHz wide about {} MHz", vec![width.into(), center.into()]));
        }
    }
    if let Some(rest) = original.strip_prefix("this window is ") {
        if let Some((width, rest)) = rest.split_once(" kHz wide and reaches ") {
            if let Some((reached, rest)) = rest.split_once(" of the ") {
                if let Some((total, rest)) = rest.split_once(" channels, ") {
                    if let Some((span, rest)) = rest.split_once(" MHz — the other ") {
                        if let Some((low, high)) = span.split_once(" to ") {
                            if let Some((missing, plural)) = rest.split_once(' ') {
                                if plural == "is outside it" || plural == "are outside it" {
                                    return Some(("display.backend.status.vdl2-partial-window", "this window is {} kHz wide and reaches {} of the {} channels, {} to {} MHz — the other {} channels are outside it", vec![width.into(), reached.into(), total.into(), low.into(), high.into(), missing.into()]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for (unit, suffix, key) in [("symbol", " the symbol timing has too little to work with", "display.backend.status.vdl2-low-samples"), ("bit", " the bit timing has too little to work with", "display.backend.status.ais-low-samples")] {
        let lead = if unit == "symbol" { "this window leaves only " } else { "this window leaves only " };
        let middle = if unit == "symbol" { " samples per symbol; below " } else { " samples a bit; below " };
        if let Some(rest) = original.strip_prefix(lead) {
            if let Some((current, rest)) = rest.split_once(&middle) {
                if let Some(minimum) = rest.strip_suffix(suffix) {
                    let source = if unit == "symbol" { "this window leaves only {} samples per symbol; below {} the symbol timing has too little to work with" } else { "this window leaves only {} samples a bit; below {} the bit timing has too little to work with" };
                    return Some((key, source, vec![current.into(), minimum.into()]));
                }
            }
        }
    }
    if let Some(rest) = original.strip_prefix("AIS needs at least ") {
        if let Some((minimum, rest)) = rest.split_once(" kHz of stream and this one is ") {
            if let Some(current) = rest.strip_suffix(" kHz — lower the front-end decimation, or raise the device sample rate") {
                return Some(("display.backend.status.ais-minimum-rate", "AIS needs at least {} kHz of stream and this one is {} kHz — lower the front-end decimation, or raise the device sample rate", vec![minimum.into(), current.into()]));
            }
        }
    }
    if let Some(rest) = original.strip_prefix("neither AIS channel is inside the receiver's window, which is ") {
        if let Some((width, center)) = rest.split_once(" kHz wide about ").and_then(|(a,b)| b.strip_suffix(" MHz").map(|b|(a,b))) {
            return Some(("display.backend.status.ais-outside-window", "neither AIS channel is inside the receiver's window, which is {} kHz wide about {} MHz", vec![width.into(), center.into()]));
        }
    }
    if let Some(rest) = original.strip_prefix("this window is ") {
        if let Some((width, rest)) = rest.split_once(" kHz wide and reaches AIS ") {
            if let Some((channel, rest)) = rest.split_once(" only — a ship alternates between the two channels, so it will be heard at half its reporting rate (AIS ") {
                if let Some(missing) = rest.strip_suffix(" is outside it)") {
                    return Some(("display.backend.status.ais-partial-window", "this window is {} kHz wide and reaches AIS {} only — a ship alternates between the two channels, so it will be heard at half its reporting rate (AIS {} is outside it)", vec![width.into(), channel.into(), missing.into()]));
                }
            }
        }
    }
    None
}

pub fn colormap_name(original: &str) -> String {
    scope_text("display.colormap.", original)
}

pub fn add_fonts(fonts: &mut FontDefinitions) {
    STATE.with(|s| {
        let s = s.borrow();
        // Discovery controls are bilingual even before enabling translation.
        // Keep CJK glyphs available; disabling restores English text immediately.
        let Some(font) = &s.font else { return };
        fonts.font_data.insert("zh-CN-plugin".into(), font.clone());
        for family in [FontFamily::Proportional, FontFamily::Monospace,
            FontFamily::Name("chakra-bold".into()), FontFamily::Name("cyber-mono".into())] {
            fonts.families.entry(family).or_default().push("zh-CN-plugin".into());
        }
    });
}

pub fn prepare_frame(ctx: &egui::Context) {
    let dirty = STATE.with(|s| std::mem::take(&mut s.borrow_mut().font_dirty));
    if dirty { crate::theme::install_fonts(ctx); ctx.request_repaint(); }
    #[cfg(target_arch = "wasm32")]
    sync_browser_audio_notice();
}

#[cfg(any(target_arch = "wasm32", test))]
fn browser_audio_notice(source: &str, origin: &str) -> String {
    format("web.audio.insecure_context", source, &[origin.to_owned()])
}

#[cfg(target_arch = "wasm32")]
fn sync_browser_audio_notice() {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Some(bar) = document.get_element_by_id("sdroxide-audio-context-warning") else { return };
    let Some(source) = bar.get_attribute("data-sdroxide-source") else { return };
    let Some(origin) = bar.get_attribute("data-sdroxide-origin") else { return };
    let translated = browser_audio_notice(&source, &origin);
    if bar.text_content().as_deref() != Some(translated.as_str()) {
        bar.set_text_content(Some(&translated));
    }
}

fn persist() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let result = preference_path().and_then(|path| {
            let prefs = STATE.with(|s| s.borrow().prefs.clone());
            let bytes = serde_json::to_vec_pretty(&prefs).map_err(|e| e.to_string())?;
            // A separate file: neither the upstream config nor its keys change.
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, bytes).map_err(|e| e.to_string())
        });
        if let Err(e) = result { STATE.with(|s| s.borrow_mut().error = Some(e)); }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let json = STATE.with(|s| serde_json::to_string(&s.borrow().prefs));
        if let Some(window) = web_sys::window()
            && let Ok(Some(storage)) = window.local_storage()
            && let Ok(json) = json {
            if let Err(e) = storage.set_item(STORAGE_KEY, &json) {
                STATE.with(|s| s.borrow_mut().error = Some(format!("localStorage: {e:?}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                initialized: true,
                catalog: Some(Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap()),
                font: Some(Arc::new(FontData::from_static(include_bytes!("../../../plugins/zh-CN/fonts/NotoSansSC.ttf")))),
                ..Default::default()
            };
        });
    }

    #[test]
    fn repeater_simplex_label_switches_language_and_keeps_shift_signs() {
        test_state();
        use sdroxide_types::Shift;
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            assert_eq!(repeater_shift_label(Shift::Simplex), if enabled { "单工" } else { "SIMPLEX" });
            assert_eq!(repeater_shift_label(Shift::Minus), "−");
            assert_eq!(repeater_shift_label(Shift::Plus), "+");
        }
    }

    #[test]
    fn repeater_tone_mode_switches_language_and_preserves_standard_codes() {
        test_state();
        use sdroxide_types::ToneMode;
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            assert_eq!(repeater_tone_mode_label(ToneMode::Off), if enabled { "关闭" } else { "OFF" });
            assert_eq!(repeater_tone_mode_label(ToneMode::Ctcss), "CTCSS");
            assert_eq!(repeater_tone_mode_label(ToneMode::Dcs), "DCS");
        }
    }

    #[test]
    fn static_symbol_radio_and_solar_overlay_consumers_switch_and_fall_back() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let radio_key = "boundaries.app.settings.radio.text_25_f63475";
        let radio_source = "Waiting for the machine the radio is attached to: these ask about its hardware and its network, and it answers one at a time.";
        let radio_translation = &catalog.entries[radio_key].translation;
        let symbols: Vec<_> = catalog.entries.iter().filter(|(key, _)| key.starts_with("panels.setup.symbols.")).map(|(_, e)| e).collect();
        let layers: Vec<_> = catalog.entries.iter().filter(|(key, _)| key.starts_with("solar.overlay.layers.")).map(|(_, e)| e).collect();
        assert_eq!(symbols.len(), 18);
        assert_eq!(layers.len(), 24);
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            assert_eq!(text(radio_key, radio_source), if enabled { radio_translation.clone() } else { radio_source.to_owned() });
            for entry in &symbols {
                assert_eq!(scope_text("panels.setup.symbols.", &entry.source), if enabled { entry.translation.clone() } else { entry.source.clone() });
            }
            for entry in &layers {
                assert_eq!(scope_text("solar.overlay.layers.", &entry.source), if enabled { entry.translation.clone() } else { entry.source.clone() });
            }
        }
    }

    #[test]
    fn wspr_hop_block_reason_switches_language_and_preserves_unknown_status() {
        test_state();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            assert_eq!(wspr_hop_blocked_status("transmitting"), if enabled { "发射中" } else { "transmitting" });
            assert_eq!(wspr_hop_blocked_status("future backend state"), "future backend state");
        }
    }

    #[test]
    fn wspr_transmit_refusals_switch_language_and_preserve_dynamic_values_and_unknowns() {
        test_state();
        let cases = [
            ("Set your callsign and grid before transmitting.", "请先设置呼号和网格，再进行发射。"),
            ("somewhere is not a Maidenhead locator, so there is nothing to transmit as. Two letters and two digits — JN47 — is what the message carries.", "somewhere 不是 Maidenhead 网格定位符，因此无法用于发射。消息需要两个字母和两位数字，例如 JN47。"),
            ("PJ4/K1ABC cannot be sent: WSPR's message carries a plain callsign, and a compound one needs a layout this station cannot encode. Receiving is unaffected.", "PJ4/K1ABC 无法发射：WSPR 消息只支持普通呼号，本站无法编码复合呼号。接收不受影响。"),
        ];
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (source, chinese) in cases {
                assert_eq!(wspr_tx_blocked_status(source), if enabled { chinese } else { source });
            }
            assert_eq!(wspr_tx_blocked_status("new WSPR refusal from upstream"), "new WSPR refusal from upstream");
        }
    }

    #[test]
    fn winlink_activity_translates_known_prefixes_and_keeps_route_identity() {
        test_state();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            assert_eq!(winlink_activity_status("connecting to cms.winlink.org:8772…"), if enabled { "连接到 cms.winlink.org:8772…" } else { "connecting to cms.winlink.org:8772…" });
            assert_eq!(winlink_activity_status("calling OE1XIK…"), if enabled { "正在呼叫 OE1XIK…" } else { "calling OE1XIK…" });
            assert_eq!(winlink_activity_status("stopping…"), if enabled { "正在停止…" } else { "stopping…" });
            assert_eq!(winlink_activity_status("new worker status"), "new worker status");
        }
    }

    #[test]
    fn winlink_errors_translate_known_context_and_keep_route_and_server_details() {
        test_state();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            let expected = if enabled { "对端不支持 B2 压缩转发（SID 代码 C0）" } else { "peer does not support B2 compressed forwarding (SID codes C0)" };
            assert_eq!(winlink_error_status("peer does not support B2 compressed forwarding (SID codes C0)"), expected);
            let expected = if enabled { "连接 cms.winlink.org:8772 失败：connection refused" } else { "connecting to cms.winlink.org:8772: connection refused" };
            assert_eq!(winlink_error_status("connecting to cms.winlink.org:8772: connection refused"), expected);
            assert_eq!(winlink_error_status("peer sent no SID, so it is not speaking B2F"), if enabled { "对端未发送 SID，因此不是 B2F 协议" } else { "peer sent no SID, so it is not speaking B2F" });
            assert_eq!(winlink_error_status("future Winlink error"), "future Winlink error");
        }
    }

    #[test]
    fn login_refusal_translates_known_server_reasons_and_keeps_unknowns() {
        test_state();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            assert_eq!(login_refusal_status("username or password not accepted"), if enabled { "用户名或密码不正确" } else { "username or password not accepted" });
            assert_eq!(login_refusal_status("another sign-in is being checked — try again"), if enabled { "另一个登录请求正在验证，请稍后重试" } else { "another sign-in is being checked — try again" });
            assert_eq!(login_refusal_status("future authentication reason"), "future authentication reason");
        }
    }

    #[test]
    fn login_test_success_log_localizes_its_fixed_prefix_and_preserves_service_text() {
        test_state();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            let message = "signed in as BI4APF".to_owned();
            assert_eq!(format("app.frame.login_test_success", "ok, {message}", &[message.clone()]), if enabled { "成功：signed in as BI4APF" } else { "ok, signed in as BI4APF" });
        }
    }

    #[test]
    fn decode_sort_chips_localize_all_order_labels_and_restore_source() {
        test_state();
        use sdroxide_types::DecodeSort;
        let cases = [
            (DecodeSort::None, "接收顺序"),
            (DecodeSort::Signal, "信噪比"),
            (DecodeSort::Distance, "距离"),
            (DecodeSort::Country, "国家/地区"),
        ];
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (sort, chinese) in cases {
                assert_eq!(decode_sort_label(sort), if enabled { chinese } else { sort.label() });
            }
        }
    }

    #[test]
    fn rifp_known_protocol_errors_translate_and_preserve_session_ids_and_unknown_details() {
        test_state();
        let cases = [
            ("session 0123456789abcdef cancelled by the sender", "RIFP 会话 0123456789abcdef 已由发送方取消"),
            ("dropped incomplete session fedcba9876543210", "不完整的 RIFP 会话 fedcba9876543210 已丢弃"),
            ("session 0123456789abcdef: conflicting manifests, abandoned", "RIFP 会话 0123456789abcdef 的清单冲突，已放弃"),
            ("session 0123456789abcdef: object CRC-32 mismatch", "RIFP 会话 0123456789abcdef 的对象 CRC-32 校验失败"),
            ("session 0123456789abcdef: object SHA-256 mismatch", "RIFP 会话 0123456789abcdef 的对象 SHA-256 校验失败"),
            ("session 0123456789abcdef: expired incomplete", "RIFP 会话 0123456789abcdef 已超时且未完成"),
            ("session 0123456789abcdef: reassembled 120 octets, manifest says 121", "RIFP 会话 0123456789abcdef：重组数据为 120 字节，清单声明为 121 字节"),
            ("session 0123456789abcdef: truncated raster: 120 of 121 octets", "RIFP 会话 0123456789abcdef：光栅数据被截断，收到 120 字节，应为 121 字节"),
            ("session 0123456789abcdef: manifest session ID does not match the frame header", "RIFP 会话 0123456789abcdef：清单会话 ID 与帧头不一致"),
            ("session 0123456789abcdef: manifest chunk count does not match the frame header", "RIFP 会话 0123456789abcdef：清单分块数与帧头不一致"),
            ("session 0123456789abcdef: manifest digest is not a 64-digit hex string", "RIFP 会话 0123456789abcdef：清单摘要不是 64 位十六进制字符串"),
            ("session 0123456789abcdef: manifest is not valid UTF-8", "RIFP 会话 0123456789abcdef：清单不是有效的 UTF-8 文本"),
            ("session 0123456789abcdef: odd-length RLE8 payload", "RIFP 会话 0123456789abcdef：RLE8 负载长度为奇数"),
            ("session 0123456789abcdef: zero run length in RLE8 payload", "RIFP 会话 0123456789abcdef：RLE8 负载包含长度为零的游程"),
            ("session 0123456789abcdef: RLE8 payload expands past the declared raster", "RIFP 会话 0123456789abcdef：RLE8 负载解压后超出声明的光栅大小"),
            ("session 0123456789abcdef: Group 3 is receive-only in this build; pick Group 4", "RIFP 会话 0123456789abcdef：此版本仅支持接收 Group 3，请选择 Group 4"),
            ("session 0123456789abcdef: no content encoding in the manifest", "RIFP 会话 0123456789abcdef：清单未指定内容编码"),
            ("session 0123456789abcdef: chunk 7 arrived twice with different contents, abandoned", "RIFP 会话 0123456789abcdef：第 7 块重复到达且内容不同，已放弃该会话"),
            ("session 0123456789abcdef: unsupported object type image/png / gzip", "RIFP 会话 0123456789abcdef：不支持对象类型 image/png / gzip"),
            ("session 0123456789abcdef: unsupported grayscale depth 16", "RIFP 会话 0123456789abcdef：不支持 16 位灰度深度"),
            ("session 0123456789abcdef: unsupported manifest version 2.0", "RIFP 会话 0123456789abcdef：不支持清单版本 2.0"),
            ("session 0123456789abcdef: image dimensions 9000×7000 exceed local limits", "RIFP 会话 0123456789abcdef：图像尺寸 9000×7000 超出本机限制"),
            ("session 0123456789abcdef: manifest of 5000 octets is too large", "RIFP 会话 0123456789abcdef：清单大小为 5000 字节，超过上限"),
            ("session 0123456789abcdef: not a RIFP manifest (protocol \"other\")", "RIFP 会话 0123456789abcdef：清单协议标识不是 RIFP（收到 \"other\"）"),
            ("session 0123456789abcdef: object of 9000000 octets exceeds local limits", "RIFP 会话 0123456789abcdef：对象大小为 9000000 字节，超出本机限制"),
            ("session 0123456789abcdef: implausible chunk size 0", "RIFP 会话 0123456789abcdef：分块大小 0 无效"),
            ("session 0123456789abcdef: implausible chunk count 0", "RIFP 会话 0123456789abcdef：分块数 0 无效"),
            ("session 0123456789abcdef: manifest: expected value at line 1", "RIFP 会话 0123456789abcdef：清单解析失败：expected value at line 1"),
        ];
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (source, chinese) in cases {
                assert_eq!(rifp_error_status(source), if enabled { chinese } else { source });
            }
            assert_eq!(rifp_error_status("session 0123456789abcdef: future decoder detail"), "session 0123456789abcdef: future decoder detail");
            assert_eq!(rifp_error_status("transmit: unsupported grayscale depth 16"), if enabled { "RIFP 发射失败：unsupported grayscale depth 16" } else { "transmit: unsupported grayscale depth 16" });
        }
    }

    #[test]
    fn fixed_backend_status_reasons_switch_and_unknown_reasons_fall_back() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let statuses: Vec<_> = catalog.entries.iter().filter(|(key, _)| key.starts_with("display.backend.status.")).map(|(_, e)| e).collect();
        assert_eq!(statuses.len(), 24);
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for entry in &statuses {
                assert_eq!(backend_status(&entry.source), if enabled { entry.translation.clone() } else { entry.source.clone() });
            }
            assert_eq!(backend_status("new backend status from upstream"), "new backend status from upstream");
        }
    }

    #[test]
    fn numeric_backend_refusal_reasons_localize_values_and_restore_exact_source() {
        test_state();
        let cases = [
            ("ADS-B needs at least 2.5 Msps and this stream is 2.000 Msps — lower the front-end decimation, or raise the device sample rate", "ADS-B 至少需要 2.5 Msps 采样率，当前数据流为 2.000 Msps。请降低前端抽取率或提高设备采样率。"),
            ("1090.000 MHz is outside the receiver's window, which is 2.500 MHz wide about 1090.000 MHz", "接收机窗口不包含 1090.000 MHz；窗口宽度为 2.500 MHz，中心频率约为 1090.000 MHz。"),
            ("this stream is 2.000 Msps and a Mode S chip is half a microsecond, so the signal is barely sampled: the strong aircraft decode and the weak ones are lost. 2.5 Msps or more is what it takes — widen the receiver's window if it has the setting.", "当前数据流速率为 2.000 Msps。Mode S 芯片脉冲仅半微秒，采样不足会导致弱信号丢失。需要至少 2.5 Msps；如设备支持，请加宽接收机窗口。"),
            ("VDL2 needs at least 1000 kHz of stream and this one is 800.0 kHz — lower the front-end decimation, or raise the device sample rate", "VDL2 至少需要 1000 kHz 的数据流，当前为 800.0 kHz。请降低前端抽取率或提高设备采样率。"),
            ("no VDL2 channel is inside the receiver's window, which is 1.500 MHz wide about 136.800 MHz", "接收机窗口内没有 VDL2 信道；窗口宽度为 1.500 MHz，中心频率约为 136.800 MHz。"),
            ("this window is 500 kHz wide and reaches 5 of the 14 channels, 136.000 to 137.000 MHz — the other 9 are outside it", "当前窗口宽 500 kHz，可覆盖 5 个信道（共 14 个），范围为 136.000 至 137.000 MHz；其余 9 个信道位于窗口外。"),
            ("this window leaves only 1.5 samples per symbol; below 4 the symbol timing has too little to work with", "当前窗口每个符号仅有 1.5 个采样点；低于 4 时，符号定时将难以正常工作。"),
            ("AIS needs at least 96 kHz of stream and this one is 48.0 kHz — lower the front-end decimation, or raise the device sample rate", "AIS 至少需要 96 kHz 的数据流，当前为 48.0 kHz。请降低前端抽取率或提高设备采样率。"),
            ("neither AIS channel is inside the receiver's window, which is 100 kHz wide about 162.000 MHz", "接收机窗口内不包含 AIS 两个信道；窗口宽度为 100 kHz，中心频率约为 162.000 MHz。"),
            ("this window is 25 kHz wide and reaches AIS A only — a ship alternates between the two channels, so it will be heard at half its reporting rate (AIS B is outside it)", "当前窗口宽 25 kHz，仅覆盖 AIS A 信道。船舶会在两个信道间交替，因此报告频率减半（AIS B 位于窗口外）。"),
            ("this window leaves only 1.5 samples a bit; below 5 the bit timing has too little to work with", "当前窗口每比特仅有 1.5 个采样点；低于 5 时，比特定时将难以正常工作。"),
            ("the receiver's window is too narrow for 1024 kHz — choose a narrower bandwidth, or AUTO", "接收机窗口过窄，无法容纳 1024 kHz 带宽。请选择更窄的带宽，或选择 AUTO。"),
            ("HD Radio needs nrsc5's decoder library (libnrsc5.dll), which is not installed on this machine. sdroxide loads it at run time rather than building it in: install nrsc5 from your distribution or from github.com/theori-io/nrsc5, then restart sdroxide.", "HD Radio 需要 nrsc5 解码库（libnrsc5.dll），但本机尚未安装。sdroxide 会在运行时加载该库，而不是将其编译进程序：请通过系统软件源或 github.com/theori-io/nrsc5 安装 nrsc5，然后重新启动 sdroxide。"),
        ];
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (source, chinese) in cases {
                assert_eq!(backend_status(source), if enabled { chinese.to_owned() } else { source.to_owned() }, "{source}");
            }
        }
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            let source = "no enabled band is inside the receiver's window";
            assert_eq!(backend_status(source), if enabled { "接收机窗口内没有启用的频段" } else { source });
        }
    }

    #[test]
    fn public_receiver_block_reasons_translate_but_keep_device_names_and_counts() {
        test_state();
        let cases = [
            ("PhantomSDR's KiwiSDR bridge sends demodulated audio, not I/Q — sdroxide has no client for it yet", "PhantomSDR 的 KiwiSDR 桥接仅提供解调音频，不提供 I/Q 数据流；sdroxide 目前尚不支持该接口"),
            ("no client for this receiver's protocol yet", "尚不支持此接收机的通信协议"),
            ("no client for ExampleReceiver's protocol yet", "尚不支持 ExampleReceiver 使用的接收机协议"),
            ("operator has not enabled connections from non-browser apps", "操作员尚未允许浏览器以外的应用程序连接"),
            ("full — 3 of 3 channels in use", "容量已满：已有 3 个连接，最多允许 3 个。"),
        ];
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (source, chinese) in cases {
                assert_eq!(public_sdr_blocked_status(source), if enabled { chinese.to_owned() } else { source.to_owned() });
            }
            assert_eq!(public_sdr_blocked_status("new receiver refusal from upstream"), "new receiver refusal from upstream");
        }
    }

    #[test]
    fn agc_and_device_rate_notes_switch_preserve_values_and_unknown_source() {
        test_state();
        use sdroxide_types::{AgcMode, AirspyConfig, AirspyHfConfig, HackRfConfig, HydraSdrConfig, LimeConfig};
        let rates = [250_000.0,500_000.0,1_000_000.0,1_500_000.0,2_000_000.0,2_500_000.0,
                     3_000_000.0,4_000_000.0,4_096_000.0,5_000_000.0,6_000_000.0,8_000_000.0,
                     10_000_000.0,12_000_000.0,12_500_000.0,16_000_000.0,20_000_000.0,30_000_000.0,40_000_000.0,
                     912_000.0,768_000.0,650_000.0,456_000.0,228_000.0,256_000.0];
        for enabled in [false,true,false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled=enabled);
            for agc in AgcMode::ALL {
                assert_eq!(agc_text(agc.label())==agc.label(), !enabled);
                assert_eq!(agc.next().next().next().next(),agc);
            }
            for rate in rates {
                for source in [AirspyHfConfig::rate_note(rate),AirspyConfig::rate_note(rate),
                               HackRfConfig::rate_note(rate),HydraSdrConfig::rate_note(rate)] {
                    let translated=rate_note_text(source);
                    let numeric=source.is_empty() || matches!(source,"20 MB/s"|"25 MB/s"|"32 MB/s");
                    assert_eq!(translated==source, !enabled || numeric, "{source}");
                    let source_numbers:Vec<_>=source.split(|c:char|!c.is_ascii_digit()&&c!='.').filter(|s|s.bytes().any(|b|b.is_ascii_digit())).collect();
                    let translated_numbers:Vec<_>=translated.split(|c:char|!c.is_ascii_digit()&&c!='.').filter(|s|s.bytes().any(|b|b.is_ascii_digit())).collect();
                    assert_eq!(source_numbers,translated_numbers,"{source}");
                }
                if let Some(source)=LimeConfig::rate_note(rate) { assert_eq!(rate_note_text(source)==source,!enabled); }
            }
            assert_eq!(rate_note_text("new rate {raw} 123 MB/s"),"new rate {raw} 123 MB/s");
            assert_eq!(agc_text("New AGC"),"New AGC");
        }
    }

    #[test]
    fn nr_levels_engines_strengths_and_speeds_switch_without_changing_enums() {
        test_state();
        use sdroxide_types::{NrEngine, NrLevel, NrStrength, Speed};
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for level in NrLevel::ALL {
                let wire = serde_json::to_string(&level).unwrap();
                assert_eq!(serde_json::from_str::<NrLevel>(&wire).unwrap(), level);
                assert_eq!(nr_text(level.label()) == level.label(), !enabled);
                if let Some(engine) = level.engine() {
                    let strength = level.strength().unwrap();
                    assert_eq!(engine.at(strength), level);
                }
                assert_eq!(serde_json::to_string(&level).unwrap(), wire);
            }
            for engine in NrEngine::ALL { assert_eq!(nr_text(engine.name()) == engine.name(), !enabled); }
            for strength in NrStrength::ALL { assert_eq!(nr_text(strength.label()) == strength.label(), !enabled); }
            for speed in Speed::WATERFALL {
                let chip = speed_chip(speed);
                assert_eq!(chip == speed.label().to_uppercase(), !enabled);
            }
            assert_eq!(nr_text("NEW ALGORITHM {name}"), "NEW ALGORITHM {name}");
        }
    }

    #[test]
    fn decoder_summaries_translate_states_and_preserve_external_station_names() {
        test_state();
        use sdroxide_types::{DrmStatus, DrmSync, HdRadioStatus};
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            let mut drm = DrmStatus::default();
            assert_eq!(drm_summary(&drm), if enabled { "无信号" } else { "no signal" });
            drm.time_sync = DrmSync::Ok;
            assert_eq!(drm_summary(&drm), if enabled { "正在同步" } else { "syncing" });
            drm.locked = true;
            assert_eq!(drm_summary(&drm), if enabled { "正在获取业务信息" } else { "acquiring service" });
            let mut hd = HdRadioStatus::default();
            assert_eq!(hd_summary(&hd), if enabled { "无信号" } else { "no signal" });
            hd.locked = true;
            assert_eq!(hd_summary(&hd), if enabled { "正在获取业务信息" } else { "acquiring service" });
            for name in ["no signal", "acquiring service", "unavailable", "台站 {raw}"] {
                drm.service.label = name.into();
                hd.station_name = name.into();
                assert_eq!(drm_summary(&drm), name);
                assert_eq!(hd_summary(&hd), name);
            }
            hd.unavailable = Some("backend failure".into());
            assert_eq!(hd_summary(&hd), if enabled { "不可用" } else { "unavailable" });
        }
    }

    #[test]
    fn receiver_and_meter_catalog_keep_values_and_changed_source_fallback() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let entries: Vec<_> = catalog.entries.iter().filter(|(key, _)|
            key.starts_with("widgets.smeter.dynamic.") || key.starts_with("topbar.receiver.dynamic.")).collect();
        assert_eq!(entries.len(), 48);
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (key, entry) in &entries {
                let args: Vec<_> = entry.parameters.iter().enumerate()
                    .map(|(i, _)| format!("value-{i} {{raw}} -73.5")).collect();
                let expected = sdroxide_language_pack::render(
                    if enabled { &entry.translation } else { &entry.source }, &args).unwrap();
                assert_eq!(format(key, &entry.source, &args), expected);
                assert_eq!(text(key, "Changed upstream sentence"), "Changed upstream sentence");
                assert_eq!(format(key, "Changed upstream {}", &["-73.5 {raw}".into()]),
                           "Changed upstream -73.5 {raw}");
            }
        }
    }

    #[test]
    fn meter_hover_switches_actual_readings_and_optional_telemetry_without_radio() {
        test_state();
        use sdroxide_types::{Meters, PsMeter, TxMeters};
        let mut reading = Meters {
            s_dbm: -63.0, adc_peak_dbfs: -1.0, adc_clip: 0.0, adc_overload: Some(true),
            pa_temp_c: Some(42.0), tx: None, stereo: false, tone: None,
            passband_dbfs: -45.0,
            puresignal: Some(PsMeter { locked: true, correction_db: 3.5, score: 0.87, frozen: true }),
        };
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            let absent = crate::widgets::smeter::hover_text(None);
            assert!(absent.starts_with(if enabled { "尚无信号报告" } else { "No signal report yet" }));
            reading.tx = None;
            reading.s_dbm = -63.0;
            reading.puresignal.as_mut().unwrap().locked = true;
            reading.puresignal.as_mut().unwrap().frozen = true;
            let received = crate::widgets::smeter::hover_text(Some(&reading));
            assert!(received.contains("S9 + 10 dB") && received.contains("-63 dBm"));
            assert!(received.contains("42 °C") && received.contains("3.5 dB") && received.contains("0.87"));
            assert!(received.contains(if enabled { "校正表已冻结" } else { "table held" }));
            assert!(received.contains(if enabled { "模数转换器已过载" } else { "converter is into its rails" }));
            reading.s_dbm = -85.0;
            reading.puresignal.as_mut().unwrap().frozen = false;
            let weak = crate::widgets::smeter::hover_text(Some(&reading));
            assert!(weak.contains("S7") && weak.contains("-85 dBm"));
            assert!(!weak.contains("table held") && !weak.contains("校正表已冻结"));
            reading.puresignal.as_mut().unwrap().locked = false;
            let unlocked = crate::widgets::smeter::hover_text(Some(&reading));
            assert!(unlocked.contains(if enabled { "尚未在反馈通道中找到" } else { "has not found the transmission" }));
            reading.tx = Some(TxMeters { fwd_w: Some(12.5), swr: Some(1.4), alc: 0.5, po: Some(0.75) });
            let telemetry = crate::widgets::smeter::hover_text(Some(&reading));
            assert!(telemetry.contains("12.5 W") && telemetry.contains("1.4:1") && telemetry.contains("75 %"));
            assert!(telemetry.contains(if enabled { "正向功率" } else { "Forward power" }));
            reading.tx.as_mut().unwrap().fwd_w = None;
            reading.tx.as_mut().unwrap().swr = None;
            let missing = crate::widgets::smeter::hover_text(Some(&reading));
            assert!(missing.contains(if enabled { "不报告正向功率" } else { "reports no forward power" }));
            assert!(missing.contains(if enabled { "无驻波比读数" } else { "No SWR" }));
        }
    }

    #[test]
    fn browser_audio_notice_switches_and_preserves_origin_and_source_fallback() {
        test_state();
        let original = "No audio: {} is not a secure origin, so this browser withholds audio playback and microphone access. Use HTTPS or an SSH tunnel to localhost. (Click to dismiss.)";
        let origin = "http://radio-{raw}:4950";
        assert_eq!(browser_audio_notice(original, origin), original.replace("{}", origin));
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        let translated = browser_audio_notice(original, origin);
        assert!(translated.starts_with("无音频："));
        assert!(translated.contains(origin));
        assert_eq!(browser_audio_notice("Changed source: {}", origin), format!("Changed source: {origin}"));
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(browser_audio_notice(original, origin), original.replace("{}", origin));
    }

    #[test]
    fn switching_and_source_changes_fall_back_without_touching_values() {
        test_state();
        assert_eq!(text("settings.ui.layout", "Layout"), "Layout");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(text("settings.ui.layout", "Layout"), "布局");
        assert_eq!(text("settings.ui.layout", "Updated upstream label"), "Updated upstream label");
        assert_eq!(format("missing", "Callsign {}", &["BI4APF {raw}".into()]), "Callsign BI4APF {raw}");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(text("settings.ui.layout", "Layout"), "Layout");
    }

    #[test]
    fn fixed_radio_placeholders_localize_and_real_capability_labels_are_preserved() {
        test_pack_enabled(true);
        for (source, expected) in [
            ("No radio", "无电台"),
            ("Switched off", "已关闭"),
            ("Panadapter receiver", "频谱接收机"),
            ("Signal generator", "信号发生器"),
        ] {
            assert_eq!(radio_label(source), expected);
        }
        assert_eq!(radio_label("SDRplay RSP1A"), "SDRplay RSP1A");
        test_pack_enabled(false);
        for source in ["No radio", "Switched off", "Panadapter receiver", "Signal generator"] {
            assert_eq!(radio_label(source), source);
        }
        assert_eq!(radio_label("SDRplay RSP1A"), "SDRplay RSP1A");
        test_pack_enabled(true);
        assert_eq!(radio_label("A translated-looking 设备 label"), "A translated-looking 设备 label");
    }

    #[test]
    fn bundled_complete_pack_has_no_incomplete_warning_but_imported_catalog_does() {
        assert!(!pack_is_partial("complete", false));
        assert!(pack_is_partial("partial", false));
        assert!(pack_is_partial("complete", true));
    }

    #[test]
    fn upstream_ft8_decode_depth_labels_and_help_translate_and_fall_back() {
        test_state();
        let labels = [
            ("upstream.ft8.decode_depth.label", "Decode depth", "解码深度"),
            ("upstream.ft8.decode_depth.fast", "Fast", "快速"),
            ("upstream.ft8.decode_depth.normal", "Normal", "普通"),
            ("upstream.ft8.decode_depth.deep", "Deep", "深度"),
        ];
        for (key, source, _) in labels {
            assert_eq!(text(key, source), source);
        }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        for (key, _, translation) in labels {
            assert_eq!(text(key, match key {
                "upstream.ft8.decode_depth.label" => "Decode depth",
                "upstream.ft8.decode_depth.fast" => "Fast",
                "upstream.ft8.decode_depth.normal" => "Normal",
                _ => "Deep",
            }), translation, "{key}");
        }
        assert_eq!(
            text("upstream.ft8.decode_depth.fast", "Changed upstream label"),
            "Changed upstream label",
        );
        let source_help = "How hard the FT8 decoder works for weak signals.\n\nFast — one pass, no signal subtraction: quickest, fewest decodes.\nNormal — flat multi-pass subtraction: a little quicker than Deep, a little less thorough.\nDeep — the checkpointed multi-pass: the most decodes, and ~1.2 s on a busy slot.";
        assert_eq!(
            text("upstream.ft8.decode_depth.help", source_help),
            "设置 FT8 解码器搜寻弱信号的力度。\n\n快速：单轮解码，不做信号相减；速度最快，解码量最少。\n普通：平坦式多轮信号相减；比深度模式稍快，搜索也略少。\n深度：带检查点的多轮解码；解码量最多，繁忙时隙约需 1.2 秒。",
        );
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        for (key, source, _) in labels {
            assert_eq!(text(key, source), source, "{key}");
        }
        assert_eq!(text("upstream.ft8.decode_depth.help", source_help), source_help);
    }

    #[test]
    fn topbar_squelch_label_translates_and_falls_back_exactly() {
        test_state();
        assert_eq!(text("topbar.sql", "SQL"), "SQL");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(text("topbar.sql", "SQL"), "静噪");
        assert_eq!(text("topbar.sql", "Squelch"), "Squelch");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(text("topbar.sql", "SQL"), "SQL");
    }

    #[test]
    fn remaining_protocol_feature_titles_translate_and_fall_back_exactly() {
        test_state();
        let labels = [
            ("panels.adsb.mode_s", "1090 MHz Mode S", "1090 MHz S 模式"),
            ("settings.net.freedv_reporter_title", "FreeDV Reporter", "FreeDV 通联上报"),
        ];
        for (key, source, _) in labels {
            assert_eq!(text(key, source), source, "{key}");
        }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        for (key, _, translation) in labels {
            assert_eq!(text(key, match key {
                "panels.adsb.mode_s" => "1090 MHz Mode S",
                _ => "FreeDV Reporter",
            }), translation, "{key}");
        }
        assert_eq!(
            text("panels.adsb.mode_s", "Changed upstream label"),
            "Changed upstream label",
        );
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        for (key, source, _) in labels {
            assert_eq!(text(key, source), source, "{key}");
        }
    }

    #[test]
    fn plugin_settings_labels_follow_language_switch_without_bilingual_mixture() {
        test_state();
        let labels = [
            ("plugins.language.title", "Language packs", "语言包"),
            ("plugins.language.name", "Simplified Chinese", "简体中文"),
            ("plugins.language.version", "Language pack version", "语言包版本"),
            ("plugins.language.incomplete", "This language pack is incomplete.", "此语言包尚未完整翻译。"),
            ("plugins.language.import_folder", "Import language pack folder", "导入语言包文件夹"),
            ("plugins.language.select_folder", "Select language pack", "选择语言包文件夹"),
            ("plugins.language.import_json", "Import translation JSON", "导入翻译 JSON"),
            ("plugins.language.restore_bundled", "Use bundled pack", "恢复内置语言包"),
            ("plugins.language.manual_unavailable", "Chinese manual unavailable; using English", "中文手册不可用，已切换为英文。"),
            ("plugins.language.manual_details", "Manual details", "手册详情"),
            ("plugins.language.error_title", "Language pack error", "语言包错误"),
            ("plugins.language.details", "Technical details", "技术详情"),
        ];
        for (key, source, _) in labels { assert_eq!(text(key, source), source); }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        for (key, source, translation) in labels { assert_eq!(text(key, source), translation, "{key}"); }
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        for (key, source, _) in labels { assert_eq!(text(key, source), source, "{key}"); }
    }

    #[test]
    fn rtty_psk_speech_modes_use_chinese_list_punctuation_and_fall_back() {
        test_state();
        let source = "RTTY, PSK, Olivia, THOR, FSQ";
        assert_eq!(text("settings.ui.speech.rtty_psk_label", source), source);
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(text("settings.ui.speech.rtty_psk_label", source), "RTTY、PSK、Olivia、THOR、FSQ");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(text("settings.ui.speech.rtty_psk_label", source), source);
    }

    #[test]
    fn remaining_hd_title_and_tls_label_switch_and_restore_source() {
        test_state();
        assert_eq!(text("window.hd.title", "HD Radio"), "HD Radio");
        assert_eq!(text("settings.net.tls_label", "TLS (wss://)"), "TLS (wss://)");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(text("window.hd.title", "HD Radio"), "高清数字广播（HD Radio）");
        assert_eq!(text("settings.net.tls_label", "TLS (wss://)"), "TLS（安全 WebSocket，wss://）");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(text("window.hd.title", "HD Radio"), "HD Radio");
        assert_eq!(text("settings.net.tls_label", "TLS (wss://)"), "TLS (wss://)");
    }

    #[test]
    fn drm_measurement_and_sstv_id_labels_switch_and_restore_source() {
        test_state();
        for (key, source) in [("window.drm.snr_label", "SNR"), ("window.drm.mer_label", "MER"), ("panels.sstv.fsk_id_label", "FSK ID")] {
            assert_eq!(text(key, source), source);
        }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(text("window.drm.snr_label", "SNR"), "信噪比");
        assert_eq!(text("window.drm.mer_label", "MER"), "调制误差比");
        assert_eq!(text("panels.sstv.fsk_id_label", "FSK ID"), "FSK 呼号标识");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(text("window.drm.snr_label", "SNR"), "SNR");
        assert_eq!(text("window.drm.mer_label", "MER"), "MER");
        assert_eq!(text("panels.sstv.fsk_id_label", "FSK ID"), "FSK ID");
    }

    #[test]
    fn sstv_received_id_translates_only_its_label_and_restores_the_original() {
        test_state();
        assert_eq!(format("panels.sstv.rx_id_label", "ID {id}", &["BI4APF".into()]), "ID BI4APF");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(format("panels.sstv.rx_id_label", "ID {id}", &["BI4APF".into()]), "呼号标识 BI4APF");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(format("panels.sstv.rx_id_label", "ID {id}", &["BI4APF".into()]), "ID BI4APF");
    }

    #[test]
    fn rade_signal_to_noise_readout_localizes_and_keeps_the_measurement() {
        test_state();
        assert_eq!(format("panels.rade.snr_reading", "SNR {:.0} dB", &["12".into()]), "SNR 12 dB");
        assert_eq!(text("panels.rade.snr_unavailable", "SNR —"), "SNR —");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(format("panels.rade.snr_reading", "SNR {:.0} dB", &["12".into()]), "信噪比 12 dB");
        assert_eq!(text("panels.rade.snr_unavailable", "SNR —"), "信噪比 —");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(format("panels.rade.snr_reading", "SNR {:.0} dB", &["12".into()]), "SNR 12 dB");
        assert_eq!(text("panels.rade.snr_unavailable", "SNR —"), "SNR —");
    }

    #[test]
    fn chinese_count_has_no_english_suffix_and_fallback_keeps_inflection() {
        test_state();
        let key = "window.sat.text_825_0e36d9";
        let source = "⚠ {} rigctld client{} connected — satellite software steering the dial there would correct Doppler twice";
        assert_eq!(plural_suffix(key, source, false), "s");
        assert_eq!(plural_suffix(key, source, true), "");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        let suffix = plural_suffix(key, source, false);
        let sentence = format(key, source, &["2".into(), suffix.into()]);
        assert_eq!(sentence, "⚠ 已连接 2 个 rigctld 客户端；若卫星软件同时控制频率，将重复进行多普勒校正");
        assert_eq!(plural_suffix(key, "Updated client{} warning", false), "s");
        assert_eq!(plural_suffix("missing", source, false), "s");
    }

    #[test]
    fn solar_names_search_and_calendar_preserve_source_identity_and_fallback() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (_, entry) in catalog.entries.iter().filter(|(key, _)| key.starts_with("display.solar.names.")) {
                for original in [&entry.source, &entry.source.to_uppercase()] {
                    assert_eq!(solar_name(original), if enabled { entry.translation.clone() } else { original.clone() });
                }
            }
            for body in sdroxide_solar::smallbody::BODIES {
                assert!(solar_body_matches(body, body.name));
                assert!(solar_body_matches(body, body.designation));
                assert!(!solar_body_matches(body, "   "));
                assert!(solar_body_matches(body, &solar_name(body.name)));
            }
            let pluto = sdroxide_solar::smallbody::find("Pluto").unwrap();
            assert_eq!(solar_body_matches(pluto, "  冥王星  "), enabled);
            assert!(solar_body_matches(pluto, "134340"));
            assert_eq!(pluto.name, "Pluto");
            assert_eq!(sdroxide_solar::Planet::Mars.name(), "Mars");
            assert_eq!(solar_name("NEW {body} NAME"), "NEW {body} NAME");
            assert_eq!(solar_name("2024 YR4"), "2024 YR4");
            assert_eq!(solar_group("HOME"), if enabled { "太阳与地月" } else { "HOME" });
            assert_eq!(solar_group("DWARF PLANETS"), if enabled { "矮行星" } else { "DWARF PLANETS" });
            assert_eq!(solar_group("Mars"), if enabled { "火星" } else { "Mars" });
            for channel in sdroxide_solar::SdoChannel::ALL {
                assert_eq!(sdroxide_solar::SdoChannel::from_u8(channel.to_u8()), channel);
                let original = channel.description();
                assert_eq!(scope_text("display.solar.channel.", original) == original, !enabled);
            }
            for (y, m, d, expected) in [(2024, 2, 29, "2024年2月29日"),
                                      (2026, 12, 31, "2026年12月31日"), (2027, 1, 1, "2027年1月1日")] {
                let unix = sdroxide_types::ymd_hms_to_unix(y,m,d,0,0,0);
                assert_eq!(solar_calendar(unix), if enabled { expected.to_owned() } else { sdroxide_solar::timefmt::dmy(unix) });
            }
            for (_, entry) in catalog.entries.iter().filter(|(key, _)| key.starts_with("solar.overlay.layers.")) {
                assert_eq!(scope_text("solar.overlay.layers.", &entry.source), if enabled { entry.translation.clone() } else { entry.source.clone() });
            }
        }
        STATE.with(|s| {
            let mut s=s.borrow_mut();
            s.prefs.enabled=true;
            for (key, entry) in &mut s.catalog.as_mut().unwrap().entries {
                if key.starts_with("display.solar.calendar.") { entry.source="Updated calendar source".into(); }
                if key.starts_with("display.solar.names.") && entry.source=="Mercury" { entry.source="Updated Mercury source".into(); }
            }
        });
        let unix=sdroxide_types::ymd_hms_to_unix(2027,1,1,0,0,0);
        assert_eq!(solar_calendar(unix),sdroxide_solar::timefmt::dmy(unix));
        assert_eq!(solar_name("MERCURY"),"MERCURY");
        assert_eq!(scope_text("solar.overlay.layers.","Updated layer description"),"Updated layer description");
        STATE.with(|s| s.borrow_mut().prefs.enabled=false);
    }

    #[test]
    fn solar_freshness_symbols_and_forecasts_revert_without_changing_engine_data() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let samples = [(-1, "0 秒"), (90, "90 秒"), (91, "2 分钟"),
            (5399, "90 分钟"), (5400, "2 小时"), (172799, "48 小时"), (172800, "2 天")];
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for (seconds, chinese) in samples {
                let original = sdroxide_solar::timefmt::age(seconds);
                assert_eq!(solar_age(seconds), if enabled { chinese.to_owned() } else { original.clone() });
                assert_eq!(sdroxide_solar::timefmt::age(seconds), original);
            }
            for prefix in ["panels.setup.symbols.", "display.solar.confidence.", "display.solar.activity."] {
                for (_, entry) in catalog.entries.iter().filter(|(key, _)| key.starts_with(prefix)) {
                    assert_eq!(scope_text(prefix, &entry.source), if enabled { entry.translation.clone() } else { entry.source.clone() });
                }
                assert_eq!(scope_text(prefix, "Updated {raw} source"), "Updated {raw} source");
            }
            assert_eq!(sdroxide_solar::HemisphericPower::words(0.0), "quiet");
            assert_eq!(sdroxide_types::AprsSymbol::new('/', '-').text(), "/-");
        }
    }

    #[test]
    fn receive_panel_counts_and_paths_revert_without_rewriting_external_values() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        for enabled in [false, true, false] {
            STATE.with(|s| s.borrow_mut().prefs.enabled = enabled);
            for prefix in ["panels.pi4.dynamic.", "panels.wspr.dynamic."] {
                let (key, entry) = catalog.entries.iter().find(|(key, entry)| {
                    key.starts_with(prefix) && entry.source == "{} beacon{}"
                }).unwrap();
                for count in [0, 1, 2] {
                    let suffix = plural_suffix(key, &entry.source, count == 1);
                    let rendered = format(key, &entry.source, &[count.to_string(), suffix.into()]);
                    let expected = if enabled { format!("{count} 个信标") }
                        else { format!("{count} beacon{}", if count == 1 { "" } else { "s" }) };
                    assert_eq!(rendered, expected);
                }
                let changed = "Updated {} beacon{} count";
                let suffix = plural_suffix(key, changed, false);
                assert_eq!(format(key, changed, &["2".into(), suffix.into()]), "Updated 2 beacons count");
            }
            let (key, entry) = catalog.entries.iter().find(|(key, entry)| {
                key.starts_with("panels.sstv.dynamic.") && entry.source == "on the radio, in {dir}"
            }).unwrap();
            let path = r"C:\RX {raw}\pictures";
            let rendered = format(key, &entry.source, &[path.into()]);
            assert!(rendered.contains(path));
            if enabled { assert!(rendered.starts_with("电台所在机器的 ")); }
            else { assert_eq!(rendered, format!("on the radio, in {path}")); }
        }
    }

    #[test]
    fn ais_tables_translate_at_display_and_revert_when_disabled() {
        test_state();
        let mut originals = Vec::new();
        for code in 0..=u8::MAX {
            originals.push(sdroxide_types::nav_status_label(code));
            originals.push(sdroxide_types::ship_type_label(code));
            originals.push(sdroxide_types::aid_type_label(code));
            if let Some(label) = sdroxide_types::ship_type_hazard(code) { originals.push(label); }
        }
        for kind in [sdroxide_types::AisKind::ClassA, sdroxide_types::AisKind::ClassB,
                     sdroxide_types::AisKind::BaseStation, sdroxide_types::AisKind::SarAircraft,
                     sdroxide_types::AisKind::AidToNavigation, sdroxide_types::AisKind::Sart,
                     sdroxide_types::AisKind::Craft] { originals.push(kind.label()); }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        for original in &originals { assert_ne!(ais_label(original), *original, "{original}"); }
        assert_eq!(ais_label(sdroxide_types::nav_status_label(2)), "失去控制");
        assert_eq!(sdroxide_types::nav_status_label(2), "not under command");
        assert_eq!(ais_label("Updated upstream AIS label"), "Updated upstream AIS label");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        for original in &originals { assert_eq!(ais_label(original), *original); }
    }

    #[test]
    fn adsb_category_and_emergency_display_reverts_and_keeps_unknown_values() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let entries: Vec<_> = catalog.entries.iter().filter(|(key, _)| key.starts_with("display.adsb.")).collect();
        assert_eq!(entries.len(), 22);
        for (_, entry) in &entries { assert_eq!(adsb_label(&entry.source), entry.source); }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        for (_, entry) in &entries { assert_eq!(adsb_label(&entry.source), entry.translation); }
        assert_eq!(adsb_label("minimum fuel"), "最低油量");
        assert_eq!(adsb_label("BI4APF {unknown label}"), "BI4APF {unknown label}");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        for (_, entry) in &entries { assert_eq!(adsb_label(&entry.source), entry.source); }
    }

    #[test]
    fn rds_tables_switch_language_without_rewriting_station_data() {
        test_state();
        let catalog = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let entries: Vec<_> = catalog.entries.iter().filter(|(key, _)| key.starts_with("display.rds.")).collect();
        assert_eq!(entries.len(), 80);
        let mut originals = Vec::new();
        for standard in sdroxide_types::RdsStandard::ALL {
            originals.push(standard.label());
            for code in 0..=u8::MAX { originals.push(sdroxide_types::pty_name(code, standard)); }
        }
        for code in 0..=u8::MAX {
            if let Some(label) = sdroxide_types::rt_plus_class(code) { originals.push(label); }
        }
        for original in &originals { assert_eq!(rds_label(original), *original); }
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        for original in &originals { assert_ne!(rds_label(original), *original, "{original}"); }
        for (_, entry) in &entries { assert_eq!(rds_label(&entry.source), entry.translation); }
        assert_eq!(rds_label(sdroxide_types::pty_name(5, sdroxide_types::RdsStandard::Rds)), "教育");
        assert_eq!(rds_label(sdroxide_types::pty_name(5, sdroxide_types::RdsStandard::Rbds)), "摇滚音乐");
        assert_eq!(sdroxide_types::pty_name(5, sdroxide_types::RdsStandard::Rds), "Education");
        assert_eq!(sdroxide_types::rt_plus_class(63), None);
        assert_eq!(rds_label("Unknown {station text}"), "Unknown {station text}");
        let data = sdroxide_types::RdsData {
            pi: Some(0x1000), ps: Some("News".into()), radiotext: Some("Title {raw}".into()),
            ptyn: Some("Rock".into()), ..Default::default()
        };
        assert_eq!(data.title(sdroxide_types::RdsStandard::Rbds).as_deref(), Some("News"));
        assert_eq!(data.radiotext.as_deref(), Some("Title {raw}"));
        assert_eq!(data.ptyn.as_deref(), Some("Rock"));
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        for original in &originals { assert_eq!(rds_label(original), *original); }
    }

    #[test]
    fn common_time_direction_date_and_es_inflection_follow_language() {
        test_state();
        let key = "map.hfdl.text_367_d2b285";
        let original = "\n{} fix{}";
        assert_eq!(plural_suffix_with(key, original, false, "es"), "es");
        assert_eq!(month_day(1, 3, "3 Jan".into()), "3 Jan");
        assert_eq!(compass(-90.0), "W");
        assert_eq!(crate::app::util::fmt_age(60), "1m");
        STATE.with(|s| s.borrow_mut().prefs.enabled = true);
        assert_eq!(month_day(1, 3, "3 Jan".into()), "1月3日");
        assert_eq!(month_day(0, 3, "unknown".into()), "unknown");
        let points = ["北", "北北东", "东北", "东北东", "东", "东南东", "东南", "南南东",
                      "南", "南南西", "西南", "西南西", "西", "西北西", "西北", "北北西"];
        for (i, expected) in points.iter().enumerate() { assert_eq!(compass(i as f64 * 22.5), *expected); }
        assert_eq!(compass(-90.0), "西");
        assert_eq!(compass(360.0), "北");
        for (seconds, expected) in [(-1,"0秒"),(59,"59秒"),(60,"1分钟"),(3599,"59分钟"),(3600,"1小时")] {
            assert_eq!(crate::app::util::fmt_age(seconds), expected);
        }
        let suffix = plural_suffix_with(key, original, false, "es");
        assert_eq!(format(key, original, &["2".into(), suffix.into()]), "\n2 次定位");
        assert_eq!(plural_suffix_with(key, "Updated {} fix{}", false, "es"), "es");
        STATE.with(|s| s.borrow_mut().prefs.enabled = false);
        assert_eq!(format(key, original, &["2".into(), plural_suffix_with(key, original, false, "es").into()]), "\n2 fixes");
    }

    #[test]
    fn discovery_has_chinese_glyphs_before_translation_is_enabled() {
        test_state();
        let mut fonts = FontDefinitions::default();
        add_fonts(&mut fonts);
        assert!(fonts.font_data.contains_key("zh-CN-plugin"));
        for family in [FontFamily::Proportional, FontFamily::Monospace,
            FontFamily::Name("chakra-bold".into()), FontFamily::Name("cyber-mono".into())] {
            assert!(fonts.families[&family].iter().any(|f| f == "zh-CN-plugin"));
        }
        validate_font(include_bytes!("../../../plugins/zh-CN/fonts/NotoSansSC.ttf")).unwrap();
        assert!(validate_font(b"invalid font").is_err());
    }

    #[test]
    fn native_loader_reads_the_separate_pack() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/zh-CN");
        let prefs = Preferences { folder: folder.to_string_lossy().into_owned(), ..Default::default() };
        let (c, _, v, partial, manual, help_error) = load_default(&prefs).unwrap();
        assert_eq!(c.text("settings.ui.layout", "Layout"), "布局");
        assert!(v.ends_with("preview"));
        assert!(!partial);
        assert!(!prefs.enabled);
        assert!(manual.is_some());
        assert!(help_error.is_none());
    }

    #[test]
    fn manual_rejects_incomplete_or_changed_heading_structure() {
        let raw = include_str!("../../../plugins/zh-CN/manual.zh-CN.json");
        assert!(parse_manual(raw).is_ok());
        let mut value: serde_json::Value = serde_json::from_str(raw).unwrap();
        value["markdown"] = serde_json::json!("## Only one heading\n");
        assert!(parse_manual(&value.to_string()).is_err());
        value["source_sha256"] = serde_json::json!("stale source");
        assert!(parse_manual(&value.to_string()).is_err());
    }
}

pub fn settings(ui: &mut egui::Ui) {
    ui.heading(text("plugins.language.title", "Language packs"));
    let (mut enabled, available, version, partial, error) = STATE.with(|s| {
        let s = s.borrow();
        (s.prefs.enabled, s.catalog.is_some() && s.font.is_some(), s.version.clone(), s.partial, s.error.clone())
    });
    if ui.add_enabled(available, egui::Checkbox::new(&mut enabled, text("plugins.language.name", "Simplified Chinese"))).changed() {
        STATE.with(|s| { let mut s = s.borrow_mut(); s.prefs.enabled = enabled; s.font_dirty = true; });
        persist(); ui.ctx().request_repaint();
    }
    if available {
        ui.horizontal(|ui| {
            ui.label(text("plugins.language.version", "Language pack version"));
            ui.label(version);
        });
        if partial { ui.colored_label(Color32::YELLOW, text("plugins.language.incomplete", "This language pack is incomplete.")); }
    }
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button(text("plugins.language.import_folder", "Import language pack folder")).clicked()
        && let Some(root) = rfd::FileDialog::new().set_title(text("plugins.language.select_folder", "Select language pack")).pick_folder() {
        let mut prefs = STATE.with(|s| s.borrow().prefs.clone());
        prefs.folder = root.to_string_lossy().into_owned();
        match load_default(&prefs) {
            Ok((c, font, v, partial, manual, help_error)) => {
                STATE.with(|s| { let mut s = s.borrow_mut(); s.prefs = prefs; s.catalog = Some(c);
                    s.font = Some(font); s.version = v; s.partial = partial; s.error = None; s.font_dirty = true; });
                STATE.with(|s| { let mut s = s.borrow_mut(); s.manual = manual; s.help_error = help_error; });
                persist(); ui.ctx().request_repaint();
            }
            Err(e) => STATE.with(|s| s.borrow_mut().error = Some(e)),
        }
    }
    #[cfg(target_arch = "wasm32")]
    if ui.button(text("plugins.language.import_json", "Import translation JSON")).clicked() {
        let ctx = ui.ctx().clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(file) = rfd::AsyncFileDialog::new().add_filter("Language catalog", &["json"]).pick_file().await else { return };
            let bytes = file.read().await;
            let result = String::from_utf8(bytes).map_err(|e| e.to_string()).and_then(|raw| {
                let catalog = Catalog::parse(&raw)?; Ok((raw, catalog))
            });
            match result {
                Ok((raw, catalog)) => { STATE.with(|s| {
                    let mut s = s.borrow_mut(); s.prefs.imported_catalog = Some(raw); s.catalog = Some(catalog);
                    s.partial = true; s.error = None;
                }); persist(); }
                Err(e) => STATE.with(|s| s.borrow_mut().error = Some(e)),
            }
            ctx.request_repaint();
        });
    }
    if ui.button(text("plugins.language.restore_bundled", "Use bundled pack")).clicked() {
        let prefs = Preferences { enabled, ..Default::default() };
        match load_default(&prefs) {
            Ok((c, font, v, partial, manual, help_error)) => {
                STATE.with(|s| { let mut s = s.borrow_mut(); s.prefs = prefs; s.catalog = Some(c);
                    s.font = Some(font); s.version = v; s.partial = partial; s.error = None; s.font_dirty = true; });
                STATE.with(|s| { let mut s = s.borrow_mut(); s.manual = manual; s.help_error = help_error; });
                persist(); ui.ctx().request_repaint();
            }
            Err(e) => STATE.with(|s| s.borrow_mut().error = Some(e)),
        }
    }
    if let Some(error) = STATE.with(|s| s.borrow().help_error.clone()) {
        ui.colored_label(Color32::YELLOW, text("plugins.language.manual_unavailable", "Chinese manual unavailable; using English"));
        egui::CollapsingHeader::new(text("plugins.language.manual_details", "Manual details")).id_salt("language-plugin-help-error")
            .show(ui, |ui| { ui.label(error); });
    }
    if let Some(error) = error {
        ui.colored_label(Color32::RED, text("plugins.language.error_title", "Language pack error"));
        egui::CollapsingHeader::new(text("plugins.language.details", "Technical details")).id_salt("language-plugin-error-details").show(ui, |ui| { ui.label(error); });
    }
    ui.separator();
}

/// Render freshness at its UI boundary, using the engine's original rounding.
pub fn solar_age(seconds: i64) -> String {
    let original = sdroxide_solar::timefmt::age(seconds);
    let Some((number, unit)) = original.split_once(' ') else { return original; };
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) { return original; }
    let template = match unit { "s" => "{s} s", "min" => "{} min", "h" => "{} h", "d" => "{} d", _ => return original };
    let translated = scope_text("display.solar.age.", template);
    sdroxide_language_pack::render(&translated, &[number.to_owned()]).unwrap_or(original)
}

/// Uppercase scene labels and canonical names share the same reviewed entry.
pub fn solar_name(original: impl AsRef<str>) -> String {
    let original = original.as_ref();
    STATE.with(|state| {
        let state = state.borrow();
        if state.prefs.enabled && let Some(catalog) = &state.catalog
            && let Some((key, entry)) = catalog.entries.range::<str, _>((std::ops::Bound::Included("display.solar.names."), std::ops::Bound::Unbounded))
                .take_while(|(key, _)| key.starts_with("display.solar.names."))
                .find(|(_, entry)| entry.source.eq_ignore_ascii_case(original)) {
            let translated = catalog.text(key, &entry.source);
            if translated != entry.source { return translated; }
        }
        original.to_owned()
    })
}

pub fn solar_group(original: &str) -> String {
    let translated = scope_text("display.solar.groups.", original);
    if translated != original { translated } else { solar_name(original) }
}

/// Add visible Chinese names as search aliases while keeping raw designators.
pub fn solar_body_matches(body: &sdroxide_solar::SmallBody, query: &str) -> bool {
    if body.matches(query) { return true; }
    let query = query.trim();
    !query.is_empty() && solar_name(body.name).to_lowercase().contains(&query.to_lowercase())
}

pub fn solar_calendar(unix: i64) -> String {
    let original = sdroxide_solar::timefmt::dmy(unix);
    let template = "{year}/{month}/{day}";
    let translated = scope_text("display.solar.calendar.", template);
    if translated == template { return original; }
    let (year, month, day, _, _, _) = sdroxide_types::utc_ymd_hms(unix);
    sdroxide_language_pack::render(&translated, &[format!("{year:04}"), month.to_string(), day.to_string()]).unwrap_or(original)
}


#[cfg(test)]
mod colormap_name_language_tests {
    use eframe::egui;
    #[test]
    fn palette_names_render_in_chinese_and_fall_back_without_changing_order() {
        let cases = [("Classic", "经典"), ("Viridis", "Viridis"), ("Gray", "灰度"),
            ("Icom", "Icom"), ("Neon", "霓虹"), ("Synthwave", "合成波"),
            ("Matrix", "矩阵"), ("Tron", "Tron"), ("Amber", "琥珀"),
            ("Rainbow", "彩虹"), ("Blue", "蓝色")];
        let mut fonts = egui::FontDefinitions::default();
        crate::language_plugin::add_fonts(&mut fonts);
        let ctx = egui::Context::default(); ctx.set_fonts(fonts);
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (source, chinese) in cases {
                let label = crate::language_plugin::colormap_name(source);
                assert_eq!(label, if enabled { chinese } else { source });
                let output = ctx.run_ui(egui::RawInput::default(), |ui| { ui.label(&label); });
                let drawn: Vec<_> = output.shapes.iter().filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()), _ => None,
                }).collect();
                output.drop_without_applying_deltas();
                assert!(drawn.iter().any(|text| text == &label), "{drawn:?}");
            }
        }
        assert_eq!(crate::colormap::NAMES, ["Classic", "Viridis", "Gray", "Icom", "Neon", "Synthwave", "Matrix", "Tron", "Amber", "Rainbow", "Blue"]);
    }
}


#[cfg(test)]
mod atchat_roster_status_language_tests {
    use eframe::egui;
    #[test]
    fn roster_status_tooltips_translate_known_states_and_keep_unknown_data() {
        let cases = [("active", "在线"), ("lost", "已失联"), ("future-status", "future-status")];
        let mut fonts = egui::FontDefinitions::default();
        crate::language_plugin::add_fonts(&mut fonts);
        let ctx = egui::Context::default(); ctx.set_fonts(fonts);
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (source, chinese) in cases {
                let label = crate::language_plugin::atchat_roster_status(source);
                assert_eq!(label, if enabled && source != "future-status" { chinese } else { source });
                let output = ctx.run_ui(egui::RawInput::default(), |ui| { ui.label(&label); });
                let drawn: Vec<_> = output.shapes.iter().filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()), _ => None,
                }).collect();
                output.drop_without_applying_deltas();
                assert!(drawn.iter().any(|text| text == &label), "{drawn:?}");
            }
        }
    }
}
