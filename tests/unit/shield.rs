//! Модульные тесты src/shield.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn classifies_trackers_and_ads() {
    assert_eq!(classify("https://www.google-analytics.com/g/collect?v=2"), Some(Kind::Tracker));
    assert_eq!(classify("https://mc.yandex.ru/watch/1"), Some(Kind::Tracker));
    assert_eq!(classify("https://connect.facebook.net/en_US/fbevents.js"), Some(Kind::Tracker));
    assert_eq!(classify("https://www.facebook.com/tr?id=1"), Some(Kind::Tracker));
    assert_eq!(classify("https://securepubads.g.doubleclick.net/tag/js/gpt.js"), Some(Kind::Ad));
    assert_eq!(classify("https://pagead2.googlesyndication.com/x.js"), Some(Kind::Ad));
}

#[test]
fn leaves_normal_requests_alone() {
    for ok in [
        "https://connect.facebook.net/en_US/sdk.js",
        "https://www.facebook.com/login",
        "https://www.google.com/search?q=x",
        "https://notdoubleclick.net/",
        "https://doubleclick.net.example.com/",
        "https://github.com/",
        "not a url",
    ] {
        assert_eq!(classify(ok), None, "{ok}");
    }
}

#[test]
fn every_entry_matches_its_own_filters_example() {
    for e in TRACKERS.iter().chain(ADS) {
        let (host, path) =
            e.split_once('/').map_or((*e, "/x.js".to_string()), |(h, p)| (h, format!("/{}", p.replace('*', "en"))));
        assert!(classify(&format!("https://{host}{path}")).is_some(), "{e}");
        assert!(classify(&format!("https://sub.{host}{path}")).is_some(), "{e}");
    }
    assert_eq!(filters("hotjar.com"), ["*://hotjar.com/*", "*://*.hotjar.com/*"]);
    assert_eq!(filters("facebook.com/tr"), ["*://facebook.com/tr*", "*://*.facebook.com/tr*"]);
    assert_eq!(all_filters().len(), 2 * (TRACKERS.len() + ADS.len()));
}

#[test]
fn path_wildcards() {
    assert!(path_matches("/en_US/fbevents.js", "/*/fbevents.js"));
    assert!(!path_matches("/en_US/sdk.js", "/*/fbevents.js"));
    assert!(path_matches("/tr", "/tr"));
}

#[test]
fn daily_counter_resets_each_day() {
    let mut d = Daily::default();
    d.add("2026-10-16", Kind::Tracker);
    d.add("2026-10-16", Kind::Ad);
    d.add("2026-10-16", Kind::Tracker);
    assert_eq!(d.for_day("2026-10-16"), (2, 1));
    assert_eq!(d.for_day("2026-10-17"), (0, 0));
    d.add("2026-10-17", Kind::Ad);
    assert_eq!((d.day.as_str(), d.trackers, d.ads), ("2026-10-17", 0, 1));
}

#[test]
fn daily_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("shield.json");
    let d = Daily { day: "2026-10-17".into(), trackers: 3, ads: 4 };
    d.save(&p);
    assert_eq!(Daily::load(&p), d);
    assert_eq!(Daily::load(&dir.path().join("none.json")), Daily::default());
}

#[test]
fn site_strips_www() {
    assert_eq!(site_of("https://www.Example.com/a").as_deref(), Some("example.com"));
    assert_eq!(site_of("about:blank"), None);
}
