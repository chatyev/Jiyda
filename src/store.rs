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
    /// Пары Split View: индексы в `tabs`.
    pub splits: Vec<SavedSplit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SavedSplit {
    pub left: usize,
    pub right: usize,
    pub ratio: f64,
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

    pub fn clear(&mut self) {
        self.entries.clear();
        self.dirty = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: u64,
    pub url: String,
    /// Куда сохраняется файл (может быть пустым, если WebView2 не сообщил).
    pub path: String,
    pub state: DownloadState,
    pub ts: u64,
}

impl Download {
    /// Имя файла для списка: из пути, иначе из адреса.
    pub fn name(&self) -> String {
        let from_path = Path::new(&self.path).file_name().map(|n| n.to_string_lossy().into_owned());
        from_path
            .filter(|n| !n.is_empty())
            .or_else(|| {
                let no_query = self.url.split(['?', '#']).next().unwrap_or("");
                no_query.rsplit('/').find(|s| !s.is_empty()).map(str::to_string)
            })
            .unwrap_or_else(|| self.url.clone())
    }
}

/// Список загрузок для палитры (Ctrl+J).
pub struct Downloads {
    pub items: Vec<Download>,
    next_id: u64,
}

const DOWNLOADS_LIMIT: usize = 200;

impl Downloads {
    pub fn load(path: &Path) -> Self {
        let mut items: Vec<Download> = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Браузер закрыли посреди загрузки — она уже не завершится.
        for d in &mut items {
            if d.state == DownloadState::Running {
                d.state = DownloadState::Failed;
            }
        }
        let next_id = items.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        Self { items, next_id }
    }

    pub fn save(&self, path: &Path) {
        if let Ok(s) = serde_json::to_string(&self.items) {
            write_atomic(path, &s);
        }
    }

    pub fn started(&mut self, url: &str, path: &str, ts: u64) {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Download { id, url: url.into(), path: path.into(), state: DownloadState::Running, ts });
        if self.items.len() > DOWNLOADS_LIMIT {
            let extra = self.items.len() - DOWNLOADS_LIMIT;
            self.items.drain(..extra);
        }
    }

    /// Отметить окончание загрузки с этого адреса (последней незавершённой).
    /// Если начало не было замечено, загрузка добавляется сразу завершённой.
    pub fn finished(&mut self, url: &str, path: Option<&str>, ok: bool, ts: u64) -> &Download {
        let state = if ok { DownloadState::Done } else { DownloadState::Failed };
        let idx = match self.items.iter().rposition(|d| d.url == url && d.state == DownloadState::Running) {
            Some(i) => i,
            None => {
                self.started(url, "", ts);
                self.items.len() - 1
            }
        };
        let d = &mut self.items[idx];
        d.state = state;
        if let Some(p) = path.filter(|p| !p.is_empty()) {
            d.path = p.into();
        }
        d
    }

    pub fn get(&self, id: u64) -> Option<&Download> {
        self.items.iter().find(|d| d.id == id)
    }

    /// Новые сверху.
    pub fn recent(&self) -> impl Iterator<Item = &Download> {
        self.items.iter().rev()
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
            splits: vec![SavedSplit { left: 0, right: 0, ratio: 0.3 }],
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
        assert_eq!(loaded.splits, [SavedSplit { left: 0, right: 0, ratio: 0.3 }]);
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

    #[test]
    fn clear_history_empties_and_marks_dirty() {
        let mut h = history(vec![entry("https://a/", 1), entry("https://b/", 2)]);
        h.clear();
        assert!(h.entries.is_empty());
        assert!(h.is_dirty());
    }

    // ---------- Загрузки ----------

    fn downloads() -> Downloads {
        Downloads { items: vec![], next_id: 1 }
    }

    #[test]
    fn download_lifecycle() {
        let mut d = downloads();
        d.started("https://x/file.zip", r"C:\Dl\file.zip", 10);
        assert_eq!(d.items[0].state, DownloadState::Running);

        let done = d.finished("https://x/file.zip", Some(r"C:\Dl\file (1).zip"), true, 20);
        assert_eq!(done.state, DownloadState::Done);
        assert_eq!(done.path, r"C:\Dl\file (1).zip");
        assert_eq!(d.items.len(), 1);
    }

    #[test]
    fn finished_without_path_keeps_started_path() {
        let mut d = downloads();
        d.started("https://x/a.pdf", r"C:\Dl\a.pdf", 10);
        let f = d.finished("https://x/a.pdf", None, false, 20);
        assert_eq!(f.state, DownloadState::Failed);
        assert_eq!(f.path, r"C:\Dl\a.pdf");
    }

    #[test]
    fn finished_matches_latest_running_download_of_same_url() {
        let mut d = downloads();
        d.started("https://x/a", "first", 1);
        d.finished("https://x/a", None, true, 2);
        d.started("https://x/a", "second", 3);
        d.started("https://x/a", "third", 4);
        d.finished("https://x/a", None, true, 5);
        let states: Vec<_> = d.items.iter().map(|i| (i.path.as_str(), i.state)).collect();
        assert_eq!(
            states,
            [("first", DownloadState::Done), ("second", DownloadState::Running), ("third", DownloadState::Done)]
        );
    }

    #[test]
    fn finished_without_start_adds_entry() {
        let mut d = downloads();
        let f = d.finished("https://x/b.bin", Some(r"C:\Dl\b.bin"), true, 7);
        assert_eq!((f.path.as_str(), f.state, f.ts), (r"C:\Dl\b.bin", DownloadState::Done, 7));
        assert_eq!(d.items.len(), 1);
    }

    #[test]
    fn downloads_have_unique_ids_and_recent_is_newest_first() {
        let mut d = downloads();
        d.started("https://x/1", "", 1);
        d.started("https://x/2", "", 2);
        let ids: Vec<_> = d.recent().map(|i| i.id).collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert_eq!(d.recent().next().unwrap().url, "https://x/2");
        assert_eq!(d.get(ids[1]).unwrap().url, "https://x/1");
        assert!(d.get(999).is_none());
    }

    #[test]
    fn downloads_list_is_capped() {
        let mut d = downloads();
        for i in 0..DOWNLOADS_LIMIT + 5 {
            d.started(&format!("https://x/{i}"), "", i as u64);
        }
        assert_eq!(d.items.len(), DOWNLOADS_LIMIT);
        assert_eq!(d.items[0].url, "https://x/5");
    }

    #[test]
    fn downloads_roundtrip_marks_interrupted_as_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("downloads.json");
        let mut d = downloads();
        d.started("https://x/done", "a", 1);
        d.finished("https://x/done", None, true, 2);
        d.started("https://x/running", "b", 3);
        d.save(&path);

        let mut loaded = Downloads::load(&path);
        let states: Vec<_> = loaded.items.iter().map(|i| i.state).collect();
        assert_eq!(states, [DownloadState::Done, DownloadState::Failed]);
        // Новые id не пересекаются со старыми.
        loaded.started("https://x/new", "", 4);
        let max_old = loaded.items[..2].iter().map(|i| i.id).max().unwrap();
        assert!(loaded.items[2].id > max_old);
    }

    #[test]
    fn downloads_load_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Downloads::load(&dir.path().join("nope.json")).items.is_empty());
    }

    #[test]
    fn download_name_from_path_or_url() {
        let mut d = downloads();
        d.started("https://x/dir/report.pdf?token=1", r"C:\Dl\report.pdf", 1);
        d.started("https://x/dir/archive.zip?token=1#frag", "", 2);
        d.started("https://x/", "", 3);
        let names: Vec<_> = d.items.iter().map(Download::name).collect();
        assert_eq!(names, ["report.pdf", "archive.zip", "x"]);
    }
}
