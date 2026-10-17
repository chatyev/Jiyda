//! Модульные тесты src/orbit.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn categories_by_site() {
    let cases = [
        ("https://github.com/rust-lang/rust", Some("development")),
        ("https://gist.github.com/x", Some("development")),
        ("http://localhost:3000/", Some("development")),
        ("https://www.figma.com/file/1", Some("design")),
        ("https://docs.rs/serde", Some("documentation")),
        ("https://docs.tauri.app/start/", Some("documentation")),
        ("https://example.com/docs/intro", Some("documentation")),
        ("https://en.wikipedia.org/wiki/Rust", Some("research")),
        ("https://www.amazon.co.uk/dp/1", Some("shopping")),
        ("https://smile.amazon.de/", Some("shopping")),
        ("https://mail.google.com/mail/u/0", Some("communication")),
        ("https://docs.google.com/document/d/1", Some("productivity")),
        ("https://www.google.com/maps/place/x", Some("travel")),
        ("https://www.google.com/search?q=rust", Some("search")),
        ("https://duckduckgo.com/?q=rust", Some("search")),
        ("https://yandex.ru/search/?text=rust", Some("search")),
        ("https://www.youtube.com/watch?v=1", Some("media")),
        ("https://claude.ai/new", Some("ai")),
        ("https://news.ycombinator.com/", Some("news")),
        ("https://example.com/", None),
        ("about:blank", None),
        ("not a url", None),
    ];
    for (url, cat) in cases {
        assert_eq!(category(url), cat, "{url}");
    }
}

#[test]
fn site_suffix_does_not_match_lookalikes() {
    assert_eq!(category("https://notgithub.com/"), None);
    assert_eq!(category("https://github.com.evil.test/"), None);
    assert_eq!(category("https://www.google.com/mapsearch"), None);
    // «amazon.*» — не доменная зона.
    assert_eq!(category("https://shop.amazon/"), None);
}

#[test]
fn groups_sorted_by_size_with_other_last() {
    let pages = [
        (1, "https://github.com/a"),
        (2, "https://example.com/"),
        (3, "https://en.wikipedia.org/wiki/A"),
        (4, "https://github.com/b"),
        (5, "https://stackoverflow.com/q/1"),
        (6, "https://arxiv.org/abs/1"),
    ];
    let g = group(&pages);
    let summary: Vec<_> = g.iter().map(|g| (g.key, g.ids.clone())).collect();
    assert_eq!(summary, [("development", vec![1, 4, 5]), ("research", vec![3, 6]), ("other", vec![2])]);
}

#[test]
fn many_pages_of_unknown_site_make_their_own_group() {
    let pages = [
        (1, "https://intranet.corp/a"),
        (2, "https://intranet.corp/b"),
        (3, "https://www.intranet.corp/c"),
        (4, "https://blog.test/"),
    ];
    let g = group(&pages);
    assert_eq!(g[0], Group { key: "site", label: "intranet.corp".into(), ids: vec![1, 2, 3] });
    assert_eq!(g[1].key, "other");
}

#[test]
fn ties_follow_default_order() {
    let g = group(&[(1, "https://amazon.com/"), (2, "https://github.com/")]);
    assert_eq!(g.iter().map(|g| g.key).collect::<Vec<_>>(), ["development", "shopping"]);
    assert!(group(&[]).is_empty());
}

#[test]
fn domain_strips_www() {
    assert_eq!(domain("https://www.github.com/x"), "github.com");
    assert_eq!(domain("about:blank"), "");
}
