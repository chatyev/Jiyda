//! Автообновление: узнать последнюю версию на GitHub Releases, скачать установщик
//! и запустить его. Установщик (installer/jiyda.iss) ставит новую версию поверх и,
//! раз запущен с /update=1, открывает Jiyda снова.
//!
//! Последнюю версию узнаём не через API GitHub (без входа — 60 запросов в час на адрес,
//! их легко исчерпать за одним роутером), а по перенаправлению обычной страницы:
//! github.com/…/releases/latest ведёт на …/releases/tag/v<версия> последнего выпуска
//! (черновики и предрелизы туда не попадают). Установщик — по известному адресу рядом.
//!
//! Сеть — через WinHTTP из Windows: системные прокси и сертификаты, без своих
//! библиотек HTTPS. Запросы идут только на github.com.

use std::path::{Path, PathBuf};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST_PAGE: &str = "https://github.com/chatyev/jiyda/releases/latest";
/// Куда ведёт LATEST_PAGE: страница выпуска с меткой.
const TAG_PREFIX: &str = "https://github.com/chatyev/jiyda/releases/tag/";
/// Скачиваем только отсюда.
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

/// Почему не удалось проверить обновления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// GitHub временно ограничил запросы с этого адреса (403 / 429) — пройдёт само.
    Limited,
    /// Сеть, неожиданный ответ и прочее — подробности для журнала.
    Other(String),
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckError::Limited => f.write_str("GitHub ограничил запросы (403/429)"),
            CheckError::Other(e) => f.write_str(e),
        }
    }
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

/// Куда перенаправила LATEST_PAGE → версия и адрес установщика. Имя репозитория
/// сравниваем без учёта регистра: GitHub пишет его так, как назвали (chatyev/Jiyda).
pub fn release_from_location(location: &str) -> Option<Release> {
    let prefix = location.get(..TAG_PREFIX.len()).filter(|p| p.eq_ignore_ascii_case(TAG_PREFIX))?;
    let tag = &location[prefix.len()..];
    if tag.is_empty() || !tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.') {
        return None;
    }
    let version = tag.trim_start_matches('v');
    parse_version(version)?;
    Some(Release { version: version.to_string(), url: format!("{DOWNLOAD_PREFIX}{tag}/JiydaSetup-{version}.exe") })
}

/// Файл JiydaSetup-<версия>.exe.sha256 (его выкладывает CI): «<64 hex>  <имя>» или просто хэш.
pub fn parse_sha256(text: &str) -> Option<[u8; 32]> {
    let hex = text.split_whitespace().next()?;
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Совпадает ли SHA-256 скачанного с опубликованным.
pub fn sha256_matches(data: &[u8], expected: &[u8; 32]) -> bool {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).as_slice() == expected
}

/// Похоже ли скачанное на установщик: размер в разумных пределах и заголовок exe.
pub fn looks_like_installer(data: &[u8]) -> bool {
    (MIN_INSTALLER..=MAX_INSTALLER).contains(&data.len()) && data.starts_with(b"MZ")
}

/// Ключи установщика: с окном прогресса, но без вопросов; /update=1 — открыть Jiyda
/// после установки (см. [Run] в installer/jiyda.iss).
pub const INSTALLER_ARGS: [&str; 5] = ["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/update=1"];

/// Есть ли версия новее этой. Ok(None) — у нас последняя.
pub fn check() -> Result<Option<Release>, CheckError> {
    let r = crate::net::redirect(LATEST_PAGE, std::time::Duration::from_secs(60)).map_err(CheckError::Other)?;
    let release = match (r.status, r.location) {
        (301 | 302 | 303 | 307 | 308, Some(location)) => release_from_location(&location)
            .ok_or_else(|| CheckError::Other(format!("непонятное перенаправление: {location}")))?,
        (403 | 429, _) => return Err(CheckError::Limited),
        (status, _) => return Err(CheckError::Other(format!("сервер ответил {status}"))),
    };
    Ok(is_newer(&release.version, CURRENT).then_some(release))
}

/// Скачать установщик в `dir` и вернуть путь к нему. Сначала — его SHA-256 из того же
/// выпуска: повреждённый или подменённый при скачивании файл не запустится. (От взлома
/// самого GitHub-аккаунта защитит только подпись установщика — сертификат подписи кода.)
pub fn download(release: &Release, dir: &Path) -> Result<PathBuf, String> {
    if !release.url.to_ascii_lowercase().starts_with(DOWNLOAD_PREFIX) {
        return Err("адрес установщика не с GitHub Releases Jiyda".into());
    }
    let sum = http_get(&format!("{}.sha256", release.url), 4 * 1024)?;
    let expected = parse_sha256(&String::from_utf8_lossy(&sum)).ok_or("в выпуске нет контрольной суммы установщика")?;
    let data = http_get(&release.url, MAX_INSTALLER)?;
    if !looks_like_installer(&data) {
        return Err(format!("скачанный файл не похож на установщик ({} байт)", data.len()));
    }
    if !sha256_matches(&data, &expected) {
        return Err("контрольная сумма установщика не совпала — файл повреждён или подменён".into());
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
#[path = "../tests/unit/update.rs"]
mod tests;
