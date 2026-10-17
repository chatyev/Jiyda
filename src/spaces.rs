//! Пространства (Spaces): имена, цвета, папки профилей. Только логика без окна —
//! сами вкладки и WebView пространств держит `App`.
//!
//! У каждого пространства свой профиль WebView2 (куки, логины, кэш) в своей папке.
//! «Основное» (id 1) живёт в старой папке webview\, поэтому при переходе на
//! пространства логины на сайтах сохраняются.

use crate::config::Paths;
use crate::store::SavedSpace;
use std::path::PathBuf;

/// Пространство «Основное»: его нельзя удалить, на его профиле работает интерфейс.
pub const MAIN_ID: u32 = 1;
/// Больше пространств не нужно: на каждое — свои процессы браузера.
pub const MAX_SPACES: usize = 9;
const NAME_MAX: usize = 40;

/// Ключи цветов; сами цвета — в ui/index.html (SPACE_COLORS).
pub const COLORS: [&str; 7] = ["violet", "blue", "teal", "green", "orange", "rose", "gray"];

pub fn default_name(id: u32) -> String {
    if id == MAIN_ID { crate::i18n::tr("Personal").into() } else { crate::i18n::trf("Space {}", &[&id]) }
}

/// Имя от пользователя: без лишних пробелов и не длиннее NAME_MAX. None — пустое.
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() { None } else { Some(name.chars().take(NAME_MAX).collect()) }
}

pub fn is_color(c: &str) -> bool {
    COLORS.contains(&c)
}

/// Папка профиля WebView2 пространства.
pub fn profile_dir(paths: &Paths, id: u32) -> PathBuf {
    if id == MAIN_ID { paths.webview_data.clone() } else { space_dir(paths, id).join("webview") }
}

/// Вся папка пространства (её стирают после удаления). Для «Основного» её
/// не вызывают: его профиль — папка webview, и он не удаляется никогда.
pub fn space_dir(paths: &Paths, id: u32) -> PathBuf {
    paths.spaces.join(id.to_string())
}

/// Привести пространства из сессии в порядок: без повторов id и id 0, с именами
/// и известными цветами; «Основное» есть всегда и стоит первым.
pub fn normalize(spaces: Vec<SavedSpace>) -> Vec<SavedSpace> {
    let mut out: Vec<SavedSpace> = Vec::new();
    for s in spaces {
        if s.id == 0 || out.iter().any(|o| o.id == s.id) {
            continue;
        }
        out.push(s);
    }
    match out.iter().position(|s| s.id == MAIN_ID) {
        Some(i) => {
            let main = out.remove(i);
            out.insert(0, main);
        }
        None => out.insert(0, SavedSpace { id: MAIN_ID, ..Default::default() }),
    }
    out.truncate(MAX_SPACES);
    for (i, s) in out.iter_mut().enumerate() {
        if clean_name(&s.name).is_none() {
            s.name = default_name(s.id);
        }
        if !is_color(&s.color) {
            s.color = COLORS[i % COLORS.len()].into();
        }
    }
    out
}

/// Новое пространство: следующий свободный id, имя по умолчанию и первый
/// незанятый цвет. None — пространств уже MAX_SPACES.
/// `reserved` — номера удалённых в этом сеансе: их папки стираются при следующем запуске,
/// поэтому номер не занимаем, но и в лимит они не входят.
pub fn new_space(existing: &[(u32, &str)], reserved: &[u32]) -> Option<SavedSpace> {
    if existing.len() >= MAX_SPACES {
        return None;
    }
    let id = existing.iter().map(|(id, _)| *id).chain(reserved.iter().copied()).max().unwrap_or(MAIN_ID) + 1;
    let color = COLORS
        .iter()
        .find(|c| !existing.iter().any(|(_, used)| used == *c))
        .unwrap_or(&COLORS[existing.len() % COLORS.len()]);
    Some(SavedSpace { id, name: default_name(id), color: (*color).into(), ..Default::default() })
}

/// Первый id вкладок пространства: у пространств непересекающиеся номера вкладок,
/// потому что события от WebView приходят по номеру вкладки.
pub fn page_id_base(space_id: u32) -> u64 {
    u64::from(space_id) * 1_000_000_000
}

#[cfg(test)]
#[path = "../tests/unit/spaces.rs"]
mod tests;
