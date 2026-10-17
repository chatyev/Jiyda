//! Модульные тесты src/i18n.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn russian_and_other_windows_languages() {
    assert_eq!(from_langid(0x0419), Lang::Ru); // ru-RU
    assert_eq!(from_langid(0x0819), Lang::Ru); // ru-MD
    assert_eq!(from_langid(0x0409), Lang::En); // en-US
    assert_eq!(from_langid(0x0443), Lang::En); // uz-Latn-UZ
    assert_eq!(from_langid(0x0422), Lang::En); // uk-UA
}

#[test]
fn default_language_is_russian() {
    assert_eq!(lang(), Lang::Ru);
    assert_eq!(tr("Link copied"), "Ссылка скопирована");
    assert_eq!(trf("Space \u{201c}{}\u{201d} deleted", &[&"Работа"]), "Пространство «Работа» удалено");
    // Перевода нет — английский текст как есть.
    assert_eq!(tr("no such string"), "no such string");
}

/// Литерал из исходника → строка: раскрыть `\u{…}`.
fn unescape(lit: &str) -> String {
    let mut out = String::new();
    let mut rest = lit;
    while let Some(at) = rest.find("\\u{") {
        out.push_str(&rest[..at]);
        let end = at + rest[at..].find('}').unwrap();
        let code = u32::from_str_radix(&rest[at + 3..end], 16).unwrap();
        out.push(char::from_u32(code).unwrap());
        rest = &rest[end + 1..];
    }
    out + rest
}

/// У каждой строки `tr("…")`/`trf("…")` в коде есть русский перевод.
#[test]
fn every_used_string_is_translated() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut used = 0;
    for f in files.iter().filter(|f| !f.ends_with("i18n.rs")) {
        let text = std::fs::read_to_string(f).unwrap();
        for start in ["tr(\"", "trf(\""] {
            for (at, _) in text.match_indices(start) {
                // Только целое имя: не push_str("…").
                if text[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let rest = &text[at + start.len()..];
                let key = &rest[..rest.find('"').unwrap()];
                let key = unescape(key);
                assert!(STRINGS.iter().any(|(en, _)| *en == key), "нет перевода: {key} ({})", f.display());
                used += 1;
            }
        }
    }
    assert!(used > 20, "строки не найдены: {used}");
}

#[test]
fn table_is_consistent() {
    for (i, (en, ru)) in STRINGS.iter().enumerate() {
        assert_eq!(ru.matches("{}").count(), en.matches("{}").count(), "{en}");
        // В английском ключе нет кириллицы.
        assert!(!ru.is_empty() && !en.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)), "{en}");
        assert!(STRINGS[..i].iter().all(|(k, _)| k != en), "повтор: {en}");
    }
}

#[test]
fn fill_replaces_in_order_and_tolerates_missing() {
    assert_eq!(fill("{} из {}", &[&1, &5]), "1 из 5");
    assert_eq!(fill("без подстановок", &[&1]), "без подстановок");
    assert_eq!(fill("{} и {}", &[&"a"]), "a и ");
}
