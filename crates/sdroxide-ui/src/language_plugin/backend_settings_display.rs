//! Known backend sentences at the UI boundary. External fields remain data.
use super::scope_text;
use sdroxide_types::{LoginTarget, LoginTestResult, RelayChannel, RelayConfig};
const SOURCES: &[(&str, &str)] = &[
    ("display.login.", "username/password not set"),
    ("display.login.", "API key not set"),
    ("display.login.", "email/password not set"),
    ("display.login.", "station callsign not set"),
    ("display.login.", "login not set"),
    ("display.login.", "login rejected"),
    ("display.login.", "key rejected"),
    ("display.login.", "HamQTH login failed"),
    ("display.login.", "unexpected reply; check the username and password"),
    ("display.login.", "unexpected reply; check the login and password"),
    ("display.login.", "login rejected (LoTW returned its log-on page)"),
    ("display.login.", "signed in as {}"),
    ("display.login.", "{call}, {n} QSOs"),
    ("display.login.", "{call}: account accepted (API key not checked)"),
    ("display.login.", " as {call}"),
    ("display.login.", "key accepted{who}"),
    ("display.login.", "key accepted{who}, but this account has no default logbook — set one in World Radio League, or contacts will be refused"),
    ("display.login.wrl.", "WRL: the API key was rejected — generate a fresh one in World Radio League under              Integrations → Developer API"),
    ("display.login.wrl.", "WRL: this key is not allowed to log contacts{}"),
    ("display.login.wrl.", "WRL: already logged"),
    ("display.login.wrl.", "WRL: this account has several logbooks and no default one — set a default in World              Radio League, and contacts will go there"),
    ("display.login.wrl.", "WRL: the contact was rejected{}"),
    ("display.login.wrl.", "WRL: rate limited — try again shortly{}"),
    ("display.relay.refusal.", "no serial port chosen for the T/R switch"),
    ("display.relay.refusal.", "no device chosen for the T/R switch"),
    ("display.relay.refusal.", "no GPIO chip chosen for the T/R switch"),
    ("display.relay.refusal.", "no transmit command set for the T/R switch"),
    ("display.relay.refusal.", "no T/R switch channel has been given a job"),
    ("display.relay.refusal.", "a band decoder needs contacts that switch one at a time, and the \"{}\" link is a single on/off"),
    ("display.relay.refusal.", "T/R switch channel number {} is out of range (1–{MAX_CHANNEL})"),
    ("display.relay.refusal.", "T/R switch contact {} is a band decoder and has another job as well"),
    ("display.relay.refusal.", "the T/R switch drives contact {highest} but only {} GPIO line(s) are listed"),
    ("display.relay.error.", "cannot open the T/R switch on {path}: {source}"),
    ("display.relay.error.", "T/R switch I/O failed: {0}"),
    ("display.relay.error.", "T/R switch serial port: {0}"),
    ("display.relay.error.", "the T/R switch on {path} stopped answering — check the USB cable, and that nothing else has the device open"),
    ("display.relay.error.", "the T/R switch replied with {got:?}, which is not an answer to {sent}"),
    ("display.relay.error.", "no T/R switch found at {key} — it may have been unplugged, or enumerated under a different device node since it was chosen"),
    ("display.relay.error.", "permission denied opening the T/R switch at {path} — install the packaged udev rule (60-sdroxide-relay.rules) and replug the device, or add yourself to the group that owns it"),
    ("display.relay.error.", "the T/R switch stopped answering and has been left alone: {0}"),
    ("display.relay.error.", "a dcttech relay board has at most eight contacts"),
    ("display.relay.error.", "channel {ch} is out of range for a {}"),
    ("display.relay.error.", "channel {ch} is out of range"),
    ("display.relay.error.", "a serial RTS/DTR switch has two contacts: 1 is RTS and 2 is DTR"),
    ("display.relay.error.", "no GPIO lines listed for the T/R switch"),
    ("display.relay.error.", "a GPIO request carries at most {GPIO_V2_LINES_MAX} lines"),
    ("display.relay.error.", "GPIO lines are a Linux interface — on this platform use a relay board, an RTS/DTR line, or the external command hook"),
    ("display.relay.error.", "USB HID relays and sound-card GPIO are not supported on this platform — use a serial relay board, an RTS/DTR line, or the external command hook"),
    ("display.relay.describe.", "{} relay board on {}"),
    ("display.relay.describe.", "{lines} on {}"),
    ("display.relay.describe.", "USB HID relay board at {}"),
    ("display.relay.describe.", "{what} pin {} at {}"),
    ("display.relay.describe.", "GPIO lines on {}"),
    ("display.relay.describe.", "external command \"{}\""),
    ("display.relay.component.", "RTS and DTR"),
    ("display.relay.component.", "no line"),
    ("display.relay.component.", "sound-card GPIO"),
    ("display.relay.sequence.", "channel {}"),
    ("display.relay.sequence.", "Nothing is switched."),
    ("display.relay.sequence.", "{} at −{} ms"),
    ("display.relay.sequence.", "{} at +{} ms"),
    ("display.relay.sequence.", ", then "),
    ("display.relay.sequence.", "Key-down: {}. Key-up: {}."),
];

fn rendered(ns: &str, source: &str, values: &[String]) -> String {
    sdroxide_language_pack::render(&scope_text(ns, source), values)
        .unwrap_or_else(|_| sdroxide_language_pack::render(source, values).unwrap_or_else(|_| source.to_owned()))
}

/// Recover already-formatted fields only when the literal delimiters imply
/// exactly one division. Never guess where a path or server reply ends.
fn fields(source: &str, original: &str) -> Option<Vec<String>> {
    if original.len()>16_384 { return None; }
    let mut parts=Vec::new();let mut tail=source;
    while let Some(open)=tail.find('{') {
        parts.push(&tail[..open]);
        let end=tail[open..].find('}')?+open;
        tail=&tail[end+1..];
    }
    parts.push(tail);
    if parts.len()==1 { return (source==original).then(Vec::new); }
    let rest=original.strip_prefix(parts[0])?;
    let mut found=Vec::new();let mut budget=1024;
    fn walk(parts: &[&str], rest: &str, values: &mut Vec<String>, found: &mut Vec<Vec<String>>, budget: &mut usize) {
        if *budget==0 || found.len()>1 { return; }
        *budget-=1;
        if parts.len()==1 {
            if let Some(field)=rest.strip_suffix(parts[0]) { values.push(field.to_owned());found.push(values.clone());values.pop(); }
            return;
        }
        if parts[0].is_empty() { return; }
        for (pos,_) in rest.match_indices(parts[0]) {
            values.push(rest[..pos].to_owned());
            walk(&parts[1..],&rest[pos+parts[0].len()..],values,found,budget);
            values.pop();
            if *budget==0 || found.len()>1 { break; }
        }
    }
    walk(&parts[1..],rest,&mut Vec::new(),&mut found,&mut budget);
    if budget>0 && found.len()==1 { found.pop() } else { None }
}

fn backend_message(ns: &str, original: &str) -> String {
    backend_message_at(ns,original,0)
}
fn backend_message_at(ns: &str, original: &str, depth: u8) -> String {
    let mut matches=Vec::new();
    for &(scope,source) in SOURCES {
        if scope!=ns { continue; }
        let translated=scope_text(ns,source);
        if translated==source { continue; }
        let Some(mut values)=fields(source,original) else { continue; };
        let params=sdroxide_language_pack::slots(source).unwrap_or_default();
        let numeric=["n","ch","highest","MAX_CHANNEL","GPIO_V2_LINES_MAX"];
        if params.iter().zip(&values).any(|(p,v)| numeric.contains(&p.as_str()) && (v.is_empty() || !v.bytes().all(|b|b.is_ascii_digit()))) { continue; }
        let mut valid=true;
        for (param,value) in params.iter().zip(&mut values) {
            if ns=="display.login." && param=="who" {
                if let Some(call)=value.strip_prefix(" as ") {
                    *value=rendered(ns," as {call}",&[call.to_owned()]);
                } else if !value.is_empty() { valid=false; }
            }
            if ns=="display.relay.describe." && param=="lines" {
                // This is generated from handshake-line flags, not a name.
                *value=scope_text("display.relay.component.",value);
            }
            if ns=="display.relay.refusal." && source.starts_with("a band decoder needs contacts") {
                *value=super::display_label(&*value);
            }
            if ns=="display.relay.error." && source.starts_with("the T/R switch stopped answering and has been left alone:") && depth<4 {
                *value=backend_message_at(ns,value,depth+1);
            }
        }
        if valid && let Ok(result)=sdroxide_language_pack::render(&translated,&values) {
            let specificity=source.len()-params.iter().map(|p|p.len()+2).sum::<usize>();
            matches.push((specificity,result));
        }
    }
    // Prefer the sentence with more fixed text over its generic prefix form
    // (WRL's accepted-key sentence has an optional no-default-logbook tail).
    // Equally specific candidates remain ambiguous and fall back unchanged.
    let Some(best)=matches.iter().map(|m|m.0).max() else {return original.to_owned()};
    let mut best_matches=matches.into_iter().filter(|m|m.0==best);
    let first=best_matches.next().unwrap().1;
    if best_matches.next().is_none() {first} else {original.to_owned()}
}

pub fn login_test_message(result: &LoginTestResult) -> String {
    if result.target==LoginTarget::Wrl && !result.ok && result.message!="API key not set" {
        // wrl_error() has its service prefix removed by test_wrl(). Restore
        // it only for matching against the exact source-bound templates.
        let qualified=format!("WRL: {}",result.message);
        let localized=backend_message("display.login.wrl.",&qualified);
        return if localized==qualified {result.message.clone()} else {localized};
    }
    // Per-service allowlist: a QRZ call or arbitrary free-form service body
    // must not be treated as another service's successful sign-in sentence.
    let source=&result.message;
    let allowed=match result.target {
        LoginTarget::Eqsl|LoginTarget::HamQth|LoginTarget::Lotw => {
            if result.ok {source.starts_with("signed in as ")} else {matches!(source.as_str(),
                "username/password not set"|"login not set"|"login rejected"|"HamQTH login failed"|
                "unexpected reply; check the username and password"|"unexpected reply; check the login and password"|
                "login rejected (LoTW returned its log-on page)")}
        },
        LoginTarget::QrzLogbook=> if result.ok {source.contains(" QSOs") && source.contains(", ")} else {matches!(source.as_str(),"API key not set"|"key rejected")},
        LoginTarget::ClubLog=> if result.ok {source.ends_with(": account accepted (API key not checked)")} else {matches!(source.as_str(),"email/password not set"|"station callsign not set")},
        LoginTarget::Wrl=> if result.ok {source.starts_with("key accepted")} else {source=="API key not set"},
    };
    if allowed {backend_message("display.login.",source)} else {source.clone()}
}
pub fn relay_refusal_message(original: &str) -> String {backend_message("display.relay.refusal.",original)}
pub fn relay_error_message(original: &str) -> String {
    let result=backend_message("display.relay.error.",original);
    if result==original {relay_refusal_message(original)} else {result}
}
pub fn relay_description(original: &str) -> String {backend_message("display.relay.describe.",original)}
pub fn relay_channel_name(channel: &RelayChannel) -> String {
    if channel.label.trim().is_empty() {rendered("display.relay.sequence.","channel {}",&[channel.index.to_string()])}
    else {channel.name()}
}
pub fn relay_sequence_note(cfg: &RelayConfig) -> String {
    let original=cfg.sequence_note();
    let mut chans: Vec<&RelayChannel>=cfg.active_channels().collect();
    if chans.is_empty() {return scope_text("display.relay.sequence.",&original);}
    chans.sort_by_key(|c|std::cmp::Reverse(cfg.lead_for(c)));
    let on: Vec<String>=chans.iter().map(|c|format!("{} at −{} ms",c.name(),cfg.lead_for(c))).collect();
    let zh_on: Vec<String>=chans.iter().map(|c|rendered("display.relay.sequence.","{} at −{} ms",&[relay_channel_name(c),cfg.lead_for(c).to_string()])).collect();
    chans.sort_by_key(|c|cfg.hold_for(c));
    let off: Vec<String>=chans.iter().map(|c|format!("{} at +{} ms",c.name(),cfg.hold_for(c))).collect();
    let zh_off: Vec<String>=chans.iter().map(|c|rendered("display.relay.sequence.","{} at +{} ms",&[relay_channel_name(c),cfg.hold_for(c).to_string()])).collect();
    if original!=format!("Key-down: {}. Key-up: {}.",on.join(", then "),off.join(", then ")) {return original;}
    let joiner=scope_text("display.relay.sequence.",", then ");
    rendered("display.relay.sequence.","Key-down: {}. Key-up: {}.",&[zh_on.join(&joiner),zh_off.join(&joiner)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdroxide_types::{RelayLink,RelayRole};
    #[test]
    fn template_fields_reject_ambiguous_delimiters_and_preserve_unicode_braces() {
        assert_eq!(fields("open {path}: {error}","open C:\\file: bad"),Some(vec!["C:\\file".into(),"bad".into()]));
        assert_eq!(fields("open {path}: {error}","open C:\\a: file: bad"),None);
        assert_eq!(fields("name {}","name 中文 {raw}"),Some(vec!["中文 {raw}".into()]));
        assert_eq!(fields("{} and {}","a and b and c"),None);
        assert_eq!(fields("key accepted{who}","key accepted"),Some(vec![String::new()]));
        assert_eq!(fields("new source {}","old source text"),None);
        assert_eq!(fields("{}{}","ab"),None);
        assert_eq!(fields("{} at {}","x"),None);
        assert_eq!(fields("{} at {}",&"x".repeat(17000)),None);
    }
    #[test]
    fn all_services_switch_known_messages_but_preserve_accounts_and_raw_failures() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for target in LoginTarget::ALL {
                let message=match target {
                    LoginTarget::Eqsl|LoginTarget::HamQth|LoginTarget::Lotw=>"signed in as call {raw}",
                    LoginTarget::QrzLogbook=>"BI4APF, 102 QSOs",LoginTarget::ClubLog=>"BI4APF: account accepted (API key not checked)",
                    LoginTarget::Wrl=>"key accepted as call {raw}, but this account has no default logbook — set one in World Radio League, or contacts will be refused"};
                let result=LoginTestResult{target,ok:true,message:message.into()};let before=result.clone();
                assert_eq!(login_test_message(&result)==message,!enabled);assert_eq!(result,before);
                if message.contains("{raw}"){assert!(login_test_message(&result).contains("call {raw}"));}
                let raw=LoginTestResult{target,ok:false,message:"Server detail: {raw} permission denied".into()};
                assert_eq!(login_test_message(&raw),raw.message);
            }
            for msg in ["key accepted","key accepted as BI4APF"] {
                let r=LoginTestResult{target:LoginTarget::Wrl,ok:true,message:msg.into()};assert_eq!(login_test_message(&r)==msg,!enabled);
            }
            let r=LoginTestResult{target:LoginTarget::QrzLogbook,ok:true,message:"signed in as external station name".into()};
            assert_eq!(login_test_message(&r),r.message);
        }
    }
    #[test]
    fn relay_messages_retain_paths_reply_bytes_numbers_and_unknown_source() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for source in ["no serial port chosen for the T/R switch","T/R switch channel number 33 is out of range (1–32)",
                "the T/R switch drives contact 7 but only 2 GPIO line(s) are listed"] {
                assert_eq!(relay_refusal_message(source)==source,!enabled);
            }
            let path="C:\\中文 path\\{raw}";
            let error=format!("the T/R switch on {path} stopped answering — check the USB cable, and that nothing else has the device open");
            let translated=relay_error_message(&error);assert_eq!(translated==error,!enabled);assert!(translated.contains(path));
            for source in ["T/R switch I/O failed: OS detail {raw}","the T/R switch replied with \"relay {raw}\", which is not an answer to relay read 01",
                "the T/R switch stopped answering and has been left alone: T/R switch I/O failed: raw{0}"] {
                assert_eq!(relay_error_message(source)==source,!enabled);assert!(relay_error_message(source).contains("raw"));
            }
            let ambiguous="cannot open the T/R switch on C:\\a: file: bad";
            assert_eq!(relay_error_message(ambiguous),ambiguous);
            let unknown="new upstream relay failure {raw}";assert_eq!(relay_error_message(unknown),unknown);
            let description=format!("RTS and DTR on {path}");
            assert_eq!(relay_description(&description)==description,!enabled);assert!(relay_description(&description).contains(path));
            assert_eq!(relay_description("custom external device {raw}"),"custom external device {raw}");
        }
    }
    #[test]
    fn every_registered_message_shape_renders_and_missing_resources_fall_back() {
        for enabled in [true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for &(ns,source) in SOURCES {
                if !["display.login.","display.login.wrl.","display.relay.refusal.","display.relay.error.","display.relay.describe."].contains(&ns) {continue;}
                let values:Vec<String>=sdroxide_language_pack::slots(source).unwrap().iter().map(|p|match p.as_str() {
                    "n"|"ch"|"highest"|"MAX_CHANNEL"|"GPIO_V2_LINES_MAX"=>"7",
                    "who"=>" as BI4APF {raw}","lines"=>"RTS and DTR",_=>"opaque {raw}"
                }.to_owned()).collect();
                let original=sdroxide_language_pack::render(source,&values).unwrap();
                let actual=backend_message(ns,&original);
                assert_eq!(actual==original,!enabled,"{ns}: {source}");
                if original.contains("{raw}") {assert!(actual.contains("{raw}"),"{source}");}
            }
            assert_eq!(backend_message("missing.namespace.","known source {raw}"),"known source {raw}");
        }
    }
    #[test]
    fn wrl_failures_preserve_server_details_and_disabled_source_prefix() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for detail in ["",": offending field {raw}: callsign"] {
                for prefix in ["this key is not allowed to log contacts","the contact was rejected","rate limited — try again shortly"] {
                    let message=format!("{prefix}{detail}");
                    let r=LoginTestResult{target:LoginTarget::Wrl,ok:false,message:message.clone()};
                    let result=login_test_message(&r);assert_eq!(result==message,!enabled);assert!(result.ends_with(detail));
                }
            }
            for raw in ["HTTP 503: Service detail {raw}","UNRECOGNIZED_CODE: opaque detail","changed upstream sentence"] {
                let r=LoginTestResult{target:LoginTarget::Wrl,ok:false,message:raw.into()};assert_eq!(login_test_message(&r),raw);
            }
        }
    }
    #[test]
    fn satellite_pass_quality_keeps_thresholds_and_unknown_labels() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (el,source,zh) in [(0.0,"marginal","条件较差"),(14.99,"marginal","条件较差"),(15.0,"fair","一般"),(29.99,"fair","一般"),(30.0,"good","良好"),(59.99,"good","良好"),(60.0,"overhead","接近天顶")] {
                let pass=sdroxide_solar::satellites::Pass{rise_unix:0,set_unix:100,rise_az:0.0,set_az:100.0,max_el:el,max_el_unix:50,max_el_az:50.0};
                assert_eq!(pass.quality(),source);
                assert_eq!(scope_text("display.solar.pass_quality.",pass.quality()),if enabled {zh} else {source});
            }
            assert_eq!(scope_text("display.solar.pass_quality.","new pass quality"),"new pass quality");
        }
    }
    #[test]
    fn navtex_subject_descriptions_leave_ids_body_and_mandatory_classes_unchanged() {
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for kind in 'A'..='Z' {
                let msg=sdroxide_types::NavtexMessage{station:'N',kind,serial:17,text:"Navigational warning {raw} 1200 UTC".into(),at:0,complete:true,lost:0};
                let before=msg.clone();let source=msg.kind_label();
                let expected_retained=matches!(kind,'G'|'H'|'I'|'J');
                assert_eq!(scope_text("display.navtex.subject.",source)==source,!enabled || expected_retained);
                assert_eq!(msg.is_mandatory(),matches!(kind,'A'|'B'|'D'));assert_eq!(msg,before);
            }
            assert_eq!(scope_text("display.navtex.subject.","new subject description"),"new subject description");
        }
    }
    #[test]
    fn relay_sequence_order_timing_and_user_names_survive_switching_without_transport() {
        let cfg=RelayConfig{link:RelayLink::Serial,channels:vec![
            RelayChannel{index:1,role:RelayRole::SdrAntenna,lead_ms:30,hold_ms:40,..Default::default()},
            RelayChannel{index:2,role:RelayRole::Amplifier,label:"channel 99, then IC-7300 {raw}".into(),lead_ms:0,hold_ms:0,..Default::default()}],..Default::default()};
        let before=cfg.clone();
        for enabled in [true,false,true,false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let result=relay_sequence_note(&cfg);
            assert_eq!(result==cfg.sequence_note(),!enabled);
            assert!(result.contains("channel 99, then IC-7300 {raw}"));
            if enabled {assert_eq!(result,"按下发射键：通道 1：提前 30 ms，然后 channel 99, then IC-7300 {raw}：提前 0 ms。松开发射键：channel 99, then IC-7300 {raw}：延后 0 ms，然后 通道 1：延后 40 ms。");}
            assert_eq!(cfg,before);
            let empty=RelayConfig{channels:vec![],..Default::default()};
            assert_eq!(relay_sequence_note(&empty),if enabled {"不切换任何触点。"} else {"Nothing is switched."});
        }
    }
}
