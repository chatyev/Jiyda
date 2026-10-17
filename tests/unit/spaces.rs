//! Модульные тесты src/spaces.rs (подключены там через `#[path]`, видят приватное).

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
    assert_eq!(out[1].name, "Space 5");
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
    assert_eq!(s.name, "Space 5");
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
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let paths = Paths::in_dir(dir);
    assert_eq!(profile_dir(&paths, MAIN_ID), dir.join("webview"));
    assert_eq!(profile_dir(&paths, 7), dir.join("spaces").join("7").join("webview"));
    assert_eq!(space_dir(&paths, 7), dir.join("spaces").join("7"));
}

#[test]
fn tab_ids_do_not_overlap_between_spaces() {
    assert!(page_id_base(2) > page_id_base(1) + 100_000_000);
    assert_eq!(page_id_base(0), 0);
}
