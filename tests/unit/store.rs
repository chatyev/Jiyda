//! Модульные тесты src/store.rs (подключены там через `#[path]`, видят приватное).

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

fn tab(url: &str) -> SavedTab {
    SavedTab { url: url.into(), ..Default::default() }
}

#[test]
fn session_save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    let full = SavedTab {
        url: "https://a.test/".into(),
        title: "A".into(),
        pinned: true,
        pinned_url: Some("https://a.test/home".into()),
        favicon: "https://a.test/favicon.ico".into(),
        last_active: 42,
        later: true,
        created_at: 40,
        parent: Some(1),
        closed_at: 0,
    };
    let s = Session {
        spaces: vec![
            SavedSpace {
                id: 1,
                name: "Основное".into(),
                color: "violet".into(),
                tabs: vec![full.clone(), tab("https://b.test/")],
                active: Some(1),
                closed: vec![tab("https://old.test/")],
                splits: vec![SavedSplit { left: 0, right: 1, ratio: 0.3 }],
            },
            SavedSpace { id: 4, name: "Работа".into(), color: "blue".into(), ..Default::default() },
        ],
        active_space: 1,
        sidebar_visible: Some(false),
        deleted_spaces: vec![2, 3],
        ..Default::default()
    };
    s.save(&path);

    let mut loaded = Session::load(&path);
    assert_eq!((loaded.active_space, loaded.sidebar_visible), (1, Some(false)));
    assert_eq!(loaded.deleted_spaces, [2, 3]);
    let spaces = loaded.take_spaces();
    assert_eq!(spaces.len(), 2);
    let main = &spaces[0];
    assert_eq!((main.id, main.name.as_str(), main.color.as_str()), (1, "Основное", "violet"));
    let t = &main.tabs[0];
    assert_eq!(t.url, "https://a.test/");
    assert!(t.pinned);
    assert_eq!(t.pinned_url.as_deref(), Some("https://a.test/home"));
    assert_eq!(t.last_active, 42);
    assert!(t.later);
    assert_eq!((t.created_at, t.parent), (40, Some(1)));
    assert!(!main.tabs[1].later && main.tabs[1].parent.is_none());
    assert_eq!(main.active, Some(1));
    assert_eq!(main.closed[0].url, "https://old.test/");
    assert_eq!(main.splits, [SavedSplit { left: 0, right: 1, ratio: 0.3 }]);
    assert_eq!((spaces[1].id, spaces[1].name.as_str()), (4, "Работа"));
}

#[test]
fn new_format_does_not_write_old_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    Session { spaces: vec![SavedSpace { id: 1, ..Default::default() }], ..Default::default() }.save(&path);
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for old in ["tabs", "active", "closed", "splits"] {
        assert!(v.get(old).is_none(), "{old}");
    }
}

#[test]
fn old_session_becomes_main_space() {
    // Так выглядела session.json до пространств.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    std::fs::write(
        &path,
        r#"{"tabs":[{"url":"https://www.google.com/","pinned":true},{"url":"https://a.test/"}],
                "active":1,"sidebar_visible":true,"closed":[{"url":"https://yandex.uz/"}],
                "splits":[{"left":0,"right":1,"ratio":0.4}]}"#,
    )
    .unwrap();
    let mut s = Session::load(&path);
    let spaces = s.take_spaces();
    assert_eq!(spaces.len(), 1);
    let main = &spaces[0];
    assert_eq!(main.id, 1);
    assert_eq!(main.tabs.len(), 2);
    assert!(main.tabs[0].pinned);
    assert_eq!(main.active, Some(1));
    assert_eq!(main.closed[0].url, "https://yandex.uz/");
    assert_eq!(main.splits[0].ratio, 0.4);
    assert_eq!(s.sidebar_visible, Some(true));
    assert_eq!(s.active_space, 0);
    // Старые поля перенесены, а не скопированы.
    assert!(s.tabs.is_empty() && s.closed.is_empty() && s.splits.is_empty() && s.active.is_none());
}

#[test]
fn session_tolerates_missing_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    std::fs::write(&path, r#"{"spaces":[{"id":2,"tabs":[{"url":"https://a.test/"}]}]}"#).unwrap();
    let mut s = Session::load(&path);
    let spaces = s.take_spaces();
    assert_eq!(spaces[0].tabs[0].url, "https://a.test/");
    assert!(!spaces[0].tabs[0].pinned);
    assert!(spaces[0].name.is_empty() && spaces[0].closed.is_empty());
    assert!(s.deleted_spaces.is_empty());
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
fn favicons_keep_only_data_uris_and_evict_oldest() {
    let mut f = Favicons { items: vec![], dirty: false };
    f.set("a.test", "https://a.test/favicon.ico");
    f.set("", "data:image/png;base64,AA");
    assert!(f.items.is_empty() && !f.dirty);
    f.set("a.test", "data:image/png;base64,AA");
    f.set("b.test", "data:image/png;base64,BB");
    f.set("a.test", "data:image/png;base64,A2");
    assert_eq!(f.get("a.test"), Some("data:image/png;base64,A2"));
    assert_eq!(f.items.last().unwrap().0, "a.test");
    for i in 0..FAVICONS_LIMIT {
        f.set(&format!("h{i}.test"), "data:image/png;base64,CC");
    }
    assert_eq!(f.items.len(), FAVICONS_LIMIT);
    assert_eq!(f.get("b.test"), None);
}

#[test]
fn favicons_roundtrip_drops_foreign_urls() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("favicons.json");
    std::fs::write(&p, r#"[["a.test","data:image/png;base64,AA"],["b.test","https://b.test/x.ico"]]"#).unwrap();
    let mut f = Favicons::load(&p);
    assert_eq!(f.get("a.test"), Some("data:image/png;base64,AA"));
    assert_eq!(f.get("b.test"), None);
    f.set("c.test", "data:image/png;base64,CC");
    f.save(&p);
    assert_eq!(Favicons::load(&p).get("c.test"), Some("data:image/png;base64,CC"));
}

#[test]
fn now_is_after_2020() {
    assert!(now() > 1_577_836_800);
}

#[test]
fn top_sites_sum_visits_per_site() {
    let mut a = entry("https://github.com/rust-lang", 10);
    a.visits = 3;
    let mut b = entry("https://www.github.com/", 20);
    b.visits = 2;
    let mut c = entry("https://example.com/a", 30);
    c.visits = 4;
    let h = history(vec![a, b, c, entry("about:blank", 40), entry("file:///C:/x.html", 50), entry("not a url", 60)]);
    let top = h.top_sites(10);
    let hosts: Vec<_> = top.iter().map(|s| (s.host.as_str(), s.visits)).collect();
    assert_eq!(hosts, [("github.com", 5), ("example.com", 4)]);
    assert_eq!(top[0].url, "https://github.com/");
    assert_eq!(top[0].last, 20);
}

#[test]
fn top_sites_ties_go_to_recent_and_respect_limit() {
    let h = history(vec![entry("https://a.test/", 1), entry("https://b.test/", 5), entry("https://c.test/", 3)]);
    let hosts: Vec<_> = h.top_sites(2).into_iter().map(|s| s.host).collect();
    assert_eq!(hosts, ["b.test", "c.test"]);
    assert!(history(vec![]).top_sites(5).is_empty());
}

#[test]
fn remove_and_restore_history_entry() {
    let mut h = history(vec![entry("https://a/", 1), entry("https://b/", 2)]);
    let e = h.remove("https://a/").unwrap();
    assert!(h.is_dirty());
    assert_eq!(h.entries.len(), 1);
    assert!(h.remove("https://missing/").is_none());
    h.restore(e);
    assert_eq!(h.entries.len(), 2);
    // Адрес открыли снова до отмены — запись одна, посещения сложены.
    let e = h.remove("https://b/").unwrap();
    h.visit("https://b/", "B");
    h.restore(e);
    let b: Vec<_> = h.entries.iter().filter(|e| e.url == "https://b/").collect();
    assert_eq!((b.len(), b[0].visits, b[0].title.as_str()), (1, 2, "B"));
}

#[test]
fn around_finds_nearby_visits_closest_first() {
    let h = history(vec![
        entry("https://x/", 1000),
        entry("https://a/", 1100),
        entry("https://b/", 950),
        entry("https://far/", 5000),
    ]);
    let urls: Vec<_> = h.around("https://x/", 1000, 600, 5).iter().map(|e| e.url.as_str()).collect();
    assert_eq!(urls, ["https://b/", "https://a/"]);
    assert_eq!(h.around("https://x/", 1000, 600, 1).len(), 1);
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
fn progress_and_remove() {
    let mut d = downloads();
    d.started("https://x/a", "", 1);
    assert!(d.progress("https://x/a", 10, 100));
    assert!(!d.progress("https://x/b", 1, 1));
    assert_eq!((d.items[0].received, d.items[0].total), (10, 100));
    d.finished("https://x/a", None, true, 2);
    assert_eq!(d.items[0].received, 100);
    let id = d.items[0].id;
    assert!(d.remove(id));
    assert!(!d.remove(id));
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

// ---------- Перенос из другого браузера ----------

#[test]
fn history_import_merges_known_urls_and_adds_new() {
    let mut h = history(vec![HistEntry { url: "https://a.test/".into(), title: String::new(), ts: 100, visits: 2 }]);
    let added = h.import(vec![
        HistEntry { url: "https://a.test/".into(), title: "A".into(), ts: 50, visits: 9 },
        HistEntry { url: "https://b.test/".into(), title: "B".into(), ts: 70, visits: 1 },
        HistEntry { url: "file:///C:/x".into(), title: "X".into(), ts: 80, visits: 1 },
    ]);
    assert_eq!(added, 1);
    assert!(h.is_dirty());
    let a = h.entries.iter().find(|e| e.url == "https://a.test/").unwrap();
    // Больше посещений, своё (более позднее) время, заголовок — раз своего не было.
    assert_eq!((a.visits, a.ts, a.title.as_str()), (9, 100, "A"));
    assert_eq!(h.entries.len(), 2, "file: не переносится");
}

#[test]
fn history_import_keeps_newest_within_limit() {
    let mut h = history(vec![entry("https://mine.test/", 10_000_000)]);
    let many: Vec<HistEntry> =
        (0..HISTORY_LIMIT as u64 + 10).map(|i| entry(&format!("https://x.test/{i}"), i)).collect();
    h.import(many);
    assert_eq!(h.entries.len(), HISTORY_LIMIT);
    assert!(h.entries.iter().any(|e| e.url == "https://mine.test/"));
    assert!(!h.entries.iter().any(|e| e.url == "https://x.test/0"), "самые старые отброшены");
}

#[test]
fn imported_favicons_never_replace_or_push_out_own() {
    let mut f = Favicons { items: vec![("a.test".into(), "data:image/png;base64,MINE".into())], dirty: false };
    f.set_if_missing("a.test", "data:image/png;base64,THEIRS");
    f.set_if_missing("b.test", "data:image/png;base64,B");
    f.set_if_missing("c.test", "https://c.test/icon.png");
    assert_eq!(f.get("a.test"), Some("data:image/png;base64,MINE"));
    assert_eq!(f.get("b.test"), Some("data:image/png;base64,B"));
    assert_eq!(f.get("c.test"), None);
    // Перенесённый значок — самый старый: он уйдёт первым.
    assert_eq!(f.items[0].0, "b.test");
    // Места нет — не добавляется.
    let mut full = Favicons {
        items: (0..FAVICONS_LIMIT).map(|i| (format!("s{i}"), "data:image/png;base64,x".into())).collect(),
        dirty: false,
    };
    full.set_if_missing("new.test", "data:image/png;base64,N");
    assert_eq!(full.get("new.test"), None);
}
