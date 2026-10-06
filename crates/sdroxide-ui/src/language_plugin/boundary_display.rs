//! Localized faces only: allocation/frequency data and protocol labels stay raw.
pub fn bandplan_label(raw: &str)->String {super::scope_text("display.bandplan.",raw)}
pub fn vdl2_channel_role(raw: &str)->String {super::scope_text("display.vdl2.channel.",raw)}
/// Called on the UI thread before moving into a native picker worker.
pub fn image_picker_filter()->String {super::scope_text("display.picker.filter.","Image")}
/// Drop English plural suffix only when this exact template is translated.
pub fn ui_count_plural(n:u64,key:&str,source:&str)->String {
    if n!=1 && super::text(key,source)==source {"s".into()} else {String::new()}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_roles_switch_while_frequencies_and_source_data_stay_exact() {
        let frequencies=sdroxide_types::VDL2_CHANNELS_HZ;let raw=sdroxide_types::VDL2_CHANNEL_LABELS;
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for label in raw {assert_eq!(vdl2_channel_role(label)==label,!enabled);}
            assert_eq!(sdroxide_types::VDL2_CHANNELS_HZ,frequencies);assert_eq!(sdroxide_types::VDL2_CHANNEL_LABELS,raw);
            assert_eq!(vdl2_channel_role("new upstream role {raw}"),"new upstream role {raw}");
            for label in ["CW","SSB","FT8","FSK441","WSPR"] {assert_eq!(bandplan_label(label),label);}
        }
    }
    #[cfg(not(target_arch="wasm32"))]
    #[test]
    fn native_picker_captures_ui_language_before_entering_worker_thread() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let filter=image_picker_filter();assert_eq!(filter,if enabled {"图像"} else {"Image"});
            let delivered=std::thread::spawn(move || filter).join().unwrap();
            assert_eq!(delivered,if enabled {"图像"} else {"Image"});
        }
    }
    #[test]
    fn plurals_do_not_leak_english_suffix_or_break_missing_template_fallback() {
        let catalog:serde_json::Value=serde_json::from_str(include_str!("../../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let (key,entry)=catalog["entries"].as_object().unwrap().iter().find(|(k,e)|k.starts_with("boundaries.app.qo100.")&&e["source"]=="locked — {} block{} tried, {} locked").unwrap();
        let source=entry["source"].as_str().unwrap();
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for count in [0,1,2,1000] {
                assert_eq!(ui_count_plural(count,key,source),if !enabled&&count!=1 {"s"} else {""});
                assert_eq!(ui_count_plural(count,"unknown.template",source),if count!=1 {"s"} else {""});
                assert_eq!(ui_count_plural(count,key,"Changed upstream source"),if count!=1 {"s"} else {""});
            }
        }
    }
}
