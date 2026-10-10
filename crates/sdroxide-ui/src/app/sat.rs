//! The SAT window: pick a satellite, lock on, and operate through it.
//!
//! The lock itself lives in the engine — this window only *asks* for one
//! ([`Command::SetSatLock`]) and displays the [`sdroxide_types::SatTrackStatus`]
//! stream that comes back: look angles, range, and the Doppler corrections as
//! actually applied. That split is why a lock keeps correcting with this
//! window closed, and why a remote client sees the same numbers.
//!
//! The picker is built from the same element sets the 3D view tracks: the
//! operator's pasted TLEs and the cached subscription listings (native; the
//! browser gets the pasted sets and the curated names, and the engine — which
//! has the caches — resolves the elements when it locks). Pass prediction for
//! the list is memoized one satellite per frame: a search over 48 hours is
//! cheap enough to do once, and far too dear to do ninety times per repaint.

use eframe::egui::{self, RichText};
use sdroxide_types::{Command, Mode, SatLockConfig, SatUplink, Vfo};

use crate::app::SdroxideApp;
use crate::theme;
use crate::theme::ThemedScroll;

/// Everything the window remembers between frames.
/// Which page of the SATELLITE window is showing.
///
/// QO-100 lives here rather than in a window of its own because it *is* a
/// satellite — a geostationary one with a transponder, which is the only kind
/// this program can work through without Doppler. Its calibration simply
/// arrived first, before there was a satellite window to put it in.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum SatTab {
    #[default]
    Satellites,
    Qo100,
}

#[derive(Default)]
pub(in crate::app) struct SatWinState {
    pub search: String,
    /// Satellite whose detail/link pane is open, by catalogue number.
    pub selected: Option<u64>,
    /// Which of the selected satellite's published links is chosen.
    pub link_idx: usize,
    /// The lock we last asked the engine for — what the DOPPLER/ROTATOR
    /// toggles re-send modified. `None` when the running lock came from
    /// another client, in which case the toggles are not shown.
    pub sent: Option<SatLockConfig>,
    list: Option<SatList>,
    /// The sampled arcs behind the two pass diagrams — the locked bird's and
    /// the selected one's. Two slots because both are on screen at once when
    /// the operator is eyeing up the next pass during the current one, and a
    /// single slot would rebuild itself twice a frame.
    lock_curve: Option<PassCurve>,
    pick_curve: Option<PassCurve>,
}

/// The parsed satellite list, rebuilt when the station's satellite config
/// changes or ages out (a TLE refresh rewrites the caches underneath it).
struct SatList {
    cfg: std::sync::Arc<sdroxide_types::SatConfig>,
    built_unix: i64,
    entries: Vec<SatEntry>,
}

struct SatEntry {
    norad_id: u64,
    name: String,
    /// The element lines, handed to the engine with the lock so it needs no
    /// lookup of its own. `None` for a curated name whose elements this
    /// client does not hold — the engine still resolves those from its side.
    tle: Option<(String, String)>,
    sat: Option<sdroxide_solar::Satellite>,
    /// Next-pass memo: when it was computed and what it said.
    pass: Option<(i64, PassMemo)>,
}

enum PassMemo {
    Pass(sdroxide_types::SatPass),
    Always,
    Never,
}

/// How long a pass memo (and the list itself) is trusted before recomputing.
const MEMO_FRESH_S: i64 = 600;

impl SatEntry {
    fn matches(&self, q: &str) -> bool {
        q.is_empty()
            || self.name.to_ascii_lowercase().contains(&q.to_ascii_lowercase())
            || self.norad_id.to_string().contains(q)
    }
}

fn build_list(cfg: &std::sync::Arc<sdroxide_types::SatConfig>, now: i64) -> SatList {
    let mut entries: Vec<SatEntry> = Vec::new();
    let add_text = |text: &str, entries: &mut Vec<SatEntry>| {
        for t in sdroxide_types::parse_tle_block(text) {
            let Some(id) = t.norad_id() else { continue };
            if entries.iter().any(|e| e.norad_id == id) {
                continue;
            }
            let Some(sat) = sdroxide_solar::satellites::parse_pasted_tles(&format!(
                "{}\n{}\n{}",
                t.name, t.line1, t.line2
            ))
            .pop() else {
                continue;
            };
            entries.push(SatEntry {
                norad_id: id,
                name: sat.name.clone(),
                tle: Some((t.line1.clone(), t.line2.clone())),
                sat: Some(sat),
                pass: None,
            });
        }
    };
    add_text(&cfg.tle_text(), &mut entries);
    // The subscription caches live on disk beside the engine — the browser
    // client has neither, and leans on the engine to resolve elements instead.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cache = sdroxide_solar::cache::Cache::open();
        for sub in cfg.live_subs() {
            if let Some(text) = sdroxide_solar::tlesub::cached_text(&cache, sub) {
                add_text(&text, &mut entries);
            }
        }
    }
    // The curated names always appear, elements or not: a row that says "no
    // elements" tells the operator what to fix, an absent row says nothing.
    for (id, name) in sdroxide_solar::satellites::POPULAR {
        if !entries.iter().any(|e| e.norad_id == *id) {
            entries.push(SatEntry {
                norad_id: *id,
                name: (*name).to_string(),
                tle: None,
                sat: None,
                pass: None,
            });
        }
    }
    SatList { cfg: std::sync::Arc::clone(cfg), built_unix: now, entries }
}

/// How many points the elevation arc is sampled at. Far too many sgp4 runs to
/// do per repaint, which is the whole reason a [`PassCurve`] is kept.
const CURVE_SAMPLES: usize = 121;
/// The lead-in and run-out the diagram shows either side of the pass: a minute
/// of each, so the arc is seen crossing the horizon rather than starting on it.
const PASS_MARGIN_S: i64 = 60;
/// Height of the diagram under the lock pane, and of the shorter one the
/// picker draws for whatever satellite is selected.
const DIAGRAM_H: f32 = 92.0;
const DIAGRAM_H_SMALL: f32 = 76.0;

/// One pass sampled into an elevation arc: the pass diagram's data, held
/// across frames.
struct PassCurve {
    norad_id: u64,
    /// Which pass this is, identified by its *set* time. A search that starts
    /// mid-pass reports the search start as the rise — so `rise_unix` creeps
    /// forward with the clock every time the engine recomputes and cannot key
    /// a memo. The set is a real horizon crossing and stays put.
    key_set_unix: i64,
    /// The window drawn: the true AOS less [`PASS_MARGIN_S`], to LOS plus it.
    t0: i64,
    t1: i64,
    rise_unix: i64,
    set_unix: i64,
    /// Elevation and azimuth in degrees at [`CURVE_SAMPLES`] evenly spaced
    /// instants from `t0` to `t1`.
    pts: Vec<(f32, f32)>,
}

impl PassCurve {
    fn build(
        norad_id: u64,
        sat: &sdroxide_solar::Satellite,
        lat: f64,
        lon: f64,
        p: &sdroxide_types::SatPass,
    ) -> Option<PassCurve> {
        let el_at = |t: i64| sat.at(t as f64).map(|s| s.az_el_from(lat, lon).1);
        let rise = true_rise(&el_at, p.rise_unix);
        let (t0, t1) = (rise - PASS_MARGIN_S, p.set_unix + PASS_MARGIN_S);
        if t1 - t0 < 2 {
            return None;
        }
        let span = (t1 - t0) as f64;
        let mut pts = Vec::with_capacity(CURVE_SAMPLES);
        for i in 0..CURVE_SAMPLES {
            let t = t0 as f64 + span * i as f64 / (CURVE_SAMPLES - 1) as f64;
            let (az, el) = sat.at(t)?.az_el_from(lat, lon);
            pts.push((el as f32, az as f32));
        }
        Some(PassCurve {
            norad_id,
            key_set_unix: p.set_unix,
            t0,
            t1,
            rise_unix: rise,
            set_unix: p.set_unix,
            pts,
        })
    }

    /// Where an instant falls across the window, 0 at `t0` and 1 at `t1`.
    fn frac(&self, unix: f64) -> f64 {
        (unix - self.t0 as f64) / (self.t1 - self.t0).max(1) as f64
    }

    /// Elevation and azimuth at an instant inside the window, interpolated
    /// between samples so the marker sits *on* the drawn arc rather than near
    /// it. `None` outside the window — a pass still hours away has no present
    /// position on it.
    fn sample(&self, unix: f64) -> Option<(f32, f32)> {
        let f = self.frac(unix);
        if !(0.0..=1.0).contains(&f) || self.pts.len() < 2 {
            return None;
        }
        let x = f * (self.pts.len() - 1) as f64;
        let i = (x.floor() as usize).min(self.pts.len() - 2);
        let k = (x - i as f64) as f32;
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        // Azimuth wraps: interpolate the short way round, or a pass crossing
        // north swings the compass letter through south on the way.
        let mut daz = b.1 - a.1;
        if daz > 180.0 {
            daz -= 360.0;
        } else if daz < -180.0 {
            daz += 360.0;
        }
        Some((a.0 + (b.0 - a.0) * k, (a.1 + daz * k).rem_euclid(360.0)))
    }

    /// The highest sample: its index, elevation and azimuth. Taken from the
    /// samples rather than the pass summary so the label and the drawn apex
    /// cannot disagree.
    fn apex(&self) -> (usize, f32, f32) {
        let mut best = (0usize, f32::MIN, 0.0);
        for (i, &(el, az)) in self.pts.iter().enumerate() {
            if el > best.1 {
                best = (i, el, az);
            }
        }
        best
    }
}

/// The horizon crossing that really began the pass.
///
/// A search that starts mid-pass reports its own start time as the rise — the
/// satellite was already up when it looked — so a diagram drawn from that
/// would begin part-way up the arc and, worse, shift leftwards every time the
/// engine recomputed. Walk back to where the elevation last crossed zero and
/// bisect it to the second. Returns `reported` untouched in the normal case,
/// where it *is* the crossing, for the cost of one propagation.
fn true_rise(el_at: &impl Fn(i64) -> Option<f64>, reported: i64) -> i64 {
    if el_at(reported).unwrap_or(-90.0) <= 0.1 {
        return reported;
    }
    /// Coarse step back, then bisect. A pass never turns over inside this.
    const STEP: i64 = 15;
    /// An hour covers any low-Earth pass several times; a bird that stays up
    /// longer than that is drawn from wherever the search picked it up.
    const LIMIT: i64 = 3600;
    let mut up = reported;
    while reported - up < LIMIT {
        let below = up - STEP;
        if el_at(below).unwrap_or(-90.0) <= 0.0 {
            let (mut lo, mut hi) = (below, up);
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if el_at(mid).unwrap_or(-90.0) > 0.0 { hi = mid } else { lo = mid }
            }
            return hi;
        }
        up = below;
    }
    up
}

/// Bring `slot` up to date for this satellite and pass and hand back the arc
/// to draw. `None` when the element set will not propagate that far.
fn curve_for<'a>(
    slot: &'a mut Option<PassCurve>,
    norad_id: u64,
    sat: &sdroxide_solar::Satellite,
    lat: f64,
    lon: f64,
    p: &sdroxide_types::SatPass,
) -> Option<&'a PassCurve> {
    let fresh =
        slot.as_ref().is_some_and(|c| c.norad_id == norad_id && c.key_set_unix == p.set_unix);
    if !fresh {
        *slot = PassCurve::build(norad_id, sat, lat, lon, p);
    }
    slot.as_ref()
}

/// `12:03` — the clock on the diagram's axis, UTC like every other time here.
fn hhmm(unix: i64) -> String {
    let (_, _, _, h, mi, _) = sdroxide_types::utc_ymd_hms(unix);
    format!("{h:02}:{mi:02}")
}

/// `13:08 UTC (14:08 local)` — a pass time in both zones. Pass times are UTC,
/// and read as local they are wrong by the operator's offset (#635), so the
/// local clock goes beside it wherever the two differ.
fn utc_and_local(unix: i64) -> String {
    let offset = crate::time::local_offset_at(unix);
    if offset == 0 {
        format!("{} UTC", hhmm(unix))
    } else {
        format!("{} UTC ({} local)", hhmm(unix), hhmm(unix + offset))
    }
}

/// Paint the pass profile: elevation up the side, the clock and the compass
/// along the bottom, and a marker down the arc at the satellite's present
/// position.
///
/// A painted panel rather than a plot widget, because it has to read as part
/// of this window's chrome: night sky over a lit ground band, the arc glowing
/// out of the horizon and back into it, and a hot marker at *now*.
fn pass_diagram(ui: &mut egui::Ui, curve: &PassCurve, now: i64, height: f32) {
    use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, epaint::Mesh};
    use egui::{pos2, vec2};

    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if !ui.is_rect_visible(rect) || curve.pts.len() < 2 {
        return;
    }
    let p = ui.painter_at(rect);
    let font = FontId::proportional(8.5);
    let dim = theme::CYAN_DIM();
    let green = theme::GREEN();
    let ghost = |c: Color32, a: u8| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a);

    // Three bands: the caption strip, the sky, and the ground the axis labels
    // are set into. The ground is deeper than one line of text because the
    // minute of run-in either side dips the arc `DIP` below the horizon, and
    // that dip must not land on the lettering.
    const HEAD_H: f32 = 13.0;
    const GROUND_H: f32 = 17.0;
    const DIP: f32 = 5.0;
    let plot = Rect::from_min_max(
        pos2(rect.left() + 1.0, rect.top() + HEAD_H),
        pos2(rect.right() - 1.0, rect.bottom() - 1.0),
    );
    let horizon_y = plot.bottom() - GROUND_H;
    let sky_top = plot.top() + 1.0;
    if plot.width() < 60.0 || horizon_y - sky_top < 16.0 {
        return; // nothing legible would fit
    }

    // An axis scaled to the pass. A 12° grazer flattened onto a 90° axis says
    // nothing at all, so the top of the scale is the next step above the peak
    // — and labelled, so the grazer is never mistaken for an overhead pass.
    let (apex_i, apex_el, apex_az) = curve.apex();
    let top_el = match apex_el {
        e if e <= 12.0 => 15.0f32,
        e if e <= 25.0 => 30.0,
        e if e <= 40.0 => 45.0,
        e if e <= 55.0 => 60.0,
        _ => 90.0,
    };
    let ey = |el: f32| {
        if el >= 0.0 {
            horizon_y - (el / top_el).clamp(0.0, 1.0) * (horizon_y - sky_top)
        } else {
            // The sub-horizon band has its own compressed scale — it exists to
            // show the crossing, not to be measured.
            horizon_y + (-el / (top_el / 9.0)).clamp(0.0, 1.0) * DIP
        }
    };
    let ex = |unix: f64| plot.left() + plot.width() * curve.frac(unix).clamp(0.0, 1.0) as f32;

    // --- ground and sky ---------------------------------------------------
    p.rect_filled(rect, 0.0, theme::INPUT_BG());
    let mut sky = Mesh::default();
    sky.colored_vertex(pos2(plot.left(), sky_top), Color32::TRANSPARENT);
    sky.colored_vertex(pos2(plot.right(), sky_top), Color32::TRANSPARENT);
    sky.colored_vertex(pos2(plot.right(), horizon_y), ghost(theme::CYAN(), 22));
    sky.colored_vertex(pos2(plot.left(), horizon_y), ghost(theme::CYAN(), 22));
    sky.add_triangle(0, 1, 2);
    sky.add_triangle(0, 2, 3);
    p.add(Shape::mesh(sky));
    p.rect_filled(
        Rect::from_min_max(pos2(plot.left(), horizon_y), pos2(plot.right(), plot.bottom())),
        0.0,
        theme::FILL(),
    );

    // --- grid -------------------------------------------------------------
    let grid = ghost(theme::LINE_LIT(), 150);
    for frac in [1.0f32, 2.0 / 3.0, 1.0 / 3.0] {
        let (y, el) = (ey(top_el * frac), top_el * frac);
        p.extend(Shape::dashed_line(
            &[pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, grid),
            3.0,
            4.0,
        ));
        p.text(
            pos2(plot.left() + 4.0, y + 1.0),
            Align2::LEFT_TOP,
            format!("{el:.0}°"),
            font.clone(),
            dim,
        );
    }
    // Clock divisions, picked so the pass gets a handful of them whether it is
    // eight minutes long or eight hours.
    let span = (curve.t1 - curve.t0) as f64;
    let step = [60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0, 10800.0]
        .into_iter()
        .find(|s| span / s <= 8.0)
        .unwrap_or(21600.0);
    let mut tick = (curve.t0 as f64 / step).ceil() * step;
    while tick <= curve.t1 as f64 {
        let x = ex(tick);
        p.extend(Shape::dashed_line(
            &[pos2(x, sky_top), pos2(x, horizon_y)],
            Stroke::new(1.0, ghost(theme::LINE_LIT(), 90)),
            2.0,
            5.0,
        ));
        tick += step;
    }

    // --- the arc ----------------------------------------------------------
    let xs: Vec<f32> = (0..curve.pts.len())
        .map(|i| plot.left() + plot.width() * i as f32 / (curve.pts.len() - 1) as f32)
        .collect();
    let line: Vec<Pos2> = curve.pts.iter().zip(&xs).map(|(&(el, _), &x)| pos2(x, ey(el))).collect();
    // Fill the sky under the arc, fading out towards the horizon.
    let mut under = Mesh::default();
    for i in 0..line.len() - 1 {
        let (a, b) = (line[i], line[i + 1]);
        let base = under.vertices.len() as u32;
        under.colored_vertex(pos2(a.x, a.y.min(horizon_y)), ghost(green, 64));
        under.colored_vertex(pos2(b.x, b.y.min(horizon_y)), ghost(green, 64));
        under.colored_vertex(pos2(b.x, horizon_y), ghost(green, 6));
        under.colored_vertex(pos2(a.x, horizon_y), ghost(green, 6));
        under.add_triangle(base, base + 1, base + 2);
        under.add_triangle(base, base + 2, base + 3);
    }
    p.add(Shape::mesh(under));
    p.add(Shape::line(line.clone(), Stroke::new(4.0, ghost(green, 34))));
    p.add(Shape::line(line, Stroke::new(1.6, green)));

    // The horizon itself, over the arc: the one line on the diagram that is a
    // hard boundary rather than a reading.
    p.line_segment(
        [pos2(plot.left(), horizon_y), pos2(plot.right(), horizon_y)],
        Stroke::new(1.2, theme::CYAN()),
    );

    // --- the apex ---------------------------------------------------------
    let apex_p = pos2(xs[apex_i], ey(apex_el));
    p.circle_filled(apex_p, 2.0, ghost(green, 200));
    let apex_label =
        format!("{apex_el:.0}° {}", crate::language_plugin::compass(apex_az as f64));
    let (align, dy) = if apex_p.y - sky_top < 12.0 {
        (Align2::CENTER_TOP, 4.0)
    } else {
        (Align2::CENTER_BOTTOM, -4.0)
    };
    p.text(pos2(apex_p.x, apex_p.y + dy), align, apex_label, font.clone(), theme::TEXT_STRONG());

    // --- horizon crossings, on the axis where they happen ------------------
    let compass = |unix: i64| {
        curve
            .sample(unix as f64)
            .map(|(_, az)| crate::language_plugin::compass(az as f64))
            .unwrap_or_default()
    };
    for t in [curve.rise_unix, curve.set_unix] {
        let x = ex(t as f64);
        p.line_segment(
            [pos2(x, horizon_y - 3.0), pos2(x, horizon_y + 3.0)],
            Stroke::new(1.2, theme::CYAN()),
        );
    }
    let aos = crate::language_plugin::format("window.satellite.aos", "AOS {} {}", &[hhmm(curve.rise_unix), compass(curve.rise_unix)]);
    let los = crate::language_plugin::format("window.satellite.los", "LOS {} {}", &[hhmm(curve.set_unix), compass(curve.set_unix)]);
    let baseline = plot.bottom() - 1.0;
    let aos_r =
        p.text(pos2(plot.left() + 5.0, baseline), Align2::LEFT_BOTTOM, &aos, font.clone(), dim);
    if plot.right() - 5.0 - crate::chrome::text_width(ui, &los, font.clone()) > aos_r.right() + 6.0
    {
        p.text(pos2(plot.right() - 5.0, baseline), Align2::RIGHT_BOTTOM, &los, font.clone(), dim);
    }

    // --- now ---------------------------------------------------------------
    if let Some((el, _)) = curve.sample(now as f64) {
        let pink = theme::PINK();
        let x = ex(now as f64);
        p.line_segment(
            [pos2(x, plot.top()), pos2(x, plot.bottom())],
            Stroke::new(1.0, ghost(pink, 110)),
        );
        p.line_segment([pos2(x, sky_top), pos2(x, horizon_y)], Stroke::new(1.4, pink));
        p.add(Shape::convex_polygon(
            vec![pos2(x - 3.5, plot.top()), pos2(x + 3.5, plot.top()), pos2(x, plot.top() + 5.0)],
            pink,
            Stroke::NONE,
        ));
        let dot = pos2(x, ey(el));
        p.circle_filled(dot, 4.5, ghost(pink, 70));
        p.circle_filled(dot, 2.2, pink);
        // The marker is the only thing on here that moves; keep it moving even
        // when nothing else asks for a frame.
        crate::repaint::after(ui.ctx(), std::time::Duration::from_secs(1));
    }

    // --- caption and frame -------------------------------------------------
    p.text(
        pos2(rect.left() + 7.0, rect.top() + 2.0),
        Align2::LEFT_TOP,
        crate::language_plugin::text("window.satellite.custom.text_511_efb77a", "PASS PROFILE"),
        font.clone(),
        dim,
    );
    p.text(
        pos2(rect.right() - 13.0, rect.top() + 2.0),
        Align2::RIGHT_TOP,
        { let __lp_arg_0 = &(crate::language_plugin::solar_age(curve.set_unix - curve.rise_unix)); crate::language_plugin::format("window.satellite.custom.text_519_a01bd7", "{} above the horizon", &[format!("{}", __lp_arg_0)]) },
        font,
        dim,
    );
    crate::chrome::paint_cut_border(&p, rect, theme::LINE_LIT(), theme::PANEL());
}

/// The mode a link's published emission maps onto. Satellite SSB convention
/// is USB on the downlink, whatever the band. An SSTV downlink is FM on V/UHF,
/// and tuning it as plain NFM would leave the picture undecoded (#622).
fn mode_for_link(l: &sdroxide_types::SatLink) -> Mode {
    let m = l.mode.to_ascii_uppercase();
    let sstv = m.contains("SSTV") || l.label.to_ascii_uppercase().contains("SSTV");
    if sstv && m.contains("FM") {
        Mode::SstvFm
    } else if m.contains("FM") || m.contains("APT") {
        Mode::Nfm
    } else if m.contains("SSB") || m.contains("BPSK") || m.contains("GMSK") || m.contains("AX.25") {
        Mode::Usb
    } else if m.contains("CW") {
        Mode::Cw
    } else {
        Mode::Usb
    }
}

/// The downlink dial a link starts on: the current dial if it already sits
/// inside the passband (re-locking must not yank a tuned-in operator to the
/// centre), else the passband centre.
fn lock_downlink_hz(l: &sdroxide_types::SatLink, dial_hz: f64) -> f64 {
    match l.downlink {
        Some(pb) => {
            let (lo, hi) = (pb.lo_mhz * 1e6, pb.hi_mhz * 1e6);
            if (lo..=hi).contains(&dial_hz) { dial_hz } else { pb.centre_mhz() * 1e6 }
        }
        None => dial_hz,
    }
}

fn fmt_hz_signed(hz: f64) -> String {
    if hz.abs() >= 1000.0 { format!("{:+.2} kHz", hz / 1000.0) } else { format!("{hz:+.0} Hz") }
}

fn fmt_mhz(hz: f64) -> String {
    sdroxide_types::fmt_sat_mhz(hz / 1e6)
}

/// "in 12 min" / "in 3 h 05 min" — the countdown side of `timefmt::age`.
fn in_words(s: i64) -> String {
    let s = s.max(0);
    if s < 60 {
        crate::language_plugin::format("window.satellite.custom.text_568_7884ae", "in {s} s", &[format!("{s}")])
    } else if s < 3600 {
        { let __lp_arg_0 = &(s / 60); crate::language_plugin::format("window.satellite.custom.text_570_1fd99d", "in {} min", &[format!("{}", __lp_arg_0)]) }
    } else {
        { let __lp_arg_0 = &(s / 3600); let __lp_arg_1 = &((s % 3600) / 60); crate::language_plugin::format("window.satellite.custom.text_572_9f467a", "in {} h {:02} min", &[format!("{}", __lp_arg_0), format!("{:02}", __lp_arg_1)]) }
    }
}

impl SdroxideApp {
    pub(in crate::app) fn sat_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Command>) {
        if !self.show_sat {
            return;
        }
        let mut win = std::mem::take(&mut self.sat_win);
        // The QO-100 page keeps its own waterfall rows and texture between
        // frames, exactly as the picker keeps its element list. Taken here
        // rather than inside the tab arm because `qo100_body` needs `&mut self`
        // and the state lives on `self`.
        let mut qo100 = std::mem::take(&mut self.qo100_win);
        let mut open = self.show_sat;
        let resp = egui::Window::new(crate::language_plugin::text("window.satellite.text_588_4c6d1a", "SATELLITE")).id(egui::Id::new("SATELLITE"))
            .id(crate::layout::salted_id(ctx, "SATELLITE"))
            .open(&mut open)
            .frame(crate::chrome::window_frame())
            .resizable(true)
            .default_width(crate::layout::window_w(ctx, 480.0))
            .default_height(crate::layout::window_h(ctx, 520.0))
            .show(ctx, |ui| {
                crate::chrome::window_body_bg(ui);
                self.sat_body(ui, &mut win, &mut qo100, cmds)
            });
        if let Some(r) = &resp {
            crate::chrome::paint_window_border(ctx, &r.response);
        }
        self.show_sat = open;
        self.sat_win = win;
        self.qo100_win = qo100;
    }

    fn sat_body(
        &mut self,
        ui: &mut egui::Ui,
        win: &mut SatWinState,
        qo100: &mut crate::app::qo100::Qo100WinState,
        cmds: &mut Vec<Command>,
    ) {
        // A dot on the tab whose work is running, so the *other* page's state
        // is not invisible while this one is open. The same argument the
        // System chip's green accent makes one level up — here as a mark rather
        // than a colour, because a tab's colour already means "selected".
        let live = |on: bool, label: &str| {
            if on { format!("{label} •") } else { label.to_string() }
        };
        let locked = self.sat_track.is_some();
        let hunting = self.state.qo100.enabled;
        crate::chrome::tab_bar(ui, |ui, bar| {
            for (tab, label) in [
                (SatTab::Satellites, crate::language_plugin::satellite_tab_label(locked)),
                (SatTab::Qo100, live(hunting, "QO-100")),
            ] {
                if bar.tab(ui, self.sat_tab == tab, label).clicked() {
                    self.sat_tab = tab;
                }
            }
        });
        // No separator: the strip's own baseline is the line between the tabs
        // and the page they open.
        ui.add_space(8.0);

        match self.sat_tab {
            SatTab::Satellites => self.sat_page(ui, win, cmds),
            // Its own scroll area: the beacon page is a 130 pt waterfall plus a
            // grid plus the telemetry frame, which is taller than this window
            // opens at, and the satellite page below sizes itself.
            SatTab::Qo100 => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show_themed(ui, |ui| self.qo100_body(ui, qo100, cmds));
            }
        }
    }

    /// The satellite page: the live lock, then the picker.
    fn sat_page(&mut self, ui: &mut egui::Ui, win: &mut SatWinState, cmds: &mut Vec<Command>) {
        // (Re)built here rather than down in the picker: the lock pane draws
        // its pass diagram from the same element sets, and it runs first.
        let now = crate::time::now_unix();
        let stale = win.list.as_ref().is_none_or(|l| {
            !std::sync::Arc::ptr_eq(&l.cfg, &self.sat_cfg) || now - l.built_unix > MEMO_FRESH_S
        });
        if stale {
            win.list = Some(build_list(&self.sat_cfg, now));
        }
        if let Some(track) = self.sat_track.clone() {
            self.sat_locked_pane(ui, win, cmds, &track);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
        }
        self.sat_picker(ui, win, cmds);
    }

    /// The live pane: what the engine's lock is doing right now.
    fn sat_locked_pane(
        &mut self,
        ui: &mut egui::Ui,
        win: &mut SatWinState,
        cmds: &mut Vec<Command>,
        t: &sdroxide_types::SatTrackStatus,
    ) {
        use crate::language_plugin::compass;
        let dim = |s: &str| RichText::new(s).size(9.5).color(theme::CYAN_DIM());

        ui.horizontal(|ui| {
            ui.label(
                RichText::new({ let __lp_arg_0 = &(t.name); crate::language_plugin::format("window.satellite.text_683_920111", "● LOCKED — {}", &[format!("{}", __lp_arg_0)]) })
                    .size(13.0)
                    .strong()
                    .color(if t.visible { theme::GREEN() } else { theme::YELLOW() }),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::chrome::chip(ui, false, crate::language_plugin::text("window.satellite.text_689_09911d", "UNLOCK"))
                    .on_hover_text(crate::language_plugin::text("window.satellite.text_690_04aab6", "Release the lock; the dial stays where it is"))
                    .clicked()
                {
                    cmds.push(Command::SetSatLock(None));
                    win.sent = None;
                }
            });
        });
        ui.add_space(4.0);

        egui::Grid::new("sat-lock-grid").num_columns(4).spacing([16.0, 3.0]).show(ui, |ui| {
            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_701_4037d7", "AZIMUTH")));
            ui.label(
                RichText::new(format!("{:.1}°  {}", t.az_deg, compass(t.az_deg)))
                    .size(12.5)
                    .strong(),
            );
            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_707_1f4186", "ELEVATION")));
            ui.label(
                RichText::new(format!("{:+.1}°", t.el_deg))
                    .size(12.5)
                    .strong()
                    .color(if t.el_deg > 0.0 { theme::GREEN() } else { theme::TEXT() }),
            );
            ui.end_row();

            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_716_69da96", "RANGE")));
            ui.label(RichText::new(format!("{:.0} km", t.range_km)).size(11.5));
            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_718_fced8f", "RATE")));
            let dir = if t.range_rate_km_s < -0.01 {
                crate::language_plugin::text("window.sat.motion.text_720_1a28e6", "approaching")
            } else if t.range_rate_km_s > 0.01 {
                crate::language_plugin::text("window.sat.motion.text_722_50bf81", "receding")
            } else {
                crate::language_plugin::text("window.sat.motion.text_724_e060ee", "abeam")
            };
            ui.label(RichText::new(format!("{:+.2} km/s {dir}", t.range_rate_km_s)).size(11.5));
            ui.end_row();

            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_729_022120", "DOWNLINK")));
            ui.label(
                RichText::new(format!("{} MHz", fmt_mhz(t.downlink_hz)))
                    .size(11.5)
                    .color(theme::GREEN()),
            );
            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_735_c7db17", "DOPPLER RX")));
            ui.label(RichText::new(fmt_hz_signed(t.doppler_rx_hz)).size(11.5));
            ui.end_row();

            if let Some(up) = t.uplink_hz {
                ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_740_9a569e", "UPLINK")));
                ui.label(
                    RichText::new(format!("{} MHz", fmt_mhz(up))).size(11.5).color(theme::YELLOW()),
                );
                ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_744_b76e68", "DOPPLER TX")));
                ui.label(RichText::new(fmt_hz_signed(t.doppler_tx_hz)).size(11.5));
                ui.end_row();
            }
        });
        ui.add_space(4.0);

        // Where the pass stands. `visible` already tells the near half of the
        // story; the memoized prediction tells the rest.
        let now = crate::time::now_unix();
        match (&t.next_pass, t.visible) {
            (_, true) => {
                if let Some(p) = &t.next_pass {
                    if (p.rise_unix..=p.set_unix).contains(&now) {
                        ui.label(dim(&{ let __lp_arg_0 = &(utc_and_local(p.set_unix)); let __lp_arg_1 = &(p.max_el); crate::language_plugin::format("window.satellite.custom.text_759_c0e934", "Pass until {} · max {:.0}°", &[format!("{}", __lp_arg_0), format!("{:.0}", __lp_arg_1)]) }));
                    }
                }
            }
            (Some(p), false) => {
                ui.label(dim(&{ let __lp_arg_0 = &(sdroxide_solar::timefmt::ymd(p.rise_unix)); let __lp_arg_1 = &(utc_and_local(p.rise_unix)); let __lp_arg_2 = &(in_words(p.rise_unix - now)); let __lp_arg_3 = &(p.rise_az); let __lp_arg_4 = &(compass(p.rise_az)); let __lp_arg_5 = &(p.max_el); crate::language_plugin::format("window.satellite.custom.text_771_672b13", "Next pass {} {} ({}) · rises {:.0}° {} · max {:.0}°", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1), format!("{}", __lp_arg_2), format!("{:.0}", __lp_arg_3), format!("{}", __lp_arg_4), format!("{:.0}", __lp_arg_5)]) }));
            }
            (None, false) => {
                ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_780_6dd841", "No pass inside the next 48 hours from your QTH.")));
            }
        }

        // The shape of that pass, drawn from the elements *this* client holds
        // — a curated bird whose element set only the engine has keeps the
        // numbers above and goes without the diagram.
        let qth = win
            .sent
            .as_ref()
            .and_then(|c| c.observer)
            .or_else(|| sdroxide_types::grid_to_latlon(&self.my_grid()));
        if let (Some(pass), Some((lat, lon))) = (&t.next_pass, qth) {
            let sat = win
                .list
                .as_ref()
                .and_then(|l| l.entries.iter().find(|e| e.norad_id == t.norad_id))
                .and_then(|e| e.sat.as_ref());
            if let Some(sat) = sat {
                if let Some(c) = curve_for(&mut win.lock_curve, t.norad_id, sat, lat, lon, pass) {
                    ui.add_space(5.0);
                    pass_diagram(ui, c, now, DIAGRAM_H);
                    ui.add_space(2.0);
                }
            }
        }

        // The warnings, in the order an operator has to act on them.
        if t.stale_elements {
            ui.label(
                RichText::new(crate::language_plugin::text("window.satellite.text_810_4a4d42", "⚠ Elements stale — corrections suspended until a TLE refresh"))
                    .size(10.5)
                    .color(theme::YELLOW()),
            );
        } else if !t.corrections_active && win.sent.as_ref().is_none_or(|c| c.doppler) {
            ui.label(
                RichText::new(crate::language_plugin::text("window.satellite.text_816_a800fb", "Doppler correction needs an IQ front end — a CAT rig tracks only"))
                    .size(10.5)
                    .color(theme::CYAN_DIM()),
            );
        }
        if let Some(s) = &self.rigctld_status {
            if s.running && s.clients > 0 {
                ui.label(
                    RichText::new({ let __lp_arg_0 = &(s.clients); let __lp_arg_1 = &(crate::language_plugin::plural_suffix("window.sat.text_825_0e36d9", "⚠ {} rigctld client{} connected — satellite software steering the dial there would correct Doppler twice", s.clients == 1)); crate::language_plugin::format("window.sat.text_825_0e36d9", "⚠ {} rigctld client{} connected — satellite software steering the dial \
                         there would correct Doppler twice", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) })
                    .size(10.5)
                    .color(theme::YELLOW()),
                );
            }
        }
        if let Some((connected, az, el, err)) = &self.rotator_status {
            if win.sent.as_ref().is_some_and(|c| c.rotator) {
                let line = if *connected {
                    RichText::new(crate::language_plugin::format("window.satellite.text_838_64db79", "Rotator: antenna at {az:.0}° az {el:.0}° el", &[format!("{az:.0}"), format!("{el:.0}")]))
                        .color(theme::CYAN_DIM())
                } else {
                    match err {
                        Some(e) => RichText::new(crate::language_plugin::format("window.satellite.text_842_466379", "Rotator: {e}", &[format!("{e}")])).color(theme::YELLOW()),
                        None => RichText::new(crate::language_plugin::text("window.satellite.text_843_d1cd81", "Rotator: not connected")).color(theme::CYAN_DIM()),
                    }
                };
                ui.label(line.size(10.5));
            }
        }

        // The two switches ride the lock config, so flipping one is just the
        // same command again with one field changed.
        if let Some(sent) = win.sent.as_mut() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if crate::chrome::chip(ui, sent.doppler, crate::language_plugin::text("window.satellite.text_855_791bca", "DOPPLER"))
                    .on_hover_text(crate::language_plugin::text("window.satellite.text_856_0ecdcd", "Apply the computed correction to RX and TX"))
                    .clicked()
                {
                    sent.doppler = !sent.doppler;
                    cmds.push(Command::SetSatLock(Some(Box::new(sent.clone()))));
                }
                if crate::chrome::chip(ui, sent.rotator, crate::language_plugin::text("window.satellite.text_862_354c60", "ROTATOR"))
                    .on_hover_text(
                        crate::language_plugin::text("window.satellite.text_864_3c85db", "Steer the antenna through the rotctld client (Settings ▸ Servers)"),
                    )
                    .clicked()
                {
                    sent.rotator = !sent.rotator;
                    cmds.push(Command::SetSatLock(Some(Box::new(sent.clone()))));
                }
            });
        }
    }

    /// The picker: every satellite the station tracks, searchable, with the
    /// published links of the chosen one and the TUNE / LOCK actions.
    fn sat_picker(&mut self, ui: &mut egui::Ui, win: &mut SatWinState, cmds: &mut Vec<Command>) {
        let dim = |s: &str| RichText::new(s).size(9.5).color(theme::CYAN_DIM());
        let now = crate::time::now_unix();
        let qth = sdroxide_types::grid_to_latlon(&self.my_grid());

        let list = win.list.as_mut().expect("built in sat_body");

        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").size(11.0));
            crate::chrome::field(
                ui,
                egui::TextEdit::singleline(&mut win.search)
                    .desired_width(160.0)
                    .hint_text(crate::language_plugin::text("window.satellite.text_890_aa0d40", "name or NORAD")),
            );
            if !win.search.is_empty() && ui.small_button("×").clicked() {
                win.search.clear();
            }
            if qth.is_none() {
                ui.label(
                    RichText::new(crate::language_plugin::text("window.satellite.text_897_970cd9", "set your grid for passes (Settings ▸ General)"))
                        .size(9.5)
                        .color(theme::YELLOW()),
                );
            }
        });
        ui.add_space(4.0);

        // Fill at most one pass memo per frame — see the module note.
        if let Some((lat, lon)) = qth {
            if let Some(e) = list
                .entries
                .iter_mut()
                .filter(|e| e.matches(&win.search) && e.sat.is_some())
                .find(|e| e.pass.as_ref().is_none_or(|(at, _)| now - at > MEMO_FRESH_S))
            {
                let sat = e.sat.as_ref().expect("filtered above");
                let memo = match sat.next_passes(lat, lon, now as f64, 48.0 * 3600.0, 1) {
                    sdroxide_solar::PassSearch::Passes(p) => match p.first() {
                        Some(p) => PassMemo::Pass(sdroxide_types::SatPass {
                            rise_unix: p.rise_unix,
                            set_unix: p.set_unix,
                            rise_az: p.rise_az,
                            set_az: p.set_az,
                            max_el: p.max_el,
                            max_el_unix: p.max_el_unix,
                        }),
                        None => PassMemo::Never,
                    },
                    sdroxide_solar::PassSearch::AlwaysVisible { .. } => PassMemo::Always,
                    sdroxide_solar::PassSearch::NeverVisible => PassMemo::Never,
                };
                e.pass = Some((now, memo));
                ui.ctx().request_repaint(); // keep filling the memos
            }
        }

        // Displayed rows: what matches, sorted into operating order — up now
        // first (highest first), then whoever rises next.
        let mut rows: Vec<(usize, Option<f64>, i64)> = list
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.matches(&win.search))
            .map(|(i, e)| {
                let el = match (&e.sat, qth) {
                    (Some(s), Some((lat, lon))) => {
                        s.at(now as f64).map(|st| st.elevation_from(lat, lon))
                    }
                    _ => None,
                };
                let rise = match &e.pass {
                    Some((_, PassMemo::Pass(p))) => p.rise_unix,
                    Some((_, PassMemo::Always)) => 0,
                    _ => i64::MAX,
                };
                (i, el, rise)
            })
            .collect();
        rows.sort_by(|a, b| {
            let vis_a = a.1.unwrap_or(-90.0) > 0.0;
            let vis_b = b.1.unwrap_or(-90.0) > 0.0;
            vis_b
                .cmp(&vis_a)
                .then_with(|| {
                    if vis_a && vis_b {
                        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.2.cmp(&b.2)
                    }
                })
                .then_with(|| list.entries[a.0].name.cmp(&list.entries[b.0].name))
        });

        // Full width, not content width: the grid hugs its text otherwise and
        // leaves half the window empty with the scrollbar in the middle.
        egui::ScrollArea::vertical().max_height(190.0).auto_shrink([false, true]).show(ui, |ui| {
            let col_w = ((ui.available_width() - 2.0 * 14.0) / 3.0).max(80.0);
            egui::Grid::new("sat-list")
                .num_columns(3)
                .min_col_width(col_w)
                .spacing([14.0, 2.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_981_4c6d1a", "SATELLITE")));
                    ui.label(dim("NORAD"));
                    ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_983_2f9acb", "PASS")));
                    ui.end_row();
                    for (i, el, _) in &rows {
                        let e = &list.entries[*i];
                        let selected = win.selected == Some(e.norad_id);
                        let name = RichText::new(&e.name).size(11.0);
                        // A selected row sits on the cyan selection fill, and
                        // egui keeps the label's own color there — so it has
                        // to be the dark on-cyan ink explicitly, the same ink
                        // the accent chips use. The accents are for
                        // unselected rows alone.
                        let name = match el {
                            _ if selected => name.color(theme::INK_ON_CYAN()).strong(),
                            Some(el) if *el > 0.0 => name.color(theme::GREEN()).strong(),
                            _ => name,
                        };
                        if ui.selectable_label(selected, name).clicked() {
                            win.selected = if selected { None } else { Some(e.norad_id) };
                            win.link_idx = 0;
                        }
                        ui.label(RichText::new(e.norad_id.to_string()).size(10.0));
                        let pass = match (el, &e.pass, &e.sat) {
                            (Some(el), _, _) if *el > 0.0 => {
                                RichText::new(crate::language_plugin::format("window.satellite.text_1006_0573f9", "● up now, {el:.0}°", &[format!("{el:.0}")])).color(theme::GREEN())
                            }
                            (_, Some((_, PassMemo::Pass(p))), _) => {
                                RichText::new(in_words(p.rise_unix - now))
                            }
                            (_, Some((_, PassMemo::Always)), _) => {
                                RichText::new(crate::language_plugin::text("window.satellite.text_1012_60b6d5", "always visible")).color(theme::GREEN())
                            }
                            (_, Some((_, PassMemo::Never)), _) => {
                                RichText::new(crate::language_plugin::text("window.satellite.text_1015_11f4fb", "not from here")).color(theme::CYAN_DIM())
                            }
                            (_, None, None) => RichText::new(crate::language_plugin::text("window.satellite.text_1017_6b51c7", "no elements")).color(theme::YELLOW()),
                            (_, None, Some(_)) => RichText::new("…"),
                        };
                        ui.label(pass.size(10.0));
                        ui.end_row();
                    }
                });
        });

        let Some(id) = win.selected else {
            ui.add_space(4.0);
            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_1028_093f87", "Pick a satellite to see its links and lock on.")));
            return;
        };
        let Some(entry_idx) = list.entries.iter().position(|e| e.norad_id == id) else {
            win.selected = None;
            return;
        };

        // The operator's own frequency table wins over the built-in one, the
        // same rule the 3D view's pass window applies.
        let custom_freqs = self.sat_cfg.freqs_for(id).is_some();
        let freqs = match self.sat_cfg.freqs_for(id) {
            Some(f) => Some(f.clone()),
            None => sdroxide_solar::satfreq::builtin_for(id).cloned(),
        };
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // The selected bird's next pass, unless the lock pane above is already
        // showing this one's. Ahead of the links on purpose: *when* it can be
        // worked comes before *how*, and a satellite with no frequencies on
        // file returns below this point without ever reaching the links.
        let locked_here = self.sat_track.as_ref().is_some_and(|t| t.norad_id == id);
        if !locked_here {
            let e = &list.entries[entry_idx];
            if let (Some(sat), Some((_, PassMemo::Pass(pass))), Some((lat, lon))) =
                (e.sat.as_ref(), e.pass.as_ref(), qth)
            {
                if let Some(c) = curve_for(&mut win.pick_curve, id, sat, lat, lon, pass) {
                    pass_diagram(ui, c, now, DIAGRAM_H_SMALL);
                    ui.add_space(6.0);
                }
            }
        }

        let links: Vec<sdroxide_types::SatLink> =
            freqs.as_ref().map(|f| f.usable_links().cloned().collect()).unwrap_or_default();
        if links.is_empty() {
            ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_1066_fbb098", "No frequencies on file for this one — add them in Settings ▸ TLE.")));
            return;
        }
        win.link_idx = win.link_idx.min(links.len() - 1);

        // Spread across the window like the list above, for the same reason.
        let link_col_w = ((ui.available_width() - 3.0 * 14.0) / 4.0).max(70.0);
        egui::Grid::new("sat-links")
            .num_columns(4)
            .min_col_width(link_col_w)
            .spacing([14.0, 2.0])
            .show(ui, |ui| {
                ui.label(dim(&crate::language_plugin::text("common.link", "LINK")));
                ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_1079_d5abb6", "DOWNLINK (MHz)")));
                ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_1080_6026f4", "UPLINK (MHz)")));
                ui.label(dim(&crate::language_plugin::text("window.satellite.custom.text_1081_ac6c84", "MODE")));
                ui.end_row();
                for (i, l) in links.iter().enumerate() {
                    let sel = i == win.link_idx;
                    // The selected row sits on the cyan selection fill and
                    // keeps the label's own color, so that color has to be
                    // the dark on-cyan ink or it vanishes into the fill.
                    let mut label = RichText::new(crate::language_plugin::satellite_link_label(l, custom_freqs)).size(10.5);
                    if sel {
                        label = label.color(theme::INK_ON_CYAN()).strong();
                    }
                    let resp = ui.selectable_label(sel, label);
                    if !l.note.is_empty() {
                        resp.clone().on_hover_text(crate::language_plugin::satellite_link_note(l, custom_freqs));
                    }
                    if resp.clicked() {
                        win.link_idx = i;
                    }
                    ui.label(
                        RichText::new(l.downlink.map_or_else(|| "—".into(), |b| b.to_string()))
                            .size(10.5)
                            .color(theme::GREEN()),
                    );
                    ui.label(
                        RichText::new(l.uplink.map_or_else(|| "—".into(), |b| b.to_string()))
                            .size(10.5)
                            .color(theme::YELLOW()),
                    );
                    let mode =
                        crate::language_plugin::satellite_link_mode_with_inversion(l, custom_freqs);
                    ui.label(RichText::new(mode).size(10.5));
                    ui.end_row();
                }
            });
        let link = &links[win.link_idx];
        if !link.note.is_empty() {
            ui.label(dim(&format!("{} — {}", crate::language_plugin::satellite_link_label(link, custom_freqs), crate::language_plugin::satellite_link_note(link, custom_freqs))));
        }
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if crate::chrome::chip(ui, false, crate::language_plugin::text("window.satellite.text_1122_eaca96", "TUNE"))
                .on_hover_text(crate::language_plugin::text("window.satellite.text_1123_4fbeb2", "Set the dial and mode to this link — no lock, no correction"))
                .clicked()
            {
                if link.downlink.is_some() {
                    cmds.push(Command::SetVfo {
                        vfo: Vfo::A,
                        hz: lock_downlink_hz(link, self.state.active_freq_hz()),
                    });
                }
                cmds.push(Command::SetMode {
                    rx: sdroxide_types::RxId::Main,
                    mode: mode_for_link(link),
                });
            }
            if crate::chrome::chip_accent(
                ui,
                locked_here,
                RichText::new(if locked_here { crate::language_plugin::text("window.satellite.text_1140_d78a51", " LOCKED ") } else { crate::language_plugin::text("window.satellite.text_1140_82a522", " LOCK ON ") }).strong(),
                theme::GREEN(),
                theme::INK_ON_CYAN(),
            )
            .on_hover_text(
                crate::language_plugin::text("window.satellite.text_1145_ef7da1", "Track this satellite: Doppler correction on RX and TX, transponder-mapped \
                 uplink, and the 3D view follows it"),
            )
            .clicked()
                && !locked_here
            {
                let e = &list.entries[entry_idx];
                let cfg = SatLockConfig {
                    norad_id: id,
                    name: e.name.clone(),
                    tle: e.tle.clone(),
                    observer: qth,
                    downlink_hz: lock_downlink_hz(link, self.state.active_freq_hz()),
                    uplink: SatUplink::from_link(link),
                    doppler: true,
                    rotator: win.sent.as_ref().map(|c| c.rotator).unwrap_or(true),
                };
                cmds.push(Command::SetMode {
                    rx: sdroxide_types::RxId::Main,
                    mode: mode_for_link(link),
                });
                cmds.push(Command::SetSatLock(Some(Box::new(cfg.clone()))));
                win.sent = Some(cfg);
            }
        });
    }

    /// Lock on from outside the window — the 3D view's LOCK button lands
    /// here. Picks the satellite's first two-way link (or first link at all),
    /// and opens the window on it either way so the operator can refine.
    /// Native-only caller (the 3D viewport), hence the wasm allowance.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(in crate::app) fn sat_lock_request(&mut self, norad_id: u64, cmds: &mut Vec<Command>) {
        self.show_sat = true;
        self.sat_win.selected = Some(norad_id);
        self.sat_win.link_idx = 0;
        let freqs = match self.sat_cfg.freqs_for(norad_id) {
            Some(f) => Some(f.clone()),
            None => sdroxide_solar::satfreq::builtin_for(norad_id).cloned(),
        };
        let links: Vec<sdroxide_types::SatLink> =
            freqs.as_ref().map(|f| f.usable_links().cloned().collect()).unwrap_or_default();
        // Prefer the first link that can be worked both ways; a beacon-only
        // satellite still locks for RX.
        let Some((idx, link)) = links
            .iter()
            .enumerate()
            .find(|(_, l)| l.uplink.is_some() && l.downlink.is_some())
            .or_else(|| links.first().map(|l| (0, l)))
        else {
            // No frequencies at all: open the window; the picker explains.
            return;
        };
        self.sat_win.link_idx = idx;
        let entry = self
            .sat_win
            .list
            .as_ref()
            .and_then(|l| l.entries.iter().find(|e| e.norad_id == norad_id));
        let name = entry
            .map(|e| e.name.clone())
            .or_else(|| freqs.as_ref().map(|f| f.name.clone()).filter(|n| !n.is_empty()))
            .unwrap_or_else(|| crate::language_plugin::format("window.satellite.norad_fallback", "NORAD {norad_id}", &[norad_id.to_string()]));
        let tle = entry.and_then(|e| e.tle.clone());
        let cfg = SatLockConfig {
            norad_id,
            name,
            tle,
            observer: sdroxide_types::grid_to_latlon(&self.my_grid()),
            downlink_hz: lock_downlink_hz(link, self.state.active_freq_hz()),
            uplink: SatUplink::from_link(link),
            doppler: true,
            rotator: true,
        };
        cmds.push(Command::SetMode { rx: sdroxide_types::RxId::Main, mode: mode_for_link(link) });
        cmds.push(Command::SetSatLock(Some(Box::new(cfg.clone()))));
        self.sat_win.sent = Some(cfg);
    }
}

#[cfg(test)]
mod language_countdown_tests {
    use super::*;

    #[test]
    fn sstv_satellite_links_select_sstv_fm_mode() {
        use sdroxide_types::{Passband, SatLink};
        let sstv = SatLink::down("SSTV 70 cm", "SSTV FM / PD120", Passband::at(437.550));
        assert_eq!(mode_for_link(&sstv), Mode::SstvFm);
        let own = SatLink::down("SSTV", "FM", Passband::at(145.800));
        assert_eq!(mode_for_link(&own), Mode::SstvFm);
        let voice = SatLink::down("FM voice", "FM", Passband::at(145.800));
        assert_eq!(mode_for_link(&voice), Mode::Nfm);
    }

    #[test]
    fn countdown_boundaries_round_as_before_and_switch_back_to_exact_english() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (seconds,english,chinese) in [
                (-1,"in 0 s","0 秒后"),(0,"in 0 s","0 秒后"),(59,"in 59 s","59 秒后"),
                (60,"in 1 min","1 分钟后"),(3599,"in 59 min","59 分钟后"),
                (3600,"in 1 h 00 min","1 小时 00 分钟后"),(3900,"in 1 h 05 min","1 小时 05 分钟后"),
                (86400,"in 24 h 00 min","24 小时 00 分钟后")
            ] {assert_eq!(in_words(seconds),if enabled {chinese} else {english});}
        }
    }
    #[test]
    fn pass_caption_uses_localized_duration_and_unchanged_clock_values() {
        for enabled in [true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let seconds=3900;
            assert_eq!(crate::language_plugin::solar_age(seconds),if enabled {"65 分钟"} else {"65 min"});
            let (key,entry)=serde_json::from_str::<serde_json::Value>(include_str!("../../../../plugins/zh-CN/translations.zh-CN.json")).unwrap()["entries"]
                .as_object().unwrap().iter().find(|(_,entry)|entry["source"]=="Next pass {} {} ({}) · rises {:.0}° {} · max {:.0}°").map(|(k,e)|(k.clone(),e.clone())).unwrap();
            let text=crate::language_plugin::format(&key,entry["source"].as_str().unwrap(),&["2026-10-04".into(),"12:34 UTC (20:34 local)".into(),in_words(seconds),"123".into(),"SE".into(),"67".into()]);
            for raw in ["2026-10-04","12:34 UTC (20:34 local)","123°","SE","67°"] {assert!(text.contains(raw));}
            assert_eq!(text.contains("下次过境"),enabled);
        }
    }

    #[test]
    fn horizon_crossing_labels_and_norad_fallback_switch_and_preserve_values() {
        for enabled in [true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(crate::language_plugin::format("window.satellite.aos", "AOS {} {}", &["12:34".into(), "SE".into()]),
                if enabled { "升起 12:34 SE" } else { "AOS 12:34 SE" });
            assert_eq!(crate::language_plugin::format("window.satellite.los", "LOS {} {}", &["23:45".into(), "NW".into()]),
                if enabled { "落下 23:45 NW" } else { "LOS 23:45 NW" });
            assert_eq!(crate::language_plugin::format("window.satellite.norad_fallback", "NORAD {norad_id}", &["12345".into()]),
                if enabled { "NORAD 编号 12345" } else { "NORAD 12345" });
        }
    }
}
