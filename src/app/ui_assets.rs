//! Файлы интерфейса внутри seyro.exe и их выдача WebView по адресу seyro://ui/…
//!
//! Список файлов собирает build.rs из папки ui/. WebView2 видит этот адрес как
//! https://seyro.ui/ — защищённый источник, отдельный от любых сайтов. Отдаются только
//! встроенные файлы: путь сравнивается со списком, ничего с диска не читается.

use std::borrow::Cow;
use wry::http::{Request, Response, StatusCode, header::CONTENT_TYPE};

include!(concat!(env!("OUT_DIR"), "/ui_files.rs"));

/// Имя протокола и адрес страницы интерфейса.
pub(super) const SCHEME: &str = "seyro";
pub(super) const INDEX_URL: &str = "seyro://ui/index.html";
/// Так WebView2 видит адреса интерфейса (wry: with_https_scheme).
pub(super) const ORIGIN: &str = "https://seyro.ui/";

/// Встроенный файл: байты и тип. Путь — относительно ui/, например «js/core.js».
pub(super) fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    UI_FILES.iter().find(|(p, _, _)| *p == path).map(|(_, data, mime)| (*data, *mime))
}

/// Ответ на запрос WebView к seyro://ui/…
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
    url.starts_with(ORIGIN) || url.starts_with("seyro://ui/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_index_and_known_files_only() {
        assert!(asset("").is_some_and(|(_, m)| m.starts_with("text/html")));
        assert!(asset("/index.html").is_some());
        assert!(asset("styles.css").is_some_and(|(_, m)| m.starts_with("text/css")));
        assert!(asset("js/core.js").is_some_and(|(_, m)| m.starts_with("text/javascript")));
        for bad in ["nope.js", "../Cargo.toml", "js/../../src/main.rs", "JS/core.js"] {
            assert!(asset(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn every_module_import_exists() {
        // Каждый import в ui/js/*.js указывает на встроенный файл.
        for (path, data, _) in UI_FILES.iter().filter(|(p, _, _)| p.ends_with(".js")) {
            let text = std::str::from_utf8(data).unwrap();
            for line in text.lines().filter(|l| l.starts_with("import ")) {
                let target = line.split(['\'', '"']).nth(1).unwrap_or_default();
                let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
                let resolved = format!("{dir}/{}", target.trim_start_matches("./"));
                assert!(asset(&resolved).is_some(), "{path}: {line}");
            }
        }
    }

    #[test]
    fn index_loads_styles_and_main_module() {
        let html = std::str::from_utf8(asset("index.html").unwrap().0).unwrap();
        assert!(html.contains(r#"href="styles.css""#));
        assert!(html.contains(r#"<script type="module" src="js/main.js">"#));
    }

    #[test]
    fn ui_urls() {
        assert!(is_ui_url("https://seyro.ui/index.html"));
        assert!(!is_ui_url("https://seyro.ui.evil.test/"));
        assert!(!is_ui_url("https://example.com/"));
    }
}
