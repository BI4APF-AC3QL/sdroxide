//! UI-only event notices retain source provenance and are localized at display time.
//! Plain external text is never looked up in the catalog, even if it collides with a source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiNotice {
    original: String,
    template: Option<(&'static str, Vec<String>)>,
}
impl UiNotice {
    pub fn literal(source: &'static str) -> Self {
        Self::new(source.to_owned(), source, Vec::new())
    }
    pub fn new(original: String, source: &'static str, values: Vec<String>) -> Self {
        Self { original, template: Some((source, values)) }
    }
    pub fn original(&self) -> &str { &self.original }
    pub fn is_empty(&self) -> bool { self.original.is_empty() }
    pub fn clear(&mut self) { *self = Self::default(); }
    pub fn display(&self) -> String {
        let Some((source, values)) = &self.template else { return self.original.clone() };
        // Changed producer wording or inconsistent metadata must retain its actual original.
        if sdroxide_language_pack::render(source, values).ok().as_deref() != Some(self.original.as_str()) {
            return self.original.clone();
        }
        let translated = super::scope_text("display.notice.", source);
        if translated == *source { return self.original.clone(); }
        sdroxide_language_pack::render(&translated, values).unwrap_or_else(|_| self.original.clone())
    }
}
impl From<String> for UiNotice {
    fn from(original: String) -> Self { Self { original, template: None } }
}
impl From<&str> for UiNotice {
    fn from(original: &str) -> Self { original.to_owned().into() }
}
// Existing diagnostic/raw note APIs retain exactly their old output.
impl std::fmt::Display for UiNotice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.original) }
}

/// Pure projection of an already-received event; it performs no fetch or cache I/O.
pub(crate) fn satellite_update_notice(done: &[sdroxide_types::TleSubStatus]) -> UiNotice {
    let failed = done.iter().filter(|s| s.error.is_some()).count();
    let total: usize = done.iter().map(|s| s.count).sum();
    match (done.len(), failed) {
        (0, _) => UiNotice::literal("No enabled subscriptions to update."),
        (n, 0) => UiNotice::new(format!("Updated {n} subscription(s): {total} satellites."),
            "Updated {n} subscription(s): {total} satellites.", vec![n.to_string(),total.to_string()]),
        (n, f) => UiNotice::new(format!("Updated {} of {n}; {f} failed — see the rows above.",n-f),
            "Updated {} of {n}; {f} failed — see the rows above.", vec![(n-f).to_string(),n.to_string(),f.to_string()]),
    }
}

pub(crate) fn speech_status_note(status: &crate::app::SpeechStatus) -> Option<String> {
    use crate::app::SpeechStatus as S;
    let original = status.note()?;
    Some(match status {
        S::Idle => return None,
        S::Loading => UiNotice::literal("Loading the voice…").display(),
        S::Ready(v) => UiNotice::new(original, "Voice: {v}", vec![v.clone()]).display(),
        S::Failed(e) => {
            let detail = e.display();
            // The nested error may have its own trusted source. Its raw diagnostic is unchanged.
            let source = "Speech is unavailable: {e}";
            let translated = super::scope_text("display.notice.", source);
            if translated == source { original } else {
                sdroxide_language_pack::render(&translated, &[detail]).unwrap_or(original)
            }
        }
    })
}
pub(crate) fn alert_status_note(status: &crate::app::AlertStatus) -> Option<String> {
    use crate::app::AlertStatus as A;
    let original = status.note()?;
    Some(match status {
        A::Idle => return None,
        A::Running(d) => {
            let source = "alarming on {d}";
            let translated = super::scope_text("display.notice.", source);
            if translated == source { original } else {
                sdroxide_language_pack::render(&translated, &[d.display()]).unwrap_or(original)
            }
        }
        A::Failed(e) => {
            let source = "alarm device unavailable: {e}";
            let translated = super::scope_text("display.notice.", source);
            if translated == source { original } else {
                sdroxide_language_pack::render(&translated, &[e.display()]).unwrap_or(original)
            }
        }
    })
}

/// None is the producer's system-default choice; a device actually named "default" is external.
pub(crate) fn alert_output_notice(device: Option<&str>) -> UiNotice {
    device.map(UiNotice::from).unwrap_or_else(|| UiNotice::literal("default"))
}

pub fn chrome_control_text(raw: &str) -> String {
    let face=super::scope_text("display.topbar.control.",raw);
    if face!=raw {return face;}
    // Earlier menu resources and translated width samples share their original labels.
    super::scope_text("topbar.",raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    #[test]
    fn worker_created_notice_follows_ui_language_and_retains_its_original() {
        let notice=std::thread::spawn(|| UiNotice::new("Subscribed to My {N} 站台. Press UPDATE NOW to fetch it.".into(),
            "Subscribed to {}. Press UPDATE NOW to fetch it.",vec!["My {N} 站台".into()])).join().unwrap();
        let before=notice.clone();
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(notice.display(),if enabled {"已订阅 My {N} 站台。请点击“立即更新”获取数据。"} else {"Subscribed to My {N} 站台. Press UPDATE NOW to fetch it."});
            assert_eq!(notice,before);assert_eq!(notice.to_string(),notice.original());
        }
    }
    #[test]
    fn external_text_and_invalid_or_changed_notice_metadata_fall_back_exactly() {
        let values=[UiNotice::from("Loading the voice…"),UiNotice::from("no audio output: {e}\r\n外部错误"),
            UiNotice::new("Changed upstream message 12".into(),"Added {n} element set(s).",vec!["12".into()]),
            UiNotice::new("Added 12 element set(s).".into(),"Added {n} element set(s).",vec![]),
            UiNotice::new("New source 12".into(),"New source {n}",vec!["12".into()])];
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for n in &values {assert_eq!(n.display(),n.original());}
        }
        let mut n=UiNotice::literal("Loading the voice…");n.clear();assert!(n.is_empty());assert_eq!(n.display(),"");assert_eq!(n,UiNotice::default());
    }
    #[test]
    fn satellite_refresh_counts_failures_and_status_data_round_trip_unchanged() {
        use sdroxide_types::TleSubStatus;
        let cases=[(vec![],"No enabled subscriptions to update.","没有需要更新的已启用订阅。"),
            (vec![TleSubStatus{count:10,..Default::default()}],"Updated 1 subscription(s): 10 satellites.","已更新 1 项订阅：共 10 颗卫星。"),
            (vec![TleSubStatus{count:10,..Default::default()},TleSubStatus{count:3,error:Some("timeout {id}: 失败".into()),..Default::default()}],
                "Updated 1 of 2; 1 failed — see the rows above.","已更新 1 项（共 2 项）；1 项失败 — 请查看上方各行。"),
            (vec![TleSubStatus{count:1000,error:Some("error".into()),..Default::default()}],
                "Updated 0 of 1; 1 failed — see the rows above.","已更新 0 项（共 1 项）；1 项失败 — 请查看上方各行。")];
        for (status,en,zh) in cases {
            let before=status.clone();let cached=satellite_update_notice(&status);
            for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
                assert_eq!(cached.display(),if enabled {zh} else {en});assert_eq!(cached.original(),en);assert_eq!(status,before);}
        }
    }
    #[test]
    fn speech_and_alert_status_preserve_raw_errors_names_and_internal_provenance() {
        use crate::app::{SpeechStatus as S,AlertStatus as A};
        let error="C:\\外部\\Voice: {v}.json: Loading the voice…\r\n{e}";
        let speech=[S::Idle,S::Loading,S::Ready("My {v} 中文 voice".into()),S::Failed(error.into()),
            S::Failed(UiNotice::literal("the browser client cannot speak yet")),S::Failed("the browser client cannot speak yet".into())];
        let alerts=[A::Idle,A::Running("USB {d} 声卡".into()),A::Failed(error.into()),
            A::Failed(UiNotice::literal("could not start the alert thread")),A::Failed("could not start the alert thread".into())];
        for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
            for status in &speech {let before=status.clone();let raw=status.note();let shown=speech_status_note(status);
                if !enabled {assert_eq!(shown,raw);}assert_eq!(*status,before);
                if let S::Failed(e)=status {if e.original()==error {assert!(shown.unwrap().ends_with(error));}}
            }
            for status in &alerts {let before=status.clone();let raw=status.note();let shown=alert_status_note(status);
                if !enabled {assert_eq!(shown,raw);}assert_eq!(*status,before);
                if let A::Failed(e)=status {if e.original()==error {assert!(shown.unwrap().ends_with(error));}}
            }
            assert_eq!(speech_status_note(&speech[1]).unwrap(),if enabled {"正在加载语音模型…"} else {"Loading the voice…"});
            assert_eq!(speech_status_note(&speech[4]).unwrap(),if enabled {"语音不可用：浏览器客户端暂不支持语音播报"} else {"Speech is unavailable: the browser client cannot speak yet"});
            assert!(speech_status_note(&speech[5]).unwrap().ends_with("the browser client cannot speak yet"));
            assert_eq!(alert_status_note(&alerts[3]).unwrap(),if enabled {"报警音频设备不可用：无法启动报警音频线程"} else {"alarm device unavailable: could not start the alert thread"});
            assert!(alert_status_note(&alerts[4]).unwrap().ends_with("could not start the alert thread"));
        }
    }
    #[test]
    fn all_notice_templates_render_and_restore_including_zero_and_large_counts() {
        let c:serde_json::Value=serde_json::from_str(include_str!("../../../../plugins/zh-CN/translations.zh-CN.json")).unwrap();
        let entries:Vec<_>=c["entries"].as_object().unwrap().iter().filter(|(k,_)|k.starts_with("display.notice.")).map(|(_,e)|e).collect();assert!(entries.len() >= 26, "baseline notice resources missing");
        for n in [0,1,2,1000] {for e in &entries {
            let source=e["source"].as_str().unwrap();let args:Vec<_>=e["parameters"].as_array().unwrap().iter().map(|_|n.to_string()).collect();
            let original=sdroxide_language_pack::render(source,&args).unwrap();
            // Catalog lives in a static include; fixtures intentionally own a leaked test-only source.
            let cached=UiNotice::new(original.clone(),Box::leak(source.to_owned().into_boxed_str()),args.clone());
            for enabled in [true,false] {crate::language_plugin::test_pack_enabled(enabled);
                let expected=if enabled {sdroxide_language_pack::render(e["translation"].as_str().unwrap(),&args).unwrap()} else {original.clone()};
                assert_eq!(cached.display(),expected);assert_eq!(cached.original(),original);}
        }}
    }
    #[test]
    fn cached_error_and_status_faces_draw_without_sound_threads_or_device_access() {
        use crate::app::{SpeechStatus as S,AlertStatus as A};
        let cached=UiNotice::new("no audio output: USB {e} 设备未就绪".into(),"no audio output: {e}",vec!["USB {e} 设备未就绪".into()]);let before=cached.clone();
        let s=S::Ready("Piper {v} 中文".into());let a=A::Running("USB {d} 中文声卡".into());
        for enabled in [true,false] {crate::language_plugin::test_pack_enabled(enabled);
            for width in [360.0,600.0,1000.0] {
                let ctx=egui::Context::default();let mut fonts=egui::FontDefinitions::default();crate::language_plugin::add_fonts(&mut fonts);ctx.set_fonts(fonts);
                let shown=[cached.display(),speech_status_note(&s).unwrap(),alert_status_note(&a).unwrap()];
                let output=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width,240.0))),..Default::default()},|ui|{for text in &shown {ui.label(egui::RichText::new(text).size(11.0));}});
                let drawn:Vec<_>=output.shapes.iter().filter_map(|s|match &s.shape {egui::epaint::Shape::Text(t)=>Some(t.galley.job.text.clone()),_=>None}).collect();output.drop_without_applying_deltas();
                for text in &shown {assert!(drawn.contains(text),"{drawn:?}");}assert_eq!(cached,before);
            }
        }
    }
    #[test]
    fn default_audio_choice_has_provenance_and_named_default_device_stays_exact() {
        use crate::app::AlertStatus as A;
        let system=A::Running(alert_output_notice(None));
        let named=A::Running(alert_output_notice(Some("default")));
        let external=A::Running(alert_output_notice(Some("My {d} 中文声卡")));
        for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
            assert_eq!(alert_status_note(&system).unwrap(),if enabled {"报警声音输出到 系统默认"} else {"alarming on default"});
            assert_eq!(alert_status_note(&named).unwrap(),if enabled {"报警声音输出到 default"} else {"alarming on default"});
            assert!(alert_status_note(&external).unwrap().ends_with("My {d} 中文声卡"));
            assert_eq!(system.note(),named.note());
        }
    }
}
