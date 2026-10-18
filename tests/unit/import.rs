//! Модульные тесты src/import.rs (подключены там через `#[path]`, видят приватное).

use super::*;

/// Время Chromium для секунд Unix.
fn ct(unix: i64) -> i64 {
    (unix + 11_644_473_600) * 1_000_000
}

const BOOKMARKS: &str = r#"{
  "checksum": "x", "version": 1,
  "roots": {
    "bookmark_bar": { "type": "folder", "name": "Bookmarks bar", "children": [
      { "type": "url", "name": "GitHub", "url": "https://github.com/", "date_added": "13370000000000000" },
      { "type": "url", "name": "Bookmarklet", "url": "javascript:alert(1)" },
      { "type": "url", "name": "Settings", "url": "chrome://settings/" },
      { "type": "folder", "name": "Work", "children": [
        { "type": "url", "name": "Docs", "url": "https://docs.test/" },
        { "type": "folder", "name": "Empty", "children": [] }
      ]}
    ]},
    "other": { "type": "folder", "name": "Other bookmarks", "children": [
      { "type": "url", "name": "  Spaced  ", "url": "http://old.test/" }
    ]},
    "synced": { "type": "folder", "name": "Mobile bookmarks", "children": [] }
  }
}"#;

#[test]
fn bookmarks_keep_structure_and_skip_non_web_links() {
    let (bar, other, mobile) = parse_bookmarks(BOOKMARKS).unwrap();
    assert_eq!(bar.len(), 2, "GitHub и Work; javascript: и chrome:// пропущены");
    assert_eq!((bar[0].title.as_str(), bar[0].url.as_str()), ("GitHub", "https://github.com/"));
    assert_eq!(bar[0].added, chrome_time(13_370_000_000_000_000));
    let work = &bar[1];
    assert!(work.is_folder());
    // Пустая папка не переносится.
    assert_eq!(work.children.as_ref().unwrap().len(), 1);
    assert_eq!(other[0].title, "Spaced");
    assert!(mobile.is_empty());
}

#[test]
fn broken_bookmarks_file_is_an_error() {
    assert!(parse_bookmarks("{").is_err());
    assert!(parse_bookmarks("{}").is_err());
    // Нет отдельных корней — просто пусто.
    let (bar, other, mobile) = parse_bookmarks(r#"{"roots":{}}"#).unwrap();
    assert!(bar.is_empty() && other.is_empty() && mobile.is_empty());
}

#[test]
fn deeply_nested_folders_are_cut() {
    let mut json = String::from(r#"{"type":"url","name":"deep","url":"https://deep.test/"}"#);
    for _ in 0..40 {
        json = format!(r#"{{"type":"folder","name":"f","children":[{json}]}}"#);
    }
    let file = format!(r#"{{"roots":{{"bookmark_bar":{{"children":[{json}]}}}}}}"#);
    let (bar, _, _) = parse_bookmarks(&file).unwrap();
    assert!(bar.is_empty(), "ссылка глубже 32 папок не переносится");
}

#[test]
fn chrome_time_to_unix() {
    assert_eq!(chrome_time(ct(1_700_000_000)), 1_700_000_000);
    assert_eq!(chrome_time(0), 0);
    assert_eq!(chrome_time(-5), 0);
}

fn history_db(dir: &Path) -> PathBuf {
    let path = dir.join("History");
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch(
        "CREATE TABLE urls (id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, visit_count INTEGER,
            typed_count INTEGER DEFAULT 0, last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0);",
    )
    .unwrap();
    let rows: &[(&str, Option<&str>, i64, i64, i64)] = &[
        ("https://old.test/", Some("Old"), 3, ct(1_600_000_000), 0),
        ("https://new.test/", Some("New"), 10, ct(1_700_000_000), 0),
        ("https://hidden.test/", Some("Hidden"), 1, ct(1_700_000_100), 1),
        ("file:///C:/secret.txt", Some("File"), 1, ct(1_700_000_200), 0),
        ("http://notitle.test/", None, 0, ct(1_650_000_000), 0),
        ("https://never.test/", Some("Never"), 1, 0, 0),
    ];
    for (url, title, visits, last, hidden) in rows {
        c.execute(
            "INSERT INTO urls (url, title, visit_count, last_visit_time, hidden) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![url, title, visits, last, hidden],
        )
        .unwrap();
    }
    path
}

#[test]
fn history_newest_first_only_visible_web_pages() {
    let dir = tempfile::tempdir().unwrap();
    let db = history_db(dir.path());
    let h = read_history(&db, 100).unwrap();
    let urls: Vec<&str> = h.iter().map(|e| e.url.as_str()).collect();
    assert_eq!(urls, ["https://new.test/", "http://notitle.test/", "https://old.test/"]);
    assert_eq!((h[0].title.as_str(), h[0].ts, h[0].visits), ("New", 1_700_000_000, 10));
    // Без заголовка и с нулём посещений — пустой заголовок и одно посещение.
    assert_eq!((h[1].title.as_str(), h[1].visits), ("", 1));
}

#[test]
fn history_respects_limit() {
    let dir = tempfile::tempdir().unwrap();
    let db = history_db(dir.path());
    assert_eq!(read_history(&db, 1).unwrap().len(), 1);
}

#[test]
fn not_a_database_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("History");
    std::fs::write(&path, "not sqlite").unwrap();
    assert!(read_history(&path, 10).is_err());
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";

#[test]
fn favicons_pick_size_closest_to_32_for_wanted_domains() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Favicons");
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch(
        "CREATE TABLE icon_mapping (id INTEGER PRIMARY KEY, page_url LONGVARCHAR NOT NULL, icon_id INTEGER);
         CREATE TABLE favicon_bitmaps (id INTEGER PRIMARY KEY, icon_id INTEGER NOT NULL, image_data BLOB, width INTEGER);",
    )
    .unwrap();
    let png16 = [PNG, b"16"].concat();
    let png32 = [PNG, b"32"].concat();
    c.execute_batch(
        "INSERT INTO icon_mapping (page_url, icon_id) VALUES
            ('https://www.github.com/x', 1), ('https://other.test/', 2), ('https://bad.test/', 3);",
    )
    .unwrap();
    for (icon, data, w) in
        [(1, png16.clone(), 16), (1, png32.clone(), 32), (2, png32.clone(), 32), (3, b"GIF89a".to_vec(), 32)]
    {
        c.execute(
            "INSERT INTO favicon_bitmaps (icon_id, image_data, width) VALUES (?1, ?2, ?3)",
            rusqlite::params![icon, data, w],
        )
        .unwrap();
    }
    drop(c);
    let wanted: HashSet<String> = ["github.com", "bad.test"].into_iter().map(String::from).collect();
    let icons = read_favicons(&path, &wanted).unwrap();
    // other.test не нужен; у bad.test не PNG.
    assert_eq!(icons, vec![("github.com".to_string(), png32)]);
}

/// Папка «LOCALAPPDATA» с Chrome (два профиля) и Edge.
fn fake_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let chrome = root.path().join(r"Google\Chrome\User Data");
    for p in ["Default", "Profile 2", "Profile 3"] {
        std::fs::create_dir_all(chrome.join(p)).unwrap();
    }
    std::fs::write(chrome.join("Default").join("Bookmarks"), BOOKMARKS).unwrap();
    history_db(&chrome.join("Profile 2"));
    // Profile 3 без данных — не предлагается.
    std::fs::write(
        chrome.join("Local State"),
        r#"{"profile":{"profiles_order":["Profile 2","Default","..\\..\\evil"],
            "info_cache":{"Default":{"name":"Personal"},"Profile 2":{"name":"Work"},"Profile 3":{"name":"Empty"}}}}"#,
    )
    .unwrap();
    let edge = root.path().join(r"Microsoft\Edge\User Data\Default");
    std::fs::create_dir_all(&edge).unwrap();
    std::fs::write(edge.join("Bookmarks"), BOOKMARKS).unwrap();
    root
}

#[test]
fn sources_list_profiles_with_data_in_browser_order() {
    let root = fake_root();
    let s = sources(root.path());
    let got: Vec<(String, &str)> = s.iter().map(|s| (s.id(), s.profile_name.as_str())).collect();
    assert_eq!(
        got,
        [
            ("chrome:Profile 2".to_string(), "Work"),
            ("chrome:Default".to_string(), "Personal"),
            // Без Local State — профиль Default под своим именем.
            ("edge:Default".to_string(), "Default"),
        ]
    );
    assert_eq!(s[2].browser_name, "Microsoft Edge");
}

#[test]
fn no_browsers_no_sources() {
    let root = tempfile::tempdir().unwrap();
    assert!(sources(root.path()).is_empty());
}

#[test]
fn read_profile_with_bookmarks_history_and_missing_parts() {
    let root = fake_root();
    let all = What { bookmarks: true, history: true };
    let s = sources(root.path());
    let work = read(&s[0], all);
    assert!(work.bar.is_empty(), "у Work нет закладок — и это не ошибка");
    assert_eq!(work.history.len(), 3);
    assert!(work.errors.is_empty(), "{:?}", work.errors);
    let personal = read(&s[1], What { bookmarks: true, history: false });
    assert_eq!(personal.bar.len(), 2);
    assert!(personal.history.is_empty());
}

#[test]
fn unreadable_part_is_reported_but_others_still_import() {
    let root = fake_root();
    let s = sources(root.path());
    std::fs::write(s[1].dir.join("History"), "not sqlite").unwrap();
    let r = read(&s[1], What { bookmarks: true, history: true });
    assert_eq!(r.bar.len(), 2);
    assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
}
