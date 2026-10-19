//! Модульные тесты src/update.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn versions_parse() {
    let core = |s: &str| parse_version(s).map(|v| v.core);
    assert_eq!(core("0.3.0"), Some((0, 3, 0)));
    assert_eq!(core("v1.12.7"), Some((1, 12, 7)));
    assert_eq!(core("2.1"), Some((2, 1, 0)));
    assert_eq!(core("0.6.5-beta.1"), Some((0, 6, 5)));
    assert!(parse_version("0.6.5-beta.1").unwrap().is_prerelease());
    assert!(parse_version("0.7.0-rc").unwrap().is_prerelease());
    assert!(!parse_version("0.6.4").unwrap().is_prerelease());
    for bad in [
        "",
        "v",
        "1",
        "1.x.0",
        "1.2.3.4",
        "1..2",
        " ",
        "0.4.0-",
        "0.4.0-Beta",
        "0.4.0-beta.",
        "0.4.0-beta.x",
        "0.4.0-beta.1.2",
        "0.4.0-be-ta",
    ] {
        assert_eq!(parse_version(bad), None, "{bad}");
    }
}

#[test]
fn prerelease_is_older_than_its_release() {
    assert!(is_newer("0.6.5", "0.6.5-beta.3"));
    assert!(is_newer("0.6.5-beta.2", "0.6.5-beta.1"));
    assert!(is_newer("0.6.5-beta.10", "0.6.5-beta.9"));
    assert!(is_newer("0.6.5-rc.1", "0.6.5-beta.4"));
    assert!(is_newer("0.6.5-beta.1", "0.6.4"));
    assert!(!is_newer("0.6.5-beta.1", "0.6.5"));
    assert!(!is_newer("0.6.4", "0.6.5-beta.1"));
}

#[test]
fn beta_channel_takes_the_newest_tag_from_the_feed() {
    let feed = r#"<feed><link rel="alternate" href="https://github.com/chatyev/jiyda/releases"/>
      <entry><link rel="alternate" type="text/html" href="https://github.com/chatyev/jiyda/releases/tag/v0.6.4"/></entry>
      <entry><link rel="alternate" type="text/html" href="https://github.com/chatyev/jiyda/releases/tag/v0.6.5-beta.2"/></entry>
      <entry><link rel="alternate" type="text/html" href="https://github.com/chatyev/jiyda/releases/tag/v0.6.5-beta.10"/></entry>
      <entry><link rel="alternate" href="https://evil.example/chatyev/jiyda/releases/tag/v9.9.9"/></entry>
      <entry><link rel="alternate" href="https://github.com/chatyev/jiyda/releases/tag/nightly"/></entry></feed>"#;
    let r = latest_in_feed(feed).unwrap();
    assert_eq!(r.version, "0.6.5-beta.10");
    assert_eq!(r.url, "https://github.com/chatyev/jiyda/releases/download/v0.6.5-beta.10/JiydaSetup-0.6.5-beta.10.exe");
    assert_eq!(latest_in_feed("<feed></feed>"), None);
}

#[test]
fn newer_compares_numbers_not_text() {
    assert!(is_newer("0.10.0", "0.9.9"));
    assert!(is_newer("1.0.0", "0.99.99"));
    assert!(is_newer("0.3.1", "0.3.0"));
    assert!(!is_newer("0.3.0", "0.3.0"));
    assert!(!is_newer("0.2.9", "0.3.0"));
    assert!(!is_newer("garbage", "0.3.0"));
    assert!(!is_newer("0.4.0", "garbage"));
}

#[test]
fn latest_release_from_redirect() {
    let r = release_from_location("https://github.com/chatyev/jiyda/releases/tag/v0.6.1").unwrap();
    assert_eq!(
        r,
        Release {
            version: "0.6.1".into(),
            url: "https://github.com/chatyev/jiyda/releases/download/v0.6.1/JiydaSetup-0.6.1.exe".into(),
        }
    );
}

#[test]
fn repo_name_case_does_not_matter() {
    // Так GitHub отвечал, пока репозиторий назывался chatyev/Jiyda.
    let r = release_from_location("https://github.com/chatyev/Jiyda/releases/tag/v0.6.0").unwrap();
    assert_eq!(r.version, "0.6.0");
    assert!(r.url.starts_with(DOWNLOAD_PREFIX));
}

#[test]
fn foreign_or_odd_redirects_are_rejected() {
    for location in [
        // Выпусков нет — GitHub ведёт на список.
        "https://github.com/chatyev/jiyda/releases",
        "https://evil.example/chatyev/jiyda/releases/tag/v0.6.0",
        "http://github.com/chatyev/jiyda/releases/tag/v0.6.0",
        "https://github.com/someone/jiyda/releases/tag/v0.6.0",
        "https://github.com/chatyev/jiyda/releases/tag/nightly",
        "https://github.com/chatyev/jiyda/releases/tag/v0.7.0-Beta",
        "https://github.com/chatyev/jiyda/releases/tag/v0.6.0/../../x",
        "https://github.com/chatyev/jiyda/releases/tag/",
        "",
    ] {
        assert_eq!(release_from_location(location), None, "{location}");
    }
}

#[test]
fn installer_sanity_check() {
    let mut exe = vec![0u8; MIN_INSTALLER];
    exe[..2].copy_from_slice(b"MZ");
    assert!(looks_like_installer(&exe));
    assert!(!looks_like_installer(b"MZ tiny"));
    let mut html = vec![b' '; MIN_INSTALLER];
    html[..5].copy_from_slice(b"<html");
    assert!(!looks_like_installer(&html));
}

#[test]
fn checksum_file_parses_and_matches() {
    // SHA-256 пустой строки и строки «abc» — известные значения.
    let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let abc = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD";
    let e = parse_sha256(&format!("{empty}  JiydaSetup-0.6.2.exe\r\n")).unwrap();
    assert!(sha256_matches(b"", &e));
    assert!(!sha256_matches(b"x", &e));
    // Регистр и «только хэш» тоже годятся (Get-FileHash пишет заглавными).
    assert!(sha256_matches(b"abc", &parse_sha256(abc).unwrap()));
    for bad in ["", "abc", &empty[..63], &format!("{}zz", &empty[..62]), "<html>not found</html>"] {
        assert_eq!(parse_sha256(bad), None, "{bad}");
    }
}

#[test]
fn download_refuses_foreign_url_without_network() {
    let r = Release { version: "9.9.9".into(), url: "https://evil.example/x.exe".into() };
    assert!(download(&r, Path::new(".")).unwrap_err().contains("не с GitHub Releases"));
}
