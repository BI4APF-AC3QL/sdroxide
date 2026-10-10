//! Reference prose at display boundaries; stored text and radio state stay raw.
use super::scope_text;
use sdroxide_types::{SatLink, WefaxChartMeta, WefaxStation};

pub fn wefax_station_name(name: &str) -> String {scope_text("display.wefax.station.",name)}

pub fn wefax_where_label(meta: &WefaxChartMeta) -> Option<String> {
    let original=meta.where_label()?;
    let hz=meta.dial_hz?;
    Some(match WefaxStation::at_dial(hz) {
        Some((s,carrier)) if original==format!("{} · {carrier:.1} kHz",s.name) =>
            format!("{} · {carrier:.1} kHz",wefax_station_name(s.name)),
        None if original==format!("{:.1} kHz dial",hz/1000.0) => {
            let source="{:.1} kHz dial";
            sdroxide_language_pack::render(&scope_text("display.wefax.frequency.",source),&[format!("{:.1}",hz/1000.0)]).unwrap_or(original)
        },
        _=>original,
    })
}
pub fn wefax_chart_title(meta: Option<WefaxChartMeta>, name: &str) -> String {
    let Some(meta)=meta else {return name.to_owned()};
    let original=meta.label();
    match (meta.where_label(),wefax_where_label(&meta)) {
        (Some(w),Some(localized)) if original==format!("{}  ·  {w}",meta.when_label()) =>
            format!("{}  ·  {localized}",meta.when_label()),
        _=>original,
    }
}
fn builtin_text(ns: &str, text: &str, custom: bool) -> String {
    if custom {text.to_owned()} else {scope_text(ns,text)}
}
pub fn satellite_link_label(link: &SatLink, custom: bool) -> String {
    builtin_text("display.satellite.link.",&link.label,custom)
}
pub fn satellite_link_note(link: &SatLink, custom: bool) -> String {
    builtin_text("display.satellite.note.",&link.note,custom)
}
pub fn satellite_link_mode(link: &SatLink, custom: bool) -> String {
    builtin_text("display.satellite.mode.",&link.mode,custom)
}
pub fn satellite_link_mode_with_inversion(link: &SatLink, custom: bool) -> String {
    let mode=satellite_link_mode(link,custom);
    if !link.inverting {return mode;}
    let source="{} · inv";
    // The inversion suffix is UI prose even for an operator-supplied link;
    // the operator's mode string itself must remain exactly as entered.
    sdroxide_language_pack::render(&scope_text("display.satellite.mode.",source),&[mode]).unwrap_or_else(|_|format!("{} · inv",link.mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_station_and_carrier_switches_without_changing_chart_metadata() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for station in sdroxide_types::WEFAX_STATIONS {
                let call=station.name.split_whitespace().next().unwrap();
                assert_eq!(wefax_station_name(station.name)==station.name,!enabled);
                assert!(wefax_station_name(station.name).starts_with(call));
                for &carrier in station.carriers_khz {
                    for offset in [-199.0,0.0,199.0] {
                        let meta=WefaxChartMeta{unix:1785334500,dial_hz:Some(WefaxStation::dial_hz(carrier)+offset)};
                        let before=meta;let filename=meta.file_name();let date=meta.when_label();
                        let where_text=wefax_where_label(&meta).unwrap();
                        assert_eq!(where_text==meta.where_label().unwrap(),!enabled);
                        assert!(where_text.contains(&format!("{carrier:.1} kHz")));
                        assert!(where_text.starts_with(call));
                        let title=wefax_chart_title(Some(meta),&filename);
                        assert!(title.starts_with(&date));assert!(title.ends_with(&where_text));
                        assert_eq!(meta,before);assert_eq!(meta.file_name(),filename);
                        assert_eq!(WefaxChartMeta::from_file_name(&filename).unwrap().unix,meta.unix);
                    }
                }
            }
        }
    }
    #[test]
    fn unknown_frequency_old_chart_and_foreign_filename_have_exact_fallbacks() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let meta=WefaxChartMeta{unix:1785334500,dial_hz:Some(12345678.0)};
            let text=wefax_where_label(&meta).unwrap();
            assert_eq!(text,if enabled {"调谐频率 12345.7 kHz"} else {"12345.7 kHz dial"});
            let old=WefaxChartMeta{unix:1785334500,dial_hz:None};
            assert_eq!(wefax_where_label(&old),None);
            assert_eq!(wefax_chart_title(Some(old),"legacy.png"),old.when_label());
            for name in ["DWD Pinneberg (Germany).png","Telemetry {raw}.png","中文地图.png"] {
                assert_eq!(wefax_chart_title(None,name),name);
            }
            assert_eq!(wefax_station_name("New upstream station {raw}"),"New upstream station {raw}");
            for offset in [-200.0,200.0] {
                let m=WefaxChartMeta{unix:0,dial_hz:Some(WefaxStation::dial_hz(3855.0)+offset)};
                assert_eq!(WefaxStation::at_dial(m.dial_hz.unwrap()).is_some(),false);
                assert!(wefax_where_label(&m).unwrap().contains(&format!("{:.1} kHz",m.dial_hz.unwrap()/1000.0)));
            }
        }
    }
    #[test]
    fn all_builtin_satellite_rows_switch_and_custom_copies_stay_verbatim() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for sat in sdroxide_solar::satfreq::builtin() {
                for link in &sat.links {
                    let before=link.clone();let serialized=serde_json::to_string(link).unwrap();
                    assert_eq!(satellite_link_label(link,false)==link.label,!enabled||matches!(link.label.as_str(),"SSTV"|"APT"));
                    assert_eq!(satellite_link_note(link,false)==link.note,!enabled||link.note.is_empty());
                    assert_eq!(satellite_link_mode(link,false)==link.mode,!enabled||!matches!(link.mode.as_str(),"SSB/CW/digital"|"SSTV FM / PD120"));
                    let decorated=satellite_link_mode_with_inversion(link,false);
                    assert_eq!(decorated.ends_with("反相"),enabled&&link.inverting);
                    if !enabled {assert_eq!(decorated,if link.inverting {format!("{} · inv",link.mode)} else {link.mode.clone()});}
                    assert_eq!(satellite_link_label(link,true),link.label);
                    assert_eq!(satellite_link_note(link,true),link.note);
                    assert_eq!(satellite_link_mode(link,true),link.mode);
                    assert_eq!(*link,before);assert_eq!(serde_json::to_string(link).unwrap(),serialized);
                }
            }
        }
    }
    #[test]
    fn unknown_satellite_text_and_custom_braces_are_data_in_both_languages() {
        for enabled in [true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let link=SatLink{label:"FM repeater".into(),note:"inverting".into(),mode:"my {raw} mode".into(),inverting:true,..Default::default()};
            assert_eq!(satellite_link_label(&link,true),"FM repeater");
            assert_eq!(satellite_link_note(&link,true),"inverting");
            assert!(satellite_link_mode_with_inversion(&link,true).starts_with("my {raw} mode"));
            let changed=SatLink{label:"new upstream label {raw}".into(),note:"new upstream caution".into(),..link};
            assert_eq!(satellite_link_label(&changed,false),changed.label);
            assert_eq!(satellite_link_note(&changed,false),changed.note);
        }
    }
}
