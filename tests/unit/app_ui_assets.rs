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

// ---------- Перевод интерфейса (js/i18n.js — русский, js/i18n_uz.js — узбекский) ----------

fn ui_text(path: &str) -> &'static str {
    std::str::from_utf8(asset(path).unwrap_or_else(|| panic!("нет {path}")).0).unwrap()
}

/// Ключи словаря: строки в одинарных кавычках, за которыми идёт двоеточие.
fn dict_keys(text: &str) -> Vec<&str> {
    let mut keys = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('\'') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('\'') else { break };
        let (s, tail) = (&after[..end], &after[end + 1..]);
        if tail.trim_start().starts_with(':') {
            keys.push(s);
        }
        rest = tail;
    }
    keys
}

/// Строки интерфейса: S.t('…') с постоянным текстом (не 'wants to ' + переменная).
fn used_keys() -> Vec<&'static str> {
    let mut out = Vec::new();
    for (path, data, _) in UI_FILES.iter().filter(|(p, _, _)| p.ends_with(".js") && !p.contains("i18n")) {
        let text = std::str::from_utf8(data).unwrap();
        for (at, _) in text.match_indices("S.t('") {
            let rest = &text[at + 5..];
            let Some(end) = rest.find('\'') else { panic!("{path}: незакрытая строка") };
            if !rest[end + 1..].trim_start().starts_with('+') {
                out.push(&rest[..end]);
            }
        }
    }
    out
}

#[test]
fn every_ui_string_is_translated_to_russian_and_uzbek() {
    let ru = dict_keys(ui_text("js/i18n.js"));
    let uz = dict_keys(ui_text("js/i18n_uz.js"));
    let used = used_keys();
    assert!(ru.len() > 300 && used.len() > 200, "словари не разобраны: {} {}", ru.len(), used.len());
    for key in &used {
        assert!(ru.contains(key), "нет русского перевода: {key}");
        assert!(uz.contains(key), "нет узбекского перевода: {key}");
    }
}

#[test]
fn uzbek_dictionary_has_the_same_keys_as_russian() {
    let ru = dict_keys(ui_text("js/i18n.js"));
    let uz = dict_keys(ui_text("js/i18n_uz.js"));
    for key in &ru {
        assert!(uz.contains(key), "нет узбекского перевода: {key}");
    }
    for key in &uz {
        assert!(ru.contains(key), "лишний ключ в узбекском: {key}");
    }
    for (lang, keys) in [("русском", &ru), ("узбекском", &uz)] {
        let mut seen = std::collections::HashSet::new();
        for key in keys.iter() {
            assert!(seen.insert(key), "повтор в {lang}: {key}");
        }
    }
}

#[test]
fn uzbek_dictionary_is_latin() {
    let text = ui_text("js/i18n_uz.js");
    // Кириллица допустима только в комментариях.
    for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
        assert!(!line.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)), "кириллица: {line}");
    }
}
