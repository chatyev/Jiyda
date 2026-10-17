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
    (
        "GitHub is limiting update checks right now. Try again later.",
        "GitHub временно ограничил проверки обновлений — попробуйте позже",
    ),
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
#[path = "../tests/unit/i18n.rs"]
mod tests;
