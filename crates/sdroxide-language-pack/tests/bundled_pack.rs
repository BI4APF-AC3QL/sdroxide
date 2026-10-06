use sdroxide_language_pack::{Catalog, Manifest};
#[test]
fn bundled_pack_matches_host_and_all_parameters() {
    let manifest = include_str!("../../../plugins/zh-CN/manifest.json");
    let catalog = include_str!("../../../plugins/zh-CN/translations.zh-CN.json");
    let m = Manifest::parse(manifest, env!("CARGO_PKG_VERSION")).unwrap();
    let c = Catalog::parse(catalog).unwrap();
    assert_eq!(m.locale, c.locale);
    assert_eq!(m.completeness, "complete");
    assert!(c.entries.len() >= 962);
    assert_eq!(c.text("settings.ui.layout", "Layout"), "布局");
}

#[test]
fn bundled_font_rasterizer_covers_every_chinese_translation() {
    use ab_glyph::Font;
    let bytes = include_bytes!("../../../plugins/zh-CN/fonts/NotoSansSC.ttf");
    let font = ab_glyph::FontRef::try_from_slice(bytes).unwrap();
    let c = Catalog::parse(include_str!("../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
    for (key, e) in &c.entries {
        for ch in e.translation.chars().filter(|ch| matches!(*ch as u32, 0x3000..=0x303f | 0x3400..=0x9fff | 0xff00..=0xffef)) {
            assert_ne!(font.glyph_id(ch).0, 0, "missing glyph {ch} in {key}");
        }
    }
}

#[test]
fn bundled_font_covers_translated_manual_characters() {
    use ab_glyph::Font;
    use sdroxide_language_pack::HelpManual;
    let manual = HelpManual::parse(
        include_str!("../../../plugins/zh-CN/manual.zh-CN.json"),
        include_str!("../../../docs/USER_MANUAL.md"),
    ).unwrap();
    assert_eq!(manual.completeness, "complete");
    let font = ab_glyph::FontRef::try_from_slice(include_bytes!("../../../plugins/zh-CN/fonts/NotoSansSC.ttf")).unwrap();
    let characters: std::collections::BTreeSet<_> = manual.markdown.chars()
        .filter(|ch| matches!(*ch as u32, 0x3000..=0x303f | 0x3400..=0x9fff | 0xff00..=0xffef))
        .collect();
    assert!(characters.len() > 1000);
    for ch in characters { assert_ne!(font.glyph_id(ch).0, 0, "manual is missing glyph {ch}"); }
}
