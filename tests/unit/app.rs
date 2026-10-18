//! Модульные тесты src/app/mod.rs (подключены там через `#[path]`, видят приватное).

use super::{
    KeyEvent, PageEvent, SPLIT_GAP, UserEvent, base64, find_script, js_string_result, page_event, parse_find_result,
    split_boxes,
};

#[test]
fn base64_matches_standard_alphabet_and_padding() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    assert_eq!(base64(&[0xff, 0xfe, 0x00]), "//4A");
}

#[test]
fn split_boxes_fill_area_around_gap() {
    let [l, r] = split_boxes((100.0, 8.0, 806.0, 500.0), 0.5);
    assert_eq!(l, (100.0, 8.0, 400.0, 500.0));
    assert_eq!(r, (100.0 + 400.0 + SPLIT_GAP, 8.0, 400.0, 500.0));
    // Половины и промежуток занимают ровно всю ширину.
    let [l, r] = split_boxes((0.0, 0.0, 1001.0, 10.0), 0.3);
    assert_eq!(l.2 + SPLIT_GAP + r.2, 1001.0);
    assert_eq!(r.0, l.2 + SPLIT_GAP);
}

#[test]
fn split_boxes_never_negative() {
    let [l, r] = split_boxes((0.0, 0.0, 4.0, 10.0), 0.5);
    assert!(l.2 >= 0.0 && r.2 >= 0.0);
}

#[test]
fn page_text_result_is_unquoted() {
    assert_eq!(js_string_result(r#""a\nb""#), "a\nb");
    assert_eq!(js_string_result("null"), "");
}

#[test]
fn find_result_with_matches() {
    assert_eq!(parse_find_result(r#"{"n":7,"i":2}"#), Some((7, Some(2))));
    assert_eq!(parse_find_result(r#"{"n":1,"i":0,"extra":"<script>"}"#), Some((1, Some(0))));
}

#[test]
fn find_result_without_matches() {
    assert_eq!(parse_find_result(r#"{"n":0,"i":-1}"#), Some((0, None)));
}

#[test]
fn find_result_rejects_bad_values() {
    for bad in [
        r#"{"n":3,"i":3}"#,
        r#"{"n":3,"i":-1}"#,
        r#"{"n":0,"i":0}"#,
        r#"{"n":-1,"i":-1}"#,
        r#"{"n":"3","i":0}"#,
        r#"{"n":3}"#,
        r#""{\"n\":1,\"i\":0}""#,
        "null",
        "not json",
    ] {
        assert_eq!(parse_find_result(bad), None, "{bad}");
    }
}

#[test]
fn find_script_calls_function_with_json_arguments() {
    let js = find_script("a\"b'</script>\n", "next");
    assert!(js.starts_with('(') && js.ends_with(r#")("a\"b'</script>\n", "next")"#), "{js}");
}

// ---------- События движка → события ядра (page_event) ----------

#[test]
fn engine_messages_go_to_ui_or_page() {
    assert!(matches!(page_event(None, PageEvent::Message("m".into())), Some(UserEvent::FromUi(m)) if m == "m"));
    assert!(matches!(
        page_event(Some(7), PageEvent::Message("m".into())),
        Some(UserEvent::FromTab { id: 7, msg }) if msg == "m"
    ));
}

#[test]
fn page_events_keep_their_page() {
    assert!(
        matches!(page_event(Some(3), PageEvent::Title("T".into())), Some(UserEvent::Title { id: 3, title }) if title == "T")
    );
    assert!(matches!(
        page_event(Some(3), PageEvent::Load { started: false, url: "u".into() }),
        Some(UserEvent::Load { id: 3, started: false, .. })
    ));
    assert!(matches!(
        page_event(Some(3), PageEvent::Fullscreen(true)),
        Some(UserEvent::PageFullscreen { id: 3, on: true })
    ));
    assert!(matches!(
        page_event(Some(3), PageEvent::PermissionAsk { ask: 9, origin: "https://a.test".into(), kind: "camera" }),
        Some(UserEvent::PermissionAsk { tab: 3, id: 9, kind: "camera", .. })
    ));
}

#[test]
fn keys_from_ui_and_pages() {
    use crate::command::Command;
    assert!(matches!(
        page_event(None, PageEvent::Key(KeyEvent::Command(Command::Orbit))),
        Some(UserEvent::Key { id: None, cmd: Command::Orbit })
    ));
    assert!(matches!(
        page_event(Some(5), PageEvent::Key(KeyEvent::Command(Command::Orbit))),
        Some(UserEvent::Key { id: Some(5), .. })
    ));
    assert!(
        matches!(page_event(Some(5), PageEvent::Key(KeyEvent::Recorded("Ctrl+K".into()))), Some(UserEvent::KeyRecorded(c)) if c == "Ctrl+K")
    );
}

#[test]
fn page_only_events_without_page_are_dropped() {
    assert!(page_event(None, PageEvent::Title("T".into())).is_none());
    assert!(page_event(None, PageEvent::Favicon(vec![1])).is_none());
}
