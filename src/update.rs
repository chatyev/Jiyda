//! Автообновление: узнать последнюю версию на GitHub Releases, скачать установщик
//! и запустить его. Установщик (installer/jiyda.iss) ставит новую версию поверх и,
//! раз запущен с /update=1, открывает Jiyda снова.
//!
//! Сеть — через WinHTTP из Windows: системные прокси и сертификаты, без своих
//! библиотек HTTPS. Запросы идут только на api.github.com и github.com.

use std::path::{Path, PathBuf};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_API: &str = "https://api.github.com/repos/chatyev/jiyda/releases/latest";
/// Скачиваем только отсюда — что бы ни оказалось в ответе API.
const DOWNLOAD_PREFIX: &str = "https://github.com/chatyev/jiyda/releases/download/";
/// Установщик весит ~3 МБ; меньше — точно не он, больше — точно что-то не то.
const MIN_INSTALLER: usize = 100 * 1024;
const MAX_INSTALLER: usize = 200 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// Адрес установщика JiydaSetup-<версия>.exe.
    pub url: String,
}

/// «0.3.0», «v0.3.0», «1.2» → (0, 3, 0). Предрелизы («0.4.0-beta») не принимаем.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let mut parts = s.split('.');
    let mut next = |required: bool| match parts.next() {
        Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => p.parse().ok(),
        None if !required => Some(0),
        _ => None,
    };
    let v = (next(true)?, next(true)?, next(false)?);
    parts.next().is_none().then_some(v)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}

/// Разобрать ответ /releases/latest: версия из метки и адрес установщика к ней.
pub fn parse_latest(json: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = v["tag_name"].as_str()?;
    let version = tag.trim_start_matches('v');
    parse_version(version)?;
    let name = format!("JiydaSetup-{version}.exe");
    let url = v["assets"].as_array()?.iter().find_map(|a| {
        let url = a["browser_download_url"].as_str()?;
        (a["name"].as_str()? == name && url == format!("{DOWNLOAD_PREFIX}{tag}/{name}")).then(|| url.to_string())
    })?;
    Some(Release { version: version.to_string(), url })
}

/// Похоже ли скачанное на установщик: размер в разумных пределах и заголовок exe.
pub fn looks_like_installer(data: &[u8]) -> bool {
    (MIN_INSTALLER..=MAX_INSTALLER).contains(&data.len()) && data.starts_with(b"MZ")
}

/// Ключи установщика: с окном прогресса, но без вопросов; /update=1 — открыть Jiyda
/// после установки (см. [Run] в installer/jiyda.iss).
pub const INSTALLER_ARGS: [&str; 5] = ["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/update=1"];

/// Есть ли версия новее этой. Ok(None) — у нас последняя.
pub fn check() -> Result<Option<Release>, String> {
    let body = http_get(LATEST_API, 1024 * 1024)?;
    let release = parse_latest(&String::from_utf8_lossy(&body)).ok_or("в ответе GitHub нет установщика")?;
    Ok(is_newer(&release.version, CURRENT).then_some(release))
}

/// Скачать установщик в `dir` и вернуть путь к нему.
pub fn download(release: &Release, dir: &Path) -> Result<PathBuf, String> {
    if !release.url.starts_with(DOWNLOAD_PREFIX) {
        return Err("адрес установщика не с GitHub Releases Jiyda".into());
    }
    let data = http_get(&release.url, MAX_INSTALLER)?;
    if !looks_like_installer(&data) {
        return Err(format!("скачанный файл не похож на установщик ({} байт)", data.len()));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("JiydaSetup-{}.exe", release.version));
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path)
}

/// GET с ответом 200 — иначе ошибка.
fn http_get(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let r = crate::net::https("GET", url, &[], None, max_bytes, std::time::Duration::from_secs(60))?;
    if r.status != 200 {
        return Err(format!("сервер ответил {}", r.status));
    }
    Ok(r.body)
}

#[cfg(test)]
mod tests {
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

    fn release_json(tag: &str, assets: &str, extra: &str) -> String {
        format!(r#"{{"tag_name":"{tag}","draft":false,"prerelease":false{extra},"assets":[{assets}]}}"#)
    }

    fn asset(name: &str, url: &str) -> String {
        format!(r#"{{"name":"{name}","browser_download_url":"{url}"}}"#)
    }

    #[test]
    fn latest_release_with_installer() {
        let url = "https://github.com/chatyev/jiyda/releases/download/v0.4.0/JiydaSetup-0.4.0.exe";
        let assets = [
            asset("notes.txt", "https://github.com/chatyev/jiyda/releases/download/v0.4.0/notes.txt"),
            asset("JiydaSetup-0.4.0.exe", url),
        ]
        .join(",");
        let r = parse_latest(&release_json("v0.4.0", &assets, "")).unwrap();
        assert_eq!(r, Release { version: "0.4.0".into(), url: url.into() });
    }

    #[test]
    fn foreign_download_urls_are_rejected() {
        for url in [
            "https://evil.example/JiydaSetup-0.4.0.exe",
            "http://github.com/chatyev/jiyda/releases/download/v0.4.0/JiydaSetup-0.4.0.exe",
            "https://github.com/someone/jiyda/releases/download/v0.4.0/JiydaSetup-0.4.0.exe",
            "https://github.com/chatyev/jiyda/releases/download/v0.3.0/JiydaSetup-0.4.0.exe",
        ] {
            let json = release_json("v0.4.0", &asset("JiydaSetup-0.4.0.exe", url), "");
            assert_eq!(parse_latest(&json), None, "{url}");
        }
    }

    #[test]
    fn release_without_installer_or_bad_tag_is_ignored() {
        let url = "https://github.com/chatyev/jiyda/releases/download/v0.4.0/Other.exe";
        assert_eq!(parse_latest(&release_json("v0.4.0", &asset("Other.exe", url), "")), None);
        assert_eq!(parse_latest(&release_json("nightly", "", "")), None);
        assert_eq!(parse_latest(r#"{"message":"Not Found"}"#), None);
        assert_eq!(parse_latest("not json"), None);
    }

    #[test]
    fn drafts_and_prereleases_are_ignored() {
        let url = "https://github.com/chatyev/jiyda/releases/download/v0.4.0/JiydaSetup-0.4.0.exe";
        let a = asset("JiydaSetup-0.4.0.exe", url);
        let draft = format!(r#"{{"tag_name":"v0.4.0","draft":true,"assets":[{a}]}}"#);
        let pre = format!(r#"{{"tag_name":"v0.4.0","prerelease":true,"assets":[{a}]}}"#);
        assert_eq!(parse_latest(&draft), None);
        assert_eq!(parse_latest(&pre), None);
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
    fn download_refuses_foreign_url_without_network() {
        let r = Release { version: "9.9.9".into(), url: "https://evil.example/x.exe".into() };
        assert!(download(&r, Path::new(".")).is_err());
    }
}
