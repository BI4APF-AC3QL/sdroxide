//! Projection of an error with local producer provenance; no socket or language lookup at production.
pub fn connection_error_notice(original: String, origin: Option<sdroxide_types::RadioEventTextOrigin>) -> super::UiNotice {
    use sdroxide_types::RadioEventTextOrigin as O;
    match origin {
        Some(O::ServerBusy) => super::UiNotice::new(original, "server busy — another client is connected", vec![]),
        Some(O::SocketClosed) => super::UiNotice::new(original, "connection closed", vec![]),
        Some(O::ProtocolDecode) => {
            let Some(detail) = original.strip_prefix("protocol error: ") else { return original.into() };
            let detail = detail.to_owned();
            super::UiNotice::new(original, "protocol error: {e}", vec![detail])
        }
        Some(O::SilentMic) | None => original.into(),
    }
}

pub fn radio_notice_projection(original: Option<String>, origin: Option<sdroxide_types::RadioEventTextOrigin>) -> Option<super::UiNotice> {
    original.map(|raw| match origin {
        Some(sdroxide_types::RadioEventTextOrigin::SilentMic) => super::UiNotice::new(raw,
            "Transmitting, but this client's microphone is producing no audio at all — nothing is being modulated. In a browser, check that the page was allowed the microphone and that no other application holds it.", vec![]),
        // These exact messages come from sdroxide's own null radio source.
        // Other device, server and driver notices retain their original text.
        None => fixed_null_radio_notice(raw),
        _ => raw.into(),
    })
}

fn fixed_null_radio_notice(raw: String) -> super::UiNotice {
    const NO_INTERFACE: &str = "no radio interface selected — choose one in Settings → Radio";
    const NO_INTERFACE_RETRY: &str = "no radio interface selected — choose one in Settings → Radio. Retrying — or open Settings → Radio to choose another interface.";
    const NO_INTERFACE_REOPEN: &str = "no radio interface selected — choose one in Settings → Radio Retrying — or open Settings → Radio to choose another interface.";
    const SWITCHED_OFF: &str = "This radio is switched off — its interface is not open. Switch it back on with the power button above the A/B selector (in the VFO menu on a compact layout), on its tab, or in Settings → Radio.";
    const ATTACHED: &str = "This radio is the panadapter for radio {} — its spectrum and audio are on that radio's tab. Clear the Panadapter receiver box there to use it on its own again.";
    match raw.as_str() {
        NO_INTERFACE => return super::UiNotice::new(raw, NO_INTERFACE, vec![]),
        NO_INTERFACE_RETRY => return super::UiNotice::new(raw, NO_INTERFACE_RETRY, vec![]),
        NO_INTERFACE_REOPEN => return super::UiNotice::new(raw, NO_INTERFACE_REOPEN, vec![]),
        SWITCHED_OFF => return super::UiNotice::new(raw, SWITCHED_OFF, vec![]),
        _ => {}
    }
    let prefix = "This radio is the panadapter for radio ";
    let suffix = " — its spectrum and audio are on that radio's tab. Clear the Panadapter receiver box there to use it on its own again.";
    if let Some(owner) = raw.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(suffix))
        && owner.parse::<u32>().is_ok()
    {
        return super::UiNotice::new(raw.clone(), ATTACHED, vec![owner.to_owned()]);
    }
    raw.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdroxide_types::{RadioEventTextOrigin as O, RadioController, RadioEvent, Command};
    #[test]
    fn cached_connection_errors_switch_without_rewriting_originals() {
        let detail = "D:\\radio\\{raw}.bin: 400\nCRC 错误";
        let cases = [
            ("server busy — another client is connected".to_owned(), O::ServerBusy, "服务器忙：已有其他客户端连接".to_owned()),
            ("connection closed".to_owned(), O::SocketClosed, "连接已关闭".to_owned()),
            (format!("protocol error: {detail}"), O::ProtocolDecode, format!("协议错误：{detail}")),
        ];
        for (raw, origin, zh) in cases {
            let notice = connection_error_notice(raw.clone(), Some(origin));
            let snapshot = notice.clone();
            for enabled in [true, false, true, false] {
                crate::language_plugin::test_pack_enabled(enabled);
                assert_eq!(notice.display(), if enabled { zh.clone() } else { raw.clone() });
                assert_eq!(notice.original(), raw);
                assert_eq!(notice, snapshot);
                assert_eq!(connection_error_notice(raw.clone(), None).display(), raw);
            }
        }
    }
    #[test]
    fn changed_producer_and_mismatched_origin_fall_back_to_exact_original() {
        for enabled in [true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            for (raw, origin) in [("closed by new upstream", O::SocketClosed), ("connection closed", O::ServerBusy),
                                  ("new protocol error: detail", O::ProtocolDecode)] {
                assert_eq!(connection_error_notice(raw.into(), Some(origin)).display(), raw);
            }
        }
    }
    #[test]
    fn existing_controllers_default_to_external_error_provenance() {
        struct Offline;
        impl RadioController for Offline {
            fn send(&mut self, _: Command) { panic!("No commands may be sent by this offline test") }
            fn poll_event(&mut self) -> Option<RadioEvent> { None }
        }
        let ctrl = Offline;
        assert_eq!(ctrl.event_text_origin(), None);
        crate::language_plugin::test_pack_enabled(true);
        assert_eq!(connection_error_notice("connection closed".into(), ctrl.event_text_origin()).display(), "connection closed");
    }
    #[test]
    fn cached_local_mic_notice_switches_but_external_and_changed_notes_do_not() {
        let raw = "Transmitting, but this client's microphone is producing no audio at all — nothing is being modulated. In a browser, check that the page was allowed the microphone and that no other application holds it.";
        let notice = radio_notice_projection(Some(raw.into()), Some(O::SilentMic)).unwrap();
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let text = notice.display();
            if enabled { assert!(text.contains("麦克风完全没有音频输出")); } else { assert_eq!(text, raw); }
            assert_eq!(notice.original(), raw);
            assert_eq!(radio_notice_projection(Some(raw.into()), None).unwrap().display(), raw);
            assert_eq!(radio_notice_projection(Some("changed source".into()), Some(O::SilentMic)).unwrap().display(), "changed source");
            assert_eq!(radio_notice_projection(Some(raw.into()), Some(O::ServerBusy)).unwrap().display(), raw);
            assert!(radio_notice_projection(None, Some(O::SilentMic)).is_none());
        }
    }

    #[test]
    fn null_radio_notice_switches_language_without_changing_device_errors() {
        let raw = "no radio interface selected — choose one in Settings → Radio. Retrying — or open Settings → Radio to choose another interface.";
        let off = "This radio is switched off — its interface is not open. Switch it back on with the power button above the A/B selector (in the VFO menu on a compact layout), on its tab, or in Settings → Radio.";
        let attached = "This radio is the panadapter for radio 2 — its spectrum and audio are on that radio's tab. Clear the Panadapter receiver box there to use it on its own again.";
        for enabled in [true, false, true, false] {
            crate::language_plugin::test_pack_enabled(enabled);
            let notice = radio_notice_projection(Some(raw.into()), None).unwrap();
            assert_eq!(notice.display(), if enabled { "未选择电台接口。请在“设置 → 电台”选择接口；程序正在重试，您也可以改选其他接口。" } else { raw });
            assert_eq!(notice.original(), raw);
            let power = radio_notice_projection(Some(off.into()), None).unwrap();
            assert_eq!(power.display(), if enabled { "该电台已关闭，接口未打开。可通过 A/B 选择器上方的电源按钮（紧凑布局中位于 VFO 菜单）、电台标签页或“设置 → 电台”重新开启。" } else { off });
            let paired = radio_notice_projection(Some(attached.into()), None).unwrap();
            assert_eq!(paired.display(), if enabled { "此电台是第 2 台电台的频谱接收机，其频谱和音频显示在那台电台的标签页。若要单独使用，请在那里取消勾选“频谱接收机”。" } else { attached });
            assert_eq!(radio_notice_projection(Some("SDRplay device is busy".into()), None).unwrap().display(), "SDRplay device is busy");
            assert_eq!(radio_notice_projection(Some(raw.into()), Some(O::ServerBusy)).unwrap().display(), raw);
        }
    }
}
