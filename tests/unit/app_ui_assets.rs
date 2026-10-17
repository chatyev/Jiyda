//! Модульные тесты src/app/ui_assets.rs (подключены там через `#[path]`, видят приватное).

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
    assert!(is_ui_url("https://jiyda.ui/index.html"));
    assert!(!is_ui_url("https://jiyda.ui.evil.test/"));
    assert!(!is_ui_url("https://example.com/"));
}
