//! Модульные тесты src/recovery.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jiyda-recovery-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn first_run_is_not_a_crash() {
    let path = temp_dir("first").join("running");
    let (_marker, crashed) = RunMarker::begin(&path);
    assert!(!crashed);
    assert!(path.exists(), "метка стоит, пока Jiyda работает");
}

#[test]
fn clean_exit_removes_marker() {
    let path = temp_dir("clean").join("running");
    let (marker, _) = RunMarker::begin(&path);
    marker.finish();
    assert!(!path.exists());
    let (_marker, crashed) = RunMarker::begin(&path);
    assert!(!crashed, "после нормального выхода — не сбой");
}

#[test]
fn marker_left_behind_means_crash() {
    let path = temp_dir("crash").join("running");
    // Прошлый запуск оборвался: метку не сняли.
    drop(RunMarker::begin(&path));
    let (marker, crashed) = RunMarker::begin(&path);
    assert!(crashed);
    // Этот запуск закрылся как надо — следующий снова обычный.
    marker.finish();
    assert!(!RunMarker::begin(&path).1);
}

#[test]
fn finish_twice_is_harmless() {
    let path = temp_dir("twice").join("running");
    let (marker, _) = RunMarker::begin(&path);
    marker.finish();
    marker.finish();
    assert!(!path.exists());
}
