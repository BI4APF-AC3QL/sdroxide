//! What each band is doing: the published forecast beside the measured field.
//!
//! Two answers to one question, and they are not the same answer.
//!
//! * **CONDX** is N0NBH's calculated verdict — one word per band group per half
//!   of the day, computed globally from the solar indices. It says nothing
//!   about this station, this antenna or any particular path, and it covers
//!   80 m through 10 m and nothing else.
//! * **PATHS**, **REACH** and **BEST** come from the propagation field: real
//!   receptions, by this station and — when the Reverse Beacon Network is
//!   switched on — by everyone else's. That is a measurement.
//!
//! PATHS and REACH are both shown because either alone misleads: a contest
//! pile-up on one bearing is a great many paths through a very small piece of
//! sky, and a band that is quietly open everywhere is the reverse.
//!
//! Where they disagree the measurement is right, which is why they are shown
//! side by side rather than reconciled into one number.
//!
//! The empty cell is load-bearing throughout. A band with no verdict has none
//! published; a band with no paths was either shut or unattended, and this
//! window does not claim to know which.

use eframe::egui::{self, Color32, RichText};
use sdroxide_solar::BandRating;
use sdroxide_types::Band;

use crate::app::SdroxideApp;

/// Column headings and everything the reader is not meant to look at first —
/// the same grey the propagation chip row uses for its scale caption.
fn dim_ink() -> Color32 {
    crate::theme::gray(110)
}

/// The colour a verdict is shown in.
///
/// `None` for a band nothing is published about, and for a wording this build
/// does not recognise — in both cases the neutral text colour is the honest
/// one, and the words are printed either way.
pub(in crate::app) fn rating_color(r: BandRating) -> Option<Color32> {
    match r {
        BandRating::Good => Some(crate::theme::GREEN()),
        BandRating::Fair => Some(crate::theme::YELLOW()),
        BandRating::Poor => Some(crate::theme::PINK()),
        BandRating::Closed => Some(dim_ink()),
        BandRating::Unknown => None,
    }
}

/// How old the verdicts are, as words, or `None` if there are none.
pub(in crate::app) fn conditions_age(app: &SdroxideApp) -> Option<String> {
    let c = app.band_conditions.as_ref()?;
    if c.observed_unix <= 0 {
        return None;
    }
    Some(sdroxide_solar::timefmt::age(crate::time::now_unix() - c.observed_unix))
}

impl SdroxideApp {
    /// The BANDS window: one row per band, forecast beside evidence.
    pub(in crate::app) fn bands_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_bands;
        let resp = egui::Window::new(crate::language_plugin::text("window.bands.text_64_c41902", "BANDS")).id(egui::Id::new("BANDS"))
            .id(crate::layout::salted_id(ctx, "BANDS"))
            .open(&mut open)
            .frame(crate::chrome::window_frame())
            .resizable(true)
            .default_width(crate::layout::window_w(ctx, 520.0))
            .default_height(crate::layout::window_h(ctx, 460.0))
            .show(ctx, |ui| {
                crate::chrome::window_body_bg(ui);
                self.bands_body(ui)
            });
        if let Some(r) = &resp {
            crate::chrome::paint_window_border(ctx, &r.response);
        }
        self.show_bands = open;
        if self.show_bands {
            // The IBP beacon and its countdown move every second; keep painting
            // while the window is open rather than showing a stale slot.
            crate::repaint::after_ms(ctx, 1000);
        }
    }

    fn bands_body(&mut self, ui: &mut egui::Ui) {
        let field = self.prop.peek();
        let dim = |s: &str| RichText::new(s).size(9.5).color(dim_ink());

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if self.daylight { crate::language_plugin::text("window.bands.text_92_12d2da", "☀ DAYLIGHT") } else { crate::language_plugin::text("window.bands.text_92_cebbad", "☾ NIGHT") })
                    .size(10.0)
                    .color(crate::theme::CYAN_DIM()),
            );
            match conditions_age(self) {
                Some(a) => {
                    ui.label(dim(&crate::language_plugin::format("window.bands.text_98_ad3a41", "· forecast from HAMQSL.com, {a} old", &[format!("{a}")])));
                }
                None => {
                    ui.label(dim(&crate::language_plugin::text("window.bands.text_101_8195b4", "· no forecast yet — fetching from HAMQSL.com")));
                }
            }
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("bands_grid").num_columns(5).spacing([14.0, 3.0]).striped(true).show(
                ui,
                |ui| {
                    ui.label(dim(&crate::language_plugin::text("window.bands.text_111_52f1b5", "BAND")));
                    ui.label(dim(&crate::language_plugin::text("window.bands.text_112_588595", "CONDX")));
                    ui.label(dim(&crate::language_plugin::text("window.bands.text_113_0c34a0", "PATHS")));
                    ui.label(dim(&crate::language_plugin::text("window.bands.text_114_48cef9", "REACH")));
                    ui.label(dim(&crate::language_plugin::text("window.bands.text_115_16b794", "BEST")));
                    ui.end_row();

                    for b in Band::ALL {
                        if b == Band::Gen {
                            continue;
                        }
                        ui.label(RichText::new(b.label()).size(11.0).strong());

                        // The forecast. Blank where none is published — see the
                        // module note; 160 m, 60 m and everything above 10 m
                        // live here permanently.
                        match self
                            .band_conditions
                            .as_ref()
                            .and_then(|c| c.for_band(b, self.daylight))
                        {
                            Some(v) => {
                                let t = RichText::new(v).size(10.5);
                                ui.label(match rating_color(BandRating::of(v)) {
                                    Some(c) => t.color(c),
                                    None => t,
                                });
                            }
                            None => {
                                ui.label(dim("—"));
                            }
                        }

                        // The evidence.
                        match field.plane(b).filter(|p| !p.is_empty()) {
                            Some(p) => {
                                ui.label(
                                    RichText::new(format!("{:.0}", p.total_paths())).size(10.5),
                                );
                                ui.label(
                                    RichText::new(format!("{:.0}%", p.reach() * 100.0)).size(10.5),
                                );
                                ui.label(match p.best_margin_db() {
                                    Some(m) => RichText::new(format!("{m:+.0} dB")).size(10.5),
                                    // Paths but no reports: a plane built from
                                    // logged contacts alone. An RST is not an
                                    // SNR, so there is no number to print.
                                    None => dim("—"),
                                });
                            }
                            None => {
                                // Not "closed": nothing was heard, and nobody
                                // may have been transmitting. The two look
                                // identical from here and must not be conflated.
                                ui.label(dim("—"));
                                ui.label(dim("—"));
                                ui.label(dim("—"));
                            }
                        }
                        ui.end_row();
                    }
                },
            );
            self.ibp_section(ui);
            self.meteor_section(ui);
        });

        ui.add_space(6.0);
        ui.label(
            dim(&crate::language_plugin::text("window.bands.text_180_baf17e", "CONDX is a global forecast; the rest is what has actually been heard. \
                 An empty row means nothing was decoded — which may mean the band was \
                 shut, or only that nobody was on it."))
            .italics(),
        );
    }

    /// The IBP beacon list at the foot of the BANDS window.
    ///
    /// The schedule is deterministic — eighteen beacons, five bands, a
    /// three-minute cycle — so this is a clock and a table rather than a feed.
    /// A beacon you can hear is a path that is open, measured rather than
    /// forecast.
    fn ibp_section(&self, ui: &mut egui::Ui) {
        let now = crate::time::now_unix();
        let home = sdroxide_types::grid_to_latlon(&self.my_grid());
        let active = sdroxide_types::ibp_active_at(now, home);
        let dim = |s: &str| RichText::new(s.to_string()).size(9.5).color(dim_ink());
        ui.add_space(12.0);
        ui.label(RichText::new(crate::language_plugin::text("window.bands.text_199_e3d250", "IBP BEACONS")).size(10.0).strong().color(crate::theme::CYAN_DIM()));
        ui.add_space(2.0);
        ui.label(dim(&{ let __lp_arg_0 = &(sdroxide_types::ibp_seconds_left(now)); crate::language_plugin::format("window.bands.text_202_6329c0", "NCDXF/IARU · next beacon in {} s", &[format!("{}", __lp_arg_0)]) }));
        ui.add_space(3.0);
        for a in &active {
            ui.horizontal(|ui| {
                ui.label(RichText::new(a.band.label).size(10.5).strong());
                ui.label(RichText::new(format!("{:.3} MHz", a.band.freq_hz / 1e6)).size(10.5));
                ui.label(RichText::new(a.beacon.callsign).size(11.0).strong());
                ui.label(dim(a.beacon.location));
                if let (Some(b), Some(d)) = (a.bearing_deg, a.distance_km) {
                    ui.label(dim(&format!(
                        "{} · {:.0} km",
                        crate::language_plugin::compass(b),
                        d
                    )));
                }
            })
            .response
            .on_hover_text({ let __lp_arg_0 = &(a.beacon.callsign); let __lp_arg_1 = &(a.beacon.location); let __lp_arg_2 = &(a.band.label); crate::language_plugin::format("window.bands.text_222_45f514", "{} at {} — hearing it means the {} path is open. Each beacon steps up a \
                 band every 10 s, so this row changes every slot.", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1), format!("{}", __lp_arg_2)]) });
        }
    }

    /// The meteor-shower list at the foot of the BANDS window.
    ///
    /// The one propagation forecast that has nothing to do with the ionosphere:
    /// a shower's radiant and its date window are fixed, so what is shown is
    /// whether it is active now, how strong its peak is, and whether the radiant
    /// is above this station's horizon. Radiants are placed for the operator's
    /// own locator, which is what makes "is it up" true here rather than on
    /// average.
    fn meteor_section(&self, ui: &mut egui::Ui) {
        let Some((lat, lon)) = sdroxide_types::grid_to_latlon(&self.my_grid()) else {
            return;
        };
        let active = sdroxide_solar::active_at(lat, lon, crate::time::now_unix());
        if active.is_empty() {
            return;
        }
        let dim = |s: &str| RichText::new(s.to_string()).size(9.5).color(dim_ink());
        ui.add_space(12.0);
        ui.label(
            RichText::new(crate::language_plugin::text("window.bands.text_248_3facc2", "METEOR SHOWERS")).size(10.0).strong().color(crate::theme::CYAN_DIM()),
        );
        ui.add_space(2.0);
        ui.label(dim(&crate::language_plugin::format("window.bands.text_251_f8d02e", "radiants placed for {lat:.0}°, {lon:.0}°", &[format!("{lat:.0}"), format!("{lon:.0}")])));
        ui.add_space(3.0);
        for a in &active {
            let s = a.shower;
            ui.horizontal(|ui| {
                ui.label(RichText::new(s.name).size(11.0).strong());
                ui.label(dim(s.code));
                ui.label(RichText::new({ let __lp_arg_0 = &(s.zhr); crate::language_plugin::format("window.bands.text_258_9f216b", "ZHR {}", &[format!("{}", __lp_arg_0)]) }).size(10.5));
                if a.at_peak() {
                    ui.label(RichText::new(crate::language_plugin::text("window.bands.text_260_c797f4", "PEAK")).size(9.5).strong().color(crate::theme::GREEN()));
                }
                if a.radiant_up() {
                    ui.label(
                        RichText::new({ let __lp_arg_0 = &(a.alt_deg); let __lp_arg_1 = &(crate::language_plugin::compass(a.az_deg)); crate::language_plugin::format("window.bands.text_265_4db28a", "radiant {:.0}° {}", &[format!("{:.0}", __lp_arg_0), format!("{}", __lp_arg_1)]) })
                        .size(10.5),
                    );
                } else {
                    ui.label(
                        RichText::new({ let __lp_arg_0 = &(a.alt_deg); crate::language_plugin::format("window.bands.text_273_392da8", "radiant down ({:.0}°)", &[format!("{:.0}", __lp_arg_0)]) })
                            .size(10.5)
                            .color(dim_ink()),
                    );
                }
            })
            .response
            .on_hover_text({ let __lp_arg_0 = &(s.name); let __lp_arg_1 = &(s.code); let __lp_arg_2 = &(s.velocity_kms); let __lp_arg_3 = &(s.parent); let __lp_arg_4 = &(s.zhr); let __lp_arg_5 = &(peak_label(s.peak)); crate::language_plugin::format("window.bands.text_281_63c8cc", "{} ({}) — {} km/s, parent {}. Peak rate ZHR {} around {}.\n\nA radiant \
                 above the horizon means the trails can reach you; a fast shower leaves \
                 longer-lived ionised trails for meteor scatter on 6 m and 2 m, and the \
                 brief 10 m openings.", &[format!("{}", __lp_arg_0), format!("{}", __lp_arg_1), format!("{}", __lp_arg_2), format!("{}", __lp_arg_3), format!("{}", __lp_arg_4), format!("{}", __lp_arg_5)]) });
        }
    }
}

/// "3 Jan" from a `(month, day)` pair, for the hover.
fn peak_label(peak: (u32, u32)) -> String {
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let name = MONTHS.get(peak.0.saturating_sub(1) as usize).copied().unwrap_or("?");
    crate::language_plugin::month_day(peak.0, peak.1, format!("{} {name}", peak.1))
}
