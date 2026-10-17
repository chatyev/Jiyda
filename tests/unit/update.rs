//! Модульные тесты src/update.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn versions_parse() {
    assert_eq!(parse_version("0.3.0"), Some((0, 3, 0)));
    assert_eq!(parse_version("v1.12.7"), Some((1, 12, 7)));
    assert_eq!(parse_version("2.1"), Some((2, 1, 0)));
    for bad in ["", "v", "1", "1.x.0", "0.4.0-beta", "1.2.3.4", "1..2", " "] {
        assert_eq!(parse_version(bad), None, "{bad}");
    }
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
        "https://github.com/chatyev/jiyda/releases/tag/v0.7.0-beta",
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
