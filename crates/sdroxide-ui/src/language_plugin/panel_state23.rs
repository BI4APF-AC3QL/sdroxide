//! Narrow typed presentation boundaries; no state-machine, text expansion or I/O changes.
pub fn packet_state_display(link: &sdroxide_types::PacketLink) -> String {
    match link.state.as_str() {
        "Disconnected"|"AwaitingConnection"|"Connected"|"TimerRecovery"|"AwaitingRelease" => super::scope_text("display.packet.state.",&link.state),
        _ => link.state.clone(),
    }
}
pub fn sstv_placeholder_help(token: &str, description: &str) -> String {
    if crate::sstv::BANNER_PLACEHOLDERS.contains(&(token,description)) {
        super::scope_text("display.sstv.placeholder.",description)
    } else {description.to_owned()}
}
pub fn rifp_sender_display(sender: Option<&str>) -> String {
    sender.map(str::to_owned).unwrap_or_else(||super::scope_text("display.rifp.sender.","unidentified"))
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn packet_status_display_preserves_raw_schema_and_unknown_states() {
  for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
   for (state,zh) in [("Disconnected","已断开"),("AwaitingConnection","等待连接"),("Connected","已连接"),("TimerRecovery","定时器恢复"),("AwaitingRelease","等待断开")]{
    let link=sdroxide_types::PacketLink{state:state.into(),peer:Some("Connected {call}".into()),via:vec!["AwaitingRelease".into()],..Default::default()};
    let before=serde_json::to_value(&link).unwrap();assert_eq!(packet_state_display(&link),if enabled {zh}else{state});assert_eq!(serde_json::to_value(&link).unwrap(),before);
   }
   for state in ["", "NewUpstreamState", "Connected {call}", "connected"] {let link=sdroxide_types::PacketLink{state:state.into(),..Default::default()};assert_eq!(packet_state_display(&link),state);}
  }
 }
 #[test]
 fn sstv_hint_localization_does_not_translate_substitution_tokens_or_on_air_text() {
  let original=crate::sstv::BANNER_PLACEHOLDERS;
  let banner="{call} {grid} {version} {unknown} 中文";
  let baseline=crate::sstv::expand(banner,"bi4apf","PM01");
  for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
   for (token,desc) in original {let help=sstv_placeholder_help(token,desc);assert_eq!(help==desc,!enabled);assert_eq!(sstv_placeholder_help("{unknown}",desc),desc);}
   assert_eq!(sstv_placeholder_help("{call}","upstream changed"),"upstream changed");assert_eq!(crate::sstv::BANNER_PLACEHOLDERS,original);
   assert_eq!(crate::sstv::expand(banner,"bi4apf","PM01"),baseline);assert!(baseline.starts_with("BI4APF PM01"));assert!(baseline.ends_with("{unknown} 中文"));
  }
 }
 #[test]
 fn rifp_missing_sender_and_external_same_name_have_distinct_provenance() {
  for enabled in [true,false,true,false] {crate::language_plugin::test_pack_enabled(enabled);
   assert_eq!(rifp_sender_display(None),if enabled {"未识别的发送方"}else{"unidentified"});
   for name in ["unidentified","","未识别的发送方","BI4APF {call}\r\n原文"] {assert_eq!(rifp_sender_display(Some(name)),name);}
  }
 }
}
