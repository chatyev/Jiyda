//! Модульные тесты src/bookmarks.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn titles(nodes: &[Node]) -> Vec<&str> {
    nodes.iter().map(|n| n.title.as_str()).collect()
}

fn sample() -> (Bookmarks, u64, u64, u64) {
    let mut b = Bookmarks::from_items(vec![]);
    let work = b.add_folder(None, "Work").unwrap();
    let a = b.add(Some(work), "A", "https://a.test/", 1).unwrap();
    let c = b.add(None, "C", "https://c.test/", 2).unwrap();
    (b, work, a, c)
}

#[test]
fn add_links_and_folders() {
    let (b, work, a, c) = sample();
    assert_eq!(titles(b.items()), ["Work", "C"]);
    assert!(b.get(work).unwrap().is_folder());
    assert_eq!(b.get(a).unwrap().url, "https://a.test/");
    assert!(b.contains("https://a.test/") && b.contains("https://c.test/"));
    assert!(!b.contains("https://other.test/"));
    assert_eq!(b.find_url("https://c.test/"), Some(c));
    assert_eq!(b.len(), 2);
}

#[test]
fn cannot_add_into_a_link_or_missing_folder() {
    let (mut b, _, a, _) = sample();
    assert_eq!(b.add(Some(a), "X", "https://x.test/", 0), None);
    assert_eq!(b.add(Some(999), "X", "https://x.test/", 0), None);
    assert!(!b.contains("https://x.test/"));
}

#[test]
fn remove_and_undo_put_it_back_in_place() {
    let (mut b, work, a, _) = sample();
    b.add(Some(work), "B", "https://b.test/", 0);
    let r = b.remove(a).unwrap();
    assert!(!b.contains("https://a.test/"));
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["B"]);
    b.restore(r);
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["A", "B"]);
    assert!(b.contains("https://a.test/"));
}

#[test]
fn removing_a_folder_removes_its_links_and_undo_restores_them() {
    let (mut b, work, _, _) = sample();
    let r = b.remove(work).unwrap();
    assert!(!b.contains("https://a.test/"));
    assert_eq!(titles(b.items()), ["C"]);
    b.restore(r);
    assert_eq!(titles(b.items()), ["Work", "C"]);
    assert!(b.contains("https://a.test/"));
}

#[test]
fn undo_into_deleted_folder_goes_to_top_level() {
    let (mut b, work, a, _) = sample();
    let r = b.remove(a).unwrap();
    b.remove(work);
    b.restore(r);
    assert_eq!(titles(b.items()), ["C", "A"]);
}

#[test]
fn edit_title_and_url() {
    let (mut b, work, a, _) = sample();
    assert!(b.edit(a, " Alpha ", Some("https://alpha.test/")));
    let n = b.get(a).unwrap();
    assert_eq!((n.title.as_str(), n.url.as_str()), ("Alpha", "https://alpha.test/"));
    assert!(b.contains("https://alpha.test/") && !b.contains("https://a.test/"));
    // У папки адреса нет; пустой адрес не затирает старый.
    b.edit(work, "Job", Some("https://x.test/"));
    assert_eq!(b.get(work).unwrap().url, "");
    b.edit(a, "Alpha", Some("  "));
    assert_eq!(b.get(a).unwrap().url, "https://alpha.test/");
    assert!(!b.edit(999, "X", None));
}

#[test]
fn move_between_folders_and_reorder() {
    let (mut b, work, a, c) = sample();
    assert!(b.move_to(c, Some(work), None));
    assert_eq!(titles(b.items()), ["Work"]);
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["A", "C"]);
    // В той же папке: C — в начало.
    assert!(b.move_to(c, Some(work), Some(0)));
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["C", "A"]);
    // C вниз, на место после A.
    assert!(b.move_to(c, Some(work), Some(2)));
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["A", "C"]);
    assert!(b.move_to(a, None, Some(0)));
    assert_eq!(titles(b.items()), ["A", "Work"]);
}

#[test]
fn folder_cannot_move_into_itself_or_its_child() {
    let (mut b, work, _, _) = sample();
    let inner = b.add_folder(Some(work), "Inner").unwrap();
    assert!(!b.move_to(work, Some(work), None));
    assert!(!b.move_to(work, Some(inner), None));
    // Ни в ссылку, ни в несуществующую папку.
    let c = b.find_url("https://c.test/").unwrap();
    assert!(!b.move_to(work, Some(c), None));
    assert!(!b.move_to(c, Some(999), None));
    assert_eq!(titles(b.items()), ["Work", "C"]);
}

#[test]
fn import_creates_folder_and_skips_known_urls() {
    let (mut b, _, _, _) = sample();
    let n = b.import(
        "Chrome",
        vec![
            Node::link("A again", "https://a.test/", 0),
            Node::link("D", "https://d.test/", 0),
            Node::folder("Only known", vec![Node::link("C", "https://c.test/", 0)]),
            Node::folder("News", vec![Node::link("E", "https://e.test/", 0), Node::link("E2", "https://e.test/", 0)]),
        ],
    );
    assert_eq!(n, 2, "D и E");
    let chrome = b.items().iter().find(|n| n.title == "Chrome").unwrap();
    // Папка, где всё уже было, пропущена; повтор адреса внутри импорта — тоже.
    assert_eq!(titles(chrome.children.as_ref().unwrap()), ["D", "News"]);
    assert_eq!(chrome.children.as_ref().unwrap()[1].children.as_ref().unwrap().len(), 1);
    // У всех новые уникальные номера.
    let mut ids = HashSet::new();
    walk(b.items(), &mut |n| assert!(n.id > 0 && ids.insert(n.id), "номер {}", n.id));
}

#[test]
fn reimport_adds_only_new_into_the_same_folder() {
    let mut b = Bookmarks::from_items(vec![]);
    assert_eq!(b.import("Chrome", vec![Node::link("A", "https://a.test/", 0)]), 1);
    assert_eq!(b.import("Chrome", vec![Node::link("A", "https://a.test/", 0)]), 0);
    assert_eq!(
        b.import("Chrome", vec![Node::link("A", "https://a.test/", 0), Node::link("B", "https://b.test/", 0)]),
        1
    );
    assert_eq!(b.items().len(), 1, "одна папка Chrome");
    assert_eq!(titles(b.items()[0].children.as_ref().unwrap()), ["A", "B"]);
}

#[test]
fn import_stops_at_limit() {
    let mut b = Bookmarks::from_items(vec![]);
    let many: Vec<Node> = (0..LIMIT + 5).map(|i| Node::link("x", &format!("https://x.test/{i}"), 0)).collect();
    assert_eq!(b.import("Big", many), LIMIT);
    assert_eq!(b.len(), LIMIT);
}

#[test]
fn save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bookmarks.json");
    let (mut b, work, a, _) = sample();
    b.save(&path);
    let l = Bookmarks::load(&path);
    assert_eq!(l.items(), b.items());
    assert!(l.contains("https://a.test/"));
    // Новые номера продолжаются после сохранённых.
    let mut l = l;
    let n = l.add(Some(work), "N", "https://n.test/", 0).unwrap();
    assert!(n > a && n > work);
}

#[test]
fn load_repairs_missing_and_duplicate_ids() {
    let items = vec![
        Node { id: 5, ..Node::link("A", "https://a.test/", 0) },
        Node { id: 5, ..Node::link("B", "https://b.test/", 0) },
        Node::folder("F", vec![Node::link("C", "https://c.test/", 0)]),
    ];
    let b = Bookmarks::from_items(items);
    let mut ids = HashSet::new();
    walk(b.items(), &mut |n| assert!(n.id > 0 && ids.insert(n.id)));
    assert_eq!(ids.len(), 4);
}

#[test]
fn broken_file_gives_empty_bookmarks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bookmarks.json");
    std::fs::write(&path, "{not json").unwrap();
    assert!(Bookmarks::load(&path).items().is_empty());
    assert!(Bookmarks::load(&dir.path().join("missing.json")).items().is_empty());
}

#[test]
fn changes_are_written_once() {
    let (mut b, _, _, _) = sample();
    assert!(b.take_changes().is_some());
    assert!(b.take_changes().is_none());
}

#[test]
fn save_page_adds_once_and_then_edits_in_place() {
    let (mut b, work, a, _) = sample();
    let (id, new) = b.save_page(None, " Dev ", "https://dev.test/", 5);
    assert!(new);
    assert_eq!(b.get(id).unwrap().title, "Dev");
    assert_eq!(titles(b.items()), ["Work", "C", "Dev"]);
    // Тот же адрес ещё раз — не вторая закладка, а новое название и папка у прежней.
    let (again, new) = b.save_page(Some(work), "Dev docs", "https://dev.test/", 6);
    assert_eq!((again, new), (id, false));
    assert_eq!(b.len(), 3);
    assert_eq!(titles(b.items()), ["Work", "C"]);
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["A", "Dev docs"]);
    // Папка та же — порядок не меняется.
    b.save_page(Some(work), "A2", "https://a.test/", 7);
    assert_eq!(titles(b.get(work).unwrap().children.as_ref().unwrap()), ["A2", "Dev docs"]);
    // Папки нет или это ссылка — на верхний уровень.
    let (x, _) = b.save_page(Some(999), "X", "https://x.test/", 8);
    let (y, _) = b.save_page(Some(a), "Y", "https://y.test/", 9);
    assert!(b.items().iter().any(|n| n.id == x) && b.items().iter().any(|n| n.id == y));
}
