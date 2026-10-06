//! Settings presentation only; all device commands and persistent radio names stay raw.
pub fn transverter_header_display(header: &str) -> String {
    super::scope_text("settings24.transverter.",header)
}
pub fn pan_receiver_selected(id: Option<u32>, candidates: &[(u32,String)]) -> String {
    match id {
        None => super::scope_text("settings24.receiver.","None"),
        Some(id) => candidates.iter().find(|(c,_)|*c==id).map(|(_,name)|name.clone())
            .unwrap_or_else(|| {
                let source="radio {}";
                let translated=super::scope_text("settings24.receiver.",source);
                sdroxide_language_pack::render(&translated,&[(u64::from(id)+1).to_string()])
                    .unwrap_or_else(|_|format!("radio {}",u64::from(id)+1))
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiver_fallbacks_do_not_translate_same_named_external_radios() {
        let names=vec![(0,"None".into()),(1,"radio 3".into()),(2,"电台 {call}\r\nTest".into())];
        let before=names.clone();
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(pan_receiver_selected(None,&names),if enabled{"无"}else{"None"});
            assert_eq!(pan_receiver_selected(Some(3),&names),if enabled{"电台 4"}else{"radio 4"});
            for (id,name) in &names {assert_eq!(pan_receiver_selected(Some(*id),&names),*name);}
            assert_eq!(pan_receiver_selected(Some(u32::MAX),&[]),if enabled{"电台 4294967296"}else{"radio 4294967296"});
            assert_eq!(names,before);
        }
    }
    #[test]
    fn transverter_headers_switch_without_changing_empty_or_unknown_labels() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (raw,zh) in [("Name","名称"),("Band low","波段下限"),("Band high","波段上限"),("Offset","频率偏移"),("Transmit","发射"),("Max drive","最大驱动")] {
                assert_eq!(transverter_header_display(raw),if enabled{zh}else{raw});
            }
            for raw in ["","Upstream new header","Name {call}"] {assert_eq!(transverter_header_display(raw),raw);}
        }
    }
}

pub fn radio_fallback(id:u32)->String {
    let source="radio {}";
    let translated=super::scope_text("settings24.receiver.",source);
    sdroxide_language_pack::render(&translated,&[(u64::from(id)+1).to_string()])
        .unwrap_or_else(|_|format!("radio {}",u64::from(id)+1))
}
pub fn satellite_tab_label(active:bool)->String {
    let label=super::text("window.satellite.tabs.text_625_bcdc9d","SATELLITES");
    if active {format!("{label} •")} else {label}
}
#[cfg(test)]
mod batch30_display_tests {
    use super::*;
    #[test]
    fn radio_fallback_and_satellite_tab_switch_both_ways() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(radio_fallback(3),if enabled{"电台 4"}else{"radio 4"});
            assert_eq!(satellite_tab_label(true),if enabled{"卫星 •"}else{"SATELLITES •"});
            assert_eq!(satellite_tab_label(false),if enabled{"卫星"}else{"SATELLITES"});
        }
    }
}
