//! Язык сообщений браузера: русский или английский — по языку Windows или настройке
//! `"language"` в config.json. Ключ перевода — английская строка, как в интерфейсе
//! (ui/js/i18n.js): для русского языка берётся перевод из таблицы. Подстановки — `{}`.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

static LANG: OnceLock<Lang> = OnceLock::new();

/// Выбрать язык: "ru", "en" или "auto" (как в Windows). Вызывается один раз при запуске.
pub fn init(setting: &str) -> Lang {
    let lang = match setting {
        "ru" => Lang::Ru,
        "en" => Lang::En,
        _ => system(),
    };
    *LANG.get_or_init(|| lang)
}

/// Текущий язык. До `init` (в тестах) — русский.
pub fn lang() -> Lang {
    LANG.get().copied().unwrap_or(Lang::Ru)
}

pub fn code() -> &'static str {
    match lang() {
        Lang::Ru => "ru",
        Lang::En => "en",
    }
}

/// Язык по коду языка Windows (LANGID): русский для русского, остальным — английский.
pub fn from_langid(id: u16) -> Lang {
    const LANG_RUSSIAN: u16 = 0x19;
    if id & 0x3ff == LANG_RUSSIAN { Lang::Ru } else { Lang::En }
}

#[cfg(windows)]
fn system() -> Lang {
    // SAFETY: функция без параметров, только читает язык интерфейса пользователя.
    from_langid(unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() })
}

#[cfg(not(windows))]
fn system() -> Lang {
    Lang::Ru
}

/// Перевод строки; ключ — английский текст.
pub fn tr(en: &'static str) -> &'static str {
    if lang() == Lang::En {
        return en;
    }
    STRINGS.iter().find(|(k, _)| *k == en).map_or(en, |(_, ru)| ru)
}

/// Перевод строки с подстановками `{}` по порядку.
pub fn trf(en: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    fill(tr(en), args)
}

fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut args = args.iter();
    let mut parts = template.split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for part in parts {
        if let Some(a) = args.next() {
            out.push_str(&a.to_string());
        }
        out.push_str(part);
    }
    out
}

/// Английская строка → русская.
const STRINGS: &[(&str, &str)] = &[
    // Пространства по умолчанию.
    ("Personal", "Основное"),
    ("Space {}", "Space {}"),
    // Обновления.
    ("You have the latest version of Jiyda ({})", "У вас последняя версия Jiyda ({})"),
    ("Couldn't check for updates. Is GitHub reachable?", "Не удалось проверить обновления — нет связи с GitHub?"),
    ("Couldn't download the update", "Не удалось скачать обновление"),
    ("Couldn't start the update installer", "Не удалось запустить установщик обновления"),
    // Страницы.
    ("WebView2 error: {}", "Ошибка WebView2: {}"),
    ("Blocked: the page is opening too many pages", "Заблокировано: страница открывает слишком много вкладок"),
    ("New page in \u{201c}{}\u{201d}", "Новая страница в «{}»"),
    ("No closed pages", "Нет закрытых вкладок"),
    ("Page closed", "Страница закрыта"),
    ("Moved to Later", "Отложено в Later"),
    ("Undo", "Отменить"),
    ("Link copied", "Ссылка скопирована"),
    ("Couldn't copy the link", "Не удалось скопировать ссылку"),
    // Пространства.
    ("You can't have more than {} Spaces", "Больше {} пространств создать нельзя"),
    ("The main Space can't be deleted", "Основное пространство удалить нельзя"),
    ("Space \u{201c}{}\u{201d} deleted", "Пространство «{}» удалено"),
    ("Moved to \u{201c}{}\u{201d}", "Перенесено в «{}»"),
    // Загрузки, история, данные.
    ("File not found. It may have been moved or deleted", "Файл не найден: возможно, его удалили или переместили"),
    ("This download can't be cancelled anymore", "Эту загрузку уже нельзя отменить"),
    ("History", "История"),
    ("History cleared", "История очищена"),
    ("Removed from history", "Удалено из истории"),
    ("Opened {} pages from the same time", "Открыто страниц из того же времени: {}"),
    ("Open any page in this Space and try again", "Откройте любую страницу в этом пространстве и повторите"),
    ("Site data in \u{201c}{}\u{201d} cleared", "Данные сайтов в «{}» очищены"),
    ("Couldn't clear site data", "Не удалось очистить данные сайтов"),
    // Настройки.
    (
        "The search address must start with https:// and contain %s",
        "Адрес поиска должен начинаться с https:// и содержать %s",
    ),
    ("That folder doesn't exist", "Такой папки нет"),
    (
        "This shortcut can't be used: it needs Ctrl or Alt (or F1\u{2013}F24)",
        "Это сочетание нельзя назначить: нужен Ctrl или Alt (или F1–F24)",
    ),
    ("This change takes effect after restarting Jiyda", "Изменение вступит в силу после перезапуска Jiyda"),
    ("Key removed", "Ключ удалён"),
    ("Key saved in Windows Credential Manager", "Ключ сохранён в диспетчере учётных данных Windows"),
    ("Couldn't save the key", "Не удалось сохранить ключ"),
    // Тема и система.
    ("Theme: light", "Тема: светлая"),
    ("Theme: dark", "Тема: тёмная"),
    ("Theme: same as Windows", "Тема: как в Windows"),
    (
        "Install Jiyda with its installer first: it adds Jiyda to Windows' list of browsers",
        "Сначала установите Jiyda установщиком — он добавит его в список браузеров Windows",
    ),
    ("Couldn't open Settings", "Не удалось открыть «Параметры»"),
    ("Invalid value", "Недопустимое значение"),
    // Ask Jiyda.
    (
        "The API key was rejected. Check it in Settings \u{2192} AI Assistant.",
        "Ключ API не принят. Проверьте его в «Настройки \u{2192} AI Assistant».",
    ),
    ("Rate limit reached. Try again in a moment.", "Слишком много запросов. Попробуйте чуть позже."),
    ("Claude is overloaded right now. Try again in a moment.", "Claude сейчас перегружен. Попробуйте чуть позже."),
    ("Claude declined this request.", "Claude отказался отвечать на этот запрос."),
    ("Claude returned an empty answer.", "Claude вернул пустой ответ."),
    ("answer cut off", "ответ обрезан"),
    ("Couldn't reach Claude ({})", "Нет связи с Claude ({})"),
];

#[cfg(test)]
mod tests {
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
}
