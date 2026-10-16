//! Автообновление: узнать последнюю версию на GitHub Releases, скачать установщик
//! и запустить его. Установщик (installer/seyro.iss) ставит новую версию поверх и,
//! раз запущен с /update=1, открывает Seyro снова.
//!
//! Сеть — через WinHTTP из Windows: системные прокси и сертификаты, без своих
//! библиотек HTTPS. Запросы идут только на api.github.com и github.com.

use std::path::{Path, PathBuf};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_API: &str = "https://api.github.com/repos/chatyev/seyro/releases/latest";
/// Скачиваем только отсюда — что бы ни оказалось в ответе API.
const DOWNLOAD_PREFIX: &str = "https://github.com/chatyev/seyro/releases/download/";
/// Установщик весит ~3 МБ; меньше — точно не он, больше — точно что-то не то.
const MIN_INSTALLER: usize = 100 * 1024;
const MAX_INSTALLER: usize = 200 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// Адрес установщика SeyroSetup-<версия>.exe.
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
    let name = format!("SeyroSetup-{version}.exe");
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

/// Ключи установщика: с окном прогресса, но без вопросов; /update=1 — открыть Seyro
/// после установки (см. [Run] в installer/seyro.iss).
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
        return Err("адрес установщика не с GitHub Releases Seyro".into());
    }
    let data = http_get(&release.url, MAX_INSTALLER)?;
    if !looks_like_installer(&data) {
        return Err(format!("скачанный файл не похож на установщик ({} байт)", data.len()));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("SeyroSetup-{}.exe", release.version));
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(windows)]
fn http_get(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;
    use windows::Win32::Networking::WinHttp::{
        INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
        WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
        WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    };
    use windows::core::PCWSTR;

    /// Дескриптор WinHTTP, который закрывается сам.
    struct Handle(*mut c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: дескриптор получен от WinHTTP и закрывается один раз.
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
    let open = |h: *mut c_void, what: &str| {
        if h.is_null() { Err(format!("{what}: {}", windows::core::Error::from_thread())) } else { Ok(Handle(h)) }
    };
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();

    let u = url::Url::parse(url).map_err(|e| e.to_string())?;
    if u.scheme() != "https" {
        return Err("только https".into());
    }
    let host = wide(u.host_str().ok_or("в адресе нет сервера")?);
    let mut path = u.path().to_string();
    if let Some(q) = u.query() {
        path.push('?');
        path.push_str(q);
    }
    let (path, agent, verb) = (wide(&path), wide(&format!("Seyro/{CURRENT}")), wide("GET"));

    // SAFETY: последовательность вызовов WinHTTP с живыми дескрипторами и строками,
    // завершёнными нулём; буферы — свои и нужного размера.
    unsafe {
        // GitHub требует User-Agent — им служит имя приложения.
        let session = open(
            WinHttpOpen(PCWSTR(agent.as_ptr()), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0),
            "WinHttpOpen",
        )?;
        let port = u.port().unwrap_or(INTERNET_DEFAULT_HTTPS_PORT);
        let connect = open(WinHttpConnect(session.0, PCWSTR(host.as_ptr()), port, 0), "соединение")?;
        let request = open(
            WinHttpOpenRequest(
                connect.0,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ),
            "запрос",
        )?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| format!("отправка запроса: {e}"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| format!("ответ сервера: {e}"))?;
        let (mut status, mut len) = (0u32, 4u32);
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut c_void),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(|e| format!("код ответа: {e}"))?;
        if status != 200 {
            return Err(format!("сервер ответил {status}"));
        }
        let mut body = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(request.0, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read)
                .map_err(|e| format!("чтение ответа: {e}"))?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buf[..read as usize]);
            if body.len() > max_bytes {
                return Err("ответ слишком большой".into());
            }
        }
        Ok(body)
    }
}

#[cfg(not(windows))]
fn http_get(_url: &str, _max_bytes: usize) -> Result<Vec<u8>, String> {
    Err("автообновление есть только в Windows".into())
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
        let url = "https://github.com/chatyev/seyro/releases/download/v0.4.0/SeyroSetup-0.4.0.exe";
        let assets = [asset("notes.txt", "https://github.com/chatyev/seyro/releases/download/v0.4.0/notes.txt"), asset("SeyroSetup-0.4.0.exe", url)].join(",");
        let r = parse_latest(&release_json("v0.4.0", &assets, "")).unwrap();
        assert_eq!(r, Release { version: "0.4.0".into(), url: url.into() });
    }

    #[test]
    fn foreign_download_urls_are_rejected() {
        for url in [
            "https://evil.example/SeyroSetup-0.4.0.exe",
            "http://github.com/chatyev/seyro/releases/download/v0.4.0/SeyroSetup-0.4.0.exe",
            "https://github.com/someone/seyro/releases/download/v0.4.0/SeyroSetup-0.4.0.exe",
            "https://github.com/chatyev/seyro/releases/download/v0.3.0/SeyroSetup-0.4.0.exe",
        ] {
            let json = release_json("v0.4.0", &asset("SeyroSetup-0.4.0.exe", url), "");
            assert_eq!(parse_latest(&json), None, "{url}");
        }
    }

    #[test]
    fn release_without_installer_or_bad_tag_is_ignored() {
        let url = "https://github.com/chatyev/seyro/releases/download/v0.4.0/Other.exe";
        assert_eq!(parse_latest(&release_json("v0.4.0", &asset("Other.exe", url), "")), None);
        assert_eq!(parse_latest(&release_json("nightly", "", "")), None);
        assert_eq!(parse_latest(r#"{"message":"Not Found"}"#), None);
        assert_eq!(parse_latest("not json"), None);
    }

    #[test]
    fn drafts_and_prereleases_are_ignored() {
        let url = "https://github.com/chatyev/seyro/releases/download/v0.4.0/SeyroSetup-0.4.0.exe";
        let a = asset("SeyroSetup-0.4.0.exe", url);
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
