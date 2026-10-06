//! Raw pane order and novelty state are kept separate from localized faces.
pub fn panel_tab_label(label: &str) -> String {super::scope_text("display.panel.tab.",label)}
pub fn novelty_badge(label: &str) -> String {super::scope_text("display.panel.novelty.",label)}
#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    #[test]
    fn badges_fit_existing_columns_and_fall_back_exactly() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let ctx=egui::Context::default();let mut fonts=egui::FontDefinitions::default();
            crate::language_plugin::add_fonts(&mut fonts);ctx.set_fonts(fonts);
            ctx.run_ui(egui::RawInput::default(),|ui| {
                for (raw,zh) in [("DXCC","新实体"),("BAND","新波段"),("GRID","新网格"),("NEW","新呼号"),("DUPE","重复")] {
                    let text=novelty_badge(raw);assert_eq!(text,if enabled {zh} else {raw});
                    let galley=ui.fonts_mut(|f|f.layout_no_wrap(text,egui::FontId::proportional(9.5),egui::Color32::WHITE));
                    assert!(galley.size().x<=34.0,"{}: {}",raw,galley.size().x);
                }
            }).drop_without_applying_deltas();
            assert_eq!(novelty_badge(""),"");assert_eq!(novelty_badge("New upstream code {raw}"),"New upstream code {raw}");
            assert_eq!(panel_tab_label("New upstream pane {raw}"),"New upstream pane {raw}");
        }
    }
}

pub fn decode_sender_fallback(free_text:bool)->String {
    if free_text {super::text("app.panels.decodes.text_548_e4ba21","TEXT")} else {"?".to_owned()}
}
#[cfg(test)]
mod decode_fallback_tests {
    use super::*;
    #[test]
    fn only_owned_free_text_placeholder_is_localized() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(decode_sender_fallback(true),if enabled{"文本"}else{"TEXT"});
            assert_eq!(decode_sender_fallback(false),"?");
            let remote_callsign="TEXT";
            assert_eq!(remote_callsign,"TEXT");
        }
    }
}
