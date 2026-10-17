//! Модульные тесты src/config.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn paths(dir: &std::path::Path) -> Paths {
    Paths::in_dir(dir)
}

#[test]
fn missing_config_creates_default_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = paths(dir.path());
    let cfg = Config::load(&p);
    assert_eq!(cfg.search_url, Config::default().search_url);

    let written: Config = serde_json::from_str(&std::fs::read_to_string(&p.config).unwrap()).unwrap();
    assert!(written.shield && written.shortcuts.is_empty());
    assert_eq!(written.archive_hours, 12);
}

#[test]
fn partial_config_fills_in_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let p = paths(dir.path());
    std::fs::write(&p.config, r#"{"search_url":"https://s.test/?q=%s","devtools":false}"#).unwrap();
    let cfg = Config::load(&p);
    assert_eq!(cfg.search_url, "https://s.test/?q=%s");
    assert!(!cfg.devtools);
    assert_eq!(cfg.sleep_minutes, 60);
    assert!(cfg.edge_reveal);
    assert!(cfg.custom_titlebar);
}

#[test]
fn corrupt_config_falls_back_to_default_and_keeps_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = paths(dir.path());
    std::fs::write(&p.config, "{oops").unwrap();
    let cfg = Config::load(&p);
    assert_eq!(cfg.search_url, Config::default().search_url);
    // Повреждённый файл не перезаписывается — его можно поправить руками.
    assert_eq!(std::fs::read_to_string(&p.config).unwrap(), "{oops");
}

#[test]
fn writer_writes_in_order_and_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("w.json");
    let mut w = Writer::new();
    for i in 0..50 {
        w.write(&path, i.to_string());
    }
    w.finish();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "49");
    // После finish запись — сразу.
    w.write(&path, "late".into());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "late");
}

#[test]
fn write_atomic_creates_and_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.json");
    write_atomic(&path, "one");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
    write_atomic(&path, "two");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
    assert!(!path.with_extension("tmp").exists());
}

#[test]
fn write_atomic_into_missing_dir_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("no/such/dir/data.json");
    write_atomic(&path, "x");
    assert!(!path.exists());
}

// ---------- Перенос данных из Seyro и Drift ----------

fn old_data(local: &std::path::Path, name: &str) {
    let old = local.join(name);
    std::fs::create_dir_all(old.join("webview").join("EBWebView")).unwrap();
    std::fs::write(old.join("session.json"), "{\"spaces\":[]}").unwrap();
    std::fs::write(old.join("webview").join("EBWebView").join("Cookies"), "c").unwrap();
}

#[test]
fn migration_moves_old_folder_with_everything_inside() {
    for name in OLD_DATA_DIRS {
        let local = tempfile::tempdir().unwrap();
        old_data(local.path(), name);
        assert!(migrate_in(local.path()).unwrap(), "{name}");
        let new = local.path().join(DATA_DIR);
        assert_eq!(std::fs::read_to_string(new.join("session.json")).unwrap(), "{\"spaces\":[]}");
        assert!(new.join("webview").join("EBWebView").join("Cookies").exists());
        assert!(!local.path().join(name).exists());
        assert_eq!(data_dir_in(local.path()), new);
        // Второй раз переносить нечего.
        assert!(!migrate_in(local.path()).unwrap());
    }
}

#[test]
fn newest_old_folder_wins() {
    // Есть и Seyro, и совсем старый Drift — переносится Seyro, Drift не трогается.
    let local = tempfile::tempdir().unwrap();
    old_data(local.path(), "Drift");
    old_data(local.path(), "Seyro");
    assert!(migrate_in(local.path()).unwrap());
    assert!(!local.path().join("Seyro").exists());
    assert!(local.path().join("Drift").exists());
}

#[test]
fn migration_never_overwrites_existing_jiyda_data() {
    let local = tempfile::tempdir().unwrap();
    old_data(local.path(), "Seyro");
    let new = local.path().join(DATA_DIR);
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(new.join("session.json"), "new").unwrap();
    assert!(!migrate_in(local.path()).unwrap());
    assert_eq!(std::fs::read_to_string(new.join("session.json")).unwrap(), "new");
    assert!(local.path().join("Seyro").exists());
    assert_eq!(data_dir_in(local.path()), new);
}

#[test]
fn fresh_install_uses_jiyda_folder() {
    let local = tempfile::tempdir().unwrap();
    assert!(!migrate_in(local.path()).unwrap());
    assert_eq!(data_dir_in(local.path()), local.path().join(DATA_DIR));
}

#[test]
fn falls_back_to_old_folder_when_not_migrated() {
    // Перенос не удался (папка занята) — работаем со старыми данными, а не с пустыми.
    let local = tempfile::tempdir().unwrap();
    old_data(local.path(), "Seyro");
    assert_eq!(data_dir_in(local.path()), local.path().join("Seyro"));
}

#[test]
fn old_name_as_a_file_is_ignored() {
    let local = tempfile::tempdir().unwrap();
    std::fs::write(local.path().join("Seyro"), "not a folder").unwrap();
    assert!(!migrate_in(local.path()).unwrap());
    assert_eq!(data_dir_in(local.path()), local.path().join(DATA_DIR));
}
