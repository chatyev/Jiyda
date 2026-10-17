//! Модульные тесты src/input.rs (подключены там через `#[path]`, видят приватное).

use super::resolve;
const S: &str = "https://s.test/?q=%s";

#[test]
fn urls_and_searches() {
    assert_eq!(resolve("github.com", S).unwrap(), "https://github.com");
    assert_eq!(resolve("ya.ru/search?x=1", S).unwrap(), "https://ya.ru/search?x=1");
    assert_eq!(resolve("localhost:3000", S).unwrap(), "http://localhost:3000");
    assert_eq!(resolve("192.168.1.1", S).unwrap(), "http://192.168.1.1");
    assert_eq!(resolve("https://x.org", S).unwrap(), "https://x.org");
    assert_eq!(resolve("rust borrow checker", S).unwrap(), "https://s.test/?q=rust+borrow+checker");
    assert_eq!(resolve("привет", S).unwrap(), "https://s.test/?q=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82");
    assert_eq!(resolve("3.14", S).unwrap(), "https://s.test/?q=3.14");
    assert_eq!(resolve("javascript:alert(1)", S).unwrap(), "https://s.test/?q=javascript%3Aalert%281%29");
    assert!(resolve("   ", S).is_none());
}

fn r(t: &str) -> String {
    resolve(t, S).unwrap()
}

#[test]
fn empty_input() {
    assert!(resolve("", S).is_none());
    assert!(resolve("\t\n", S).is_none());
}

#[test]
fn trims_whitespace() {
    assert_eq!(r("  github.com  "), "https://github.com");
    assert_eq!(r("\thttps://x.org\n"), "https://x.org");
}

#[test]
fn known_schemes_pass_through() {
    assert_eq!(r("http://x.org"), "http://x.org");
    assert_eq!(r("file:///C:/a.html"), "file:///C:/a.html");
    assert_eq!(r("about:blank"), "about:blank");
    assert_eq!(r("edge://settings"), "edge://settings");
    assert_eq!(r("view-source:https://x.org"), "view-source:https://x.org");
    // Схема узнаётся без учёта регистра, а сам ввод не меняется.
    assert_eq!(r("HTTPS://X.org/Path"), "HTTPS://X.org/Path");
}

#[test]
fn javascript_is_searched_in_any_case() {
    assert_eq!(r("JavaScript:void(0)"), "https://s.test/?q=JavaScript%3Avoid%280%29");
}

#[test]
fn domains_with_port_path_and_userinfo() {
    assert_eq!(r("example.com:8080"), "https://example.com:8080");
    assert_eq!(r("example.com#top"), "https://example.com#top");
    assert_eq!(r("user@host.com"), "https://user@host.com");
    assert_eq!(r("sub.domain.co.uk/a/b"), "https://sub.domain.co.uk/a/b");
    assert_eq!(r("my-site.dev"), "https://my-site.dev");
}

#[test]
fn unicode_domains() {
    assert_eq!(r("пример.рф"), "https://пример.рф");
}

#[test]
fn localhost_and_ipv4_use_http() {
    assert_eq!(r("localhost"), "http://localhost");
    assert_eq!(r("LOCALHOST:8000/api"), "http://LOCALHOST:8000/api");
    assert_eq!(r("127.0.0.1:5173"), "http://127.0.0.1:5173");
}

#[test]
fn invalid_hosts_are_searched() {
    assert_eq!(r("256.1.1.1"), "https://s.test/?q=256.1.1.1");
    assert_eq!(r("1.2.3"), "https://s.test/?q=1.2.3");
    assert_eq!(r(".com"), "https://s.test/?q=.com");
    assert_eq!(r("a..com"), "https://s.test/?q=a..com");
    assert_eq!(r("site.c"), "https://s.test/?q=site.c");
    assert_eq!(r("site.c0m"), "https://s.test/?q=site.c0m");
    assert_eq!(r("word"), "https://s.test/?q=word");
    assert_eq!(r("a@b"), "https://s.test/?q=a%40b");
}

#[test]
fn text_with_spaces_is_searched() {
    assert_eq!(r("github.com is down"), "https://s.test/?q=github.com+is+down");
    assert_eq!(r("c++ & rust"), "https://s.test/?q=c%2B%2B+%26+rust");
}

#[test]
fn search_url_without_placeholder_appends_query() {
    assert_eq!(resolve("a b", "https://s.test/?q=").unwrap(), "https://s.test/?q=a+b");
}

#[test]
fn search_url_with_several_placeholders() {
    assert_eq!(resolve("x", "https://s.test/%s?q=%s").unwrap(), "https://s.test/x?q=x");
}
