//! Модульные тесты src/settings.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn parse(json: &str) -> Option<SettingChange> {
    serde_json::from_str(json).ok()
}

#[test]
fn parses_typed_values() {
    assert_eq!(parse(r#"{"key":"theme","value":"dark"}"#), Some(SettingChange::Theme(Theme::Dark)));
    assert_eq!(parse(r#"{"key":"sleep_minutes","value":30}"#), Some(SettingChange::SleepMinutes(30)));
    assert_eq!(
        parse(r#"{"key":"shortcut","value":{"cmd":"orbit","keys":"Alt+O"}}"#),
        Some(SettingChange::Shortcut(Shortcut { cmd: Command::Orbit, keys: "Alt+O".into() }))
    );
    assert_eq!(parse(r#"{"key":"shortcut_reset","value":null}"#), Some(SettingChange::ShortcutReset(None)));
    assert_eq!(
        parse(r#"{"key":"shortcut_reset","value":"find"}"#),
        Some(SettingChange::ShortcutReset(Some(Command::Find)))
    );
}

#[test]
fn rejects_wrong_types_and_keys() {
    for bad in [
        r#"{"key":"theme","value":"pink"}"#,
        r#"{"key":"sleep_minutes","value":"30"}"#,
        r#"{"key":"sleep_minutes","value":-1}"#,
        r#"{"key":"shield","value":1}"#,
        r#"{"key":"nope","value":true}"#,
        r#"{"key":"shortcut","value":{"cmd":"nope","keys":"Alt+O"}}"#,
    ] {
        assert_eq!(parse(bad), None, "{bad}");
    }
}

#[test]
fn apply_validates_and_reports_restart() {
    let mut c = Config::default();
    assert_eq!(SettingChange::SleepMinutes(30).apply(&mut c), Ok(Applied { restart: false }));
    assert_eq!(c.sleep_minutes, 30);
    assert!(SettingChange::SleepMinutes(SLEEP_MINUTES_MAX + 1).apply(&mut c).is_err());
    assert!(SettingChange::SearchUrl("http://x/?q=%s".into()).apply(&mut c).is_err());
    assert!(SettingChange::SearchUrl("https://x/".into()).apply(&mut c).is_err());
    assert!(SettingChange::SearchUrl(" https://s.test/?q=%s ".into()).apply(&mut c).is_ok());
    assert_eq!(c.search_url, "https://s.test/?q=%s");
    assert_eq!(SettingChange::Devtools(false).apply(&mut c), Ok(Applied { restart: true }));
    assert_eq!(SettingChange::Language(Language::En).apply(&mut c).map(|a| a.restart), Ok(true));
    assert_eq!(c.language, "en");
    assert_eq!(SettingChange::Language(Language::Uz).apply(&mut c).map(|a| a.restart), Ok(true));
    assert_eq!(c.language, "uz");
    assert!(SettingChange::DownloadDir(r"Z:\no\such\dir".into()).apply(&mut c).is_err());
}

#[test]
fn shortcuts_are_checked_and_reset() {
    let mut c = Config::default();
    let set = |cmd, keys: &str| SettingChange::Shortcut(Shortcut { cmd, keys: keys.into() });
    assert!(set(Command::Orbit, "Alt+O").apply(&mut c).is_ok());
    assert!(set(Command::Find, "").apply(&mut c).is_ok());
    assert!(set(Command::Find, "K").apply(&mut c).is_err());
    assert!(set(Command::Space(1), "Alt+1").apply(&mut c).is_err());
    assert_eq!(c.shortcuts.get("orbit").map(String::as_str), Some("Alt+O"));
    SettingChange::ShortcutReset(Some(Command::Orbit)).apply(&mut c).unwrap();
    assert!(!c.shortcuts.contains_key("orbit"));
    SettingChange::ShortcutReset(None).apply(&mut c).unwrap();
    assert!(c.shortcuts.is_empty());
}
