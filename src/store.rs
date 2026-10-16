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
    /// Есть изменения, ещё не записанные на диск.
    dirty: bool,
}

const HISTORY_LIMIT: usize = 5000;

impl History {
    pub fn load(path: &Path) -> Self {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { entries, dirty: false }
    }

    pub fn save(&mut self, path: &Path) {
        if let Ok(s) = serde_json::to_string(&self.entries) {
            write_atomic(path, &s);
        }
        self.dirty = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Записать посещение.
    pub fn visit(&mut self, url: &str, title: &str) {
        if !url.starts_with("http") {
            return;
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
        self.dirty = true;
    }

    pub fn set_title(&mut self, url: &str, title: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url && e.title != title) {
            e.title = title.to_string();
            self.dirty = true;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: &str, ts: u64) -> HistEntry {
        HistEntry { url: url.into(), title: String::new(), ts, visits: 1 }
    }

    fn history(entries: Vec<HistEntry>) -> History {
        History { entries, dirty: false }
    }

    #[test]
    fn visit_adds_new_entry() {
        let mut h = history(vec![]);
        h.visit("https://a.test/", "A");
        assert_eq!(h.entries.len(), 1);
        let e = &h.entries[0];
        assert_eq!((e.url.as_str(), e.title.as_str(), e.visits), ("https://a.test/", "A", 1));
        assert!(e.ts > 0);
    }

    #[test]
    fn visit_ignores_non_http() {
        let mut h = history(vec![]);
        h.visit("about:blank", "x");
        h.visit("file:///C:/a.html", "x");
        h.visit("edge://settings", "x");
        assert!(h.entries.is_empty());
        assert!(!h.is_dirty());
    }

    #[test]
    fn repeat_visit_updates_existing_entry() {
        let mut h = history(vec![entry("https://a.test/", 1)]);
        h.entries[0].title = "Old".into();
        h.visit("https://a.test/", "New");
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0].visits, 2);
        assert_eq!(h.entries[0].title, "New");
        assert!(h.entries[0].ts > 1);
    }

    #[test]
    fn repeat_visit_with_empty_title_keeps_old_title() {
        let mut h = history(vec![entry("https://a.test/", 1)]);
        h.entries[0].title = "Old".into();
        h.visit("https://a.test/", "");
        assert_eq!(h.entries[0].title, "Old");
    }

    #[test]
    fn visit_marks_history_dirty() {
        let mut h = history(vec![]);
        assert!(!h.is_dirty());
        h.visit("https://a.test/", "");
        assert!(h.is_dirty());
    }

    #[test]
    fn set_title_marks_dirty_only_on_change() {
        let mut h = history(vec![entry("https://a.test/", 1)]);
        h.set_title("https://a.test/", "");
        h.set_title("https://missing.test/", "X");
        assert!(!h.is_dirty());
        h.set_title("https://a.test/", "A");
        assert!(h.is_dirty());
    }

    #[test]
    fn history_is_capped_and_drops_oldest() {
        let entries = (0..HISTORY_LIMIT as u64).map(|i| entry(&format!("https://old.test/{i}"), i + 1)).collect();
        let mut h = history(entries);
        h.visit("https://new.test/", "");
        assert_eq!(h.entries.len(), HISTORY_LIMIT);
        assert!(h.entries.iter().any(|e| e.url == "https://new.test/"));
        // Самая старая запись (ts = 1) вытеснена.
        assert!(!h.entries.iter().any(|e| e.url == "https://old.test/0"));
    }

    #[test]
    fn set_title_changes_only_matching_entry() {
        let mut h = history(vec![entry("https://a.test/", 1), entry("https://b.test/", 2)]);
        h.set_title("https://b.test/", "B");
        h.set_title("https://missing.test/", "X");
        assert_eq!(h.entries[0].title, "");
        assert_eq!(h.entries[1].title, "B");
        assert_eq!(h.entries.len(), 2);
    }

    #[test]
    fn recent_returns_newest_first() {
        let h = history(vec![entry("https://a/", 10), entry("https://b/", 30), entry("https://c/", 20)]);
        let urls: Vec<_> = h.recent(2).iter().map(|e| e.url.as_str()).collect();
        assert_eq!(urls, ["https://b/", "https://c/"]);
        assert_eq!(h.recent(100).len(), 3);
        assert!(h.recent(0).is_empty());
    }

    #[test]
    fn history_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut h = history(vec![entry("https://a.test/", 5)]);
        h.dirty = true;
        h.save(&path);
        assert!(!h.is_dirty());

        let loaded = History::load(&path);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].url, "https://a.test/");
        assert_eq!(loaded.entries[0].ts, 5);
    }

    #[test]
    fn history_load_missing_or_corrupt_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(History::load(&dir.path().join("nope.json")).entries.is_empty());

        let path = dir.path().join("history.json");
        std::fs::write(&path, "{не json").unwrap();
        assert!(History::load(&path).entries.is_empty());
    }

    #[test]
    fn session_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let tab = SavedTab {
            url: "https://a.test/".into(),
            title: "A".into(),
            pinned: true,
            pinned_url: Some("https://a.test/home".into()),
            favicon: "https://a.test/favicon.ico".into(),
            last_active: 42,
        };
        let s = Session {
            tabs: vec![tab.clone()],
            active: Some(0),
            sidebar_visible: Some(false),
            closed: vec![SavedTab { pinned: false, pinned_url: None, ..tab }],
        };
        s.save(&path);

        let loaded = Session::load(&path);
        assert_eq!(loaded.tabs.len(), 1);
        let t = &loaded.tabs[0];
        assert_eq!(t.url, "https://a.test/");
        assert!(t.pinned);
        assert_eq!(t.pinned_url.as_deref(), Some("https://a.test/home"));
        assert_eq!(t.last_active, 42);
        assert_eq!(loaded.active, Some(0));
        assert_eq!(loaded.sidebar_visible, Some(false));
        assert_eq!(loaded.closed.len(), 1);
        assert!(!loaded.closed[0].pinned);
    }

    #[test]
    fn session_tolerates_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        std::fs::write(&path, r#"{"tabs":[{"url":"https://a.test/"}]}"#).unwrap();
        let s = Session::load(&path);
        assert_eq!(s.tabs.len(), 1);
        assert_eq!(s.tabs[0].url, "https://a.test/");
        assert!(!s.tabs[0].pinned);
        assert_eq!(s.active, None);
        assert!(s.closed.is_empty());
    }

    #[test]
    fn session_load_corrupt_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        std::fs::write(&path, "garbage").unwrap();
        let s = Session::load(&path);
        assert!(s.tabs.is_empty());
        assert_eq!(s.active, None);
    }

    #[test]
    fn now_is_after_2020() {
        assert!(now() > 1_577_836_800);
    }
}
