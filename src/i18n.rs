//! Язык сообщений браузера: русский или английский — по языку Windows или настройке
//! `"language"` в config.json. Ключ перевода — сама русская строка: если язык
//! английский и перевод есть — берётся он, иначе русский текст. Подстановки — `{}`.
//! Строки интерфейса переводит ui/index.html (свой словарь, тот же принцип).

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

/// Перевод строки.
pub fn tr(ru: &'static str) -> &'static str {
    if lang() == Lang::Ru {
        return ru;
    }
    STRINGS.iter().find(|(k, _)| *k == ru).map_or(ru, |(_, en)| en)
}

/// Перевод строки с подстановками `{}` по порядку.
pub fn trf(ru: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    fill(tr(ru), args)
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

/// Русская строка → английская.
const STRINGS: &[(&str, &str)] = &[
    // Пространства по умолчанию.
    ("Основное", "Main"),
    ("Пространство {}", "Space {}"),
    // Обновления.
    ("У вас последняя версия Seyro ({})", "You have the latest version of Seyro ({})"),
    ("Не удалось проверить обновления — нет связи с GitHub?", "Couldn't check for updates. Is GitHub reachable?"),
    ("Не удалось скачать обновление", "Couldn't download the update"),
    ("Не удалось запустить установщик обновления", "Couldn't start the update installer"),
    // Вкладки и страницы.
    ("Ошибка WebView2: {}", "WebView2 error: {}"),
    ("Заблокировано: страница открывает слишком много вкладок", "Blocked: the page is opening too many tabs"),
    ("Новая вкладка в пространстве «{}»", "New tab in the \u{201c}{}\u{201d} space"),
    ("Нет закрытых вкладок", "No closed tabs"),
    ("Ссылка скопирована", "Link copied"),
    ("Не удалось скопировать ссылку", "Couldn't copy the link"),
    // Пространства.
    ("Больше {} пространств создать нельзя", "You can't have more than {} spaces"),
    ("Пространство «Основное» удалить нельзя", "The main space can't be deleted"),
    ("Пространство «{}» удалено", "Space \u{201c}{}\u{201d} deleted"),
    // Загрузки, история, данные.
    ("Загрузка: {} (Ctrl+J — список)", "Downloading {} (Ctrl+J to see all)"),
    ("Загружено: {}", "Downloaded {}"),
    ("Загрузка не удалась: {}", "Download failed: {}"),
    ("Файл не найден: возможно, его удалили или переместили", "File not found. It may have been moved or deleted"),
    ("История очищена", "History cleared"),
    ("Откройте любую вкладку в этом пространстве и повторите", "Open any tab in this space and try again"),
    ("Данные сайтов в «{}» очищены", "Site data in \u{201c}{}\u{201d} cleared"),
    ("Не удалось очистить данные сайтов", "Couldn't clear site data"),
    // Тема и система.
    ("Тема: светлая", "Theme: light"),
    ("Тема: тёмная", "Theme: dark"),
    ("Тема: как в Windows", "Theme: same as Windows"),
    (
        "Сначала установите Seyro установщиком — он добавит его в список браузеров Windows",
        "Install Seyro with its installer first: it adds Seyro to Windows' list of browsers",
    ),
    ("Не удалось открыть «Параметры»", "Couldn't open Settings"),
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
    fn default_language_is_russian_and_keeps_text() {
        assert_eq!(lang(), Lang::Ru);
        assert_eq!(tr("Ссылка скопирована"), "Ссылка скопирована");
        assert_eq!(trf("Пространство «{}» удалено", &[&"Работа"]), "Пространство «Работа» удалено");
    }

    #[test]
    fn table_is_consistent() {
        for (i, (ru, en)) in STRINGS.iter().enumerate() {
            assert_eq!(ru.matches("{}").count(), en.matches("{}").count(), "{ru}");
            // В английском переводе не осталось кириллицы.
            assert!(!en.is_empty() && !en.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)), "{en}");
            assert!(STRINGS[..i].iter().all(|(k, _)| k != ru), "повтор: {ru}");
        }
    }

    #[test]
    fn fill_replaces_in_order_and_tolerates_missing() {
        assert_eq!(fill("{} из {}", &[&1, &5]), "1 из 5");
        assert_eq!(fill("без подстановок", &[&1]), "без подстановок");
        assert_eq!(fill("{} и {}", &[&"a"]), "a и ");
    }
}
