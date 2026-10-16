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
    if id == MAIN_ID { "Основное".into() } else { format!("Пространство {id}") }
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
pub fn new_space(existing: &[(u32, &str)]) -> Option<SavedSpace> {
    if existing.len() >= MAX_SPACES {
        return None;
    }
    let id = existing.iter().map(|(id, _)| *id).max().unwrap_or(MAIN_ID) + 1;
    let color = COLORS
        .iter()
        .find(|c| !existing.iter().any(|(_, used)| used == *c))
        .unwrap_or(&COLORS[existing.len() % COLORS.len()]);
    Some(SavedSpace { id, name: default_name(id), color: (*color).into(), ..Default::default() })
}

/// Первый id вкладок пространства: у пространств непересекающиеся номера вкладок,
/// потому что события от WebView приходят по номеру вкладки.
pub fn tab_id_base(space_id: u32) -> u64 {
    u64::from(space_id) * 1_000_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(id: u32, name: &str, color: &str) -> SavedSpace {
        SavedSpace { id, name: name.into(), color: color.into(), ..Default::default() }
    }

    fn ids(v: &[SavedSpace]) -> Vec<u32> {
        v.iter().map(|s| s.id).collect()
    }

    #[test]
    fn normalize_adds_main_first() {
        let out = normalize(vec![sp(3, "Работа", "blue")]);
        assert_eq!(ids(&out), [1, 3]);
        assert_eq!(out[0].name, "Основное");
        assert!(is_color(&out[0].color));
        assert_eq!(out[1].name, "Работа");
    }

    #[test]
    fn normalize_moves_main_to_front_and_drops_bad_ids() {
        let out = normalize(vec![sp(2, "a", "blue"), sp(0, "zero", "blue"), sp(1, "M", "teal"), sp(2, "dup", "rose")]);
        assert_eq!(ids(&out), [1, 2]);
        assert_eq!(out[0].name, "M");
        assert_eq!(out[1].name, "a");
    }

    #[test]
    fn normalize_fills_names_and_colors() {
        let out = normalize(vec![sp(1, "  ", "nope"), sp(5, "", "")]);
        assert_eq!(out[0].name, "Основное");
        assert_eq!(out[1].name, "Пространство 5");
        assert!(out.iter().all(|s| is_color(&s.color)));
    }

    #[test]
    fn normalize_caps_count() {
        let many = (1..=20).map(|i| sp(i, "x", "blue")).collect();
        assert_eq!(normalize(many).len(), MAX_SPACES);
    }

    #[test]
    fn clean_name_trims_and_limits() {
        assert_eq!(clean_name("  Работа   и  дом "), Some("Работа и дом".into()));
        assert_eq!(clean_name(" \t "), None);
        assert_eq!(clean_name(&"я".repeat(100)).unwrap().chars().count(), NAME_MAX);
    }

    #[test]
    fn new_space_takes_next_id_and_free_color() {
        let s = new_space(&[(1, "violet"), (4, "blue")]).unwrap();
        assert_eq!(s.id, 5);
        assert_eq!(s.name, "Пространство 5");
        assert_eq!(s.color, "teal");
        assert!(s.tabs.is_empty());
    }

    #[test]
    fn new_space_is_limited() {
        let full: Vec<(u32, &str)> = (1..=MAX_SPACES as u32).map(|i| (i, "blue")).collect();
        assert!(new_space(&full).is_none());
    }

    #[test]
    fn new_space_reuses_colors_when_all_taken() {
        let used: Vec<(u32, &str)> = COLORS.iter().enumerate().map(|(i, c)| (i as u32 + 1, *c)).collect();
        assert!(is_color(&new_space(&used).unwrap().color));
    }

    #[test]
    fn main_space_keeps_old_profile_folder() {
        let dir = std::path::Path::new(r"C:\D");
        let paths = Paths {
            config: dir.join("config.json"),
            session: dir.join("session.json"),
            history: dir.join("history.json"),
            downloads: dir.join("downloads.json"),
            webview_data: dir.join("webview"),
            spaces: dir.join("spaces"),
            extensions: dir.join("extensions"),
            log: dir.join("drift.log"),
        };
        assert_eq!(profile_dir(&paths, MAIN_ID), dir.join("webview"));
        assert_eq!(profile_dir(&paths, 7), dir.join("spaces").join("7").join("webview"));
        assert_eq!(space_dir(&paths, 7), dir.join("spaces").join("7"));
    }

    #[test]
    fn tab_ids_do_not_overlap_between_spaces() {
        assert!(tab_id_base(2) > tab_id_base(1) + 100_000_000);
        assert_eq!(tab_id_base(0), 0);
    }
}
