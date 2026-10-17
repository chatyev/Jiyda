//! Модульные тесты src/limit.rs (подключены там через `#[path]`, видят приватное).

use super::*;

const SEC: Duration = Duration::from_secs(1);

#[test]
fn allows_up_to_max_in_window() {
    let mut l = RateLimit::new(3, SEC);
    let t = Instant::now();
    assert!(l.allow(t));
    assert!(l.allow(t));
    assert!(l.allow(t));
    assert!(!l.allow(t));
    assert!(!l.allow(t + SEC / 2));
}

#[test]
fn frees_slots_when_window_passes() {
    let mut l = RateLimit::new(2, SEC);
    let t = Instant::now();
    assert!(l.allow(t));
    assert!(l.allow(t + SEC / 2));
    assert!(!l.allow(t + SEC / 2));
    // Первое событие вышло из окна — освободилось одно место.
    assert!(l.allow(t + SEC));
    assert!(!l.allow(t + SEC));
    assert!(l.allow(t + SEC * 3));
}

#[test]
fn rejected_events_do_not_take_slots() {
    let mut l = RateLimit::new(1, SEC);
    let t = Instant::now();
    assert!(l.allow(t));
    for i in 1..10 {
        assert!(!l.allow(t + SEC * i / 10));
    }
    assert!(l.allow(t + SEC));
}
