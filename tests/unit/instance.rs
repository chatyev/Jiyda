//! Модульные тесты src/instance.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn cwd() -> std::path::PathBuf {
    std::env::temp_dir()
}

#[test]
fn links_pass_through() {
    for u in ["https://example.com/a?b=1", "http://x.test", "file:///C:/a.html", "about:blank", "HTTPS://X.TEST"] {
        assert_eq!(arg_to_url(u, &cwd()).as_deref(), Some(u), "{u}");
    }
    assert_eq!(arg_to_url("\"https://q.test/\"", &cwd()).as_deref(), Some("https://q.test/"));
}

#[test]
fn flags_and_junk_are_ignored() {
    for a in ["", "  ", "--flag", "-x", "hello world", "javascript:alert(1)", "data:text/html,x", "word"] {
        assert_eq!(arg_to_url(a, &cwd()), None, "{a}");
    }
}

#[test]
fn bare_domain_becomes_https() {
    assert_eq!(arg_to_url("example.com", &cwd()).as_deref(), Some("https://example.com"));
    assert_eq!(arg_to_url("localhost:3000", &cwd()).as_deref(), Some("http://localhost:3000"));
}

#[test]
fn existing_file_becomes_file_url() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("page one.html"), "<p>x</p>").unwrap();
    let url = arg_to_url("page one.html", dir.path()).unwrap();
    assert!(url.starts_with("file:///") && url.ends_with("/page%20one.html"), "{url}");
    let abs = dir.path().join("page one.html");
    assert!(arg_to_url(abs.to_str().unwrap(), &cwd()).unwrap().starts_with("file:///"));
}

#[test]
fn urls_from_args_keeps_order_and_skips_junk() {
    let args = ["--x", "https://a.test", "nothing here", "b.test"].map(String::from);
    assert_eq!(urls_from_args(args, &cwd()), ["https://a.test", "https://b.test"]);
}

#[test]
fn message_roundtrip() {
    let urls = vec!["https://a.test/".to_string(), "file:///C:/x.html".to_string()];
    assert_eq!(decode(&encode(&urls)), urls);
    assert!(decode(&encode(&[])).is_empty());
}

#[test]
fn decode_drops_dangerous_and_oversized() {
    let long = format!("https://a.test/{}", "x".repeat(MAX_URL_LEN));
    let msg = format!("javascript:alert(1)\r\ndata:text/html,<b>\n  https://ok.test/  \n{long}\n\n");
    assert_eq!(decode(msg.as_bytes()), ["https://ok.test/"]);
    let many: String = (0..100).map(|i| format!("https://t.test/{i}\n")).collect();
    assert_eq!(decode(many.as_bytes()).len(), MAX_URLS);
    assert!(decode(&[0xff, 0xfe, b'\n']).is_empty());
}

#[test]
fn pipe_name_is_safe() {
    let n = pipe_name();
    assert!(n.starts_with(r"\\.\pipe\jiyda-"));
    assert!(n[r"\\.\pipe\".len()..].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
}
