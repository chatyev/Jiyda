//! Файлы интерфейса внутри jiyda.exe; движок отдаёт их интерфейсу по адресу jiyda://ui/…
//! (engine::UiSpec::asset).
//!
//! Список файлов собирает build.rs из папки ui/. WebView2 видит этот адрес как
//! https://jiyda.ui/ — защищённый источник, отдельный от любых сайтов. Отдаются только
//! встроенные файлы: путь сравнивается со списком, ничего с диска не читается.

include!(concat!(env!("OUT_DIR"), "/ui_files.rs"));

/// Имя протокола и адрес страницы интерфейса.
pub(super) const SCHEME: &str = "jiyda";
pub(super) const INDEX_URL: &str = "jiyda://ui/index.html";
/// Так WebView2 видит адреса интерфейса (wry: with_https_scheme).
pub(super) const ORIGIN: &str = "https://jiyda.ui/";

/// Встроенный файл: байты и тип. Путь — относительно ui/, например «js/core.js».
pub(super) fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    UI_FILES.iter().find(|(p, _, _)| *p == path).map(|(_, data, mime)| (*data, *mime))
}

/// Адрес — сам интерфейс (а не сайт, на который он пытается уйти).
pub(super) fn is_ui_url(url: &str) -> bool {
    url.starts_with(ORIGIN) || url.starts_with("jiyda://ui/")
}

#[cfg(test)]
#[path = "../../tests/unit/app_ui_assets.rs"]
mod tests;
