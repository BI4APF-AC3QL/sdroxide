//! Settings-transfer presentation only. No import/export or filesystem calls.
use super::scope_text;
use sdroxide_config::{ConfigError, transfer::{Bundle, ImportReport}};

fn rendered(namespace: &str, source: &str, values: &[String]) -> String {
    sdroxide_language_pack::render(&scope_text(namespace, source), values)
        .unwrap_or_else(|_| sdroxide_language_pack::render(source, values).unwrap_or_else(|_| source.to_owned()))
}

fn decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
}

pub fn bundle_summary(bundle: &Bundle) -> String {
    // Use the backend's summary to retain its radio-index calculation. Only
    // translate the exact current shape; a changed upstream sentence falls back.
    let original = bundle.summary();
    let Some(body) = original.strip_suffix(&bundle.version)
        .and_then(|s| s.strip_suffix(" radio(s), from sdroxide ")) else { return original };
    let Some((count, radios)) = body.split_once(" settings file(s) for ") else { return original };
    if count != bundle.files.len().to_string() || !decimal(radios) { return original; }
    rendered("display.config.transfer.", "{} settings file(s) for {radios} radio(s), from sdroxide {}",
        &[count.to_owned(), radios.to_owned(), bundle.version.clone()])
}

pub fn import_report_summary(report: &ImportReport) -> String {
    let original = report.summary();
    let expected = if report.skipped.is_empty() {
        format!("{} settings file(s) restored", report.written.len())
    } else {
        format!("{} settings file(s) restored; {} skipped", report.written.len(), report.skipped.len())
    };
    if original != expected { return original; }
    let mut result = rendered("display.config.transfer.", "{} settings file(s) restored", &[report.written.len().to_string()]);
    if !report.skipped.is_empty() {
        result.push_str(&rendered("display.config.transfer.", "; {} skipped", &[report.skipped.len().to_string()]));
    }
    result
}

pub fn transfer_skip_reason(original: &str) -> String {
    // Only the four exact destination() reasons; arbitrary OS errors retain
    // their contents, even when they contain braces, colons or English words.
    scope_text("display.config.transfer.", original)
}

fn transfer_io_detail(detail: &str) -> String {
    if let Some(rest) = detail.strip_prefix("this is not an sdroxide settings file: ") {
        return rendered("display.config.transfer.", "this is not an sdroxide settings file: {e}", &[rest.to_owned()]);
    }
    if let Some(kind) = detail.strip_prefix("this is a ")
        .and_then(|s| s.strip_suffix(" file, not an sdroxide settings export"))
        && serde_json::from_str::<String>(kind).is_ok() {
        return rendered("display.config.transfer.", "this is a {:?} file, not an sdroxide settings export", &[kind.to_owned()]);
    }
    if let Some(body) = detail.strip_prefix("this settings file is version ")
        .and_then(|s| s.strip_suffix(" — update sdroxide to import it"))
        && let Some((version, supported)) = body.split_once(" and this sdroxide reads version ")
        && decimal(version) && decimal(supported) {
        return rendered("display.config.transfer.", "this settings file is version {} and this sdroxide reads version {FORMAT} — update sdroxide to import it",
            &[version.to_owned(), supported.to_owned()]);
    }
    detail.to_owned()
}

pub fn config_error(error: &ConfigError) -> String {
    match error {
        ConfigError::NoConfigDir => scope_text("display.config.error.", "no home/config directory available"),
        ConfigError::Unreadable(path) => rendered("display.config.error.",
            "{0} could not be read, so it has not been written over — close whatever else has it open and start again", &[path.clone()]),
        ConfigError::Io(e) => {
            let detail = e.to_string();
            let translated = if e.kind()==std::io::ErrorKind::InvalidData { transfer_io_detail(&detail) } else { detail };
            rendered("display.config.error.", "I/O: {0}", &[translated])
        },
        ConfigError::Parse(e) => rendered("display.config.error.", "parse: {0}", &[e.to_string()]),
        ConfigError::Serialize(e) => rendered("display.config.error.", "serialize: {0}", &[e.to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::{Error, ErrorKind};

    #[test]
    fn bundle_counts_sparse_radio_indices_and_opaque_versions_switch_without_io() {
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for keys in [vec![],vec!["config.toml"],vec!["radio-7/radio.json","config.toml"],vec!["radio-01/radio.json"]] {
                let bundle=Bundle {kind:"sdroxide-settings".into(),format:1,version:"v{raw} settings file(s) for 99 · 中文".into(),exported_unix:0,
                    files: keys.iter().map(|k|(k.to_string(),"opaque=\"english\"".to_owned())).collect::<BTreeMap<_,_>>()};
                let before=serde_json::to_string(&bundle).unwrap();
                let translated=bundle_summary(&bundle);
                let radios=if keys.contains(&"radio-7/radio.json") {8} else if keys.contains(&"radio-01/radio.json") {2} else {1};
                assert_eq!(translated,if enabled {format!("{} 个设置文件，涉及 {radios} 台电台，导出自 SDRoxide {}",keys.len(),bundle.version)} else {bundle.summary()});
                assert_eq!(before,serde_json::to_string(&bundle).unwrap());
            }
        }
    }

    #[test]
    fn report_and_exact_skip_reasons_switch_unknown_details_remain_opaque() {
        let reasons=["not a settings file or a radio-<n> directory","nested directories are not part of a settings bundle",
            "not a plain settings file name","not a settings file this version carries"];
        for enabled in [true,false,true] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (written,skipped) in [(0,0),(1,0),(2,4),(0,5)] {
                let report=ImportReport {written:vec!["unchanged.json".into();written],skipped:vec![("path{raw}.json".into(),"OS: detail {raw}".into());skipped]};
                let before=report.clone();
                let expected=if enabled {format!("已恢复 {written} 个设置文件{}",if skipped==0 {String::new()} else {format!("；跳过 {skipped} 个")})} else {report.summary()};
                assert_eq!(import_report_summary(&report),expected);assert_eq!(report,before);
            }
            for why in reasons { assert_eq!(transfer_skip_reason(why)==why,!enabled); }
            let unknown="OS: C:\\path: {raw} — access denied";
            assert_eq!(transfer_skip_reason(unknown),unknown);
        }
    }

    #[test]
    fn typed_errors_preserve_paths_parser_details_and_toggle_back_exactly() {
        let errors=vec![ConfigError::NoConfigDir,ConfigError::Unreadable("C:\\中文 path\\{raw}: file.toml".into()),
            ConfigError::Io(Error::new(ErrorKind::PermissionDenied,"OS detail: {raw}")),
            ConfigError::Io(Error::new(ErrorKind::InvalidData,"this is not an sdroxide settings file: expected {raw} at line 1")),
            ConfigError::Io(Error::new(ErrorKind::InvalidData,"this is a \"unknown {raw}\" file, not an sdroxide settings export")),
            ConfigError::Io(Error::new(ErrorKind::InvalidData,"this settings file is version 17 and this sdroxide reads version 1 — update sdroxide to import it")),
            ConfigError::Io(Error::new(ErrorKind::InvalidData,"this settings file is version ?? and this sdroxide reads version 1 — update sdroxide to import it"))];
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for e in &errors {
                let original=e.to_string();let translated=config_error(e);
                assert_eq!(translated==original,!enabled);
                if enabled {
                    match e {
                        ConfigError::Unreadable(path)=>assert!(translated.contains(path)),
                        ConfigError::Parse(err)=>assert!(translated.contains(&err.to_string())),
                        ConfigError::Io(err)=>{
                            let detail=err.to_string();
                            if detail.contains("??") || err.kind()==ErrorKind::PermissionDenied { assert!(translated.ends_with(&detail)); }
                            if detail.contains("{raw}") { assert!(translated.contains("{raw}")); }
                            if detail.contains("version 17") { assert_eq!(translated,"输入／输出错误：此设置文件的格式版本为 17，当前 SDRoxide 支持读取的格式版本为 1；请更新 SDRoxide 后再导入"); }
                        },_=>{}
                    }
                }
            }
        }
    }

    #[test]
    fn missing_or_stale_resource_falls_back_and_external_english_is_not_retranslated() {
        crate::language_plugin::test_pack_enabled(true);
        assert_eq!(rendered("display.config.missing.","restored {count}",&["12".into()]),"restored 12");
        assert_eq!(rendered("display.config.transfer.","new summary {count}",&["12".into()]),"new summary 12");
        assert_eq!(transfer_skip_reason("not a plain settings file name (new upstream detail)"),"not a plain settings file name (new upstream detail)");
        assert_eq!(super::super::text("missing.new.source","unknown backend text"),"unknown backend text");
        let raw="this is a unquoted file, not an sdroxide settings export";
        assert_eq!(transfer_io_detail(raw),raw);
    }
}
