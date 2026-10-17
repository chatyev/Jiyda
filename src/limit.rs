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
#[path = "../tests/unit/limit.rs"]
mod tests;
