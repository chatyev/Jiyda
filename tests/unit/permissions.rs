//! Модульные тесты src/permissions.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn origins() {
    assert_eq!(origin_of("https://meet.google.com/abc-def?x=1").as_deref(), Some("https://meet.google.com"));
    assert_eq!(origin_of("http://localhost:3000/a").as_deref(), Some("http://localhost:3000"));
    assert_eq!(origin_of("about:blank"), None);
    assert_eq!(origin_of("file:///C:/a.html"), None);
}

#[test]
fn set_get_revoke_count() {
    let mut s = Store::default();
    s.set("https://a.test", "camera", Decision::Allow);
    s.set("https://a.test", "microphone", Decision::Allow);
    s.set("https://a.test", "location", Decision::Block);
    assert_eq!(s.get("https://a.test", "camera"), Some(Decision::Allow));
    assert_eq!(s.get("https://b.test", "camera"), None);
    assert_eq!(s.allowed_count("https://a.test"), 2);
    assert!(s.revoke("https://a.test", "camera"));
    assert!(!s.revoke("https://a.test", "camera"));
    assert!(s.revoke("https://a.test", "microphone"));
    assert!(s.revoke("https://a.test", "location"));
    assert!(s.sites.is_empty());
}

#[test]
fn answers_allow_and_remember() {
    assert!(Answer::Once.allows() && Answer::Always.allows());
    assert!(!Answer::Block.allows() && !Answer::Dismiss.allows());
    assert_eq!(Answer::Always.remembered(), Some(Decision::Allow));
    assert_eq!(Answer::Block.remembered(), Some(Decision::Block));
    assert_eq!(Answer::Once.remembered(), None);
    assert_eq!(Answer::Dismiss.remembered(), None);
}

#[test]
fn roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("permissions.json");
    let mut s = Store::default();
    s.set("https://a.test", "notifications", Decision::Block);
    s.save(&p);
    assert_eq!(Store::load(&p), s);
    assert_eq!(Store::load(&dir.path().join("none.json")), Store::default());
}
