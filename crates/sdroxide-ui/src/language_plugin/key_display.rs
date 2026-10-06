/// Translate the human-readable key label without changing the stored key
/// chord. Modifiers and unknown future key names retain their familiar source
/// spelling.
pub fn key_chord_label(chord: &sdroxide_types::KeyChord) -> String {
    chord
        .label()
        .split('+')
        .map(|part| match part {
            "Space" => super::text("display.key.space", "Space").to_owned(),
            "ArrowUp" => super::text("display.key.arrow_up", "ArrowUp").to_owned(),
            "ArrowDown" => super::text("display.key.arrow_down", "ArrowDown").to_owned(),
            "ArrowLeft" => super::text("display.key.arrow_left", "ArrowLeft").to_owned(),
            "ArrowRight" => super::text("display.key.arrow_right", "ArrowRight").to_owned(),
            "Enter" => super::text("display.key.enter", "Enter").to_owned(),
            "Escape" => super::text("display.key.escape", "Escape").to_owned(),
            "Backspace" => super::text("display.key.backspace", "Backspace").to_owned(),
            "Delete" => super::text("display.key.delete", "Delete").to_owned(),
            "Insert" => super::text("display.key.insert", "Insert").to_owned(),
            "Home" => super::text("display.key.home", "Home").to_owned(),
            "End" => super::text("display.key.end", "End").to_owned(),
            "PageUp" => super::text("display.key.page_up", "PageUp").to_owned(),
            "PageDown" => super::text("display.key.page_down", "PageDown").to_owned(),
            other => other.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_physical_keys_localize_without_changing_modifiers_or_unknown_keys() {
        let mut chord = sdroxide_types::KeyChord::plain("ArrowRight");
        chord.ctrl = true;
        chord.shift = true;
        let space = sdroxide_types::KeyChord::plain("Space");
        let unknown = sdroxide_types::KeyChord::plain("FutureKey");
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(key_chord_label(&chord), if enabled { "Ctrl+Shift+右方向键" } else { "Ctrl+Shift+ArrowRight" });
            assert_eq!(key_chord_label(&space), if enabled { "空格键" } else { "Space" });
            assert_eq!(key_chord_label(&unknown), "FutureKey");
        }
    }
}
