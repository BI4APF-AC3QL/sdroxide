//! Only call-resolved/reference entity names are localizable. Logs stay raw.
use std::collections::BTreeSet;

pub fn entity_name(name: &str) -> String {super::scope_text("display.entity.",name)}

/// Build once with the award tally, using its exact eligibility and band rules.
/// A name also used by an unresolved record is ambiguous: retain it verbatim.
pub fn award_builtin_names(log: &[sdroxide_types::QsoRecord], band: Option<&str>) -> BTreeSet<&'static str> {
    let mut resolved=BTreeSet::new();let mut external=BTreeSet::new();
    for q in log {
        if q.call.trim().is_empty() || band.is_some_and(|b| !q.band.eq_ignore_ascii_case(b)) {continue;}
        if let Some(e)=sdroxide_types::resolve_callsign(&q.call) {resolved.insert(e.name);}
        else if !q.country.trim().is_empty() {external.insert(q.country.trim());}
    }
    resolved.retain(|name| !external.contains(name));resolved
}
pub fn award_entity_name(name: &str, builtins: &BTreeSet<&'static str>) -> String {
    if builtins.contains(name) {entity_name(name)} else {name.to_owned()}
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    #[test]
    fn all_country_file_names_switch_back_and_database_stays_exact() {
        let catalog:serde_json::Value=serde_json::from_str(include_str!("../../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let entities=sdroxide_types::all_entities();let before=entities.to_vec();
        assert_eq!(entities.len(),346);
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for e in entities {
                let rows:Vec<_>=catalog["entries"].as_object().unwrap().iter().filter(|(key,entry)|key.starts_with("display.entity.")&&entry["source"]==e.name).collect();
                assert_eq!(rows.len(),1,"{}",e.name);
                assert_eq!(entity_name(e.name),if enabled {rows[0].1["translation"].as_str().unwrap()} else {e.name});
            }
            assert_eq!(sdroxide_types::all_entities(),before.as_slice());
            assert_eq!(entity_name("New upstream entity {raw}"),"New upstream entity {raw}");
            assert_eq!(entity_name(""),"");
            for call in ["BI4APF","JA1ABC","K1ABC","DL1ABC","3D2ABC"] {
                let e=sdroxide_types::resolve_callsign(call).unwrap();
                let before=e;let prefix=e.primary_prefix;let flag=e.flag;
                assert_eq!(entity_name(e.name)==e.name,!enabled);
                assert_eq!(sdroxide_types::resolve_callsign(call).unwrap(),before);
                assert_eq!(before.primary_prefix,prefix);assert_eq!(before.flag,flag);
            }
        }
    }
    fn qso(call:&str, country:&str, band:&str)->sdroxide_types::QsoRecord {
        sdroxide_types::QsoRecord{call:call.into(),country:country.into(),band:band.into(),..Default::default()}
    }
    #[test]
    fn award_names_protect_external_collisions_and_follow_band_eligibility() {
        let log=vec![qso("BI4APF","Operator note","20m"),qso("???","China","40m"),
            qso("???","  Custom {raw}  ","20m"),qso("   ","China","20m"),qso("JA1ABC","China","40m")];
        let raw=serde_json::to_string(&log).unwrap();
        assert!(sdroxide_types::resolve_callsign("???").is_none());
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let band=Some("20M");let builtin=award_builtin_names(&log,band);
            let awards=sdroxide_types::compute_awards(&log,band,None);
            assert_eq!(awards.dxcc.len(),2);assert!(builtin.contains("China"));
            assert_eq!(award_entity_name("China",&builtin),if enabled {"中国"} else {"China"});
            assert_eq!(award_entity_name("Custom {raw}",&builtin),"Custom {raw}");
            let all=award_builtin_names(&log,None);
            assert!(!all.contains("China"));assert!(all.contains("Japan"));
            assert_eq!(award_entity_name("China",&all),"China");
            assert_eq!(award_entity_name("Japan",&all),if enabled {"日本"} else {"Japan"});
            let fallback=vec![qso("???","China","20m")];
            assert_eq!(award_entity_name("China",&award_builtin_names(&fallback,None)),"China");
            assert!(award_builtin_names(&log,Some("10m")).is_empty());
            assert_eq!(serde_json::to_string(&log).unwrap(),raw);
            let again=sdroxide_types::compute_awards(&log,band,None);
            assert_eq!(awards.dxcc,again.dxcc);assert_eq!(awards.waz,again.waz);
            assert_eq!(awards.was,again.was);assert_eq!(awards.grids,again.grids);
        }
    }
    #[test]
    fn long_and_short_names_render_at_three_widths_in_both_languages() {
        for enabled in [true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for width in [400.0,600.0,1000.0] {
                let ctx=egui::Context::default();let mut fonts=egui::FontDefinitions::default();
                crate::language_plugin::add_fonts(&mut fonts);ctx.set_fonts(fonts);
                let names=["China","Amsterdam & St. Paul Is.","San Felix & San Ambrosio"];
                let output=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width,200.0))),..Default::default()},|ui| {
                    for name in names {ui.label(egui::RichText::new(entity_name(name)).size(11.5));}
                });
                let rendered:Vec<String>=output.shapes.iter().filter_map(|s|match &s.shape {
                    egui::epaint::Shape::Text(t)=>Some(t.galley.job.text.clone()),_=>None
                }).collect();output.drop_without_applying_deltas();
                for name in names {assert!(rendered.contains(&entity_name(name)),"{rendered:?}");}
            }
        }
    }
}
