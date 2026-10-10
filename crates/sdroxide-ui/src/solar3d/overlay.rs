//! The egui chrome drawn inside the solar-system window: a wrapping top bar of
//! captioned modules over the 3D scene, styled with the app's own
//! [`crate::chrome`] widgets so the second window reads as part of sdroxide.

use eframe::egui::{self, RichText};
use sdroxide_solar::{SdoChannel, SolarData, Source, timefmt};

use super::state::{Focus, SolarUi};
use crate::chrome;
use crate::theme;
use crate::view::solar_layer as layer;

/// Layer chips, in bar order.
///
/// The star field and the heliographic graticule are not among them: they are
/// the backdrop and the coordinate frame everything else is read against, so
/// they are always drawn rather than being switches.
const LAYERS: [(u32, &str, &str); 12] = [
    (
        layer::PROPAGATION,
        "PROP",
        "Where signals are actually getting through, band by band. Every reception this \
         station takes part in — WSPR, FT8/FT4, JS8, and the logbook — is placed at the \
         midpoint of its path, which is the patch of ionosphere that bent it, rather than \
         at the far station. So this is a map of the sky, not of where radio amateurs \
         live.\n\nIt can only show paths this station has been one end of. Oceans light \
         up because the midpoints of long paths fall there; Antarctica stays dark because \
         nobody is transmitting from it. Off by default — it paints the whole planet.",
    ),
    (layer::ORBITS, "ORBITS", "Orbital paths"),
    (
        layer::CLOUDS,
        "CLOUDS",
        "Cloud cover from NOAA's global geostationary mosaic, drawn as a depth of air \
         rather than a picture stuck on the surface: infrared says how cold each cloud \
         top is, and that is how high it stands. Thunderstorms flash inside the towers \
         they build.",
    ),
    (
        layer::PLANETS,
        "PLANETS",
        "The other seven planets, eighteen of their moons, and Saturn's and Uranus's rings. \
         Planet positions come from JPL's Keplerian element set; the moons are circular orbits \
         fitted to JPL Horizons.",
    ),
    (layer::CME, "CME", "Coronal mass ejection trajectory cones"),
    (
        layer::SUN_OBS,
        "SUN OBS",
        "Solar observations on the Sun's disk: sunspot active regions, and where the \
         flares came from",
    ),
    (layer::LABELS, "LABELS", "Body and region labels"),
    (
        layer::SMALL_LABELS,
        "SMALL BODIES",
        "Names on the asteroids and comets. They are drawn either way — this is only \
         whether thirty-five designations are drawn with them. Whatever it is set to, \
         the body the camera is on and anything the find box has matched stay named.",
    ),
    (layer::QSO, "QSO", "Decoded FT8/FT4 stations and the path to the station being worked"),
    (layer::SATS, "SATS", "Amateur-radio satellites, their orbits and elevation from your QTH"),
    (
        layer::AURORA,
        "AURORA",
        "The auroral oval from NOAA's OVATION model, drawn as emission shells at their real \
         altitudes, with the equatorward edge of the 10 % contour marked on the surface",
    ),
    (
        layer::AWARDS,
        "AWARDS",
        "Award coverage: every DXCC entity painted by what your logbook is still missing. \
         Orange burns where you have never worked the entity, amber where you have but it is \
         unconfirmed, dim green where a QSL has come back. Follows the band filter in the \
         AWARDS window.",
    ),
];

/// The inset every floating panel keeps from the edge of the viewport.
const MARGIN: f32 = 12.0;

/// Where a readout box goes: parked against the right-hand edge of the scene,
/// or allocated inline in a menu popup.
///
/// A phone has no width to spare down the side for boxes that are read once and
/// then ignored — the aurora and propagation panels together take a third of a
/// 400 pt screen — so on one they move behind a menu chip instead. Same boxes
/// and the same code drawing them: only where they are put changes.
#[derive(Clone, Copy)]
enum Place {
    Corner { scene: egui::Rect, top: f32 },
    Inline,
}

impl Place {
    /// Reserve `size` and return the rect to paint into.
    ///
    /// `None` in the corner when the scene cannot hold the box — the rule every
    /// readout in this window keeps, because a panel clipped by the viewport
    /// edge is worse than no panel. A popup always has room: it is sized to
    /// what it is given.
    fn reserve(self, ui: &mut egui::Ui, size: egui::Vec2) -> Option<egui::Rect> {
        match self {
            Place::Corner { scene, top } => {
                let r = egui::Rect::from_min_size(
                    egui::pos2(scene.right() - size.x - MARGIN, top),
                    size,
                );
                scene.contains_rect(r).then_some(r)
            }
            Place::Inline => Some(ui.allocate_space(size).1),
        }
    }
}

/// How often the view redraws when nothing is happening to it.
///
/// This is a clock, not a document: the Earth turns, the terminator moves, the
/// Moon goes round and the QSO arcs follow the contacts. It has to keep running
/// whether or not the pointer is over it and whether or not the window has
/// focus — a frozen orrery in the corner of the screen is worse than none.
const IDLE_FRAME: std::time::Duration = std::time::Duration::from_millis(33);

pub fn ui(ui: &mut egui::Ui, st: &mut SolarUi) {
    crate::repaint::after(ui.ctx(), IDLE_FRAME);
    // Take a snapshot of the feed's data for this frame. Cloning the `Arc`
    // first means the guard's lifetime is not tied to `st`, which is borrowed
    // mutably by every module below.
    let handle = st.data.clone();
    let guard = handle.as_ref().map(|d| d.lock().unwrap_or_else(|e| e.into_inner()));
    let data = guard.as_deref();

    scene(ui, st, data);
}

// Fade slots in [`SolarUi::menu_since`], in bar order.
const M_VIEW: usize = 0;
const M_LAYERS: usize = 1;
const M_SUN: usize = 2;
const M_WEATHER: usize = 3;
const M_SCALE: usize = 4;
const M_TIME: usize = 5;
const M_ACTIVITY: usize = 6;
const M_PROP: usize = 7;

/// One menu chip's popup, wearing the same fade the main window's menus have: a
/// popup opened and then forgotten takes itself away rather than sitting over
/// the scene until something else is clicked. Off on a touched layout, where
/// there is no hover to hold it open with — that rule lives in
/// [`chrome::popup_fade_alpha`].
///
/// The open time is copied out and back rather than borrowed in place, because
/// the popup body needs `st` mutably too and `&mut st.menu_since[slot]` would
/// hold the whole of it.
fn menu(
    ui: &mut egui::Ui,
    st: &mut SolarUi,
    slot: usize,
    btn: egui::Response,
    add: impl FnOnce(&mut egui::Ui, &mut SolarUi),
) {
    let mut since = st.menu_since[slot];
    chrome::fading_menu_popup(ui, &btn, &mut since, |ui| add(ui, st));
    st.menu_since[slot] = since;
}

/// The menu chips along the top of the scene: one per control box, each opening
/// what that box used to hold.
///
/// Chips over the picture rather than a bar above it, because this window *is*
/// the picture. Six captioned boxes wrapped to three rows on anything narrower
/// than a desktop, which took a third of a phone screen away from the thing
/// being looked at, and all of it to show controls that are set once and then
/// left alone. Everything in them is one tap away instead.
///
/// Laid out from the clock's right edge to the right margin, wrapping as often
/// as it must. Returns the row it came out as, so the space-weather panels can
/// stack underneath rather than be drawn over.
fn menu_bar(
    ui: &mut egui::Ui,
    st: &mut SolarUi,
    data: Option<&SolarData>,
    rect: egui::Rect,
    clock_rect: Option<egui::Rect>,
    phone: bool,
) -> Option<egui::Rect> {
    // Freshness is a fact about the wall clock, not about the scrubbed scene: a
    // view wound forward a month has not made the imagery a month stale. The
    // space weather is read against the scene's own clock, like the panels it
    // stands in for.
    let wall = super::wall_clock_unix();
    let now = wall as i64;
    let sim_now = (wall + st.sim_offset_s) as i64;
    let left = clock_rect.map_or(rect.left() + MARGIN, |r| r.right() + 8.0);
    let width = rect.right() - MARGIN - left;
    // Narrower than a single chip and there is nowhere to put the bar at all.
    if width < 64.0 {
        return None;
    }

    let area = egui::Area::new(egui::Id::new("solar-menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(left, rect.top() + MARGIN))
        .constrain_to(rect)
        .show(ui.ctx(), |ui| {
            ui.set_max_width(width);
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.horizontal_wrapped(|ui| {
                let btn = chrome::chip(ui, st.view.auto, crate::language_plugin::text("solar.overlay.text_209_28baeb", "VIEW"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_210_f5a1f7", "Which body the camera orbits, and the animated tour"));
                menu(ui, st, M_VIEW, btn, view_controls);

                let btn = chrome::chip(ui, false, crate::language_plugin::text("solar.overlay.text_213_394772", "LAYERS"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_214_417ae4", "What is drawn: orbits, clouds, CMEs, labels, QSOs, aurora"));
                menu(ui, st, M_LAYERS, btn, |ui, st| {
                    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_216_0bcd66", "Layers"));
                    layer_controls(ui, st);
                });

                let btn = chrome::chip(ui, false, crate::language_plugin::text("solar.overlay.text_220_d44fa9", "SUN"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_221_e53e11", "Which SDO channel wraps the Sun, and how old it is"));
                menu(ui, st, M_SUN, btn, |ui, st| {
                    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_223_db18f1", "Sun"));
                    sun_controls(ui, st, data, now);
                });

                // Only where the panels themselves cannot fit: on anything
                // wider they are already on screen, and a chip that opens a
                // copy of what is beside it is a chip that does nothing.
                if phone {
                    let btn = chrome::chip(ui, false, crate::language_plugin::text("solar.overlay.text_231_0a2441", "WEATHER")).on_hover_text(
                        crate::language_plugin::text("solar.overlay.text_232_0e1a02", "The aurora, the propagation numbers and which bands are open"),
                    );
                    menu(ui, st, M_WEATHER, btn, |ui, st| {
                        chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_235_47d32b", "Space weather"));
                        let aurora = aurora_panel(ui, st, data, Place::Inline, sim_now).is_some();
                        let prop = weather_panel(ui, st, data, Place::Inline, sim_now).is_some();
                        let bands = bands_panel(ui, st, Place::Inline).is_some();
                        if !aurora && !prop && !bands {
                            ui.label(
                                RichText::new(crate::language_plugin::text("solar.overlay.text_241_5f4e5b", "nothing measured yet"))
                                    .color(theme::CYAN_DIM())
                                    .size(10.5),
                            );
                        }
                    });
                }

                let btn = chrome::chip(ui, false, crate::language_plugin::text("solar.overlay.text_249_ea4bee", "SCALE"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_250_d536ff", "Body and Moon-orbit exaggeration"));
                menu(ui, st, M_SCALE, btn, |ui, st| {
                    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_252_f10298", "Scale"));
                    scale_controls(ui, st);
                });

                let btn = chrome::chip(ui, st.sim_offset_s != 0.0, crate::language_plugin::text("solar.overlay.text_256_588867", "TIME"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_257_f4c503", "Scrub the whole scene forward and back"));
                menu(ui, st, M_TIME, btn, |ui, st| {
                    chrome::menu_caption(ui, crate::language_plugin::text("common.time", "Time"));
                    time_controls(ui, st);
                });

                let btn = chrome::chip(ui, st.lapse_playing, crate::language_plugin::text("solar.overlay.text_263_2d0cb0", "ACTIVITY"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_264_2142cc", "Replay the last hour of decodes on the globe"));
                menu(ui, st, M_ACTIVITY, btn, |ui, st| {
                    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_266_38da15", "Activity"));
                    activity_controls(ui, st);
                });

                let on = st.layer(layer::PROPAGATION);
                let btn = chrome::chip(ui, on, crate::language_plugin::text("solar.overlay.text_271_8ccd1e", "PROP"))
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_272_00c783", "What the propagation heat map is showing"));
                menu(ui, st, M_PROP, btn, |ui, st| {
                    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_274_70723a", "Propagation"));
                    prop_controls(ui, st);
                });
            });
        });
    Some(area.response.rect)
}

/// The animated tour, and the camera target: the whole solar system as chips,
/// laid out the way the system is — the Sun and the Earth–Moon pair first, then
/// one row per planet with its own moons beside it.
fn view_controls(ui: &mut egui::Ui, st: &mut SolarUi) {
    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_286_dcc839", "View"));
    if chrome::chip_accent(ui, st.view.auto, crate::language_plugin::text("solar.overlay.text_287_5d3afe", "▶ AUTO"), theme::CYAN(), theme::INK_ON_CYAN())
        .on_hover_text(
            crate::language_plugin::text("solar.overlay.text_289_286f2d", "Fly a spline through a set of framed viewpoints. Any mouse input cancels it."),
        )
        .clicked()
    {
        st.view.auto = !st.view.auto;
        if st.view.auto {
            st.tour.request_resume();
        }
    }

    let mut chosen = None;
    for (caption, targets) in Focus::groups() {
        chrome::menu_caption(ui, crate::language_plugin::solar_group(caption));
        ui.horizontal_wrapped(|ui| {
            for f in targets {
                // Moons are dimmer, so a row reads as "this planet, and the
                // things that go round it".
                let text = if f.is_satellite() {
                    RichText::new(crate::language_plugin::solar_name(f.short())).size(11.5).color(theme::CYAN_DIM())
                } else {
                    RichText::new(crate::language_plugin::solar_name(f.short())).size(13.0)
                };
                if chrome::chip(ui, st.focus() == f, text).clicked() {
                    chosen = Some(f);
                }
            }
        });
    }
    ui.add_space(2.0);
    ui.label(
        RichText::new(
            crate::language_plugin::text("solar.overlay.text_320_020ffe", "Moons ride circular orbits fitted to JPL Horizons: within a degree or two \
             of where they really are, and up to six for Miranda, whose orbit plane \
             swings too fast for a circle to follow."),
        )
        .color(theme::LINE_LIT())
        .size(10.0),
    );
    if let Some(f) = chosen {
        st.set_focus(f);
    }
}

fn layer_controls(ui: &mut egui::Ui, st: &mut SolarUi) {
    ui.horizontal_wrapped(|ui| {
        for (bit, label, hint) in LAYERS {
            // The award layer has nothing to paint without a logbook to paint
            // it from — the browser tab has none — and a chip that provably
            // does nothing is worse than no chip.
            if bit == layer::AWARDS && st.awards.is_empty() {
                continue;
            }
            // `layer` is already "any of these bits", so a chip standing for a
            // pair lights when either half is on and clears both when clicked.
            let on = st.layer(bit);
            if chrome::chip(ui, on, crate::language_plugin::scope_text("solar.overlay.layers.", label)).on_hover_text(crate::language_plugin::scope_text("solar.overlay.layers.", hint)).clicked() {
                st.set_layers(bit, !on);
                // Propagation and awards both wash the entire planet, and two
                // washes at once is neither. Switching one on stands the other
                // down rather than stacking them.
                if !on && (bit == layer::PROPAGATION || bit == layer::AWARDS) {
                    let other =
                        if bit == layer::PROPAGATION { layer::AWARDS } else { layer::PROPAGATION };
                    st.set_layers(other, false);
                }
            }
        }
    });
}

/// Which SDO product wraps the Sun, plus the honest freshness readout.
fn sun_controls(ui: &mut egui::Ui, st: &mut SolarUi, data: Option<&SolarData>, now: i64) {
    ui.horizontal_wrapped(|ui| {
        let current = SdoChannel::from_u8(st.view.channel);
        for c in SdoChannel::ALL {
            if chrome::chip(ui, current == c, crate::language_plugin::scope_text("display.solar.channel.", c.label())).on_hover_text(crate::language_plugin::scope_text("display.solar.channel.", c.description())).clicked() {
                st.view.channel = c.to_u8();
            }
        }
        if chrome::chip(ui, false, "↻").on_hover_text(crate::language_plugin::text("solar.overlay.text_368_c61c12", "Fetch everything again now")).clicked() {
            st.refresh_requested = true;
        }
        if chrome::chip(ui, st.view.all_satellites, crate::language_plugin::text("solar.overlay.text_371_ce9fc9", "ALL SATS"))
            .on_hover_text(
                crate::language_plugin::text("solar.overlay.text_373_347545", "Show every satellite in the element set, not just the popular ones. \
                 Orbit rings stay on the curated few — ninety at once is unreadable."),
            )
            .clicked()
        {
            st.view.all_satellites = !st.view.all_satellites;
        }

        // Say what is actually being shown. Presenting hours-old cached data as
        // if it were current is the one thing this readout must not do.
        let (text, color) = match data {
            None => (crate::language_plugin::text("solar.overlay.dynamic.text_384_407944", "starting…").to_string(), theme::CYAN_DIM()),
            Some(d) => {
                let s = d.status(Source::Sun);
                match (s.age_secs(now), &s.last_error) {
                    (Some(age), None) => (crate::language_plugin::solar_age(age), theme::GREEN()),
                    (Some(age), Some(_)) => {
                        ({ let __lp_arg_0 = &(crate::language_plugin::solar_age(age)); crate::language_plugin::format("solar.overlay.dynamic.text_390_b79dde", "{} · offline", &[format!("{}", __lp_arg_0)]) }, theme::YELLOW())
                    }
                    (None, Some(_)) => (crate::language_plugin::text("solar.overlay.dynamic.text_392_8e2c7a", "offline").to_string(), theme::ALERT()),
                    (None, None) => ("…".to_string(), theme::CYAN_DIM()),
                }
            }
        };
        ui.label(RichText::new(text).color(color).size(10.5));
    });
}

/// Size exaggeration. Positions are always real; only radii (and optionally the
/// Moon's orbit) are scaled, or nothing at this distance would be visible.
fn scale_controls(ui: &mut egui::Ui, st: &mut SolarUi) {
    // Names ride *inside* the boxes rather than beside them: a wrapping row
    // breaks wherever it runs out of width, and a "moon orbit" left at the end
    // of one line with its value at the start of the next names nothing.
    ui.horizontal_wrapped(|ui| {
        // The Moon renders *inside* the Earth once the exaggerated radii exceed
        // the (unexaggerated) Earth–Moon distance, so cap body scale against it.
        let max_body = super::max_body_scale(st.view.moon_orbit_scale);
        ui.add(
            egui::DragValue::new(&mut st.view.body_scale)
                .speed(0.25)
                .range(1.0..=max_body as f64)
                .prefix(crate::language_plugin::text("solar.overlay.text_415_20d164", "body "))
                .suffix("×"),
        )
        .on_hover_text(crate::language_plugin::format("solar.overlay.text_419_55b0c8", "Earth/Moon radius exaggeration (max {max_body:.0}× at this moon-orbit scale)", &[format!("{max_body:.0}")]));
        ui.add(
            egui::DragValue::new(&mut st.view.moon_orbit_scale)
                .speed(0.1)
                .range(1.0..=30.0)
                .prefix(crate::language_plugin::text("solar.overlay.text_425_f524bb", "moon orbit "))
                .suffix("×"),
        )
        .on_hover_text(crate::language_plugin::text("solar.overlay.text_428_699fd5", "Stretch the Earth→Moon distance so the pair can be seen apart"));
        ui.add(
            egui::DragValue::new(&mut st.view.sun_scale)
                .speed(0.1)
                .range(1.0..=20.0)
                .prefix(crate::language_plugin::text("solar.overlay.text_433_7ee8bb", "sun "))
                .suffix("×"),
        )
        .on_hover_text(
            crate::language_plugin::text("solar.overlay.text_437_e5dd3d", "Sun radius exaggeration. Leave at 1× to keep the CME geometry readable — \
             a swollen Sun swallows the base of every cone."),
        );
        st.view.body_scale = st.view.body_scale.clamp(1.0, max_body);
    });
}

/// Scrub the whole scene forward and back in time.
///
/// The month steps are what make the small bodies mean anything. A comet's
/// apparition is weeks long and its orbit is years, so at ±24 h a click you can
/// watch a tail grow but never reach the next one; a month a click walks
/// through a whole apparition in a dozen presses and out to the following
/// perihelion in a few dozen more.
///
/// The Sun's imagery, the aurora and the weather do not travel with it: they
/// are measurements of now, and a scene scrubbed to 2061 shows today's Sun
/// behind a correctly placed Halley. That is already true of the ±24 h steps
/// and the clock says which way it has been moved.
fn time_controls(ui: &mut egui::Ui, st: &mut SolarUi) {
    // A calendar month is not a fixed number of seconds; the scene's clock is,
    // so this is the average one. Over a scrub of years the drift against the
    // calendar is days, which is nothing to an ephemeris and everything to a
    // "+1 mo" that sometimes moved 28 days and sometimes 31.
    const MONTH_S: f64 = 365.2425 / 12.0 * 86_400.0;
    ui.horizontal_wrapped(|ui| {
        if chrome::chip(ui, st.sim_offset_s == 0.0, crate::language_plugin::text("solar.overlay.text_463_97bfc0", "NOW")).clicked() {
            st.sim_offset_s = 0.0;
        }
        for (label, dt) in [
            (crate::language_plugin::text("solar.overlay.dynamic.text_467_121c53", "−1mo"), -MONTH_S),
            (crate::language_plugin::text("solar.overlay.dynamic.text_468_2910b1", "−24h"), -86400.0),
            (crate::language_plugin::text("solar.overlay.dynamic.text_469_b8fe56", "−1h"), -3600.0),
            (crate::language_plugin::text("solar.overlay.dynamic.text_470_4e9e1f", "+1h"), 3600.0),
            (crate::language_plugin::text("solar.overlay.dynamic.text_471_679385", "+24h"), 86400.0),
            (crate::language_plugin::text("solar.overlay.dynamic.text_472_4551a6", "+1mo"), MONTH_S),
        ] {
            if chrome::chip(ui, false, label).clicked() {
                st.sim_offset_s += dt;
            }
        }
    });
}

/// The FT8/FT4 activity time-lapse: where in the last hour the globe's arcs
/// are being replayed from, how long a trail they leave, and how fast the
/// replay runs.
/// What the propagation heat draws, and how long it remembers.
fn prop_controls(ui: &mut egui::Ui, st: &mut SolarUi) {
    // A popup sizes itself to its content, and with the layer switched off this
    // one's content is a single chip — which wrapped the explanation underneath
    // it into a column the width of the word SHOW. Ask for a readable width up
    // front so the box is the same size whether the layer is on or off, and so
    // the source chips below get a row rather than a stack. Clamped to the
    // viewport because this window opens on phones too; `popup_body` caps the
    // outer width at the same figure.
    let screen_w = ui.ctx().content_rect().width();
    ui.set_min_width((screen_w - 24.0).clamp(160.0, 330.0));

    let on = st.layer(layer::PROPAGATION);
    ui.horizontal_wrapped(|ui| {
        if chrome::chip_accent(ui, on, crate::language_plugin::text("solar.overlay.dynamic.text_498_45ea7f", "SHOW"), theme::CYAN(), theme::INK_ON_CYAN())
            .on_hover_text(crate::language_plugin::text("solar.overlay.text_499_6acd98", "Paint the globe by what is getting through"))
            .clicked()
        {
            st.view.layers ^= layer::PROPAGATION;
            // Two full-globe washes at once are unreadable, so picking one
            // stands the other down rather than stacking them.
            if st.view.layers & layer::PROPAGATION != 0 {
                st.view.layers &= !layer::AWARDS;
            }
        }
    });
    if !on {
        ui.label(
            RichText::new(
                crate::language_plugin::text("solar.overlay.text_513_8d03cb", "Paints the globe by what is getting through, on each band. Every reception \
                 is placed at the midpoint of its path — where the ionosphere did the work — \
                 rather than at the far station. Built from what this station hears, plus \
                 the world's skimmers when the Reverse Beacon Network is switched on."),
            )
            .size(10.5)
            .weak(),
        );
        return;
    }

    chrome::menu_caption(ui, crate::language_plugin::text("settings.ui.display", "Display"));
    ui.horizontal_wrapped(|ui| {
        let combined = st.view.prop_mode != 0;
        if chrome::chip(ui, combined, crate::language_plugin::text("solar.overlay.text_527_1eb5e7", "ALL BANDS"))
            .on_hover_text(
                crate::language_plugin::text("solar.overlay.text_529_72a34e", "Every band at once, one hue each, mixing where two overlap. The view for \
                 'what are conditions like' rather than 'is twenty open'."),
            )
            .clicked()
        {
            st.view.prop_mode = 1;
        }
        if chrome::chip(ui, !combined, crate::language_plugin::text("solar.overlay.text_536_e3ea18", "ONE BAND"))
            .on_hover_text(crate::language_plugin::text("solar.overlay.text_537_c3f4ed", "One band, cold to hot: blue, green, yellow, red."))
            .clicked()
        {
            st.view.prop_mode = 0;
        }
    });

    let live = st.prop.live_bands();
    if st.view.prop_mode == 0 {
        chrome::menu_caption(ui, crate::language_plugin::text("common.band", "Band"));
        ui.horizontal_wrapped(|ui| {
            if live.is_empty() {
                ui.label(RichText::new(crate::language_plugin::text("solar.overlay.text_549_63ee8f", "nothing heard yet")).size(10.5).weak());
            }
            for b in &live {
                let i = sdroxide_types::Band::ALL.iter().position(|x| x == b).unwrap_or(0) as u8;
                let sw = crate::colormap::band_color(*b);
                if chrome::chip(
                    ui,
                    st.view.prop_band == i,
                    RichText::new(b.label()).color(egui::Color32::from_rgb(sw[0], sw[1], sw[2])),
                )
                .clicked()
                {
                    st.view.prop_band = i;
                }
            }
        });
    } else {
        // The legend, and the only place the colours are named. Only bands with
        // something in them: a key full of dead bands is a key to nothing.
        chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_568_bf5f54", "Bands"));
        ui.horizontal_wrapped(|ui| {
            if live.is_empty() {
                ui.label(RichText::new(crate::language_plugin::text("solar.overlay.text_571_63ee8f", "nothing heard yet")).size(10.5).weak());
            }
            for b in &live {
                let i = sdroxide_types::Band::ALL.iter().position(|x| x == b).unwrap_or(0) as u8;
                let bit = 1u32 << i;
                let sw = crate::colormap::band_color(*b);
                if chrome::chip(
                    ui,
                    st.view.prop_bands & bit != 0,
                    RichText::new(b.label()).color(egui::Color32::from_rgb(sw[0], sw[1], sw[2])),
                )
                .clicked()
                {
                    st.view.prop_bands ^= bit;
                }
            }
        });
    }

    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_590_caf85b", "Sources"));
    ui.horizontal_wrapped(|ui| {
        let mut src = crate::prop_map::PropSources(st.view.prop_sources);
        for s in sdroxide_types::PropSource::ALL {
            if chrome::chip(ui, src.has(s), s.label())
                .on_hover_text(match s {
                    sdroxide_types::PropSource::Logged => {
                        crate::language_plugin::text("solar.overlay.text_597_3dac97", "Contacts in the logbook. A path that was open, with no signal report \
                         worth the name — an RST is not an SNR, so these count towards how busy \
                         a cell is and never towards its margin.")
                    }
                    sdroxide_types::PropSource::WsprHeardUs => {
                        crate::language_plugin::text("solar.overlay.text_602_3901a6", "Stations that reported hearing this one, downloaded from WSPRnet. The \
                         only feedback a transmitting beacon ever gets.")
                    }
                    sdroxide_types::PropSource::Rbn => {
                        crate::language_plugin::text("solar.overlay.text_606_f9a519", "Reverse Beacon Network skimmers: what everyone else is hearing, on \
                         every band, including the ones this radio is not on. Coarser than \
                         the rest — an RBN line carries no locators, so both ends are placed \
                         at their country's centre, which is close for a small country and \
                         badly out for a large one. Switch on under Settings → Spots.")
                    }
                    _ => {
                        crate::language_plugin::text("solar.overlay.text_613_6796c9", "Decodes this station made. Each is measured against its own mode's \
                         floor, so a WSPR report and an FT4 one are comparable.")
                    }
                })
                .clicked()
            {
                src.toggle(s);
                st.view.prop_sources = src.0;
            }
        }
    });

    chrome::menu_caption(ui, crate::language_plugin::text("solar.overlay.text_625_c3963a", "Memory"));
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::DragValue::new(&mut st.view.prop_halflife_min)
                .speed(1.0)
                .range(5.0..=240.0)
                .prefix(crate::language_plugin::text("solar.overlay.text_631_c33175", "half-life "))
                .suffix(crate::language_plugin::text("solar.overlay.dynamic.text_632_2e7573", " min")),
        )
        .on_hover_text(
            crate::language_plugin::text("solar.overlay.text_635_ca7006", "How long a reception takes to count for half as much. The ionosphere's own \
             memory is short — an opening two hours old should not be arguing with one \
             from two minutes ago."),
        );
    });
    // The absolute scale behind the colours. Without it the same colour means
    // different densities on different evenings, and the picture is lying by
    // omission about how much evidence is behind it.
    let peak =
        live.iter().filter_map(|b| st.prop.plane(*b)).map(|p| p.peak()).fold(0.0f32, f32::max);
    if peak > 0.0 {
        ui.label(
            RichText::new({ let __lp_arg_0 = &(live.len()); let __lp_arg_1 = &(crate::language_plugin::plural_suffix("solar.overlay.text_648_b81794", "brightest cell ≈ {peak:.0} paths · {} band{}", live.len() == 1)); crate::language_plugin::format("solar.overlay.text_648_b81794", "brightest cell ≈ {peak:.0} paths · {} band{}", &[format!("{peak:.0}"), format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) })
            .size(10.5)
            .weak(),
        );
    }
}

fn activity_controls(ui: &mut egui::Ui, st: &mut SolarUi) {
    ui.horizontal_wrapped(|ui| {
        let hour = crate::digi_map::HISTORY_S as f64;
        if chrome::chip(ui, st.lapse_live() && !st.lapse_playing, crate::language_plugin::text("solar.overlay.text_661_35e0d0", "LIVE"))
            .on_hover_text(crate::language_plugin::text("solar.overlay.text_662_b241f4", "Follow the band as it happens"))
            .clicked()
        {
            st.lapse_playing = false;
            st.set_lapse_back(0.0);
        }
        if chrome::chip_accent(
            ui,
            st.lapse_playing,
            if st.lapse_playing { crate::language_plugin::text("solar.overlay.text_671_c22e87", "⏸ REPLAY") } else { crate::language_plugin::text("solar.overlay.text_671_630126", "▶ REPLAY") },
            theme::CYAN(),
            theme::INK_ON_CYAN(),
        )
        .on_hover_text(crate::language_plugin::text("solar.overlay.text_675_8080d4", "Replay the last hour of decodes, over and over"))
        .clicked()
        {
            st.lapse_playing = !st.lapse_playing;
            // Starting from the present would replay nothing, so a replay
            // begun while live starts at the top of the hour.
            if st.lapse_playing && st.lapse_live() {
                st.set_lapse_back(hour);
            }
        }

        // The head, as minutes ago. Dragging it is also how you stop the
        // replay running away from where you wanted to look.
        let mut back_min = (st.lapse_back_s / 60.0) as f32;
        let resp = ui.add(
            egui::DragValue::new(&mut back_min)
                .speed(0.5)
                .range(0.0..=(hour / 60.0))
                .suffix(crate::language_plugin::text("solar.overlay.text_693_c5fad2", " min ago")),
        );
        if resp.on_hover_text(crate::language_plugin::text("solar.overlay.text_695_2ef74c", "Where in the last hour the globe is showing")).changed() {
            st.set_lapse_back(back_min as f64 * 60.0);
            st.lapse_playing = false;
        }

        // Named inside the box, for the reason the scale controls are.
        ui.add(
            egui::DragValue::new(&mut st.view.lapse_trail_min)
                .speed(0.25)
                .range(0.5..=(hour / 60.0))
                .prefix(crate::language_plugin::text("solar.overlay.text_705_59413e", "trail "))
                .suffix(crate::language_plugin::text("solar.overlay.dynamic.text_706_2e7573", " min")),
        )
        .on_hover_text(crate::language_plugin::text("solar.overlay.text_708_5fbc40", "How long a decode's arc stays on the globe behind the head"));

        ui.add(
            egui::DragValue::new(&mut st.view.lapse_speed)
                .speed(1.0)
                .range(1.0..=600.0)
                .prefix(crate::language_plugin::text("solar.overlay.text_714_8bbd47", "speed "))
                .suffix("×"),
        )
        .on_hover_text(crate::language_plugin::text("solar.overlay.text_717_3d41e0", "How much faster than real time the replay runs"));

        let hits = st.digi.history.len();
        let (text, color) = if !st.layer(layer::QSO) {
            (crate::language_plugin::text("solar.overlay.dynamic.text_721_b788c4", "QSO layer off").to_string(), theme::YELLOW())
        } else if hits == 0 {
            (crate::language_plugin::text("solar.overlay.dynamic.text_723_59b20c", "no decodes yet").to_string(), theme::CYAN_DIM())
        } else if st.lapse_live() {
            (crate::language_plugin::format("solar.overlay.dynamic.text_725_678daa", "{hits} in the hour", &[format!("{hits}")]), theme::GREEN())
        } else {
            ({ let __lp_arg_0 = &(st.lapse_back_s / 60.0); crate::language_plugin::format("solar.overlay.dynamic.text_727_416f91", "−{:.0} min", &[format!("{:.0}", __lp_arg_0)]) }, theme::CYAN())
        };
        ui.label(RichText::new(text).color(color).size(10.5));
    });
}

/// Step the activity replay head, at `speed` × real time, looping round the
/// hour. Looping and not stopping at the present on purpose: the point of a
/// replay is to watch the opening come and go more than once.
fn advance_lapse(ui: &egui::Ui, st: &mut SolarUi, dt: f32) {
    if !st.lapse_playing {
        return;
    }
    let step = dt.max(0.0) as f64 * st.view.lapse_speed.clamp(1.0, 600.0) as f64;
    let back = st.lapse_back_s - step;
    st.set_lapse_back(if back <= 0.0 { crate::digi_map::HISTORY_S as f64 } else { back });
    crate::repaint::animate(ui.ctx());
}

/// The 3D scene: mouse interaction, the wgpu paint callback, then the readouts
/// painted over it.
fn scene(ui: &mut egui::Ui, st: &mut SolarUi, data: Option<&SolarData>) {
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 4.0 || rect.height() < 4.0 {
        return;
    }
    let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    interact(ui, st, &resp);

    let ppp = ui.ctx().pixels_per_point();
    let px = [(rect.width() * ppp).round().max(1.0), (rect.height() * ppp).round().max(1.0)];
    let sim_now = super::wall_clock_unix() + st.sim_offset_s;
    let anim = ui.input(|i| i.time);
    // One elapsed-time step for everything animated off the wall clock, so the
    // tour and the activity replay cannot disagree about how long a frame was.
    let dt =
        if st.last_frame_time <= 0.0 { 1.0 / 60.0 } else { (anim - st.last_frame_time) as f32 };
    st.last_frame_time = anim;
    advance_tour(ui, st, data, sim_now, dt);
    advance_lapse(ui, st, dt);
    reframe(st, sim_now);
    let mut scene = super::scene::build(st, data, sim_now, px, anim as f32);
    // Labels and pick targets are egui's business, not the GPU's, so they come
    // out before the rest of the scene is handed to the paint callback.
    let labels = std::mem::take(&mut scene.labels);
    let picks = std::mem::take(&mut scene.picks);
    let qso_apex = scene.qso_apex;
    let view_proj = scene.globals.view_proj;

    let (sun_img, sun_gen, aurora, aurora_gen, clouds, clouds_gen) = match data {
        Some(d) => (
            d.sun.clone(),
            d.sun_gen,
            d.aurora.clone(),
            d.aurora_gen,
            d.clouds.clone(),
            d.clouds_gen,
        ),
        None => (None, 0, None, 0, None, 0),
    };
    // Resolve the propagation field to pixels, once per change rather than per
    // frame. Deliberately the same function the flat map calls: the two views
    // upload identical bytes, so there is no way for them to disagree about
    // what a cell means.
    let (prop, prop_gen) = if st.layer(crate::view::solar_layer::PROPAGATION) {
        let band = sdroxide_types::Band::ALL
            .get(st.view.prop_band as usize)
            .copied()
            .unwrap_or(sdroxide_types::Band::M20);
        let mode = if st.view.prop_mode == 0 {
            crate::prop_map::PropMode::PerBand
        } else {
            crate::prop_map::PropMode::AllBands
        };
        let key = (st.prop.generation, st.view.prop_mode, st.view.prop_band, st.view.prop_bands);
        if st.prop_rgba_key != Some(key) {
            let (rgba, _, _) =
                crate::prop_map::PropHeat::rgba(&st.prop, mode, band, st.view.prop_bands);
            st.prop_rgba = std::sync::Arc::new(rgba);
            st.prop_rgba_key = Some(key);
            st.prop_gen = st.prop_gen.wrapping_add(1);
        }
        (Some(std::sync::Arc::clone(&st.prop_rgba)), st.prop_gen)
    } else {
        (None, st.prop_gen)
    };
    ui.painter().add(crate::egui_wgpu::Callback::new_paint_callback(
        rect,
        super::gpu::SolarCallback {
            scene: std::sync::Arc::new(scene),
            px_size: [px[0] as u32, px[1] as u32],
            sun_img,
            sun_gen,
            aurora,
            aurora_gen,
            clouds,
            clouds_gen,
            prop,
            prop_gen,
        },
    ));

    let took_click = draw_labels(ui, st, rect, &view_proj, &labels, &resp);
    // Only if a label did not already take it: a name sits on top of its own
    // body, and clicking the text should not be ambiguous.
    pick_bodies(ui, st, rect, &view_proj, &picks, &resp, took_click);
    // After the labels, so the contact reads over the traffic around it rather
    // than through it — a callsign printed across the card is the one place on
    // this globe where decluttering by rank is not enough.
    qso_card(ui, st, rect, &view_proj, qso_apex, anim);
    // A phone puts the space weather behind the menu bar's WEATHER chip instead
    // of down the right-hand edge; see [`Place`].
    //
    // Measured from this window rather than read from `layout::tier`, which is
    // the whole app's: natively this view is a child viewport sharing the main
    // window's context, so the app-wide tier says how big the *radio* window is.
    // The override still counts — an operator who asked for the phone layout
    // meant everywhere — so it is either that or this window's own size.
    let phone = crate::layout::tier(ui.ctx()) == crate::layout::Tier::Phone
        || crate::layout::tier_for(rect.size(), sdroxide_types::LayoutMode::Auto)
            == crate::layout::Tier::Phone;
    let clock_rect = clock(ui, rect, sim_now, st.sim_offset_s != 0.0);
    // The menu row owns the top band outright, and everything else starts under
    // it: the alternative is measuring a panel that has not been drawn yet to
    // find out whether the chips would have reached it.
    let menu_rect = menu_bar(ui, st, data, rect, clock_rect, phone);
    find_box(ui, st, data, rect, clock_rect);
    let top = menu_rect.map_or(rect.top() + MARGIN, |r| r.bottom() + 8.0);
    if !phone {
        let aurora = aurora_panel(ui, st, data, Place::Corner { scene: rect, top }, sim_now as i64);
        let below = aurora.map_or(top, |r| r.bottom() + 8.0);
        let weather =
            weather_panel(ui, st, data, Place::Corner { scene: rect, top: below }, sim_now as i64);
        let below = weather.map_or(below, |r| r.bottom() + 8.0);
        let _ = bands_panel(ui, st, Place::Corner { scene: rect, top: below });
    }
    // The bottom-right stack, from the corner up: the date, then the award key
    // above whatever the date left.
    let date_rect = date_readout(ui, st, rect, sim_now);
    award_panel(ui, st, rect, date_rect.map_or(rect.bottom() - MARGIN, |r| r.top() - 8.0));
    clouds_note(ui, st, data, rect, sim_now as i64);
    impact_banner(ui, data, rect, sim_now as i64);
    pass_window(ui, st, data, sim_now);

    if st.qth.is_none() {
        ui.painter().text(
            rect.right_top() + egui::vec2(-12.0, 12.0),
            egui::Align2::RIGHT_TOP,
            crate::language_plugin::text("solar.overlay.dynamic.text_875_b49f98", "QTH not set — enter your grid square in Settings"),
            egui::FontId::proportional(12.5),
            theme::scope().warn,
        );
    }
}

/// Project a world point into the widget. `None` when it is behind the eye.
fn project(view_proj: &[[f32; 4]; 4], rect: egui::Rect, world: [f32; 3]) -> Option<egui::Pos2> {
    // Column-major, so column i scales input component i.
    let mut o = [0.0f32; 4];
    for (r, out) in o.iter_mut().enumerate() {
        *out = view_proj[0][r] * world[0]
            + view_proj[1][r] * world[1]
            + view_proj[2][r] * world[2]
            + view_proj[3][r];
    }
    if o[3] <= 0.0 {
        return None;
    }
    Some(egui::pos2(
        rect.left() + (o[0] / o[3] * 0.5 + 0.5) * rect.width(),
        rect.top() + (0.5 - o[1] / o[3] * 0.5) * rect.height(),
    ))
}

/// How much clear space a label keeps around itself before another may be
/// painted next to it, in pixels.
const LABEL_GAP: egui::Vec2 = egui::vec2(3.0, 1.5);

/// Project the scene's labels to screen space, paint them, and let one be
/// clicked. Returns whether the click was consumed.
///
/// The 3D pass has no text rendering, and adding one for a dozen short strings
/// would be far more machinery than projecting a dozen points with the same
/// matrix the vertex shaders use.
///
/// Crowding is settled here rather than in the scene, because it is a question
/// about *pixels*: two callsigns a thousand kilometres apart share a patch of
/// screen at one zoom and are half the window apart at the next. The scene says
/// which label matters more (`rank`); this decides which of them fits.
fn draw_labels(
    ui: &egui::Ui,
    st: &mut SolarUi,
    rect: egui::Rect,
    view_proj: &[[f32; 4]; 4],
    labels: &[super::scene::Label],
    resp: &egui::Response,
) -> bool {
    use super::scene::{Click, RANK_ALWAYS};

    let font = egui::FontId::proportional(11.5);
    let pointer = ui.input(|i| i.pointer.interact_pos());
    // A press-and-release without dragging: a drag is the camera's, a click is
    // the label's.
    let clicked = resp.clicked();
    let mut hit: Option<Click> = None;
    let p = ui.painter();

    // Best first, so the important names have taken their space by the time the
    // rest ask for what is left. A stable sort, so labels of equal rank keep the
    // order the scene built them in and a tie does not flicker between frames.
    let mut order: Vec<usize> = (0..labels.len()).collect();
    order.sort_by_key(|&i| labels[i].rank);
    let mut taken: Vec<egui::Rect> = Vec::with_capacity(labels.len());

    for l in order.into_iter().map(|i| &labels[i]) {
        let Some(anchor) = project(view_proj, rect, l.world) else { continue };
        let pos = anchor + egui::vec2(l.offset[0], l.offset[1]);
        if !rect.contains(pos) {
            continue;
        }

        let galley = p.layout_no_wrap(l.text.clone(), font.clone(), l.color);
        let text_rect =
            egui::Rect::from_min_size(pos - egui::vec2(0.0, galley.size().y * 0.5), galley.size());
        // Two callsigns overprinted are less use than one, so a name that would
        // land on one already painted is dropped instead of stacked. Zoom in and
        // it comes back, which is the same answer the map gives to every other
        // "there is too much here to read".
        let claim = text_rect.expand2(LABEL_GAP);
        if l.rank != RANK_ALWAYS && taken.iter().any(|r| r.intersects(claim)) {
            continue;
        }
        taken.push(claim);
        let hovered =
            l.click != Click::None && pointer.is_some_and(|q| text_rect.expand(4.0).contains(q));
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            // A backing plate, so it reads as something you can press.
            p.rect_filled(
                text_rect.expand2(egui::vec2(4.0, 2.0)),
                0,
                theme::FILL().gamma_multiply(0.85),
            );
            if clicked {
                hit = Some(l.click);
            }
        }

        // A dark halo, so a label stays readable over the bright solar disk as
        // well as over empty space.
        let color = if hovered { theme::scope().ink_strong } else { l.color };
        let shadow = egui::Color32::from_black_alpha(color.a().saturating_sub(40));
        p.text(
            pos + egui::vec2(1.0, 1.0),
            egui::Align2::LEFT_CENTER,
            &l.text,
            font.clone(),
            shadow,
        );
        p.text(pos, egui::Align2::LEFT_CENTER, &l.text, font.clone(), color);
    }

    match hit {
        // Clicking the open satellite's label again closes the table.
        Some(Click::Sat(id)) => {
            st.selected_sat = if st.selected_sat == Some(id) { None } else { Some(id) };
            true
        }
        Some(Click::Focus(f)) => {
            st.set_focus(f);
            true
        }
        _ => false,
    }
}

/// How long one line of the contact card takes to type itself out.
const CARD_LINE_S: f64 = 0.5;
/// Gap between the arc's apex and the bottom of the card, in pixels — enough
/// that the leader reads as a leader and the arc is not hidden under its own
/// caption.
const CARD_LEADER_PX: f32 = 18.0;
/// The block that trails the text as it is typed. A real terminal cursor rather
/// than an underscore: it has to be visible over a bright limb as well as over
/// space.
const CARD_CURSOR: &str = "▌";

/// One typed row of the contact card.
struct CardLine {
    text: String,
    font: egui::FontId,
    color: egui::Color32,
    /// Extra space under this row — the heading gets a little air.
    gap: f32,
}

/// The card over the QSO arc: what the contact is, typed out a line at a time.
///
/// It is pinned to the arc's apex in *world* space and reprojected every frame,
/// so it rides the arc as the globe turns rather than sitting at a screen
/// position that was right once. Behind the eye, or off the edge of the widget,
/// it simply is not drawn — a caption clamped to the edge no longer points at
/// anything, and the arc is what it is a caption for.
///
/// The typing is not decoration alone. A contact is a thing that happens over a
/// minute or two, and a card that assembles itself as the QSO runs reads as
/// live, where the same nine lines appearing at once read as a static panel
/// that happens to be over the globe.
fn qso_card(
    ui: &egui::Ui,
    st: &mut SolarUi,
    rect: egui::Rect,
    view_proj: &[[f32; 4]; 4],
    apex: Option<[f32; 3]>,
    now: f64,
) {
    let Some(info) = st.digi.qso.clone() else {
        // No contact: forget where the typing had got to, so the next one
        // starts from a blank card instead of finishing the last one's text.
        st.qso_card = None;
        return;
    };
    let Some(anchor) = apex.and_then(|w| project(view_proj, rect, w)) else { return };

    // Working the same station twice is two contacts, and the second gets its
    // own card — so the start time is part of the identity, not just the call.
    let key = format!("{}|{}", info.call, info.started_utc.unwrap_or(0));
    let t0 = match &st.qso_card {
        Some((k, t)) if *k == key => *t,
        _ => {
            st.qso_card = Some((key, now));
            now
        }
    };
    let elapsed = now - t0;

    let head_font = egui::FontId::monospace(13.0);
    let font = egui::FontId::monospace(11.0);
    let mut lines = vec![CardLine {
        text: info.call.clone(),
        font: head_font,
        color: theme::GREEN(),
        gap: 5.0,
    }];
    // A fixed-width label column, which is what makes a monospace card line up
    // without laying out two galleys per row and measuring between them.
    let mut row = |label: String, value: String| {
        lines.push(CardLine {
            text: format!("{label:<5}{value}"),
            font: font.clone(),
            color: theme::TEXT(),
            gap: 1.0,
        });
    };
    let mode = match (info.mode.as_str(), info.band.as_str()) {
        (m, "") => m.to_string(),
        (m, b) => format!("{m} · {b}"),
    };
    row(crate::language_plugin::text("solar.overlay.dynamic.text_1085_ac6c84", "MODE"), mode);
    if let Some(km) = info.distance_km {
        row(
            crate::language_plugin::text("solar.overlay.dynamic.text_1088_d4db71", "PATH"),
            match info.bearing_deg {
                Some(b) => format!("{km:.0} km · {b:.0}°"),
                None => format!("{km:.0} km"),
            },
        );
    }
    if let Some(started) = info.started_utc {
        row(crate::language_plugin::text("solar.overlay.dynamic.text_1096_588867", "TIME"), elapsed_hms((crate::time::now_unix() - started).max(0)));
    }
    // The two halves of the exchange, and then the live number. `SENT` and
    // `RCVD` are what the contact settled on; `SIG` is what they are doing
    // right now, which keeps moving after the reports are agreed.
    if let Some(r) = info.rpt_sent {
        row(crate::language_plugin::text("solar.overlay.dynamic.text_1102_608dbf", "SENT"), format!("{r:+} dB"));
    }
    if let Some(r) = info.rpt_rcvd {
        row(crate::language_plugin::text("solar.overlay.dynamic.text_1105_5d8ca2", "RCVD"), format!("{r:+} dB"));
    }
    if let Some(s) = info.snr_db {
        row(crate::language_plugin::text("solar.overlay.dynamic.text_1108_cc7109", "SIG"), format!("{s:+} dB"));
    }
    if let Some(g) = &info.grid {
        row(crate::language_plugin::text("solar.overlay.dynamic.text_1111_e7404b", "GRID"), g.clone());
    }
    if let Some(e) = info.entity {
        row("DXCC".to_owned(), crate::language_plugin::entity_name(e));
    }

    // Lay the whole card out at its finished size before typing a character of
    // it. A box that grows as the text arrives is a box that moves the text
    // already on it, and the reader loses their place every half second.
    let p = ui.painter();
    let galleys: Vec<_> =
        lines.iter().map(|l| p.layout_no_wrap(l.text.clone(), l.font.clone(), l.color)).collect();
    const PAD: egui::Vec2 = egui::vec2(11.0, 8.0);
    // Room for the cursor past the longest line, so it never sits on the border.
    let cursor_w = p.layout_no_wrap(CARD_CURSOR.into(), font.clone(), theme::GREEN()).size().x;
    let w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + cursor_w + PAD.x * 2.0;
    let h = galleys.iter().zip(&lines).map(|(g, l)| g.size().y + l.gap).sum::<f32>() + PAD.y * 2.0;

    let Some((card, above)) = card_rect(rect, anchor, egui::vec2(w, h)) else { return };

    // A leader to the apex, so the card is visibly *this* arc's. It leaves from
    // whichever edge faces the anchor, and from the point on that edge nearest
    // to it — the card may have been slid sideways to stay on screen.
    let foot = egui::pos2(
        anchor.x.clamp(card.left() + 6.0, card.right() - 6.0),
        if above { card.bottom() } else { card.top() },
    );
    p.line_segment([foot, anchor], egui::Stroke::new(1.0, theme::GREEN().gamma_multiply(0.7)));
    p.circle_filled(anchor, 2.0, theme::GREEN());
    p.rect_filled(card, 0, theme::BG_DEEP().gamma_multiply(0.82));
    chrome::paint_cut_border(p, card, theme::GREEN(), egui::Color32::TRANSPARENT);

    // Type it. Line `i` runs from `i * CARD_LINE_S` to the end of its own
    // half-second, so the whole card is a steady rhythm rather than long lines
    // taking longer than short ones.
    let mut y = card.top() + PAD.y;
    let x = card.left() + PAD.x;
    let mut typing = false;
    for (i, (l, g)) in lines.iter().zip(&galleys).enumerate() {
        let row_h = g.size().y;
        let total = l.text.chars().count();
        let Some(shown) = typed_chars(elapsed, i, total) else {
            break; // and every line below this one is still to come
        };
        let head: String = l.text.chars().take(shown).collect();
        let galley = p.layout_no_wrap(head, l.font.clone(), l.color);
        let end = x + galley.size().x;
        p.galley(egui::pos2(x, y), galley, l.color);
        // The cursor rides the line being typed; when the card is finished it
        // parks at the end of the last line and blinks, which is what says the
        // card is live and not a screenshot.
        let last = i == lines.len() - 1;
        if shown < total {
            typing = true;
            p.text(
                egui::pos2(end, y),
                egui::Align2::LEFT_TOP,
                CARD_CURSOR,
                l.font.clone(),
                theme::GREEN(),
            );
        } else if last && (now * 1.6).fract() < 0.5 {
            p.text(
                egui::pos2(end, y),
                egui::Align2::LEFT_TOP,
                CARD_CURSOR,
                l.font.clone(),
                theme::GREEN(),
            );
        }
        y += row_h + l.gap;
    }
    // The window animates the globe anyway; this only guarantees the cursor
    // keeps its rhythm if that ever stops.
    if typing || elapsed < lines.len() as f64 * CARD_LINE_S + 1.0 {
        crate::repaint::animate(ui.ctx());
    } else {
        crate::repaint::after_ms(ui.ctx(), 120);
    }
}

/// Where a card of `size` sits when it hangs off `anchor` inside `bounds`, and
/// whether it ended up above the anchor. `None` when it fits neither way.
///
/// Above the anchor by preference, because that is where it belongs — but under
/// it when there is no room above, which there very often is: the apex is the
/// *highest* point of the arc, so it is the point most likely to be near the top
/// of the widget, and a card that disappears whenever the operator zooms in on
/// their own contact is no card at all. Sideways it slides along the edge rather
/// than being dropped; the leader still lands on the anchor, which is what says
/// which arc it belongs to.
fn card_rect(
    bounds: egui::Rect,
    anchor: egui::Pos2,
    size: egui::Vec2,
) -> Option<(egui::Rect, bool)> {
    let above = anchor.y - CARD_LEADER_PX - size.y;
    let below = anchor.y + CARD_LEADER_PX;
    let (top, is_above) = if above >= bounds.top() {
        (above, true)
    } else if below + size.y <= bounds.bottom() {
        (below, false)
    } else {
        return None; // no room either way — the widget is smaller than the card
    };
    let left = (anchor.x - size.x * 0.5)
        .clamp(bounds.left(), (bounds.right() - size.x).max(bounds.left()));
    let card = egui::Rect::from_min_size(egui::pos2(left, top), size);
    bounds.contains_rect(card).then_some((card, is_above))
}

/// A running QSO's length: `M:SS` until it passes the hour, then `H:MM:SS`.
fn elapsed_hms(secs: i64) -> String {
    let (h, m, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// How much of line `line` is on screen `elapsed` seconds after the card
/// opened, out of `total` characters. `None` before the line's turn comes.
///
/// Every line gets the same [`CARD_LINE_S`] whatever its length, so the card
/// types at a steady beat: a nine-character line finishing in a third of the
/// time a thirty-character one takes would read as stuttering rather than as
/// typing.
fn typed_chars(elapsed: f64, line: usize, total: usize) -> Option<usize> {
    let t = (elapsed - line as f64 * CARD_LINE_S) / CARD_LINE_S;
    if t <= 0.0 {
        return None;
    }
    // `ceil`, so a line begins with its first character rather than with an
    // empty string under a cursor.
    Some(if t >= 1.0 { total } else { ((t * total as f64).ceil() as usize).min(total) })
}

/// Let the bodies themselves be clicked, not only their names.
///
/// Picking is done in screen space against the projected centres rather than by
/// reading back a depth or id buffer: there are under thirty candidates, and a
/// GPU readback would stall the frame for something a dot product answers.
fn pick_bodies(
    ui: &egui::Ui,
    st: &mut SolarUi,
    rect: egui::Rect,
    view_proj: &[[f32; 4]; 4],
    picks: &[super::scene::Pick],
    resp: &egui::Response,
    consumed: bool,
) {
    let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) else { return };
    if !rect.contains(pointer) {
        return;
    }

    // Nearest hit wins, measured in fractions of each body's own radius, so a
    // moon in front of its planet is reachable rather than swallowed by it.
    let mut best: Option<(f32, &super::scene::Pick, egui::Pos2)> = None;
    for pick in picks {
        if st.focus() == pick.focus {
            continue; // already the target; nothing to click it for
        }
        let Some(pos) = project(view_proj, rect, pick.world) else { continue };
        let d = pos.distance(pointer);
        if d > pick.radius_px {
            continue;
        }
        let score = d / pick.radius_px;
        if best.as_ref().is_none_or(|(b, _, _)| score < *b) {
            best = Some((score, pick, pos));
        }
    }
    let Some((_, pick, pos)) = best else { return };

    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    // A reticle, so it is obvious what the click will grab.
    let r = pick.radius_px.clamp(9.0, 90.0);
    ui.painter().circle_stroke(pos, r, egui::Stroke::new(1.2, theme::CYAN().gamma_multiply(0.8)));
    ui.painter().text(
        pos + egui::vec2(0.0, -r - 4.0),
        egui::Align2::CENTER_BOTTOM,
        crate::language_plugin::solar_name(pick.focus.short()),
        egui::FontId::proportional(11.0),
        theme::CYAN(),
    );
    if resp.clicked() && !consumed {
        st.set_focus(pick.focus);
    }
}

/// Frame the camera on a newly chosen target.
///
/// Without this, picking Io while framed on the whole system leaves the camera
/// 2 AU away pointed at something 3600 km across, which reads as the click
/// having done nothing. The distance is set from the target's own radius, and
/// the orientation is left alone so the move feels like a pan rather than a
/// reset.
fn reframe(st: &mut SolarUi, sim_now: f64) {
    if !std::mem::take(&mut st.retarget) {
        return;
    }
    let b = super::scene::bodies(st, sim_now);
    let (_, radius) = b.focus(st.focus());
    let (lo, hi) = super::camera::dist_range(radius);
    st.view.dist = (radius * 9.0).clamp(lo, hi);
}

/// The pass table for the selected satellite.
///
/// Prediction is expensive — stepping a whole orbit and bisecting each horizon
/// crossing — so it is computed once per selection and refreshed only as it
/// ages out, never per frame.
fn pass_window(ui: &egui::Ui, st: &mut SolarUi, data: Option<&SolarData>, sim_now: f64) {
    use sdroxide_solar::PassSearch;
    use crate::language_plugin::compass;

    let Some(id) = st.selected_sat else { return };
    let Some(d) = data else { return };
    let Some((lat, lon)) = st.qth else {
        st.selected_sat = None;
        return;
    };
    let Some(sat) = d.satellites().find(|s| s.norad_id == id) else {
        // The element set was replaced and no longer has it.
        st.selected_sat = None;
        return;
    };

    // Recompute on a change of satellite or QTH, or once the prediction has
    // aged enough that its first pass may have gone by.
    let stale = st.sat_passes.as_ref().is_none_or(|c| {
        c.norad_id != id || c.qth != (lat, lon) || (sim_now - c.computed_unix).abs() > 300.0
    });
    if stale {
        st.sat_passes = Some(super::state::SatPasses {
            norad_id: id,
            name: sat.name.clone(),
            qth: (lat, lon),
            computed_unix: sim_now,
            // Two days ahead is enough for "the next few" without spending long
            // in the search: a LEO satellite gives four to six usable passes.
            result: sat.next_passes(lat, lon, sim_now, 48.0 * 3600.0, 6),
        });
    }
    let Some(cache) = &st.sat_passes else { return };

    let mut open = true;
    egui::Window::new({ let __lp_arg_0 = &(cache.name); let __lp_arg_1 = &(st.qth_grid); crate::language_plugin::format("solar.overlay.text_1355_8dfa71", "{}  ·  PASSES FROM {}", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) })
        .id(egui::Id::new("solar-passes"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .frame(chrome::window_frame())
        .default_pos(ui.max_rect().center() - egui::vec2(230.0, 120.0))
        .show(ui.ctx(), |ui| {
            crate::chrome::window_body_bg(ui);
            ui.label(
                RichText::new({ let __lp_arg_0 = &(crate::language_plugin::solar_age(sat.element_age_s(sim_now) as i64)); crate::language_plugin::format("solar.overlay.text_1366_e43ee3", "elements {} old · SGP4", &[format!("{}", __lp_arg_0)]) })
                .color(theme::CYAN_DIM())
                .size(10.0),
            );
            ui.add_space(4.0);

            match &cache.result {
                PassSearch::AlwaysVisible { elevation, azimuth } => {
                    ui.label(
                        RichText::new(crate::language_plugin::text("solar.overlay.text_1377_48668f", "Geostationary — always above your horizon."))
                            .color(theme::GREEN())
                            .size(12.5),
                    );
                    ui.label(
                        RichText::new({ let __lp_arg_0 = &(compass(*azimuth)); crate::language_plugin::format("solar.overlay.text_1383_7c7a50", "Point at {azimuth:.0}° ({}), elevation {elevation:.0}°.", &[format!("{azimuth:.0}"), format!("{}", __lp_arg_0), format!("{elevation:.0}")]) })
                        .color(theme::TEXT()),
                    );
                }
                PassSearch::NeverVisible => {
                    ui.label(
                        RichText::new(crate::language_plugin::text("solar.overlay.text_1391_85f622", "No passes in the next 48 hours from your QTH."))
                            .color(theme::YELLOW()),
                    );
                }
                PassSearch::Passes(passes) => {
                    egui::Grid::new("solar-pass-grid").num_columns(6).spacing([14.0, 3.0]).show(
                        ui,
                        |ui| {
                            for h in [crate::language_plugin::text("solar.overlay.dynamic.text_1399_39f17e", "START"), crate::language_plugin::text("solar.overlay.dynamic.text_1399_b891f9", "END"), crate::language_plugin::text("solar.overlay.dynamic.text_1399_3c4296", "DUR"), crate::language_plugin::text("solar.overlay.dynamic.text_1399_f7c066", "AOS"), crate::language_plugin::text("solar.overlay.dynamic.text_1399_c8967d", "LOS"), crate::language_plugin::text("solar.overlay.dynamic.text_1399_e337be", "MAX EL")] {
                                ui.label(
                                    RichText::new(h).color(theme::CYAN_DIM()).size(9.5).strong(),
                                );
                            }
                            ui.end_row();
                            for p in passes {
                                let soon = p.rise_unix as f64 - sim_now;
                                // The one happening now, or next, is the one the
                                // operator cares about.
                                let color = if soon < 0.0 {
                                    theme::GREEN()
                                } else if soon < 3600.0 {
                                    theme::YELLOW()
                                } else {
                                    theme::TEXT()
                                };
                                ui.label(RichText::new(timefmt::ymd_hm(p.rise_unix)).color(color));
                                ui.label(RichText::new(hhmm(p.set_unix)).color(color));
                                ui.label(
                                    RichText::new({ let __lp_arg_0 = &(p.duration_s() / 60); crate::language_plugin::format("solar.overlay.text_1419_e823f8", "{} min", &[format!("{}", __lp_arg_0)]) })
                                        .color(color),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{:.0}° {}",
                                        p.rise_az,
                                        compass(p.rise_az)
                                    ))
                                    .color(color),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{:.0}° {}",
                                        p.set_az,
                                        compass(p.set_az)
                                    ))
                                    .color(color),
                                );
                                ui.label(
                                    RichText::new(format!("{:.0}°  {}", p.max_el, crate::language_plugin::scope_text("display.solar.pass_quality.", p.quality())))
                                        .color(match p.max_el {
                                            e if e >= 30.0 => theme::GREEN(),
                                            e if e >= 15.0 => theme::TEXT(),
                                            _ => theme::CYAN_DIM(),
                                        }),
                                );
                                ui.end_row();
                            }
                        },
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(crate::language_plugin::text("solar.overlay.text_1452_b7e662", "AOS/LOS are azimuths at the horizon. Times are UTC."))
                            .color(theme::LINE_LIT())
                            .size(10.0),
                    );
                }
            }

            // Lock on from right here — the main window activates satellite
            // mode when the root pass drains the request. Browser tab: the
            // relay carries no commands, so the button would be a lie.
            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.add_space(6.0);
                if st.sat_lock == Some(id) {
                    if chrome::chip_accent(
                        ui,
                        true,
                        egui::RichText::new(crate::language_plugin::text("solar.overlay.text_1469_7f927b", " ● LOCKED — UNLOCK ")).strong(),
                        theme::GREEN(),
                        theme::INK_ON_CYAN(),
                    )
                    .on_hover_text(crate::language_plugin::text("solar.overlay.text_1473_1da197", "Release the satellite lock"))
                    .clicked()
                    {
                        st.unlock_requested = true;
                    }
                } else if chrome::chip_accent(
                    ui,
                    false,
                    egui::RichText::new(crate::language_plugin::text("solar.overlay.text_1481_82a522", " LOCK ON ")).strong(),
                    theme::GREEN(),
                    theme::INK_ON_CYAN(),
                )
                .on_hover_text(
                    crate::language_plugin::text("solar.overlay.text_1486_d5f95c", "Track this satellite in the main window: Doppler-corrected RX and TX, \
                     transponder-mapped uplink, rotator steering"),
                )
                .clicked()
                {
                    st.lock_requested = Some(id);
                }
            }

            // The operator's own entry wins over the built-in table: they have
            // either corrected it or added a satellite it never knew about.
            let (freqs, mine) = match st.sat_cfg.freqs_for(id) {
                Some(f) => (Some(f), true),
                None => (sdroxide_solar::satfreq::builtin_for(id), false),
            };
            freq_table(ui, freqs, mine, &cache.name);
        });
    if !open {
        st.selected_sat = None;
    }
}

/// The satellite's published frequencies, under the pass table.
///
/// Knowing when a bird comes over is only half of working it; the other half is
/// what to tune to, and that is a table an operator would otherwise have open
/// in a browser next to the radio.
fn freq_table(
    ui: &mut egui::Ui,
    freqs: Option<&sdroxide_solar::SatFreqs>,
    mine: bool,
    tracked_name: &str,
) {
    let Some(freqs) = freqs.filter(|f| f.usable_links().next().is_some()) else {
        ui.add_space(6.0);
        ui.label(
            RichText::new(crate::language_plugin::text("solar.overlay.text_1522_fbb098", "No frequencies on file for this one — add them in Settings ▸ TLE."))
                .color(theme::LINE_LIT())
                .size(10.0),
        );
        return;
    };

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    let heading = if mine { crate::language_plugin::text("solar.overlay.dynamic.text_1532_69a25d", "FREQUENCIES  ·  YOURS") } else { crate::language_plugin::text("solar.overlay.dynamic.text_1532_63e717", "FREQUENCIES") };
    ui.label(RichText::new(heading).color(theme::CYAN_DIM()).size(9.5).strong());
    // The published designator can differ from what the element set calls it;
    // showing it means a table entry keyed to the wrong catalogue number is
    // visible rather than quietly presenting the wrong satellite's frequencies.
    if !freqs.name.trim().is_empty() && !freqs.name.eq_ignore_ascii_case(tracked_name) {
        ui.label(
            RichText::new({ let __lp_arg_0 = &(freqs.name); crate::language_plugin::format("solar.overlay.text_1539_453bd6", "published as {}", &[format!("{}", __lp_arg_0)]) })
                .color(theme::LINE_LIT())
                .size(10.0),
        );
    }
    ui.add_space(2.0);

    egui::Grid::new("solar-freq-grid").num_columns(4).spacing([14.0, 3.0]).show(ui, |ui| {
        for h in [crate::language_plugin::text("solar.overlay.dynamic.text_1547_e26ef1", "LINK"), crate::language_plugin::text("solar.overlay.dynamic.text_1547_d5abb6", "DOWNLINK (MHz)"), crate::language_plugin::text("solar.overlay.dynamic.text_1547_6026f4", "UPLINK (MHz)"), crate::language_plugin::text("solar.overlay.dynamic.text_1547_ac6c84", "MODE")] {
            ui.label(RichText::new(h).color(theme::CYAN_DIM()).size(9.5).strong());
        }
        ui.end_row();
        for l in freqs.links.iter().filter(|l| !l.is_empty()) {
            let mut label = RichText::new(crate::language_plugin::satellite_link_label(l, mine)).color(theme::TEXT());
            if !l.note.is_empty() {
                label = label.color(theme::TEXT_STRONG());
            }
            let resp = ui.label(label);
            if !l.note.is_empty() {
                resp.on_hover_text(crate::language_plugin::satellite_link_note(l, mine));
            }
            // The downlink is what gets tuned first, so it leads.
            ui.label(
                RichText::new(l.downlink.map_or_else(|| "—".into(), |b| b.to_string()))
                    .color(theme::GREEN()),
            );
            ui.label(
                RichText::new(l.uplink.map_or_else(|| "—".into(), |b| b.to_string()))
                    .color(theme::YELLOW()),
            );
            ui.label(RichText::new(crate::language_plugin::satellite_link_mode(l, mine)).color(theme::TEXT()));
            ui.end_row();
        }
    });

    // Everything an operator has to know before keying up sits in the notes, so
    // they go on screen rather than only in a tooltip.
    for l in freqs.links.iter().filter(|l| !l.note.is_empty() && !l.is_empty()) {
        ui.label(
            RichText::new(format!("{} — {}", crate::language_plugin::satellite_link_label(l, mine), crate::language_plugin::satellite_link_note(l, mine))).color(theme::LINE_LIT()).size(10.0),
        );
    }
    ui.add_space(2.0);
    ui.label(
        RichText::new(crate::language_plugin::text("solar.overlay.text_1583_ee4580", "Doppler shifts these by a few kHz across a LEO pass."))
            .color(theme::LINE_LIT())
            .size(10.0),
    );
}

/// `HH:MMZ` — the date is already on the start column. Marked UTC like the
/// start column, so the end is not read as a local time (#635).
fn hhmm(unix: i64) -> String {
    let (_, _, _, h, m, _) = sdroxide_types::utc_ymd_hms(unix);
    format!("{h:02}:{m:02}Z")
}

/// A dot-matrix UTC clock in the top-left corner, with the same instant in the
/// operator's own zone on a smaller second row.
///
/// UTC leads because everything else in the window is UTC: the ephemeris, the
/// DONKI timestamps, the arrival estimates and the FT8 slot boundaries. Local
/// time is the row underneath because "is that pass at a civilised hour?" is a
/// question about the wall clock in the shack, and doing that subtraction in
/// your head is how a pass gets missed.
fn clock(ui: &egui::Ui, rect: egui::Rect, sim_now: f64, scrubbed: bool) -> Option<egui::Rect> {

    let hms = |unix: i64| {
        let (_, _, _, h, m, s) = sdroxide_types::utc_ymd_hms(unix);
        format!("{h:02}:{m:02}:{s:02}")
    };
    let utc = sim_now as i64;
    let text = hms(utc);
    // The scrubbed instant, not the wall clock: two clocks side by side that
    // disagreed about when "now" is would read as one of them being broken.
    let local_text = hms(utc + crate::time::local_offset_seconds());

    // Scale with the window, but never so large it competes with the scene —
    // and the panel's own paddings scale with the dots, so the whole box grows
    // and shrinks as one piece.
    let pitch = (rect.width() * 0.004_25).clamp(1.3, 3.5);
    let size = clock_label_size(ui.painter(), &text, pitch);
    let local_pitch = pitch * 0.58;
    let mut local_size = clock_label_size(ui.painter(), &local_text, local_pitch);
    let label_pitch = pitch * 0.42;
    let label = if scrubbed { crate::language_plugin::text("solar.overlay.dynamic.text_1624_a2cda5", "-- SIM") } else { "UTC".to_owned() };
    let label_size = clock_label_size(ui.painter(), &label, label_pitch);
    let local_label_size = clock_label_size(ui.painter(), &crate::language_plugin::text("solar.overlay.dynamic.text_1626_093cba", "LOC"), label_pitch);
    local_size.y = local_size.y.max(local_label_size.y);

    // Both labels share one column to the right of the digits, so UTC and LOC
    // line up however wide the two readouts come out.
    const GAP: f32 = 5.0;
    let label_x = size.x.max(local_size.x) + GAP;
    let row_gap = 3.5;

    let pad = egui::vec2(6.0, 4.5);
    let panel = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(MARGIN, MARGIN),
        egui::vec2(label_x + label_size.x.max(local_label_size.x), size.y + row_gap + local_size.y)
            + pad * 2.0,
    );
    if !rect.contains_rect(panel) {
        return None;
    }

    ui.painter().rect_filled(panel, 0, theme::BG_DEEP().gamma_multiply(0.72));
    chrome::paint_cut_border(
        ui.painter(),
        panel,
        if scrubbed { theme::YELLOW() } else { theme::LINE_LIT() },
        egui::Color32::TRANSPARENT,
    );

    // Unlit dots at a low alpha are what make this read as a physical display
    // rather than as text in a blocky face.
    let on = if scrubbed { theme::YELLOW() } else { theme::CYAN() };
    let off = on.gamma_multiply(0.11);
    let origin = panel.min + pad;
    let p = ui.painter();
    // Each label sits on the bottom row of the digits it names.
    clock_label_draw(p, origin, &text, pitch, on, off);
    clock_label_draw(
        p,
        origin + egui::vec2(label_x, size.y - label_size.y),
        &label,
        label_pitch,
        on.gamma_multiply(0.6),
        egui::Color32::TRANSPARENT,
    );
    let local_y = size.y + row_gap;
    clock_label_draw(
        p,
        origin + egui::vec2(0.0, local_y),
        &local_text,
        local_pitch,
        on.gamma_multiply(0.72),
        off,
    );
    clock_label_draw(
        p,
        origin + egui::vec2(label_x, local_y + local_size.y - local_label_size.y),
        &crate::language_plugin::text("solar.overlay.dynamic.text_1680_093cba", "LOC"),
        label_pitch,
        on.gamma_multiply(0.6),
        egui::Color32::TRANSPARENT,
    );
    Some(panel)
}

/// A `size`-sized box tucked against the right edge of `rect`, with its base on
/// `bottom` and [`MARGIN`] clear of the edge.
fn bottom_right(rect: egui::Rect, bottom: f32, size: egui::Vec2) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(rect.right() - MARGIN - size.x, bottom - size.y), size)
}

/// The calendar date the clock's time belongs to, UTC over local, in the
/// bottom-right corner.
///
/// The clock deliberately shows only `HH:MM:SS`, which is all you need until
/// the two zones fall on different days — and then "is that pass tonight or
/// tomorrow night?" has no answer on screen at all. Scrubbing the timeline
/// makes it worse: a month of simulated time moves nothing in a readout that
/// only counts seconds.
///
/// Out of the way rather than beside the clock: the date is checked once and
/// then ignored, while the top of the view is where the menu chips and the space
/// weather live. Returns the box it drew, so the award key can stack above it.
fn date_readout(ui: &egui::Ui, st: &SolarUi, rect: egui::Rect, sim_now: f64) -> Option<egui::Rect> {
    let utc = sim_now as i64;
    let scrubbed = st.sim_offset_s != 0.0;
    // The scrubbed instant, like the clock: two readouts of "now" that disagreed
    // would read as one of them being broken.
    let rows = [
        ("UTC".to_owned(), crate::language_plugin::solar_calendar(utc)),
        (crate::language_plugin::text("solar.overlay.dynamic.text_1713_093cba", "LOC"), crate::language_plugin::solar_calendar(utc + crate::time::local_offset_seconds())),
    ];

    let font = egui::FontId::proportional(11.5);
    let label_font = egui::FontId::proportional(9.5);
    let on = if scrubbed { theme::YELLOW() } else { theme::CYAN() };
    let p = ui.painter();
    let laid: Vec<_> = rows
        .iter()
        .map(|(label, date)| {
            (
                p.layout_no_wrap(label.clone(), label_font.clone(), theme::CYAN_DIM()),
                p.layout_no_wrap(date.clone(), font.clone(), on),
            )
        })
        .collect();
    let label_w = laid.iter().map(|(l, _)| l.size().x).fold(0.0f32, f32::max);
    let date_w = laid.iter().map(|(_, d)| d.size().x).fold(0.0f32, f32::max);
    let row_h = laid.iter().map(|(_, d)| d.size().y + 2.0).fold(0.0f32, f32::max);

    // Between the label column and the dates.
    const COL_GAP: f32 = 8.0;
    let pad = egui::vec2(10.0, 7.0);
    let size = egui::vec2(label_w + COL_GAP + date_w, row_h * laid.len() as f32) + pad * 2.0;

    let panel = bottom_right(rect, rect.bottom() - MARGIN, size);
    if !rect.contains_rect(panel) {
        return None;
    }

    p.rect_filled(panel, 0, theme::BG_DEEP().gamma_multiply(0.72));
    chrome::paint_cut_border(
        p,
        panel,
        if scrubbed { theme::YELLOW() } else { theme::LINE_LIT() },
        egui::Color32::TRANSPARENT,
    );
    let mut y = panel.top() + pad.y;
    for (label, date) in laid {
        // Labels on the baseline of the date they name, dates right-aligned so
        // the two rows line up whatever length the day-of-month comes out.
        let label_y = y + (row_h - 2.0) - label.size().y;
        p.galley(egui::pos2(panel.left() + pad.x, label_y), label, theme::CYAN_DIM());
        p.galley(egui::pos2(panel.right() - pad.x - date.size().x, y), date, on);
        y += row_h;
    }
    Some(panel)
}

/// The find box, under the clock.
///
/// Two populations of dots that cannot be found by reading labels, and one box
/// for both. Ninety satellites around the Earth, of which the ones outside the
/// curated set have no label at all until `ALL SATS` is on — at which point
/// there are ninety unlabelled dots. And forty small bodies strung across fifty
/// AU: five dwarf planets, twenty asteroids and fifteen comets, which are
/// deliberately *not* all named at once because that would bury the planets.
///
/// Typing pulls a match out of either crowd with its orbit and its name,
/// whether or not it was being drawn a moment ago. Enter commits: on a
/// satellite that means its pass table, which is what you looked it up for; on
/// a body it means pointing the camera at it, which is the same thing.
///
/// Hidden only when *neither* population is being drawn, because a search that
/// highlights things nothing is drawing would look broken.
fn find_box(
    ui: &egui::Ui,
    st: &mut SolarUi,
    data: Option<&SolarData>,
    rect: egui::Rect,
    clock_rect: Option<egui::Rect>,
) {
    let Some(clock_rect) = clock_rect else { return };
    let (sats, bodies) = (st.layer(layer::SATS), st.layer(layer::PLANETS));
    if !sats && !bodies {
        // Leaving text behind in a hidden box would keep highlighting after the
        // layers came back, with nothing on screen to say why.
        st.search.clear();
        return;
    }

    // Exactly the clock's width, so the two make one column down the left edge
    // and the box stays clear of the menu chips beside them. Anything wider
    // reaches into the chips' column, and on a phone — where the row wraps down
    // the window — that pushed the box below however many rows of chips there
    // were, which is no longer under the clock at all.
    let width = clock_rect.width();
    let area = egui::Rect::from_min_size(
        clock_rect.left_bottom() + egui::vec2(0.0, 6.0),
        egui::vec2(width, 30.0),
    );
    if !rect.contains_rect(area) {
        return;
    }

    // Counted from the same predicates the scene highlights with, so "3 of 94"
    // can never disagree with what is lit up.
    let (sat_hits, sat_total) = match data.filter(|_| sats) {
        Some(d) => d.satellites().fold((0usize, 0usize), |(h, n), sat| {
            (h + st.sat_hit(&sat.name, sat.norad_id) as usize, n + 1)
        }),
        None => (0, 0),
    };
    let body_hits: Vec<usize> =
        if bodies { st.small_hits().map(|(i, _)| i).collect() } else { Vec::new() };
    let body_total = if bodies { sdroxide_solar::smallbody::BODIES.len() } else { 0 };

    let query = st.search.trim().to_string();
    let mut clear = false;
    let mut open: Option<u64> = None;
    let mut go: Option<Focus> = None;
    let hint = match (sats, bodies) {
        (true, true) => crate::language_plugin::text("solar.overlay.dynamic.text_1825_bbc248", "satellite, planet or comet"),
        (true, false) => crate::language_plugin::text("solar.overlay.dynamic.text_1826_dc4b4e", "satellite"),
        _ => crate::language_plugin::text("solar.overlay.dynamic.text_1827_3bd1c8", "planet, asteroid or comet"),
    };

    egui::Area::new(egui::Id::new("solar-find"))
        .order(egui::Order::Foreground)
        .fixed_pos(area.min)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(theme::BG_DEEP().gamma_multiply(0.72))
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.set_width(width - 16.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        // 🔍 and × over ⌕ and ✕: the latter pair is in none of
                        // the bundled faces — not Chakra Petch, not egui's
                        // Ubuntu/Noto Emoji fallbacks — so both drew as tofu.
                        ui.label(RichText::new("🔍").color(theme::CYAN_DIM()).size(12.0));
                        let edit = crate::chrome::field(
                            ui,
                            egui::TextEdit::singleline(&mut st.search)
                                .desired_width(width - 62.0)
                                .hint_text(hint)
                                .text_color(theme::TEXT_STRONG()),
                        );
                        // Enter on a single match commits to it. A body wins a
                        // tie against a satellite only when it is the sole
                        // match overall, so neither can hijack the other's key.
                        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            match (sat_hits, body_hits.len()) {
                                (1, 0) => {
                                    open = data.and_then(|d| {
                                        d.satellites()
                                            .find(|s| st.sat_hit(&s.name, s.norad_id))
                                            .map(|s| s.norad_id)
                                    })
                                }
                                (0, 1) => go = Some(Focus::Small(body_hits[0])),
                                _ => {}
                            }
                        }
                        if !query.is_empty()
                            && ui.button("×").on_hover_text(crate::language_plugin::text("solar.overlay.text_1869_155b1a", "Clear the search")).clicked()
                        {
                            clear = true;
                        }
                    });
                    if !query.is_empty() {
                        let (text, colour) = match (sat_hits, body_hits.len()) {
                            (0, 0) => (crate::language_plugin::text("solar.overlay.dynamic.text_1876_9bc94a", "no match").to_string(), theme::ALERT()),
                            // Named outright when there is exactly one, because
                            // "1 of 40" is a worse answer than "Apophis".
                            (0, 1) => (
                                { let __lp_arg_0 = &(sdroxide_solar::smallbody::BODIES[body_hits[0]].designation); crate::language_plugin::format("solar.overlay.dynamic.text_1881_6ed3c8", "{} — ↵ to fly there", &[format!("{}", __lp_arg_0)]) },
                                theme::YELLOW(),
                            ),
                            (s, 0) => (crate::language_plugin::format("solar.overlay.dynamic.text_1886_9fa117", "{s} of {sat_total} tracked", &[format!("{s}"), format!("{sat_total}")]), theme::YELLOW()),
                            (0, b) => (crate::language_plugin::format("solar.overlay.dynamic.text_1887_70eee6", "{b} of {body_total} bodies", &[format!("{b}"), format!("{body_total}")]), theme::YELLOW()),
                            (s, b) => (
                                crate::language_plugin::format("solar.overlay.dynamic.text_1889_0345e6", "{s} of {sat_total} tracked · {b} of {body_total} bodies", &[format!("{s}"), format!("{sat_total}"), format!("{b}"), format!("{body_total}")]),
                                theme::YELLOW(),
                            ),
                        };
                        ui.label(RichText::new(text).color(colour).size(10.0));
                    }
                });
        });

    if clear {
        st.search.clear();
    }
    if let Some(id) = open {
        st.selected_sat = Some(id);
    }
    if let Some(f) = go {
        st.set_focus(f);
    }
}

/// The propagation numbers, under the aurora: MUF at the QTH, K and A, the
/// 10.7 cm flux and the current GOES X-ray level.
fn weather_panel(
    ui: &mut egui::Ui,
    st: &SolarUi,
    data: Option<&SolarData>,
    place: Place,
    now: i64,
) -> Option<egui::Rect> {
    let d = data?;
    let w = &d.weather;

    // (label, value, colour). Colours say what the number means for the bands,
    // which is the only reason an operator is reading them.
    let mut rows: Vec<(String, String, egui::Color32)> = Vec::new();

    if let Some((lat, lon)) = st.qth {
        match sdroxide_solar::indices::estimate_muf(&w.ionosondes, lat, lon, now) {
            Some(m) => rows.push((
                "MUF".into(),
                format!("{:.1} MHz", m.muf_mhz),
                match m.muf_mhz {
                    f if f >= 24.0 => theme::GREEN(),
                    f if f >= 14.0 => theme::CYAN(),
                    _ => theme::YELLOW(),
                },
            )),
            None => rows.push(("MUF".into(), crate::language_plugin::text("solar.overlay.dynamic.text_1936_27d9c7", "no sounder").into(), theme::LINE_LIT())),
        }
        // What this station has actually heard get through, near the QTH.
        //
        // A floor, and labelled as one twice over — `HEARD ≥` and a `≥` on the
        // value — because it is a different kind of statement from the row
        // above it. The sounder measures and interpolates; this observes and
        // bounds. Neither replaces the other: the sounder has dreadful spatial
        // coverage and this has none at all over an ocean nobody works, and
        // where they disagree the disagreement is the useful part.
        if let Some(m) = st.prop.muf_at(lat, lon) {
            rows.push((
                crate::language_plugin::text("solar.overlay.dynamic.text_1948_726dda", "HEARD ≥").into(),
                format!("≥ {:.1} MHz", m.floor_mhz),
                match m.floor_mhz {
                    f if f >= 24.0 => theme::GREEN(),
                    f if f >= 14.0 => theme::CYAN(),
                    _ => theme::YELLOW(),
                },
            ));
        }
    }
    if let Some(g) = &w.geomagnetic {
        let color = match g.kp {
            k if k >= 5.0 => theme::PINK(),
            k if k >= 4.0 => theme::YELLOW(),
            _ => theme::GREEN(),
        };
        rows.push(("Kp / A".into(), format!("{:.1} / {:.0}", g.kp, g.a_index), color));
    }
    if let Some(f) = &w.flux {
        rows.push((
            "F10.7".into(),
            format!("{:.0} sfu", f.sfu),
            match f.sfu {
                v if v >= 150.0 => theme::GREEN(),
                v if v >= 90.0 => theme::CYAN(),
                _ => theme::YELLOW(),
            },
        ));
    }
    if let Some(x) = &w.xray {
        rows.push((
            crate::language_plugin::text("solar.overlay.dynamic.text_1979_e0ab46", "X-ray").into(),
            x.class.clone(),
            if x.causes_hf_absorption() { theme::PINK() } else { theme::CYAN_DIM() },
        ));
    }
    if rows.is_empty() {
        return None;
    }

    let font = egui::FontId::proportional(12.0);
    let small = egui::FontId::proportional(10.0);
    // Cloned so the box can be laid out before the space for it is reserved:
    // `Place::Inline` needs the `Ui` back to allocate from.
    let p = ui.painter().clone();
    let laid: Vec<_> = rows
        .iter()
        .map(|(k, v, c)| {
            (
                p.layout_no_wrap(k.clone(), small.clone(), theme::CYAN_DIM()),
                p.layout_no_wrap(v.clone(), font.clone(), *c),
            )
        })
        .collect();
    let key_w = laid.iter().map(|(k, _)| k.size().x).fold(0.0f32, f32::max);
    let val_w = laid.iter().map(|(_, v)| v.size().x).fold(0.0f32, f32::max);
    let row_h = laid.iter().map(|(_, v)| v.size().y + 3.0).fold(0.0f32, f32::max);

    // A one-line caveat under the MUF: it is interpolated from ionosondes that
    // may be a long way off, and saying so costs one line.
    let sounder = st
        .qth
        .and_then(|(lat, lon)| sdroxide_solar::indices::estimate_muf(&w.ionosondes, lat, lon, now));
    let observed = st.qth.and_then(|(lat, lon)| st.prop.muf_at(lat, lon));
    let mut notes: Vec<egui::text::Galley> = Vec::new();
    if let Some(m) = &sounder {
        notes.push(
            (*p.layout_no_wrap(
                format!("{} · {:.0} km", crate::language_plugin::scope_text("display.solar.confidence.", m.confidence()), m.nearest_km),
                small.clone(),
                theme::LINE_LIT(),
            ))
            .clone(),
        );
    }
    if let Some(o) = &observed {
        notes.push(
            (*p.layout_no_wrap(
                { let __lp_arg_0 = &(crate::language_plugin::scope_text("display.solar.confidence.", o.confidence())); let __lp_arg_1 = &(o.band.label()); crate::language_plugin::format("solar.overlay.dynamic.text_2026_ccf138", "{} on {}", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) },
                small.clone(),
                theme::LINE_LIT(),
            ))
            .clone(),
        );
        // The one comparison worth more than either number on its own. The
        // sounder is a model of a place it is not standing in; a path that got
        // through is a fact. When the fact is above the model, the band is
        // better than the model thinks — which is the most actionable thing
        // this panel can say.
        if sounder.as_ref().is_some_and(|m| o.floor_mhz > m.muf_mhz + 1.0) {
            notes.push(
                (*p.layout_no_wrap(
                    crate::language_plugin::text("solar.overlay.dynamic.text_2040_011b68", "observed above the sounder — better than modelled").into(),
                    small.clone(),
                    theme::GREEN(),
                ))
                .clone(),
            );
        }
    }

    let pad = 10.0;
    let width = key_w + val_w + 18.0 + pad * 2.0;
    let width = notes.iter().fold(width, |acc, n| acc.max(n.size().x + pad * 2.0));
    let height =
        rows.len() as f32 * row_h + notes.iter().map(|n| n.size().y + 4.0).sum::<f32>() + pad * 2.0;
    let panel = place.reserve(ui, egui::vec2(width, height))?;

    p.rect_filled(panel, 0, theme::FILL().gamma_multiply(0.82));
    chrome::paint_cut_border(&p, panel, theme::LINE_LIT(), egui::Color32::TRANSPARENT);
    let mut y = panel.top() + pad;
    for ((key, val), (_, _, color)) in laid.iter().zip(&rows) {
        p.galley(egui::pos2(panel.left() + pad, y + 2.0), key.clone(), theme::CYAN_DIM());
        p.galley(egui::pos2(panel.right() - pad - val.size().x, y), val.clone(), *color);
        y += row_h;
    }
    for n in notes {
        let h = n.size().y + 4.0;
        p.galley(
            egui::pos2(panel.left() + pad, y + 2.0),
            std::sync::Arc::new(n),
            theme::LINE_LIT(),
        );
        y += h;
    }
    Some(panel)
}

/// Aurora: how much power is going into each oval, how far towards the equator
/// it reaches, whether it is over your head, and what the planetary K series
/// did over the last day and is forecast to do over the next.
///
/// The colours here mean the same thing they do everywhere else in the window —
/// green quiet, yellow worth watching, pink a storm.
///
/// Returns the box it drew, or `None` if it drew nothing — the propagation
/// numbers stack under it either way.
fn aurora_panel(
    ui: &mut egui::Ui,
    st: &SolarUi,
    data: Option<&SolarData>,
    place: Place,
    now: i64,
) -> Option<egui::Rect> {
    use sdroxide_solar::{HemisphericPower, aurora};

    let d = data?;
    // Nothing to say until one of the three aurora feeds has landed. Drawing an
    // empty box would imply the aurora had been measured and found absent.
    if d.aurora.is_none() && d.aurora_power.is_none() && d.kp_forecast.is_empty() {
        return None;
    }
    let kp_color = |kp: f64| match kp {
        k if k >= 5.0 => theme::PINK(),
        k if k >= 4.0 => theme::YELLOW(),
        _ => theme::GREEN(),
    };

    // Both hemispheres are reported throughout: without a QTH neither is more
    // relevant than the other, and with one the QTH row already says what
    // matters locally.
    let mut rows: Vec<(String, String, egui::Color32)> = Vec::new();
    if let Some(power) = &d.aurora_power {
        let worst = power.north_gw.max(power.south_gw);
        let color = match HemisphericPower::index(worst) {
            8..=10 => theme::PINK(),
            6..=7 => theme::YELLOW(),
            _ => theme::GREEN(),
        };
        rows.push((
            crate::language_plugin::text("solar.overlay.dynamic.text_2118_8c39ef", "power N/S").into(),
            format!("{:.0} / {:.0} GW", power.north_gw, power.south_gw),
            color,
        ));
        rows.push((
            crate::language_plugin::text("solar.overlay.dynamic.text_2123_065186", "activity").into(),
            format!("{} · HPI {}", crate::language_plugin::scope_text("display.solar.activity.", HemisphericPower::words(worst)), HemisphericPower::index(worst)),
            color,
        ));
    }
    if let Some(oval) = &d.aurora {
        // The edge is where it stops being worth looking, so it is the number
        // an operator compares their own latitude against.
        let edge = |n: bool| {
            oval.equatorward_edge(n, aurora::EDGE_PCT)
                .map(|lat| format!("{:.0}°{}", lat.abs(), if n { "N" } else { "S" }))
        };
        // An oval too weak to reach the contour anywhere in one hemisphere is a
        // real state, and one worth reading as a dash rather than as a row that
        // silently disappeared.
        let (n, s) = (edge(true), edge(false));
        if n.is_some() || s.is_some() {
            let show = |e: Option<String>| e.unwrap_or_else(|| "—".into());
            rows.push((crate::language_plugin::text("solar.overlay.dynamic.text_2141_8b42c7", "edge N/S").into(), format!("{} / {}", show(n), show(s)), theme::CYAN()));
        }
        if let Some((lat, lon)) = st.qth {
            let pct = oval.probability(lat, lon);
            let color = match pct {
                p if p >= 25.0 => theme::PINK(),
                p if p >= aurora::EDGE_PCT => theme::YELLOW(),
                p if p >= aurora::NOISE_FLOOR_PCT => theme::GREEN(),
                _ => theme::LINE_LIT(),
            };
            rows.push((st.qth_grid.clone(), format!("{pct:.0} %"), color));
        }
    }
    // The forecast half: the worst three-hour bin still ahead of us, and how
    // far away it is.
    let peak = aurora::peak_forecast(&d.kp_forecast, now, 24 * 3600);
    if let Some(p) = peak {
        let in_h = (p.unix - now).max(0) as f64 / 3600.0;
        rows.push((
            crate::language_plugin::text("solar.overlay.dynamic.text_2160_6bd1e6", "Kp peak 24 h").into(),
            if in_h < 1.5 {
                { let __lp_arg_0 = &(p.kp); crate::language_plugin::format("solar.overlay.dynamic.text_2162_c714e1", "{:.1} now", &[format!("{:.1}", __lp_arg_0)]) }
            } else {
                { let __lp_arg_0 = &(p.kp); crate::language_plugin::format("solar.overlay.dynamic.text_2164_279d45", "{:.1} in {in_h:.0} h", &[format!("{:.1}", __lp_arg_0), format!("{in_h:.0}")]) }
            },
            kp_color(p.kp),
        ));
        rows.push((
            crate::language_plugin::text("solar.overlay.dynamic.text_2169_2ca8e4", "viewline").into(),
            { let __lp_arg_0 = &(aurora::viewline_geomagnetic_lat(p.kp)); crate::language_plugin::format("solar.overlay.dynamic.text_2170_129c86", "{:.0}° geomag", &[format!("{:.0}", __lp_arg_0)]) },
            kp_color(p.kp),
        ));
    }
    if rows.is_empty() {
        return None;
    }

    // The forecast strip: one bar per three-hour bin over the next day. Eight
    // numbers in a column is a table nobody reads; the same eight as a shape is
    // "it picks up after midnight" at a glance.
    // The trend: the observed bins behind `now` and the forecast ahead of it in
    // one row, so the shape of the last day flows into the shape of the next.
    // The forecast bars are filled lighter — a measurement and a prediction
    // must never be read as the same thing.
    let mut series: Vec<&sdroxide_solar::KpPoint> =
        aurora::recent(&d.kp_forecast, now, 24 * 3600).collect();
    let history_len = series.len();
    series.extend(aurora::upcoming(&d.kp_forecast, now, 24 * 3600).take(8));
    const BAR_W: f32 = 9.0;
    const BAR_GAP: f32 = 2.0;
    const STRIP_H: f32 = 26.0;

    let font = egui::FontId::proportional(12.0);
    let small = egui::FontId::proportional(10.0);
    // Cloned for the reason the propagation box clones it: laid out first, and
    // only then handed a place to sit.
    let p = ui.painter().clone();
    let laid: Vec<_> = rows
        .iter()
        .map(|(k, v, c)| {
            (
                p.layout_no_wrap(k.clone(), small.clone(), theme::CYAN_DIM()),
                p.layout_no_wrap(v.clone(), font.clone(), *c),
            )
        })
        .collect();
    let key_w = laid.iter().map(|(k, _)| k.size().x).fold(0.0f32, f32::max);
    let val_w = laid.iter().map(|(_, v)| v.size().x).fold(0.0f32, f32::max);
    let row_h = laid.iter().map(|(_, v)| v.size().y + 3.0).fold(0.0f32, f32::max);

    // What the picture is valid for, never what time it is now: OVATION is a
    // forecast for about forty minutes ahead and the grid may be an hour old.
    let footer = d.aurora.as_ref().map(|o| {
        let age = (now - o.observed_unix).max(0);
        p.layout_no_wrap(
            { let __lp_arg_0 = &(timefmt::ymd_hm(o.forecast_unix)); let __lp_arg_1 = &(crate::language_plugin::solar_age(age)); crate::language_plugin::format("solar.overlay.dynamic.text_2216_cf2538", "valid {} · {} old", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1)]) },
            small.clone(),
            theme::LINE_LIT(),
        )
    });

    let title = p.layout_no_wrap(crate::language_plugin::text("solar.overlay.dynamic.text_2222_1e28b2", "AURORA").into(), small.clone(), theme::CYAN_DIM());
    let pad = 10.0;
    let strip_w =
        if series.is_empty() { 0.0 } else { series.len() as f32 * (BAR_W + BAR_GAP) - BAR_GAP };
    let width =
        (key_w + val_w + 18.0).max(strip_w).max(footer.as_ref().map_or(0.0, |f| f.size().x))
            + pad * 2.0;
    let height = title.size().y
        + 5.0
        + rows.len() as f32 * row_h
        // Strip: the gap above it, the bars, and the hour stamps under them.
        + if series.is_empty() { 0.0 } else { STRIP_H + 18.0 }
        + footer.as_ref().map_or(0.0, |f| f.size().y + 4.0)
        + pad * 2.0;

    let panel = place.reserve(ui, egui::vec2(width, height))?;

    p.rect_filled(panel, 0, theme::FILL().gamma_multiply(0.82));
    chrome::paint_cut_border(&p, panel, theme::LINE_LIT(), egui::Color32::TRANSPARENT);

    let mut y = panel.top() + pad;
    let title_h = title.size().y;
    p.galley(egui::pos2(panel.left() + pad, y), title, theme::CYAN_DIM());
    y += title_h + 5.0;
    for ((key, val), (_, _, color)) in laid.iter().zip(&rows) {
        p.galley(egui::pos2(panel.left() + pad, y + 2.0), key.clone(), theme::CYAN_DIM());
        p.galley(egui::pos2(panel.right() - pad - val.size().x, y), val.clone(), *color);
        y += row_h;
    }

    if !series.is_empty() {
        y += 6.0;
        let base = y + STRIP_H;
        for (i, bin) in series.iter().enumerate() {
            let x = panel.left() + pad + i as f32 * (BAR_W + BAR_GAP);
            let h = (bin.kp / 9.0) as f32 * STRIP_H;
            // An unlit socket under every bar, so a quiet stretch still reads
            // as a scale rather than as missing data.
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(x + BAR_W, base)),
                0,
                theme::LINE().gamma_multiply(0.55),
            );
            // The measured half is solid, the predicted half a wash: the two
            // must not be read as one continuous observation.
            let fill = if bin.predicted {
                kp_color(bin.kp).gamma_multiply(0.45)
            } else {
                kp_color(bin.kp)
            };
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x, base - h.max(1.0)),
                    egui::pos2(x + BAR_W, base),
                ),
                0,
                fill,
            );
        }
        // Where measurement gives way to prediction. The first forecast bar may
        // also be the bin containing `now`, so the line lands where the two
        // halves meet rather than on a clock face.
        if history_len > 0 && history_len < series.len() {
            let x = panel.left() + pad + history_len as f32 * (BAR_W + BAR_GAP) - BAR_GAP / 2.0;
            p.line_segment(
                [egui::pos2(x, y - 2.0), egui::pos2(x, base)],
                egui::Stroke::new(1.0, theme::LINE_LIT()),
            );
        }
        // Hours under the ends, so the strip has a time axis without sixteen
        // labels fighting for room.
        let stamp = |unix: i64| {
            let (_, _, _, h, _, _) = sdroxide_types::utc_ymd_hms(unix);
            format!("{h:02}z")
        };
        p.text(
            egui::pos2(panel.left() + pad, base + 2.0),
            egui::Align2::LEFT_TOP,
            stamp(series[0].unix),
            small.clone(),
            theme::LINE_LIT(),
        );
        p.text(
            egui::pos2(panel.left() + pad + strip_w, base + 2.0),
            egui::Align2::RIGHT_TOP,
            stamp(series[series.len() - 1].unix),
            small.clone(),
            theme::LINE_LIT(),
        );
        // What the two halves are, once, under the axis — only when both are
        // actually present.
        if history_len > 0 && history_len < series.len() {
            p.text(
                egui::pos2(panel.left() + pad + strip_w / 2.0, base + 2.0),
                egui::Align2::CENTER_TOP,
                crate::language_plugin::text("solar.overlay.dynamic.text_2317_03b9db", "observed | forecast"),
                small.clone(),
                theme::LINE_LIT(),
            );
        }
        y = base + 12.0;
    }

    if let Some(f) = footer {
        p.galley(egui::pos2(panel.left() + pad, y + 2.0), f, theme::LINE_LIT());
    }
    Some(panel)
}

/// How open each band is, as one bar per band.
///
/// The same question the heat map answers geographically, reduced to a number
/// per band so it can be read without turning the globe: **how much of the
/// world is this band getting through to right now**. A bar chart because the
/// answer is a comparison — an operator picking a band wants the shape of the
/// spectrum, not five figures to subtract from each other — and in band order
/// rather than ranked, so that shape is the familiar one and the bars do not
/// swap places every time a decode lands.
///
/// What is measured is [`BandPlane::reach`]: the share of the world with
/// evidence in it, area-weighted. Deliberately not a path count. Forty contacts
/// into one corner of Europe are one direction open, and a count would call
/// that the best band of the evening.
///
/// It inherits the propagation field's memory exactly, because it is read off
/// that field rather than accumulated beside it: whatever the half-life is set
/// to is how long a band stays on this chart after it shuts.
fn bands_panel(ui: &mut egui::Ui, st: &SolarUi, place: Place) -> Option<egui::Rect> {
    use sdroxide_types::Band;

    // Only bands with something in them. A chart of dead bands is a chart of
    // where this station has not been listening, which is not what it claims
    // to be — see the footer, which says so out loud.
    let rows: Vec<(Band, f32)> = st
        .prop
        .live_bands()
        .into_iter()
        .filter_map(|b| st.prop.plane(b).map(|p| (b, p.reach())))
        .filter(|(_, reach)| *reach > 0.0)
        .collect();
    if rows.is_empty() {
        return None;
    }

    // Autoranged in decades rather than normalised to the best band, and the
    // top of the scale is printed underneath. Normalising would draw the same
    // full bar for a band working a quarter of the planet and for one that has
    // heard a single neighbour, and the chart would silently change meaning
    // every time the leader changed.
    const SCALE_STEPS: [f32; 5] = [0.01, 0.03, 0.10, 0.30, 1.00];
    let best = rows.iter().map(|(_, r)| *r).fold(0.0f32, f32::max);
    let scale = SCALE_STEPS.iter().copied().find(|s| best <= *s).unwrap_or(1.0);
    // Two significant figures wherever the value happens to sit: a band down to
    // its last cell is worth a hundredth of a per cent, and rounding that to
    // "0.0 %" would print a band that is open as a band that is not.
    let pct = |v: f32| {
        let v = v * 100.0;
        match v {
            v if v >= 10.0 => format!("{v:.0} %"),
            v if v >= 1.0 => format!("{v:.1} %"),
            v => format!("{v:.2} %"),
        }
    };

    // The narrowest the bars are allowed to be before the box grows for them.
    const MIN_BAR_W: f32 = 74.0;
    const BAR_H: f32 = 7.0;
    let font = egui::FontId::proportional(11.5);
    let small = egui::FontId::proportional(10.0);
    // Laid out before a place for it is reserved, for the reason the two panels
    // above it do the same.
    let p = ui.painter().clone();
    let laid: Vec<_> = rows
        .iter()
        .map(|(band, reach)| {
            let c = crate::colormap::band_color(*band);
            let hue = egui::Color32::from_rgb(c[0], c[1], c[2]);
            (
                p.layout_no_wrap(band.label().into(), small.clone(), hue),
                p.layout_no_wrap(pct(*reach), font.clone(), theme::CYAN()),
                hue,
                *reach,
            )
        })
        .collect();
    let key_w = laid.iter().map(|(k, ..)| k.size().x).fold(0.0f32, f32::max);
    let val_w = laid.iter().map(|(_, v, ..)| v.size().x).fold(0.0f32, f32::max);
    let row_h = laid.iter().map(|(_, v, ..)| v.size().y + 3.0).fold(BAR_H + 5.0, f32::max);

    let title = p.layout_no_wrap(crate::language_plugin::text("solar.overlay.dynamic.text_2411_32f4cc", "BANDS OPEN").into(), small.clone(), theme::CYAN_DIM());
    let notes = [
        { let __lp_arg_0 = &(pct(scale)); crate::language_plugin::format("solar.overlay.dynamic.text_2413_a1b1ac", "full bar = {} of the world", &[format!("{}", __lp_arg_0)]) },
        { let __lp_arg_0 = &(st.prop.halflife_s / 60.0); crate::language_plugin::format("solar.overlay.dynamic.text_2415_a07afb", "where this station's own paths went · {:.0} min memory", &[format!("{:.0}", __lp_arg_0)]) },
    ]
    .map(|t| p.layout_no_wrap(t, small.clone(), theme::LINE_LIT()));

    let pad = 10.0;
    // The footer is the longest line in the box and always has been, so rather
    // than leave the chart short in a box sized by a sentence, the bars take
    // whatever width that sentence bought.
    let gap = 8.0;
    let width = (key_w + MIN_BAR_W + val_w + gap * 2.0 + 4.0)
        .max(title.size().x)
        .max(notes.iter().map(|n| n.size().x).fold(0.0f32, f32::max))
        + pad * 2.0;
    let height = title.size().y
        + 5.0
        + rows.len() as f32 * row_h
        + notes.iter().map(|n| n.size().y + 3.0).sum::<f32>()
        + 3.0
        + pad * 2.0;
    let panel = place.reserve(ui, egui::vec2(width, height))?;

    p.rect_filled(panel, 0, theme::FILL().gamma_multiply(0.82));
    chrome::paint_cut_border(&p, panel, theme::LINE_LIT(), egui::Color32::TRANSPARENT);

    let mut y = panel.top() + pad;
    let title_h = title.size().y;
    p.galley(egui::pos2(panel.left() + pad, y), title, theme::CYAN_DIM());
    y += title_h + 5.0;
    let bar_x = panel.left() + pad + key_w + gap;
    let bar_w = (panel.right() - pad - val_w - gap - bar_x).max(MIN_BAR_W);
    for (key, val, hue, reach) in laid {
        let key_y = y + (row_h - key.size().y) * 0.5;
        p.galley(egui::pos2(panel.left() + pad, key_y), key, hue);
        // An unlit socket behind every bar, the same as the aurora forecast
        // strip: without it a quiet band reads as missing rather than as quiet.
        let top = y + (row_h - BAR_H) * 0.5;
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(bar_x, top), egui::vec2(bar_w, BAR_H)),
            0,
            theme::LINE().gamma_multiply(0.55),
        );
        // The band's own hue: what the combined view paints it in on the globe,
        // and what the legend names it by. The chart and the map share one key,
        // so neither has to be learned twice.
        let w = (reach / scale).clamp(0.0, 1.0) * bar_w;
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(bar_x, top), egui::vec2(w.max(1.0), BAR_H)),
            0,
            hue,
        );
        p.galley(
            egui::pos2(panel.right() - pad - val.size().x, y + (row_h - val.size().y) * 0.5),
            val,
            theme::CYAN(),
        );
        y += row_h;
    }
    y += 3.0;
    for n in notes {
        let h = n.size().y + 3.0;
        p.galley(egui::pos2(panel.left() + pad, y), n, theme::LINE_LIT());
        y += h;
    }
    Some(panel)
}

/// The award layer's key, bottom right: what each colour on the globe means,
/// and how many entities are in each state.
///
/// A heat map with no legend is decoration. This one is small, and it only
/// exists while the layer it explains is switched on. `bottom` is what the date
/// readout in the same corner left of it.
fn award_panel(ui: &egui::Ui, st: &SolarUi, rect: egui::Rect, bottom: f32) {
    if !st.layer(layer::AWARDS) || st.awards.is_empty() {
        return;
    }
    let (missing, worked, confirmed) = sdroxide_types::coverage_counts(&st.awards);
    let rows = [
        (crate::language_plugin::text("solar.overlay.dynamic.text_2495_ffa635", "missing"), missing, egui::Color32::from_rgb(0xff, 0x5a, 0x28)),
        (crate::language_plugin::text("solar.overlay.dynamic.text_2496_ba0f95", "worked"), worked, theme::YELLOW()),
        (crate::language_plugin::text("solar.overlay.dynamic.text_2497_959991", "confirmed"), confirmed, theme::GREEN()),
    ];

    let p = ui.painter();
    let font = egui::FontId::proportional(11.5);
    let cap = egui::FontId::proportional(9.5);
    let galleys: Vec<_> = rows
        .iter()
        .map(|(label, n, _)| p.layout_no_wrap(format!("{label}  {n}"), font.clone(), theme::TEXT()))
        .collect();
    let title = p.layout_no_wrap(crate::language_plugin::text("solar.overlay.dynamic.text_2507_cb1005", "DXCC COVERAGE").into(), cap, theme::CYAN_DIM());

    const SWATCH: f32 = 9.0;
    let w = galleys.iter().map(|g| g.size().x).fold(title.size().x, f32::max) + SWATCH + 26.0;
    let h = galleys.iter().map(|g| g.size().y + 3.0).sum::<f32>() + title.size().y + 18.0;
    let panel = bottom_right(rect, bottom, egui::vec2(w, h));
    if !rect.contains_rect(panel) {
        return; // too small a window to be worth crowding
    }
    p.rect_filled(panel, 0, theme::FILL().gamma_multiply(0.82));
    chrome::paint_cut_border(p, panel, theme::LINE_LIT(), egui::Color32::TRANSPARENT);

    let mut y = panel.top() + 7.0;
    let x = panel.left() + 10.0;
    let title_h = title.size().y;
    p.galley(egui::pos2(x, y), title, theme::CYAN_DIM());
    y += title_h + 4.0;
    for (g, (_, _, color)) in galleys.into_iter().zip(rows) {
        let dy = g.size().y + 3.0;
        p.circle_filled(egui::pos2(x + SWATCH * 0.5, y + g.size().y * 0.5), SWATCH * 0.42, color);
        p.galley(egui::pos2(x + SWATCH + 8.0, y), g, theme::TEXT());
        y += dy;
    }
}

/// The banner that justifies the whole window: a CME whose cone contains the
/// Earth, with an arrival estimate.
/// One line saying what the cloud deck is, and what about it is not measured.
///
/// The hour it shows is the hour the mosaic is *of*, which is over an hour
/// behind the clock — the same discipline the aurora footer keeps, and for the
/// same reason: a picture presented as current when it is not is worse than no
/// picture. And it says the lightning is simulated, which is not optional. The
/// storms are real and come out of the imagery; the individual flashes are
/// invented, and a globe that flickers with plausible-looking strikes has to
/// admit that rather than let anyone read them as strikes.
fn clouds_note(ui: &egui::Ui, st: &SolarUi, data: Option<&SolarData>, rect: egui::Rect, now: i64) {
    if !st.layer(layer::CLOUDS) {
        return;
    }
    let Some(field) = data.and_then(|d| d.clouds.as_ref()) else { return };

    let channels = if field.has_visible { crate::language_plugin::text("solar.overlay.dynamic.text_2549_35ceb8", "IR+VIS") } else { crate::language_plugin::text("solar.overlay.dynamic.text_2549_48eebe", "IR only") };
    let storms = match field.cells.len() {
        0 => crate::language_plugin::text("solar.overlay.dynamic.text_2551_371d34", "no deep convection").to_string(),
        1 => crate::language_plugin::text("solar.overlay.dynamic.text_2552_777001", "1 storm").to_string(),
        n => crate::language_plugin::format("solar.overlay.dynamic.text_2553_72f4a5", "{n} storms", &[format!("{n}")]),
    };
    let text = { let __lp_arg_0 = &(timefmt::ymd_hm(field.frame_unix)); let __lp_arg_1 = &(crate::language_plugin::solar_age((now - field.frame_unix).max(0))); crate::language_plugin::format("solar.overlay.dynamic.text_2556_f28786", "CLOUDS  {}  ·  {} old  ·  {channels}  ·  {storms}  ·  lightning simulated", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1), format!("{channels}"), format!("{storms}")]) };

    let p = ui.painter();
    let galley = p.layout_no_wrap(text, egui::FontId::proportional(10.5), theme::scope().line);
    if galley.size().x > rect.width() - 40.0 {
        return;
    }
    p.galley(
        egui::pos2(rect.left() + 14.0, rect.bottom() - galley.size().y - 8.0),
        galley,
        theme::scope().line,
    );
}

fn impact_banner(ui: &egui::Ui, data: Option<&SolarData>, rect: egui::Rect, now: i64) {
    let Some(d) = data else { return };
    // The soonest arrival that has not already happened.
    let mut best: Option<(&sdroxide_solar::CmeEvent, sdroxide_solar::Impact)> = None;
    for e in &d.cmes {
        let Some(a) = &e.analysis else { continue };
        let Some(hit) = sdroxide_solar::earth_impact(a) else { continue };
        // Keep it on screen for a few hours past the estimate, since arrival
        // estimates are routinely off by that much.
        if hit.eta_unix < now - 6 * 3600 {
            continue;
        }
        if best.as_ref().is_none_or(|(_, b)| hit.eta_unix < b.eta_unix) {
            best = Some((e, hit));
        }
    }
    let Some((event, hit)) = best else { return };
    let a = event.analysis.as_ref().expect("filtered above");

    let hours = (hit.eta_unix - now) as f64 / 3600.0;
    let when = if hours >= 0.0 {
        { let __lp_arg_0 = &(timefmt::ymd_hm(hit.eta_unix)); crate::language_plugin::format("solar.overlay.dynamic.text_2594_5d0b83", "ETA {} (+{hours:.0} h)", &[format!("{}", __lp_arg_0), format!("{hours:.0}")]) }
    } else {
        { let __lp_arg_0 = &(timefmt::ymd_hm(hit.eta_unix)); let __lp_arg_1 = &(-hours); crate::language_plugin::format("solar.overlay.dynamic.text_2596_221ca7", "arrival was {} ({:.0} h ago)", &[format!("{}", __lp_arg_0), format!("{:.0}", __lp_arg_1)]) }
    };
    let glancing = if hit.directness(a.half_angle_deg) < 0.35 { crate::language_plugin::text("solar.overlay.dynamic.text_2598_85096d", " · glancing") } else { String::new() };
    let estimated = if a.estimated { crate::language_plugin::text("solar.overlay.dynamic.text_2599_34e51b", " · direction estimated") } else { String::new() };
    let text = { let __lp_arg_0 = &(timefmt::ymd_hm(a.t21_5_unix)); let __lp_arg_1 = &(a.speed_km_s); crate::language_plugin::format("solar.overlay.dynamic.text_2601_5ed73c", "EARTH-DIRECTED CME  {}  ·  {:.0} km/s  ·  {when}{glancing}{estimated}", &[format!("{}", __lp_arg_0), format!("{:.0}", __lp_arg_1), format!("{when}"), format!("{glancing}"), format!("{estimated}")]) };

    // Hazard-striped tabs at both ends, the same mark the manual uses on every
    // section header. A CME arrival is the one thing in this window that wants
    // to be noticed without being read first.
    const TAB_W: f32 = 22.0;
    let font = egui::FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(text, font, theme::TEXT_STRONG());
    let size = galley.size() + egui::vec2(28.0 + TAB_W * 2.0, 15.0);
    if size.x > rect.width() - 24.0 {
        return;
    }
    let banner = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.bottom() - size.y * 0.5 - 14.0),
        size,
    );
    let p = ui.painter();
    p.rect_filled(banner, 0, theme::CQ_BG().gamma_multiply(0.94));
    for tab in [
        egui::Rect::from_min_size(banner.min, egui::vec2(TAB_W, banner.height())),
        egui::Rect::from_min_size(
            egui::pos2(banner.right() - TAB_W, banner.top()),
            egui::vec2(TAB_W, banner.height()),
        ),
    ] {
        chrome::hazard_stripes(p, tab, 7.0);
    }
    chrome::paint_cut_border(p, banner, theme::PINK(), egui::Color32::TRANSPARENT);
    p.galley(
        egui::pos2(banner.left() + TAB_W + 14.0, banner.top() + 7.0),
        galley,
        theme::TEXT_STRONG(),
    );
}

/// Fly the AUTO tour, using real elapsed time so the pacing is frame-rate
/// independent.
///
/// `data` is the frame's snapshot, passed in because the caller already holds
/// the [`SolarData`] guard for the whole pass — taking the mutex again here
/// would be this thread waiting on itself.
fn advance_tour(ui: &egui::Ui, st: &mut SolarUi, data: Option<&SolarData>, sim_now: f64, dt: f32) {
    if !st.view.auto {
        // Hand the pivot back to the focus chips.
        st.focus_override = None;
        return;
    }
    let b = super::scene::bodies(st, sim_now);
    // The contact being worked pre-empts the tour — but only while its arc is
    // actually on screen: with the layer switched off there is nothing there
    // for the camera to fly to.
    let qso = st
        .layer(crate::view::solar_layer::QSO)
        .then(|| st.qth.zip(st.digi.dx))
        .flatten()
        .map(|(home, dx)| super::camera::QsoPath { home, dx });
    // The satellite lock outranks the contact, under the same layer rule: the
    // camera only chases what is actually drawn.
    let sat = st
        .layer(crate::view::solar_layer::SATS)
        .then(|| st.sat_lock.zip(st.qth))
        .flatten()
        .and_then(|(id, home)| {
            let state = data?.satellites().find(|s| s.norad_id == id)?.at(sim_now)?;
            Some(super::camera::SatPath {
                home,
                sat_world: b.earth
                    + super::math::V3::from_f64(state.dir_ecliptic)
                        * (b.earth_r * state.radii as f32),
            })
        });
    // `Tour` is `Copy`, so step a local and write it back rather than fighting
    // the borrow of `st.view` inside it.
    let mut tour = st.tour;
    let pivot = tour.step(&mut st.view, &b, dt, qso, sat, st.qth);
    st.tour = tour;
    st.focus_override = Some(pivot);
    crate::repaint::animate(ui.ctx());
}

/// Drag to rotate, scroll or pinch to zoom, double-click to reframe. Any of them
/// cancels the animated tour — the user taking the controls is the signal to
/// stop.
fn interact(ui: &egui::Ui, st: &mut SolarUi, resp: &egui::Response) {
    let mut touched = false;

    // Two fingers on the scene zoom it: on a screen with no wheel there is no
    // other way to change the camera distance.
    //
    // First in the chain, and it swallows the drag, for the reason the
    // panadapter's pinch does: the browser keeps reporting a one-finger drag
    // from the *first* finger down for the whole gesture, so without this a
    // pinch would spin the camera at the same time as it zoomed.
    let pinch = ui.input(|i| i.multi_touch()).filter(|mt| resp.rect.contains(mt.center_pos));
    if let Some(mt) = pinch {
        if mt.zoom_delta > 0.0 {
            // Fingers apart is zoom *in*, i.e. a shorter distance to the focus.
            // Multiplicative like the wheel, so a gesture covers the same visual
            // fraction whether you are 3 Gm or 3 AU out.
            st.view.dist /= mt.zoom_delta;
            touched |= mt.zoom_delta != 1.0;
        }
    } else if resp.dragged_by(egui::PointerButton::Primary) {
        let d = resp.drag_delta();
        st.view.yaw -= d.x * 0.006;
        st.view.pitch = (st.view.pitch + d.y * 0.006)
            .clamp(-super::camera::PITCH_LIMIT, super::camera::PITCH_LIMIT);
        touched |= d != egui::Vec2::ZERO;
    }

    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            // Multiplicative, so a wheel click covers the same visual fraction
            // whether you are 3 Gm or 3 AU out.
            st.view.dist *= (1.0 - scroll * 0.0022).clamp(0.4, 2.5);
            touched = true;
        }
    }

    // Clamping needs the focus radius, which only the scene knows; `Camera`
    // re-clamps anyway, so here just keep the stored value sane.
    st.view.dist = st.view.dist.clamp(1e-5, super::camera::MAX_DIST);

    if touched {
        st.view.auto = false;
    }
    // Continuous repaint only while something is actually moving; otherwise the
    // window idles and is woken by input or by the data feed.
    if touched || st.view.auto || resp.is_pointer_button_down_on() {
        crate::repaint::animate(ui.ctx());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The date box sits in the corner itself, and the award key stacks on top
    /// of whatever it left rather than over it.
    #[test]
    fn the_bottom_right_stack_grows_upwards_from_the_corner() {
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let date = bottom_right(view, view.bottom() - MARGIN, egui::vec2(120.0, 40.0));
        assert_eq!(date.right(), 788.0);
        assert_eq!(date.bottom(), 588.0);

        let award = bottom_right(view, date.top() - 8.0, egui::vec2(140.0, 60.0));
        assert_eq!(award.right(), 788.0, "both boxes hang off the same edge");
        assert!(award.bottom() <= date.top(), "the key must not sit over the date");
    }

    /// Half a second a line, whatever the line says — which is the whole of the
    /// timing the operator asked for.
    #[test]
    fn the_contact_card_types_one_line_every_half_second() {
        // Nothing at all before the card opens.
        assert_eq!(typed_chars(0.0, 0, 10), None);
        // The first line starts with a character rather than a bare cursor.
        assert_eq!(typed_chars(0.001, 0, 10), Some(1));
        assert_eq!(typed_chars(0.25, 0, 10), Some(5));
        assert_eq!(typed_chars(0.5, 0, 10), Some(10));

        // The second line has not begun while the first is still going, and is
        // finished half a second after it starts.
        assert_eq!(typed_chars(0.4, 1, 10), None);
        assert_eq!(typed_chars(0.75, 1, 10), Some(5));
        assert_eq!(typed_chars(1.0, 1, 10), Some(10));

        // Length does not change the beat: a long line and a short one both
        // finish on their own half-second.
        assert_eq!(typed_chars(1.5, 2, 40), Some(40));
        assert_eq!(typed_chars(1.5, 2, 3), Some(3));
        // …and neither ever overruns its own length.
        assert_eq!(typed_chars(99.0, 0, 7), Some(7));

        // A nine-line card is fully typed in four and a half seconds.
        assert_eq!(typed_chars(9.0 * CARD_LINE_S, 8, 12), Some(12));
    }

    /// The card follows the apex, and the apex is the point most likely to be
    /// near the top of the view — so "no room above" has to mean "go below",
    /// not "vanish".
    #[test]
    fn the_contact_card_flips_below_the_apex_rather_than_disappearing() {
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        let size = egui::vec2(160.0, 130.0);

        // Room above: the card sits over the apex, centred on it.
        let (card, above) = card_rect(view, egui::pos2(400.0, 400.0), size).unwrap();
        assert!(above);
        assert_eq!(card.center().x, 400.0);
        assert_eq!(card.bottom(), 400.0 - CARD_LEADER_PX);

        // Apex near the top: the card goes under it instead, still centred.
        let (card, above) = card_rect(view, egui::pos2(400.0, 20.0), size).unwrap();
        assert!(!above);
        assert_eq!(card.center().x, 400.0);
        assert_eq!(card.top(), 20.0 + CARD_LEADER_PX);

        // Apex against the left edge: slid inside rather than dropped.
        let (card, _) = card_rect(view, egui::pos2(4.0, 400.0), size).unwrap();
        assert_eq!(card.left(), 0.0);
        assert!(view.contains_rect(card));
        // …and the right edge behaves the same way.
        let (card, _) = card_rect(view, egui::pos2(796.0, 400.0), size).unwrap();
        assert_eq!(card.right(), 800.0);
        assert!(view.contains_rect(card));

        // A view too short for the card either way gets none.
        let squat = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 120.0));
        assert!(card_rect(squat, egui::pos2(400.0, 60.0), size).is_none());
    }

    #[test]
    fn a_running_contact_shows_a_clock_that_grows_a_field_at_the_hour() {
        assert_eq!(elapsed_hms(0), "0:00");
        assert_eq!(elapsed_hms(9), "0:09");
        assert_eq!(elapsed_hms(84), "1:24");
        assert_eq!(elapsed_hms(3599), "59:59");
        assert_eq!(elapsed_hms(3600), "1:00:00");
        assert_eq!(elapsed_hms(3661), "1:01:01");
    }
}

// Localized clock captions use the bundled text font for Unicode.
fn clock_label_size(p: &egui::Painter, text: &str, pitch: f32) -> egui::Vec2 {
    if text.is_ascii() { return super::dotmatrix::size(text, pitch); }
    p.layout_no_wrap(text.to_owned(), egui::FontId::proportional((7.0 * pitch).max(9.0)), egui::Color32::WHITE).size()
}

fn clock_label_draw(p: &egui::Painter, origin: egui::Pos2, text: &str, pitch: f32, on: egui::Color32, off: egui::Color32) -> egui::Rect {
    if text.is_ascii() { return super::dotmatrix::draw(p, origin, text, pitch, on, off); }
    let galley = p.layout_no_wrap(text.to_owned(), egui::FontId::proportional((7.0 * pitch).max(9.0)), on);
    let rect = egui::Rect::from_min_size(origin, galley.size());
    p.galley(origin, galley, on);
    rect
}

#[cfg(test)]
mod localized_clock_tests {
    use super::*;

    #[test]
    fn chinese_clock_captions_produce_text_shapes_and_matching_bounds() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        crate::language_plugin::add_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                for text in ["本地", "-- 模拟"] {
                    let size = clock_label_size(ui.painter(), text, 1.2);
                    let drawn = clock_label_draw(ui.painter(), egui::pos2(10.0,10.0), text, 1.2, egui::Color32::WHITE, egui::Color32::TRANSPARENT);
                    assert_eq!(drawn.size(), size);
                    assert!(size.x > 0.0 && size.y >= 9.0);
                }
                assert_eq!(clock_label_size(ui.painter(), "UTC", 1.2), super::super::dotmatrix::size("UTC", 1.2));
        });
        let captions: Vec<_> = out.shapes.iter().filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape { Some(text.galley.job.text.as_str()) } else { None }
        }).collect();
        assert!(captions.contains(&"本地") && captions.contains(&"-- 模拟"));
        out.textures_delta.clear();
    }
}
