//! Язык сообщений браузера: русский, узбекский (латиница) или английский — по языку
//! Windows или настройке `"language"` в config.json. Ключ перевода — английская строка,
//! как в интерфейсе (ui/js/i18n.js, ui/js/i18n_uz.js): перевод берётся из таблицы языка
//! (RU, UZ). Подстановки — `{}`.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    Uz,
    En,
}

static LANG: OnceLock<Lang> = OnceLock::new();

/// Выбрать язык: "ru", "uz", "en" или "auto" (как в Windows). Вызывается один раз при запуске.
pub fn init(setting: &str) -> Lang {
    let lang = match setting {
        "ru" => Lang::Ru,
        "uz" => Lang::Uz,
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
        Lang::Uz => "uz",
        Lang::En => "en",
    }
}

/// Язык по коду языка Windows (LANGID): русский и узбекский (латиница и кириллица) —
/// свои, остальным — английский.
pub fn from_langid(id: u16) -> Lang {
    const LANG_RUSSIAN: u16 = 0x19;
    const LANG_UZBEK: u16 = 0x43;
    match id & 0x3ff {
        LANG_RUSSIAN => Lang::Ru,
        LANG_UZBEK => Lang::Uz,
        _ => Lang::En,
    }
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
    let table = match lang() {
        Lang::En => return en,
        Lang::Ru => RU,
        Lang::Uz => UZ,
    };
    table.iter().find(|(k, _)| *k == en).map_or(en, |(_, t)| t)
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
const RU: &[(&str, &str)] = &[
    // Закладки и перенос из других браузеров.
    ("Added to bookmarks", "Добавлено в закладки"),
    ("Removed from bookmarks", "Удалено из закладок"),
    ("Folder deleted", "Папка удалена"),
    ("Bookmark address must start with http:// or https://", "Адрес закладки должен начинаться с http:// или https://"),
    ("New folder", "Новая папка"),
    ("Opened the first {} pages", "Открыты первые {} страниц"),
    ("That browser profile is no longer available", "Этого профиля браузера больше нет"),
    ("Other bookmarks", "Другие закладки"),
    ("Mobile bookmarks", "Мобильные закладки"),
    ("Nothing new to import from {}", "Из {} нечего переносить: всё уже есть"),
    (
        "Couldn't read data from {}. Close it and try again",
        "Не удалось прочитать данные {}. Закройте его и попробуйте снова",
    ),
    ("Imported from {}: {} bookmarks, {} history entries", "Перенесено из {}: закладок — {}, записей истории — {}"),
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

/// Английская строка → узбекская (латиница). Ключи — те же, что в RU (проверяет тест).
const UZ: &[(&str, &str)] = &[
    // Закладки и перенос из других браузеров.
    ("Added to bookmarks", "Xatchoʻplarga qoʻshildi"),
    ("Removed from bookmarks", "Xatchoʻplardan olib tashlandi"),
    ("Folder deleted", "Jild oʻchirildi"),
    (
        "Bookmark address must start with http:// or https://",
        "Xatchoʻp manzili http:// yoki https:// bilan boshlanishi kerak",
    ),
    ("New folder", "Yangi jild"),
    ("Opened the first {} pages", "Dastlabki {} ta sahifa ochildi"),
    ("That browser profile is no longer available", "Bu brauzer profili endi mavjud emas"),
    ("Other bookmarks", "Boshqa xatchoʻplar"),
    ("Mobile bookmarks", "Mobil xatchoʻplar"),
    ("Nothing new to import from {}", "{}dan koʻchiriladigan yangi narsa yoʻq"),
    (
        "Couldn't read data from {}. Close it and try again",
        "{} maʼlumotlarini oʻqib boʻlmadi. Uni yoping va qayta urinib koʻring",
    ),
    ("Imported from {}: {} bookmarks, {} history entries", "{}dan koʻchirildi: {} ta xatchoʻp, tarixdan {} ta yozuv"),
    // Пространства по умолчанию.
    ("Personal", "Asosiy"),
    ("Space {}", "Space {}"),
    // Обновления.
    ("You have the latest version of Jiyda ({})", "Sizda Jiydaning soʻnggi versiyasi ({})"),
    (
        "Couldn't check for updates. Is GitHub reachable?",
        "Yangilanishlarni tekshirib boʻlmadi. GitHub bilan aloqa bormi?",
    ),
    (
        "GitHub is limiting update checks right now. Try again later.",
        "GitHub hozir yangilanish tekshiruvlarini cheklamoqda. Keyinroq urinib koʻring.",
    ),
    ("Couldn't download the update", "Yangilanishni yuklab olib boʻlmadi"),
    ("Couldn't start the update installer", "Yangilanish oʻrnatuvchisini ishga tushirib boʻlmadi"),
    // Страницы.
    ("WebView2 error: {}", "WebView2 xatosi: {}"),
    ("Blocked: the page is opening too many pages", "Bloklandi: sahifa juda koʻp sahifa ochmoqda"),
    ("New page in \u{201c}{}\u{201d}", "\u{201c}{}\u{201d}da yangi sahifa"),
    ("No closed pages", "Yopilgan sahifalar yoʻq"),
    ("Page closed", "Sahifa yopildi"),
    ("Moved to Later", "Laterga oʻtkazildi"),
    ("Undo", "Bekor qilish"),
    ("Link copied", "Havoladan nusxa olindi"),
    ("Couldn't copy the link", "Havoladan nusxa olib boʻlmadi"),
    // Пространства.
    ("You can't have more than {} Spaces", "{} tadan ortiq Space yaratib boʻlmaydi"),
    ("The main Space can't be deleted", "Asosiy Spaceni oʻchirib boʻlmaydi"),
    ("Space \u{201c}{}\u{201d} deleted", "\u{201c}{}\u{201d} Space oʻchirildi"),
    ("Moved to \u{201c}{}\u{201d}", "\u{201c}{}\u{201d}ga koʻchirildi"),
    // Загрузки, история, данные.
    (
        "File not found. It may have been moved or deleted",
        "Fayl topilmadi: u koʻchirilgan yoki oʻchirilgan boʻlishi mumkin",
    ),
    ("This download can't be cancelled anymore", "Bu yuklab olishni endi bekor qilib boʻlmaydi"),
    ("History", "Tarix"),
    ("History cleared", "Tarix tozalandi"),
    ("Removed from history", "Tarixdan olib tashlandi"),
    ("Opened {} pages from the same time", "Oʻsha vaqtdagi {} ta sahifa ochildi"),
    ("Open any page in this Space and try again", "Bu Spaceda istalgan sahifani oching va qayta urinib koʻring"),
    ("Site data in \u{201c}{}\u{201d} cleared", "\u{201c}{}\u{201d}dagi sayt maʼlumotlari tozalandi"),
    ("Couldn't clear site data", "Sayt maʼlumotlarini tozalab boʻlmadi"),
    // Настройки.
    (
        "The search address must start with https:// and contain %s",
        "Qidiruv manzili https:// bilan boshlanishi va %s belgisini oʻz ichiga olishi kerak",
    ),
    ("That folder doesn't exist", "Bunday jild yoʻq"),
    (
        "This shortcut can't be used: it needs Ctrl or Alt (or F1\u{2013}F24)",
        "Bu tugmalar birikmasini tayinlab boʻlmaydi: Ctrl yoki Alt kerak (yoki F1–F24)",
    ),
    (
        "This change takes effect after restarting Jiyda",
        "Bu oʻzgarish Jiyda qayta ishga tushirilgandan keyin kuchga kiradi",
    ),
    ("Key removed", "Kalit oʻchirildi"),
    ("Key saved in Windows Credential Manager", "Kalit Windows hisob maʼlumotlari menejerida saqlandi"),
    ("Couldn't save the key", "Kalitni saqlab boʻlmadi"),
    // Тема и система.
    ("Theme: light", "Mavzu: yorugʻ"),
    ("Theme: dark", "Mavzu: qorongʻi"),
    ("Theme: same as Windows", "Mavzu: Windows kabi"),
    (
        "Install Jiyda with its installer first: it adds Jiyda to Windows' list of browsers",
        "Avval Jiydani oʻrnatuvchi orqali oʻrnating: u Jiydani Windows brauzerlar roʻyxatiga qoʻshadi",
    ),
    ("Couldn't open Settings", "Sozlamalarni ochib boʻlmadi"),
    ("Invalid value", "Notoʻgʻri qiymat"),
    // Ask Jiyda.
    (
        "The API key was rejected. Check it in Settings \u{2192} AI Assistant.",
        "API kaliti qabul qilinmadi. Uni Sozlamalar \u{2192} AI Assistant boʻlimida tekshiring.",
    ),
    ("Rate limit reached. Try again in a moment.", "Soʻrovlar juda koʻp. Birozdan keyin urinib koʻring."),
    ("Claude is overloaded right now. Try again in a moment.", "Claude hozir band. Birozdan keyin urinib koʻring."),
    ("Claude declined this request.", "Claude bu soʻrovga javob berishdan bosh tortdi."),
    ("Claude returned an empty answer.", "Claude boʻsh javob qaytardi."),
    ("answer cut off", "javob qisqartirildi"),
    ("Couldn't reach Claude ({})", "Claude bilan aloqa yoʻq ({})"),
];

#[cfg(test)]
#[path = "../tests/unit/i18n.rs"]
mod tests;
