//! Файлы интерфейса внутри jiyda.exe и их выдача WebView по адресу jiyda://ui/…
//!
//! Список файлов собирает build.rs из папки ui/. WebView2 видит этот адрес как
//! https://jiyda.ui/ — защищённый источник, отдельный от любых сайтов. Отдаются только
//! встроенные файлы: путь сравнивается со списком, ничего с диска не читается.

use std::borrow::Cow;
use wry::http::{Request, Response, StatusCode, header::CONTENT_TYPE};

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

/// Ответ на запрос WebView к jiyda://ui/…
pub(super) fn respond(req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let found = asset(req.uri().path());
    let response = match found {
        Some((data, mime)) => Response::builder().header(CONTENT_TYPE, mime).body(Cow::Borrowed(data)),
        None => {
            log::warn!("интерфейс запросил несуществующий файл: {}", req.uri());
            Response::builder().status(StatusCode::NOT_FOUND).body(Cow::Borrowed(&b""[..]))
        }
    };
    response.unwrap_or_else(|_| Response::new(Cow::Borrowed(&b""[..])))
}

/// Адрес — сам интерфейс (а не сайт, на который он пытается уйти).
pub(super) fn is_ui_url(url: &str) -> bool {
    url.starts_with(ORIGIN) || url.starts_with("jiyda://ui/")
}

#[cfg(test)]
#[path = "../../tests/unit/app_ui_assets.rs"]
mod tests;
