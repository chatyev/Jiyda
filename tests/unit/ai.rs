//! Модульные тесты src/ai.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn body_has_model_pages_and_selection() {
    let pages = vec![("Rust".to_string(), "https://rust-lang.org/".to_string(), "Rust is fast.".to_string())];
    let b = body(&Request { prompt: "Summarize", lang: "en", selection: "fast", pages: &pages });
    assert_eq!(b["model"], MODEL);
    assert_eq!(b["fallbacks"], "default");
    assert_eq!(b["output_config"]["effort"], "medium");
    let user = b["messages"][0]["content"].as_str().unwrap();
    assert!(user.contains("<page index=\"1\" title=\"Rust\" url=\"https://rust-lang.org/\">"));
    assert!(user.contains("<selection>\nfast\n</selection>"));
    assert!(user.ends_with("Summarize"));
    assert!(b["system"].as_str().unwrap().ends_with("English."));
}

#[test]
fn long_pages_are_clipped_with_marker() {
    let pages = vec![("A".to_string(), "u".to_string(), "я".repeat(CONTEXT_LIMIT + 10))];
    let b = body(&Request { prompt: "q", lang: "ru", selection: "", pages: &pages });
    let user = b["messages"][0]["content"].as_str().unwrap();
    assert!(user.contains("[…truncated]"));
    assert!(user.chars().count() < CONTEXT_LIMIT + 200);
}

#[test]
fn parse_text_errors_and_refusals() {
    let ok = br#"{"content":[{"type":"thinking","thinking":""},{"type":"text","text":"Hi"},{"type":"text","text":"!"}],"stop_reason":"end_turn"}"#;
    assert_eq!(parse(200, ok).unwrap(), "Hi!");
    let cut = br#"{"content":[{"type":"text","text":"Hi"}],"stop_reason":"max_tokens"}"#;
    // В тестах язык — русский (см. i18n::lang).
    assert!(parse(200, cut).unwrap().ends_with("ответ обрезан]"));
    assert!(parse(200, br#"{"content":[],"stop_reason":"refusal"}"#).unwrap_err().contains("отказался"));
    assert!(parse(401, br#"{"error":{"message":"invalid x-api-key"}}"#).unwrap_err().contains("Ключ API"));
    assert!(parse(400, br#"{"error":{"message":"bad"}}"#).unwrap_err().contains("bad"));
    assert!(parse(502, b"<html>").is_err());
}
