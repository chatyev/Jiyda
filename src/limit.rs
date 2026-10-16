//! Ограничение частоты: не больше `max` событий за скользящее окно `window`.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct RateLimit {
    max: usize,
    window: Duration,
    hits: VecDeque<Instant>,
}

impl RateLimit {
    pub fn new(max: usize, window: Duration) -> Self {
        Self { max, window, hits: VecDeque::with_capacity(max) }
    }

    /// Учесть событие в момент `now`. false — лимит исчерпан, событие отбрасывается.
    pub fn allow(&mut self, now: Instant) -> bool {
        while self.hits.front().is_some_and(|t| now.duration_since(*t) >= self.window) {
            self.hits.pop_front();
        }
        if self.hits.len() >= self.max {
            return false;
        }
        self.hits.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
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
}
