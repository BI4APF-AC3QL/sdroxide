pub fn aprs_symbol_display(kind: sdroxide_types::AprsSymbolKind) -> String {
    super::scope_text("map28.aprs.symbol.", kind.label())
}

pub fn aprs_object_origin_display(reported_by: &str) -> String {
    let source = "\nobject from {}";
    let translated = super::scope_text("map28.aprs.object.", source);
    sdroxide_language_pack::render(&translated, &[reported_by.to_owned()])
        .unwrap_or_else(|_| format!("\nobject from {reported_by}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdroxide_types::AprsSymbol;
    #[test]
    fn every_primary_alternate_and_overlaid_symbol_keeps_its_wire_identity() {
        for enabled in [true, false, true] {
            crate::language_plugin::test_pack_enabled(enabled);
            for table in ['/', '\\', 'S'] {
                for code in '!'..='~' {
                    let symbol = AprsSymbol::new(table, code);
                    let before = serde_json::to_string(&symbol).unwrap();
                    let kind = symbol.kind();
                    let raw = kind.label();
                    let translated = aprs_symbol_display(kind);
                    assert_eq!(translated == raw, !enabled, "{table}{code}: {raw}");
                    assert_eq!(serde_json::to_string(&symbol).unwrap(), before);
                    assert_eq!(symbol.text(), format!("{table}{code}"));
                    assert_eq!(symbol.kind(), kind);
                }
            }
            assert_eq!(crate::language_plugin::scope_text("map28.aprs.symbol.", "new upstream label"), "new upstream label");
        }
    }
    #[test]
    fn object_origin_preserves_callsign_like_labels_braces_and_external_text() {
        let raw = "car/{raw}: 台站";
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(aprs_object_origin_display(raw), if enabled {format!("\n来自 {raw} 的对象")} else {format!("\nobject from {raw}")});
        }
    }
}
