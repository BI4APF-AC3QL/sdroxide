//! Data-only language packs. No device commands or executable plugin code.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const API: &str = "sdroxide-language-pack/1";
pub const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub source: String,
    pub translation: String,
    #[serde(default)]
    pub parameters: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub locale: String,
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Upstream {
    pub repository: String,
    pub version: String,
    pub commit: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FontAsset {
    pub file: String,
    pub license: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HelpManual {
    pub schema_version: u32,
    pub locale: String,
    pub source_sha256: String,
    pub markdown: String,
    pub completeness: String,
}

impl HelpManual {
    /// A changed upstream manual falls back to English until reassembled.
    pub fn parse(raw: &str, source: &str) -> Result<Self, String> {
        use sha2::{Digest, Sha256};
        unique_json(raw)?;
        let manual: Self = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if manual.schema_version != 1 || manual.locale != "zh-CN"
            || !matches!(manual.completeness.as_str(), "partial" | "complete")
            || manual.markdown.trim().is_empty()
        {
            return Err("unsupported translated manual schema or locale".into());
        }
        let hash: String = Sha256::digest(source.as_bytes()).iter()
            .map(|byte| format!("{byte:02x}")).collect();
        if manual.source_sha256 != hash {
            return Err("translated manual targets a different upstream document".into());
        }
        Ok(manual)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub kind: String,
    pub name: String,
    pub locale: String,
    pub version: String,
    pub upstream: Upstream,
    pub requires_host_api: String,
    pub catalog: String,
    #[serde(default)]
    pub fonts: Vec<FontAsset>,
    #[serde(default)]
    pub help: Option<String>,
    pub completeness: String,
    pub executable_code: bool,
}

// serde's default Map accepts duplicate keys. Reject them before decoding to
// Catalog so a malformed package cannot silently replace an approved entry.
struct UniqueJson;
impl<'de> serde::Deserialize<'de> for UniqueJson {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate object keys")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<UniqueJson, A::Error> {
                let mut keys = BTreeSet::new();
                while let Some(k) = a.next_key::<String>()? {
                    if !keys.insert(k) {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                    a.next_value::<UniqueJson>()?;
                }
                Ok(UniqueJson)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<UniqueJson, A::Error> {
                while a.next_element::<UniqueJson>()?.is_some() {}
                Ok(UniqueJson)
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<UniqueJson, E> { Ok(UniqueJson) }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<UniqueJson, E> { Ok(UniqueJson) }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<UniqueJson, E> { Ok(UniqueJson) }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<UniqueJson, E> { Ok(UniqueJson) }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<UniqueJson, E> { Ok(UniqueJson) }
            fn visit_unit<E: serde::de::Error>(self) -> Result<UniqueJson, E> { Ok(UniqueJson) }
            fn visit_none<E: serde::de::Error>(self) -> Result<UniqueJson, E> { Ok(UniqueJson) }
        }
        d.deserialize_any(Visit)
    }
}

fn unique_json(s: &str) -> Result<(), String> {
    if s.len() > MAX_JSON_BYTES { return Err("language pack JSON is too large".into()); }
    serde_json::from_str::<UniqueJson>(s).map(|_| ()).map_err(|e| e.to_string())
}

pub fn slots(s: &str) -> Result<Vec<String>, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut result = Vec::new();
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => i += 2,
            '}' if chars.get(i + 1) == Some(&'}') => i += 2,
            '{' => {
                let start = i + 1;
                i = start;
                while i < chars.len() && chars[i] != '}' {
                    if chars[i] == '{' { return Err("nested format placeholder".into()); }
                    i += 1;
                }
                if i == chars.len() { return Err("unclosed format placeholder".into()); }
                result.push(chars[start..i].iter().collect());
                i += 1;
            }
            '}' => return Err("unmatched format closing brace".into()),
            _ => i += 1,
        }
    }
    Ok(result)
}

impl Catalog {
    pub fn parse(s: &str) -> Result<Self, String> {
        unique_json(s)?;
        let c: Self = serde_json::from_str(s).map_err(|e| e.to_string())?;
        if c.schema_version != 1 || c.locale != "zh-CN" { return Err("unsupported catalog schema or locale".into()); }
        if c.entries.len() > 30_000 { return Err("too many language keys".into()); }
        for (k, e) in &c.entries {
            if k.is_empty() || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
                return Err(format!("invalid language key: {k}"));
            }
            if e.source.is_empty() || e.translation.is_empty() { return Err(format!("empty translation: {k}")); }
            let a = slots(&e.source)?;
            let b = slots(&e.translation)?;
            if a != b || a != e.parameters { return Err(format!("format placeholders do not match: {k}")); }
        }
        Ok(c)
    }

    pub fn text(&self, key: &str, original: &str) -> String {
        match self.entries.get(key) {
            Some(e) if e.source == original => e.translation.clone(),
            _ => original.to_owned(),
        }
    }

    // Values have already been formatted by the caller. They are appended as
    // data and never reinterpreted as templates or translated as external text.
    pub fn format(&self, key: &str, original: &str, values: &[String]) -> Result<String, String> {
        let template = self.text(key, original);
        render(&template, values)
    }
}

pub fn render(template: &str, values: &[String]) -> Result<String, String> {
    let expected = slots(template)?.len();
    if values.len() != expected { return Err("wrong number of format values".into()); }
    let mut result = String::new();
    let mut chars = template.chars().peekable();
    let mut n = 0;
    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') { chars.next(); result.push('{'); continue; }
            while chars.next().is_some_and(|x| x != '}') {}
            result.push_str(&values[n]); n += 1;
        } else if ch == '}' && chars.peek() == Some(&'}') {
            chars.next(); result.push('}');
        } else { result.push(ch); }
    }
    Ok(result)
}

impl Manifest {
    pub fn parse(s: &str, version: &str) -> Result<Self, String> {
        unique_json(s)?;
        let m: Self = serde_json::from_str(s).map_err(|e| e.to_string())?;
        if m.schema_version != 1 || m.requires_host_api != API || m.kind != "language-pack"
            || m.id != "sdroxide.language.zh-CN" || m.locale != "zh-CN" || m.executable_code {
            return Err("incompatible language pack manifest".into());
        }
        if m.upstream.repository != "dividebysandwich/sdroxide" || m.upstream.version != version {
            return Err("language pack targets a different SDRoxide version".into());
        }
        Ok(m)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn asset_path(root: &std::path::Path, relative: &str) -> Result<std::path::PathBuf, String> {
    use std::path::Component;
    // Backslash and colon matter even when tests are run on a non-Windows host.
    if relative.is_empty() || relative.contains(['\\', ':']) { return Err("invalid asset path".into()); }
    let p = std::path::Path::new(relative);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) { return Err("asset path must stay in the language pack".into()); }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let full = root.join(p).canonicalize().map_err(|e| e.to_string())?;
    if !full.starts_with(&root) { return Err("asset link escapes the language pack".into()); }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;
    const GOOD: &str = r#"{"schema_version":1,"locale":"zh-CN","entries":{"step":{"source":"Step: {}","translation":"步进：{}","parameters":[""]}}}"#;
    #[test]
    fn manual_requires_exact_upstream_and_supported_format() {
        use sha2::{Digest, Sha256};
        let source = "## Manual\r\nEnglish\r\n";
        let good = serde_json::json!({
            "schema_version": 1, "locale": "zh-CN",
            "source_sha256": Sha256::digest(source.as_bytes()).iter()
                .map(|byte| format!("{byte:02x}")).collect::<String>(),
            "markdown": "## 手册\n中文\n", "completeness": "partial"
        }).to_string();
        assert!(HelpManual::parse(&good, source).is_ok());
        assert!(HelpManual::parse(&good, "## Manual\nChanged upstream").is_err());
        assert!(HelpManual::parse(&good.replace("zh-CN", "xx"), source).is_err());
        assert!(HelpManual::parse(&good.replace("partial", "unknown"), source).is_err());
        assert!(HelpManual::parse(&good.replace("\"schema_version\":1", "\"schema_version\":2"), source).is_err());
        assert!(HelpManual::parse(&good.replace("中文", ""), "wrong source").is_err());
    }
    #[test]
    fn preserves_external_values_and_original_fallback() {
        let c = Catalog::parse(GOOD).unwrap();
        assert_eq!(c.format("step", "Step: {}", &["7.074 MHz {raw}".into()]).unwrap(), "步进：7.074 MHz {raw}");
        assert_eq!(c.text("step", "changed upstream"), "changed upstream");
        assert_eq!(c.text("missing", "callsign BI4APF"), "callsign BI4APF");
    }
    #[test]
    fn rejects_duplicate_keys_and_changed_placeholders() {
        assert!(Catalog::parse(&GOOD.replace("步进：{}", "步进：{f}")).is_err());
        assert!(Catalog::parse(&GOOD.replace("\"locale\":\"zh-CN\"", "\"locale\":\"zh-CN\",\"locale\":\"zh-CN\"")).is_err());
        assert!(Catalog::parse(&GOOD.replace("\"parameters\":[\"\"]", "\"parameters\":[]")).is_err());
    }
    #[test]
    fn escaped_braces_and_wrong_argument_count() {
        assert_eq!(render("{{}} {} {f}", &["中文".into(), "x{}".into()]).unwrap(), "{} 中文 x{}");
        assert!(render("{}", &[]).is_err());
        assert!(slots("{}").is_ok());
        assert!(slots("{unclosed").is_err());
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn rejects_windows_and_relative_path_escapes() {
        let temp = std::env::temp_dir();
        for p in ["../outside", "C:/outside", "\\\\server\\file", "/absolute", "fonts/../../outside"] {
            assert!(asset_path(&temp, p).is_err(), "{p}");
        }
    }
}
