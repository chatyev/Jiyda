//! Модульные тесты src/ipc.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn parses_known_messages() {
    assert_eq!(UiMsg::parse(r#"{"type":"activate","id":5}"#), Some(UiMsg::Activate { id: 5 }));
    assert_eq!(UiMsg::parse(r#"{"type":"cmd","k":"orbit"}"#), Some(UiMsg::Cmd { k: crate::command::Command::Orbit }));
    assert_eq!(
        UiMsg::parse(r#"{"type":"navigate","text":"rust","mode":"split"}"#),
        Some(UiMsg::Navigate { text: "rust".into(), mode: NavMode::Split })
    );
    assert_eq!(
        UiMsg::parse(r#"{"type":"navigate","text":"rust"}"#),
        Some(UiMsg::Navigate { text: "rust".into(), mode: NavMode::Current })
    );
    assert_eq!(
        UiMsg::parse(r#"{"type":"region","full":false,"rects":[[0,0,10,10]]}"#),
        Some(UiMsg::Region { full: false, rects: vec![[0.0, 0.0, 10.0, 10.0]], holes: vec![] })
    );
    assert_eq!(UiMsg::parse(r#"{"type":"split_swap"}"#), Some(UiMsg::SplitSwap));
    assert_eq!(UiMsg::parse(r#"{"type":"window_drag"}"#), Some(UiMsg::WindowDrag));
    assert_eq!(UiMsg::parse(r#"{"type":"window_menu"}"#), Some(UiMsg::WindowMenu));
    assert_eq!(
        UiMsg::parse(r#"{"type":"setting","key":"sleep_minutes","value":30}"#),
        Some(UiMsg::Setting(crate::settings::SettingChange::SleepMinutes(30)))
    );
    assert_eq!(UiMsg::parse(r#"{"type":"setting","key":"sleep_minutes","value":"30"}"#), None);
    assert_eq!(
        UiMsg::parse(r#"{"type":"permission_answer","id":3,"decision":"always"}"#),
        Some(UiMsg::PermissionAnswer { id: 3, decision: crate::permissions::Answer::Always })
    );
}

#[test]
fn rejects_unknown_or_malformed() {
    for bad in [
        r#"{"type":"nope"}"#,
        r#"{"type":"activate"}"#,
        r#"{"type":"activate","id":"5"}"#,
        r#"{"id":5}"#,
        r#"{"type":"navigate","text":"x","mode":"weird"}"#,
        // Старый формат: сайт в ответе интерфейса больше не принимается.
        r#"{"type":"permission_answer","id":3,"decision":"always|https://evil.test|camera"}"#,
        "not json",
        "null",
    ] {
        assert_eq!(UiMsg::parse(bad), None, "{bad}");
    }
}
