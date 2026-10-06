use super::UiNotice;
#[test]
fn shell_notice_cache_switches_language_and_preserves_addresses_names_errors() {
    let ext="Station {name} 中文\r\nws://127.0.0.1:1/{e}";
    let cases=[
      (UiNotice::literal("Asked the station for another radio…"),"已请求远程站添加电台…".to_owned()),
      (UiNotice::new(format!("Could not add a radio: {ext}"),"Could not add a radio: {e}",vec![ext.into()]),format!("无法添加电台：{ext}")),
      (UiNotice::new(format!("{ext} was closed at the station."),"{name} was closed at the station.",vec![ext.into()]),format!("远程站已关闭 {ext}。")),
      (UiNotice::new(format!("Could not open {ext}: {ext}"),"Could not open {}: {e}",vec![ext.into(),ext.into()]),format!("无法打开 {ext}：{ext}")),
      (UiNotice::literal("This client cannot open a connection."),"此客户端无法建立连接。".to_owned()),
      (UiNotice::new(format!("{ext} has a tab of its own now."),"{label} has a tab of its own now.",vec![ext.into()]),format!("已为 {ext} 打开独立标签页。")),
      (UiNotice::new(format!("Dialling {ext}…"),"Dialling {}…",vec![ext.into()]),format!("正在连接 {ext}…")),
    ];
    for (notice,zh) in cases {let before=notice.clone();
      for enabled in [true,false,true,false] {super::test_pack_enabled(enabled);
        assert_eq!(notice.display(),if enabled {zh.clone()} else {notice.original().to_owned()});assert_eq!(notice,before);
        // Engine/external String conversion carries no source provenance, even on collisions.
        assert_eq!(UiNotice::from(notice.original()).display(),notice.original());
      }
    }
}
#[test]
fn shell_notice_changed_source_and_bad_arguments_retain_exact_raw() {
    for enabled in [true,false] {super::test_pack_enabled(enabled);
      for n in [UiNotice::new("new source 3".into(),"New source {}",vec!["3".into()]),
        UiNotice::new("changed producer".into(),"Dialling {}…",vec!["abc".into()]),
        UiNotice::new("Dialling abc…".into(),"Dialling {}…",vec![])] {assert_eq!(n.display(),n.original());}
    }
}
#[test]
fn link_tooltips_revert_exactly_without_rewriting_control_constants() {
    let close=crate::chrome::LINK_CLOSE_TIP;let open=crate::chrome::LINK_OPEN_TIP;
    for enabled in [true,false,true,false] {super::test_pack_enabled(enabled);
      let c=super::scope_text("display.shell.link.",close);let o=super::scope_text("display.shell.link.",open);
      assert_eq!(c==close,!enabled);assert_eq!(o==open,!enabled);
      if enabled {assert!(c.contains("仍保持供电"));assert!(c.contains("CAT"));assert!(o.contains("重新连接"));}
      assert_eq!(crate::chrome::LINK_CLOSE_TIP,close);assert_eq!(crate::chrome::LINK_OPEN_TIP,open);
      assert_eq!(super::scope_text("display.shell.link.","upstream changed"),"upstream changed");
    }
}
