//! The awards dashboard: DXCC, WAS, WAZ and grid tallies over the logbook.
//!
//! The tally is expensive enough to cache (keyed by log length and the band
//! filter) and is shared with the 3D view's award layer by `Arc` rather than
//! copied, because the window republishes it every frame.

// The globe's award layer is native-only, and it is the sole user here.
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use eframe::egui::{self, RichText};

use crate::theme::ThemedScroll;

use crate::app::SdroxideApp;

/// One "N worked / M confirmed" summary line for an award map.
fn award_summary<K>(
    ui: &mut egui::Ui,
    name: impl AsRef<str>,
    map: &std::collections::BTreeMap<K, sdroxide_types::AwardStatus>,
) {
    let name = name.as_ref();
    let (w, c) = sdroxide_types::counts(map);
    ui.horizontal(|ui| {
        ui.add_sized([90.0, 20.0], egui::Label::new(RichText::new(name).strong()));
        ui.label(RichText::new(crate::language_plugin::format("window.awards.text_26_45ee6d", "{w} worked", &[format!("{w}")])).color(crate::theme::YELLOW()).monospace());
        ui.label(RichText::new(crate::language_plugin::format("window.awards.text_27_d31643", "{c} confirmed", &[format!("{c}")])).color(crate::theme::GREEN()).monospace());
    });
}

/// A wrapping grid of fixed-width award cells: grey = not worked, amber =
/// worked, green = confirmed.
fn award_cell_grid(
    ui: &mut egui::Ui,
    items: impl Iterator<Item = (String, sdroxide_types::AwardStatus)>,
    cell_w: f32,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for (label, st) in items {
            let (bg, fg) = if st.confirmed {
                (crate::theme::GREEN(), crate::theme::INK_ON_BRIGHT())
            } else if st.worked {
                (crate::theme::YELLOW(), crate::theme::INK_ON_BRIGHT())
            } else {
                (crate::theme::gray(38), crate::theme::gray(110))
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(cell_w, 20.0), egui::Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 3.0, bg);
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(11.0),
                fg,
            );
        }
    });
}

impl SdroxideApp {
    /// The cached award tally for the current band filter (recomputed when the
    /// log length or the band filter changes).
    pub(in crate::app) fn ensure_awards(&mut self) {
        let len = self.qso_log.len();
        let band = self.awards_band.clone();
        let stale =
            self.awards_cache.as_ref().map(|(l, b, _, _)| *l != len || *b != band).unwrap_or(true);
        if stale {
            let filter = (!band.is_empty()).then_some(band.as_str());
            let awards = sdroxide_types::compute_awards(&self.qso_log, filter, None);
            let builtins = crate::language_plugin::award_builtin_names(&self.qso_log, filter);
            self.awards_cache = Some((len, band, awards, builtins));
        }
    }

    /// Award coverage placed on the globe, for the 3D view's award layer. Built
    /// from the same tally the dashboard shows and cached the same way, so the
    /// two can never tell different stories about the same log.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn award_heat(&mut self) -> Arc<Vec<sdroxide_types::EntitySlot>> {
        let len = self.qso_log.len();
        let band = self.awards_band.clone();
        let stale =
            self.awards_heat.as_ref().map(|(l, b, _)| *l != len || *b != band).unwrap_or(true);
        if stale {
            self.ensure_awards();
            let slots = self
                .awards_cache
                .as_ref()
                .map(|(_, _, a, _)| sdroxide_types::entity_coverage(a))
                .unwrap_or_default();
            self.awards_heat = Some((len, band, Arc::new(slots)));
        }
        Arc::clone(&self.awards_heat.as_ref().expect("just filled").2)
    }

    pub(in crate::app) fn awards_window(&mut self, ctx: &egui::Context) {
        if !self.show_awards {
            return;
        }
        self.ensure_awards();
        let bands =
            ["", "160m", "80m", "40m", "30m", "20m", "17m", "15m", "12m", "10m", "6m", "2m"];
        let mut open = self.show_awards;
        let mut new_band: Option<String> = None;
        let awards = self.awards_cache.as_ref().map(|(_, _, a, _)| a.clone()).unwrap_or_default();
        let builtins = self.awards_cache.as_ref().map(|(_, _, _, names)| names.clone()).unwrap_or_default();
        let resp = egui::Window::new(crate::language_plugin::text("window.awards.text_108_5f20ee", "AWARDS")).id(egui::Id::new("AWARDS"))
            .id(crate::layout::salted_id(ctx, "AWARDS"))
            .open(&mut open)
            .frame(crate::chrome::window_frame())
            .resizable(true)
            .default_width(crate::layout::window_w(ctx, 540.0))
            .default_height(crate::layout::window_h(ctx, 560.0))
            .show(ctx, |ui| {
                crate::chrome::window_body_bg(ui);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crate::language_plugin::text("common.band", "Band")).size(11.0).color(crate::theme::gray(150)));
                    for b in bands {
                        let label = if b.is_empty() { crate::language_plugin::text("controls.app.awards.text_120_a52ace", "All") } else { b.to_owned() };
                        if crate::chrome::chip(ui, self.awards_band == b, label).clicked() {
                            new_band = Some(b.to_string());
                        }
                    }
                });
                ui.separator();
                // Summary counts.
                award_summary(ui, "DXCC", &awards.dxcc);
                award_summary(ui, crate::language_plugin::text("window.awards.text_129_158ab1", "WAZ"), &awards.waz);
                award_summary(ui, "WAS", &awards.was);
                award_summary(ui, crate::language_plugin::text("controls.app.awards.text_131_59fdba", "Grids"), &awards.grids);
                ui.add_space(6.0);

                egui::ScrollArea::vertical().auto_shrink([false, false]).show_themed(ui, |ui| {
                    // WAS state grid.
                    ui.label(
                        RichText::new(crate::language_plugin::text("window.awards.text_137_0b6209", "Worked All States"))
                            .size(12.0)
                            .strong()
                            .color(crate::theme::CYAN()),
                    );
                    award_cell_grid(
                        ui,
                        sdroxide_types::US_STATES.iter().map(|s| {
                            (s.to_string(), awards.was.get(*s).copied().unwrap_or_default())
                        }),
                        44.0,
                    );
                    ui.add_space(8.0);
                    // WAZ zone grid (1..40).
                    ui.label(
                        RichText::new(crate::language_plugin::text("window.awards.text_152_d072ea", "CQ Zones (WAZ)"))
                            .size(12.0)
                            .strong()
                            .color(crate::theme::CYAN()),
                    );
                    award_cell_grid(
                        ui,
                        (1u8..=40).map(|z| {
                            (format!("{z:02}"), awards.waz.get(&z).copied().unwrap_or_default())
                        }),
                        34.0,
                    );
                    ui.add_space(8.0);
                    // DXCC worked list (confirmed marked).
                    ui.label(
                        RichText::new(crate::language_plugin::text("window.awards.text_167_5e4ab2", "DXCC entities"))
                            .size(12.0)
                            .strong()
                            .color(crate::theme::CYAN()),
                    );
                    for (name, st) in &awards.dxcc {
                        let name = crate::language_plugin::award_entity_name(name, &builtins);
                        let col = if st.confirmed {
                            crate::theme::GREEN()
                        } else {
                            crate::theme::YELLOW()
                        };
                        ui.label(
                            RichText::new(format!(
                                "{} {name}",
                                if st.confirmed { "✓" } else { "•" }
                            ))
                            .size(11.5)
                            .color(col),
                        );
                    }
                });
            });
        if let Some(r) = &resp {
            crate::chrome::paint_window_border(ctx, &r.response);
        }
        self.show_awards = open;
        if let Some(b) = new_band {
            self.awards_band = b;
        }
    }
}
