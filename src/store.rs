//! Сохранение сессии (вкладки) и истории посещений на диск.

use crate::config::write_atomic;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedTab {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub pinned_url: Option<String>,
    pub favicon: String,
    pub last_active: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<SavedTab>,
    pub active: Option<usize>,
    pub sidebar_visible: Option<bool>,
    /// Недавно закрытые и архивированные вкладки (для Ctrl+Shift+T).
    pub closed: Vec<SavedTab>,
}

impl Session {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            write_atomic(path, &s);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistEntry {
    pub url: String,
    pub title: String,
    pub ts: u64,
    pub visits: u32,
}

pub struct History {
    pub entries: Vec<HistEntry>,
    dirty: u32,
}

const HISTORY_LIMIT: usize = 5000;

impl History {
    pub fn load(path: &Path) -> Self {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { entries, dirty: 0 }
    }

    pub fn save(&mut self, path: &Path) {
        if let Ok(s) = serde_json::to_string(&self.entries) {
            write_atomic(path, &s);
        }
        self.dirty = 0;
    }

    /// Записать посещение. Возвращает true, если пора сбросить историю на диск.
    pub fn visit(&mut self, url: &str, title: &str) -> bool {
        if !url.starts_with("http") {
            return false;
        }
        let ts = now();
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url) {
            e.ts = ts;
            e.visits += 1;
            if !title.is_empty() {
                e.title = title.to_string();
            }
        } else {
            self.entries.push(HistEntry { url: url.into(), title: title.into(), ts, visits: 1 });
            if self.entries.len() > HISTORY_LIMIT {
                self.entries.sort_by(|a, b| b.ts.cmp(&a.ts));
                self.entries.truncate(HISTORY_LIMIT);
            }
        }
        self.dirty += 1;
        self.dirty >= 20
    }

    pub fn set_title(&mut self, url: &str, title: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url) {
            e.title = title.to_string();
        }
    }

    /// Последние записи для командной палитры.
    pub fn recent(&self, n: usize) -> Vec<&HistEntry> {
        let mut v: Vec<_> = self.entries.iter().collect();
        v.sort_by(|a, b| b.ts.cmp(&a.ts));
        v.truncate(n);
        v
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
