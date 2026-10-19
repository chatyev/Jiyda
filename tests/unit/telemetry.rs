//! Модульные тесты src/telemetry.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jiyda-telemetry-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join("telemetry.json")
}

fn snap(pages: usize) -> Snapshot {
    Snapshot { version: "0.6.5".into(), windows: "10.0.26100".into(), pages, default_browser: true }
}

#[test]
fn off_by_default_records_and_keeps_nothing() {
    let path = tmp("off");
    std::fs::write(&path, r#"{"id":"0123456789abcdef0123456789abcdef"}"#).unwrap();
    let mut t = Telemetry::load(&path, false);
    assert!(!t.is_on());
    assert!(!path.exists(), "выключено — файл со старым номером стёрт");
    t.started("0.6.5", true, "2026-10-19");
    t.record(Event::WorkspaceUsed, "2026-10-19");
    assert!(t.due("2026-10-20", &snap(3)).is_empty());
    assert_eq!(t.preview(&snap(3)), None);
    assert_eq!(t.take_changes(), None);
}

#[test]
fn a_day_becomes_one_report_with_only_counts() {
    let mut t = Telemetry::load(&tmp("day"), true);
    t.started("0.6.4", false, "2026-10-19");
    t.record(Event::CommandPaletteUsed, "2026-10-19");
    t.record(Event::CommandPaletteUsed, "2026-10-19");
    t.record(Event::WorkspaceUsed, "2026-10-19");
    // День не закончился — отправлять нечего.
    assert!(t.due("2026-10-19", &snap(7)).is_empty());
    let reports = t.due("2026-10-20", &snap(7));
    assert_eq!(reports.len(), 1);
    let r = &reports[0];
    assert_eq!(r.day, "2026-10-19");
    assert_eq!(r.id.len(), 32);
    assert_eq!((r.counts.app_started, r.counts.command_palette_used, r.counts.workspace_used), (1, 2, 1));
    assert_eq!((r.tabs.as_str(), r.default_browser, r.windows.as_str()), ("6-10", true, "10.0.26100"));
    // Ровно эти поля и ничего больше: ни адресов, ни заголовков.
    let json: serde_json::Value = serde_json::to_value(r).unwrap();
    let keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["counts", "day", "default_browser", "id", "tabs", "version", "windows"]);
    // Отправлен — из очереди уходит.
    t.sent("2026-10-19");
    assert!(t.due("2026-10-20", &snap(7)).is_empty());
}

#[test]
fn a_newer_version_since_last_start_counts_as_update_success() {
    let path = tmp("update");
    let mut t = Telemetry::load(&path, true);
    t.started("0.6.4", false, "2026-10-19");
    std::fs::write(&path, t.take_changes().unwrap()).unwrap();
    let mut t = Telemetry::load(&path, true);
    t.started("0.6.5", true, "2026-10-19");
    assert_eq!(t.preview(&snap(1)).unwrap().counts.update_success, 1);
    assert_eq!(t.preview(&snap(1)).unwrap().counts.app_crashed, 1);
    assert_eq!(t.preview(&snap(1)).unwrap().counts.app_started, 2);
}

#[test]
fn turning_off_forgets_the_id_and_the_queue() {
    let path = tmp("disable");
    let mut t = Telemetry::load(&path, true);
    t.started("0.6.5", false, "2026-10-18");
    std::fs::write(&path, t.take_changes().unwrap()).unwrap();
    let first = t.preview(&snap(1)).unwrap().id;
    t.disable();
    assert!(!path.exists());
    assert_eq!(t.preview(&snap(1)), None);
    t.enable();
    assert_ne!(t.preview(&snap(1)).unwrap().id, first, "новый номер — не связан со старым");
    assert!(t.due("2026-10-19", &snap(1)).is_empty(), "старые счётчики не вернулись");
}

#[test]
fn queue_keeps_only_the_last_week() {
    let mut t = Telemetry::load(&tmp("queue"), true);
    for d in 1..=10 {
        t.record(Event::AppStarted, &format!("2026-10-{d:02}"));
    }
    let days: Vec<String> = t.due("2026-10-11", &snap(1)).into_iter().map(|r| r.day).collect();
    assert_eq!(days.len(), MAX_PENDING);
    assert_eq!(days.last().unwrap(), "2026-10-10");
}

#[test]
fn only_known_ui_events_are_accepted() {
    assert_eq!(Event::from_ui("command_palette_used"), Some(Event::CommandPaletteUsed));
    for bad in ["", "url", "https://example.com", "app_started"] {
        assert_eq!(Event::from_ui(bad), None, "{bad}");
    }
}

#[test]
fn tabs_are_bucketed() {
    let b: Vec<&str> = [0, 1, 2, 5, 6, 10, 11, 20, 21, 50, 51, 500].into_iter().map(tabs_bucket).collect();
    assert_eq!(b, ["0", "1", "2-5", "2-5", "6-10", "6-10", "11-20", "11-20", "21-50", "21-50", "51+", "51+"]);
}
