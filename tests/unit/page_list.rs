//! Модульные тесты src/page_list.rs (подключены там через `#[path]`, видят приватное).

use super::*;

const NOW: u64 = 1_000_000;
const HOUR: u64 = 3600;

fn st(url: &str, pinned: bool, last_active: u64) -> SavedTab {
    SavedTab { url: url.into(), pinned, pinned_url: pinned.then(|| url.to_string()), last_active, ..Default::default() }
}

/// Список из сохранённых вкладок (без архивации); у всех вкладок есть «страница».
fn list(tabs: Vec<SavedTab>) -> (PageList<()>, Vec<u64>) {
    let mut l = PageList::new(vec![]);
    l.restore(tabs, None, &[], NOW, 0);
    for t in &mut l.pages {
        t.view = Some(());
    }
    let ids = l.pages.iter().map(|t| t.id).collect();
    (l, ids)
}

fn urls(l: &PageList<()>) -> Vec<&str> {
    l.ordered().iter().map(|t| t.url.as_str()).collect()
}

fn closed_urls(l: &PageList<()>) -> Vec<&str> {
    l.closed().iter().map(|t| t.url.as_str()).collect()
}

// ---------- Восстановление и сохранение ----------

#[test]
fn restore_keeps_order_and_active() {
    let mut l: PageList<()> = PageList::new(vec![]);
    let target = l.restore(vec![st("https://a/", true, NOW), st("https://b/", false, NOW)], Some(1), &[], NOW, 0);
    assert_eq!(urls(&l), ["https://a/", "https://b/"]);
    assert_eq!(target, l.ordered().get(1).map(|t| t.id));
    assert!(l.iter().all(|t| t.view.is_none() && !t.loading));
}

#[test]
fn restore_without_active_targets_first_tab() {
    let mut l: PageList<()> = PageList::new(vec![]);
    let target = l.restore(vec![st("https://a/", false, NOW)], None, &[], NOW, 0);
    assert_eq!(target, l.ordered().first().map(|t| t.id));

    let mut empty: PageList<()> = PageList::new(vec![]);
    assert_eq!(empty.restore(vec![], None, &[], NOW, 0), None);
}

#[test]
fn restore_archives_stale_unpinned_tabs() {
    let mut l: PageList<()> = PageList::new(vec![st("https://old-closed/", false, 1)]);
    l.restore(
        vec![
            st("https://pinned-old/", true, NOW - 100 * HOUR),
            st("https://stale/", false, NOW - 13 * HOUR),
            st("https://fresh/", false, NOW - 11 * HOUR),
            st("https://unknown-time/", false, 0),
            SavedTab { later: true, ..st("https://later-old/", false, NOW - 100 * HOUR) },
        ],
        None,
        &[],
        NOW,
        12 * HOUR,
    );
    // Отложенные страницы не архивируются.
    assert_eq!(urls(&l), ["https://pinned-old/", "https://fresh/", "https://unknown-time/", "https://later-old/"]);
    // Архивированные встают после уже закрытых, со временем архивации.
    assert_eq!(closed_urls(&l), ["https://old-closed/", "https://stale/"]);
    assert_eq!(l.closed()[1].closed_at, NOW);
    // Вкладка без времени считается открытой сейчас.
    assert_eq!(l.ordered()[2].last_active, NOW);
}

#[test]
fn restore_with_archive_disabled_keeps_everything() {
    let mut l: PageList<()> = PageList::new(vec![]);
    l.restore(vec![st("https://ancient/", false, 1)], None, &[], NOW, 0);
    assert_eq!(urls(&l), ["https://ancient/"]);
}

#[test]
fn restore_falls_back_when_active_tab_was_archived() {
    let mut l: PageList<()> = PageList::new(vec![]);
    let target =
        l.restore(vec![st("https://keep/", false, NOW), st("https://stale/", false, 1)], Some(1), &[], NOW, HOUR);
    assert_eq!(target, l.ordered().first().map(|t| t.id));
    assert_eq!(urls(&l), ["https://keep/"]);
}

#[test]
fn restore_does_not_activate_later_page() {
    let mut l: PageList<()> = PageList::new(vec![]);
    let later = SavedTab { later: true, ..st("https://later/", false, NOW) };
    let target = l.restore(vec![later, st("https://now/", false, NOW)], Some(0), &[], NOW, 0);
    assert_eq!(l.get(target.unwrap()).unwrap().url, "https://now/");
}

#[test]
fn to_saved_uses_order_and_active_index() {
    let (mut l, ids) =
        list(vec![st("https://u1/", false, NOW), st("https://p/", true, NOW), st("https://u2/", false, NOW)]);
    l.activate(ids[2], NOW);
    let (saved, active, _) = l.to_saved();
    let saved_urls: Vec<_> = saved.iter().map(|t| t.url.as_str()).collect();
    assert_eq!(saved_urls, ["https://p/", "https://u1/", "https://u2/"]);
    assert_eq!(active, Some(2));
    assert_eq!(saved[0].pinned_url.as_deref(), Some("https://p/"));
}

#[test]
fn parents_and_states_survive_save_and_restore() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    let child = l.alloc_id();
    l.insert(child, "https://child/".into(), NOW, Some(ids[0]), None);
    l.move_to_later(child);
    let (tabs, active, splits) = l.to_saved();
    assert_eq!(tabs[1].parent, Some(0));
    assert!(tabs[1].later);

    let mut r: PageList<()> = PageList::new(vec![]);
    r.restore(tabs, active, &splits, NOW, 0);
    let o = r.ordered();
    assert_eq!(o[1].parent, Some(o[0].id));
    assert_eq!(o[1].state, PageState::Later);
    assert_eq!(o[1].created_at, NOW);
}

#[test]
fn restore_ignores_bad_parent_indices() {
    let mut l: PageList<()> = PageList::new(vec![]);
    let tabs = vec![
        SavedTab { parent: Some(0), ..st("https://self/", false, NOW) },
        SavedTab { parent: Some(9), ..st("https://b/", false, NOW) },
    ];
    l.restore(tabs, None, &[], NOW, 0);
    assert!(l.iter().all(|t| t.parent.is_none()));
}

#[test]
fn new_trims_closed_to_limit() {
    let closed = (0..CLOSED_LIMIT + 5).map(|i| st(&format!("https://c/{i}"), false, 1)).collect();
    let l: PageList<()> = PageList::new(closed);
    assert_eq!(l.closed().len(), CLOSED_LIMIT);
    assert_eq!(l.closed()[0].url, "https://c/5");
}

// ---------- Порядок, добавление, активация ----------

#[test]
fn order_puts_pinned_first() {
    let (l, _) = list(vec![
        st("https://u1/", false, NOW),
        st("https://p1/", true, NOW),
        st("https://u2/", false, NOW),
        st("https://p2/", true, NOW),
    ]);
    assert_eq!(urls(&l), ["https://p1/", "https://p2/", "https://u1/", "https://u2/"]);
}

#[test]
fn insert_goes_after_active_unpinned_tab() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    l.activate(ids[0], NOW);
    let id = l.alloc_id();
    l.insert(id, "https://new/".into(), NOW + 5, Some(ids[0]), None);
    assert_eq!(urls(&l), ["https://a/", "https://new/", "https://b/"]);
    let t = l.get(id).unwrap();
    assert!(t.loading && !t.pinned && t.title.is_empty());
    assert_eq!((t.parent, t.created_at, t.state), (Some(ids[0]), NOW + 5, PageState::Now));
}

#[test]
fn insert_drops_unknown_parent() {
    let (mut l, _) = list(vec![st("https://a/", false, NOW)]);
    let id = l.alloc_id();
    l.insert(id, "https://x/".into(), NOW, Some(999), None);
    assert_eq!(l.get(id).unwrap().parent, None);
}

#[test]
fn insert_goes_to_end_when_active_is_pinned_or_none() {
    let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
    let id = l.alloc_id();
    l.insert(id, "https://x/".into(), NOW, None, None);
    l.activate(ids[0], NOW);
    let id = l.alloc_id();
    l.insert(id, "https://y/".into(), NOW, None, None);
    assert_eq!(urls(&l), ["https://p/", "https://a/", "https://x/", "https://y/"]);
}

#[test]
fn alloc_id_is_unique() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    let a = l.alloc_id();
    let b = l.alloc_id();
    assert_ne!(a, b);
    assert!(!ids.contains(&a) && !ids.contains(&b));
}

#[test]
fn activate_sets_active_and_time() {
    let (mut l, ids) = list(vec![st("https://a/", false, 5)]);
    assert!(l.activate(ids[0], NOW));
    assert_eq!(l.active(), Some(ids[0]));
    assert_eq!(l.get(ids[0]).unwrap().last_active, NOW);
    assert!(!l.activate(999, NOW));
    assert_eq!(l.active(), Some(ids[0]));
}

#[test]
fn deactivate_shows_nothing_but_keeps_tabs() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    l.deactivate();
    assert_eq!(l.active(), None);
    assert!(l.visible().is_empty());
    assert_eq!(urls(&l), ["https://a/"]);
}

// ---------- Now / Later ----------

#[test]
fn later_unloads_and_moves_off_screen() {
    let (mut l, ids) =
        list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)]);
    l.activate(ids[1], NOW);
    assert_eq!(l.move_to_later(ids[1]), Some(AfterClose { was_active: true, next: Some(ids[2]) }));
    let t = l.get(ids[1]).unwrap();
    assert_eq!(t.state, PageState::Later);
    assert!(t.view.is_none());
    assert_eq!(l.active(), None);
    // Отложенная не участвует в навигации.
    assert_eq!(l.now_order(), [ids[0], ids[2]]);
    assert_eq!(l.goto_target(2), Some(ids[2]));
    assert!(!l.mru().contains(&ids[1]));
}

#[test]
fn later_breaks_pair_and_skips_pinned() {
    let (mut l, ids) =
        list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    assert_eq!(l.move_to_later(ids[0]), None);
    l.make_split(ids[1], ids[2]);
    l.activate(ids[1], NOW);
    assert_eq!(l.move_to_later(ids[1]).unwrap().next, Some(ids[2]));
    assert!(l.splits().is_empty());
    assert_eq!(l.move_to_later(999), None);
}

#[test]
fn activating_later_page_brings_it_back() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    l.move_to_later(ids[1]);
    l.activate(ids[1], NOW);
    assert_eq!(l.get(ids[1]).unwrap().state, PageState::Now);
    l.move_to_later(ids[0]);
    assert!(l.move_to_now(ids[0]));
    assert!(!l.move_to_now(ids[0]));
}

#[test]
fn pinning_brings_page_to_now() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    l.move_to_later(ids[0]);
    l.set_pinned(ids[0], true);
    assert_eq!(l.get(ids[0]).unwrap().state, PageState::Now);
}

// ---------- Сон ----------

#[test]
fn sleep_idle_unloads_only_old_background_tabs_without_sound() {
    let (mut l, ids) = list(vec![
        st("https://active/", false, 1),
        st("https://old/", false, 1),
        st("https://recent/", false, NOW - 10),
        st("https://music/", false, 1),
    ]);
    l.activate(ids[0], 1);
    l.get_mut(ids[1]).unwrap().loading = true;
    l.get_mut(ids[3]).unwrap().media = true;

    assert!(l.sleep_idle(NOW, HOUR));
    let asleep: Vec<bool> = ids.iter().map(|id| l.get(*id).unwrap().view.is_none()).collect();
    assert_eq!(asleep, [false, true, false, false]);
    assert!(!l.get(ids[1]).unwrap().loading);
    // Повторно усыплять некого.
    assert!(!l.sleep_idle(NOW, HOUR));
}

#[test]
fn offscreen_sleep_includes_active_tab() {
    let (mut l, ids) = list(vec![st("https://a/", false, 1), st("https://b/", false, 1)]);
    l.make_split(ids[0], ids[1]);
    l.activate(ids[0], 1);
    assert!(l.sleep_idle_offscreen(NOW, HOUR));
    assert!(l.iter().all(|t| t.view.is_none()));
    assert!(!l.sleep_idle_offscreen(NOW, 0));
}

#[test]
fn id_base_separates_tab_numbers() {
    let mut a: PageList<()> = PageList::with_id_base(vec![], 1_000);
    a.restore(vec![st("https://a/", false, NOW)], None, &[], NOW, 0);
    assert_eq!(a.iter().next().unwrap().id, 1_001);
    assert_eq!(a.alloc_id(), 1_002);
}

#[test]
fn sleep_idle_disabled_with_zero_limit() {
    let (mut l, _) = list(vec![st("https://a/", false, 1)]);
    assert!(!l.sleep_idle(NOW, 0));
    assert!(l.iter().all(|t| t.view.is_some()));
}

// ---------- Закрытие ----------

#[test]
fn close_active_moves_to_next_in_same_group() {
    let (mut l, ids) = list(vec![
        st("https://p/", true, NOW),
        st("https://a/", false, NOW),
        st("https://b/", false, NOW),
        st("https://c/", false, NOW),
    ]);
    l.activate(ids[2], NOW);
    assert_eq!(l.close(ids[2], NOW + 1), Some(AfterClose { was_active: true, next: Some(ids[3]) }));
    assert_eq!(l.active(), None);
    assert_eq!(urls(&l), ["https://p/", "https://a/", "https://c/"]);
    assert_eq!(closed_urls(&l), ["https://b/"]);
    assert_eq!(l.closed()[0].closed_at, NOW + 1);
}

#[test]
fn close_skips_later_pages_when_choosing_next() {
    let (mut l, ids) =
        list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)]);
    l.move_to_later(ids[1]);
    l.activate(ids[0], NOW);
    assert_eq!(l.close(ids[0], NOW).unwrap().next, Some(ids[2]));
}

#[test]
fn close_last_in_group_moves_to_previous_in_group() {
    let (mut l, ids) =
        list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    l.activate(ids[2], NOW);
    assert_eq!(l.close(ids[2], NOW).unwrap().next, Some(ids[1]));
}

#[test]
fn close_only_unpinned_moves_to_pinned() {
    let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
    l.activate(ids[1], NOW);
    assert_eq!(l.close(ids[1], NOW).unwrap().next, Some(ids[0]));
}

#[test]
fn close_the_only_tab_leaves_nothing_active() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    assert_eq!(l.close(ids[0], NOW), Some(AfterClose { was_active: true, next: None }));
    assert!(l.ordered().is_empty());
}

#[test]
fn close_background_tab_keeps_active() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    l.activate(ids[0], NOW);
    assert_eq!(l.close(ids[1], NOW), Some(AfterClose { was_active: false, next: Some(ids[0]) }));
    assert_eq!(l.active(), Some(ids[0]));
}

#[test]
fn close_unknown_tab_does_nothing() {
    let (mut l, _) = list(vec![st("https://a/", false, NOW)]);
    assert_eq!(l.close(999, NOW), None);
    assert_eq!(urls(&l), ["https://a/"]);
}

#[test]
fn closing_parent_orphans_children() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    let c = l.alloc_id();
    l.insert(c, "https://c/".into(), NOW, Some(ids[0]), None);
    l.close(ids[0], NOW);
    assert_eq!(l.get(c).unwrap().parent, None);
}

#[test]
fn close_pinned_tab_unloads_and_resets_url() {
    let (mut l, ids) = list(vec![st("https://home/", true, NOW), st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    let t = l.get_mut(ids[0]).unwrap();
    t.url = "https://home/elsewhere".into();
    t.media = true;
    assert!(t.drifted());

    assert_eq!(l.close(ids[0], NOW).unwrap().next, Some(ids[1]));
    let t = l.get(ids[0]).unwrap();
    assert_eq!(t.url, "https://home/");
    assert!(t.view.is_none() && !t.media && !t.drifted());
    assert!(l.closed().is_empty());
}

#[test]
fn close_non_http_tab_is_not_remembered() {
    let (mut l, ids) = list(vec![st("about:blank", false, NOW)]);
    l.close(ids[0], NOW);
    assert!(l.closed().is_empty());
}

#[test]
fn closed_list_is_capped() {
    let tabs = (0..CLOSED_LIMIT + 3).map(|i| st(&format!("https://t/{i}"), false, NOW)).collect();
    let (mut l, ids) = list(tabs);
    for id in ids {
        l.close(id, NOW);
    }
    assert_eq!(l.closed().len(), CLOSED_LIMIT);
    assert_eq!(l.closed()[0].url, "https://t/3");
}

#[test]
fn pop_closed_returns_last_closed_first() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    l.close(ids[0], NOW);
    l.close(ids[1], NOW);
    assert_eq!(l.pop_closed().unwrap().url, "https://b/");
    assert_eq!(l.pop_closed().unwrap().url, "https://a/");
    assert!(l.pop_closed().is_none());
}

#[test]
fn take_closed_by_index_and_clear() {
    let (mut l, ids) =
        list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)]);
    for id in ids {
        l.close(id, NOW);
    }
    assert_eq!(l.take_closed(1).unwrap().url, "https://b/");
    assert!(l.take_closed(5).is_none());
    assert_eq!(closed_urls(&l), ["https://a/", "https://c/"]);
    l.clear_closed();
    assert!(l.closed().is_empty());
}

#[test]
fn take_removes_page_for_another_space() {
    let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    let (saved, after) = l.take(ids[0]).unwrap();
    assert_eq!(saved.url, "https://p/");
    assert!(!saved.pinned && saved.pinned_url.is_none());
    assert_eq!(after, AfterClose { was_active: true, next: Some(ids[1]) });
    assert!(l.closed().is_empty());
    assert_eq!(urls(&l), ["https://a/"]);

    let mut other: PageList<()> = PageList::with_id_base(vec![], 5_000);
    let id = other.insert_saved(saved, NOW);
    assert!(id > 5_000);
    assert!(other.get(id).unwrap().view.is_none());
}

#[test]
fn clear_unpinned_with_active_unpinned_moves_to_first_pinned() {
    let (mut l, ids) =
        list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("about:blank", false, NOW)]);
    l.activate(ids[1], NOW);
    assert_eq!(l.clear_unpinned(NOW), AfterClose { was_active: true, next: Some(ids[0]) });
    assert_eq!(urls(&l), ["https://p/"]);
    assert_eq!(closed_urls(&l), ["https://a/"]);
}

#[test]
fn clear_unpinned_keeps_later_pages() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://later/", false, NOW)]);
    l.move_to_later(ids[1]);
    l.clear_unpinned(NOW);
    assert_eq!(urls(&l), ["https://later/"]);
}

#[test]
fn clear_unpinned_with_active_pinned_keeps_it() {
    let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    assert_eq!(l.clear_unpinned(NOW), AfterClose { was_active: false, next: Some(ids[0]) });
    assert_eq!(l.active(), Some(ids[0]));
}

#[test]
fn clear_unpinned_without_pinned_leaves_nothing() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    l.activate(ids[0], NOW);
    assert_eq!(l.clear_unpinned(NOW), AfterClose { was_active: true, next: None });
}

// ---------- Переключение ----------

#[test]
fn cycle_wraps_in_both_directions() {
    let (mut l, ids) =
        list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)]);
    l.activate(ids[2], NOW);
    assert_eq!(l.cycle_target(1), Some(ids[0]));
    assert_eq!(l.cycle_target(-1), Some(ids[1]));
    l.activate(ids[0], NOW);
    assert_eq!(l.cycle_target(-1), Some(ids[2]));
    assert_eq!(l.cycle_target(4), Some(ids[1]));
}

#[test]
fn cycle_without_active_starts_from_first() {
    let (l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    assert_eq!(l.cycle_target(1), Some(ids[1]));
    let empty: PageList<()> = PageList::new(vec![]);
    assert_eq!(empty.cycle_target(1), None);
}

#[test]
fn mru_puts_active_first_then_recent() {
    let (mut l, ids) =
        list(vec![st("https://a/", false, 10), st("https://b/", false, 30), st("https://c/", false, 20)]);
    assert_eq!(l.mru(), [ids[1], ids[2], ids[0]]);
    l.activate(ids[0], 5);
    assert_eq!(l.mru()[0], ids[0]);
}

#[test]
fn mru_walk_goes_deeper_on_quick_presses_and_restarts_later() {
    use std::time::{Duration, Instant};
    let (w, t0) = (Duration::from_millis(1500), Instant::now());
    let mut m = MruWalk::default();
    let fresh = || vec![1, 2, 3];
    assert_eq!(m.step(t0, w, true, fresh, |_| true), Some(2));
    assert_eq!(m.step(t0 + Duration::from_millis(500), w, true, fresh, |_| true), Some(3));
    // Назад по тому же списку.
    assert_eq!(m.step(t0 + Duration::from_millis(900), w, false, fresh, |_| true), Some(2));
    // Пауза — обход начинается заново, со списка, где текущая первой.
    assert_eq!(m.step(t0 + Duration::from_secs(5), w, true, || vec![2, 1, 3], |_| true), Some(1));
}

#[test]
fn mru_walk_back_returns_to_the_starting_page() {
    // Ctrl+Tab с B на A, затем Ctrl+Shift+Tab — снова B (найдено тестом E2E pages: next/prev).
    use std::time::{Duration, Instant};
    let (w, t0) = (Duration::from_millis(1500), Instant::now());
    let mut m = MruWalk::default();
    let fresh = || vec![2, 1];
    assert_eq!(m.step(t0, w, true, fresh, |_| true), Some(1));
    assert_eq!(m.step(t0 + Duration::from_millis(200), w, false, fresh, |_| true), Some(2));
    // По кругу вперёд — тоже через начальную.
    let mut c = MruWalk::default();
    let three = || vec![1, 2, 3];
    assert_eq!(c.step(t0, w, true, three, |_| true), Some(2));
    assert_eq!(c.step(t0, w, true, three, |_| true), Some(3));
    assert_eq!(c.step(t0, w, true, three, |_| true), Some(1));
}

#[test]
fn mru_walk_skips_closed_pages_and_needs_two() {
    use std::time::{Duration, Instant};
    let (w, t0) = (Duration::from_secs(1), Instant::now());
    let mut m = MruWalk::default();
    assert_eq!(m.step(t0, w, true, || vec![1, 2, 3], |id| id != 2), Some(3));
    let mut single = MruWalk::default();
    assert_eq!(single.step(t0, w, true, || vec![1], |_| true), None);
    let mut all_closed = MruWalk::default();
    assert_eq!(all_closed.step(t0, w, true, || vec![1, 2], |_| false), None);
}

#[test]
fn goto_by_number_and_nine_is_last() {
    let (l, ids) = list((0..10).map(|i| st(&format!("https://t/{i}"), false, NOW)).collect());
    assert_eq!(l.goto_target(1), Some(ids[0]));
    assert_eq!(l.goto_target(8), Some(ids[7]));
    assert_eq!(l.goto_target(9), Some(ids[9]));

    let (short, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    assert_eq!(short.goto_target(5), None);
    assert_eq!(short.goto_target(9), Some(ids[1]));
}

// ---------- Закрепление ----------

#[test]
fn pin_remembers_url_and_unpin_forgets_it() {
    let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
    l.set_pinned(ids[0], true);
    let t = l.get(ids[0]).unwrap();
    assert!(t.pinned);
    assert_eq!(t.pinned_url.as_deref(), Some("https://a/"));

    l.set_pinned(ids[0], false);
    let t = l.get(ids[0]).unwrap();
    assert!(!t.pinned && t.pinned_url.is_none());
    l.set_pinned(999, true);
}

#[test]
fn drifted_only_for_pinned_away_from_home() {
    let (mut l, ids) = list(vec![st("https://home/", true, NOW), st("https://a/", false, NOW)]);
    assert!(!l.get(ids[0]).unwrap().drifted());
    l.get_mut(ids[0]).unwrap().url = "https://other/".into();
    assert!(l.get(ids[0]).unwrap().drifted());
    l.get_mut(ids[1]).unwrap().url = "https://other/".into();
    assert!(!l.get(ids[1]).unwrap().drifted());
}

// ---------- Split View ----------

fn abc() -> (PageList<()>, Vec<u64>) {
    list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)])
}

#[test]
fn make_split_puts_right_next_to_left() {
    let (mut l, ids) = abc();
    assert!(l.make_split(ids[0], ids[2]));
    assert_eq!(urls(&l), ["https://a/", "https://c/", "https://b/"]);
    let s = l.split_of(ids[2]).unwrap();
    assert_eq!((s.left, s.right, s.ratio), (ids[0], ids[2], 0.5));
    assert_eq!(l.split_of(ids[1]), None);
    assert_eq!(s.partner(ids[0]), Some(ids[2]));
    assert_eq!(s.partner(ids[1]), None);
}

#[test]
fn make_split_rejects_same_or_missing_tab() {
    let (mut l, ids) = abc();
    assert!(!l.make_split(ids[0], ids[0]));
    assert!(!l.make_split(ids[0], 999));
    assert!(l.splits().is_empty());
}

#[test]
fn new_split_replaces_old_pairs_of_its_tabs() {
    let (mut l, ids) = abc();
    l.make_split(ids[0], ids[1]);
    l.make_split(ids[1], ids[2]);
    assert_eq!(l.splits().len(), 1);
    assert_eq!(l.split_of(ids[0]), None);
    assert_eq!(l.split_of(ids[2]).unwrap().left, ids[1]);
}

#[test]
fn split_brings_later_page_back() {
    let (mut l, ids) = abc();
    l.move_to_later(ids[2]);
    l.make_split(ids[0], ids[2]);
    assert_eq!(l.get(ids[2]).unwrap().state, PageState::Now);
}

#[test]
fn swap_split_exchanges_sides_and_mirrors_ratio() {
    let (mut l, ids) = abc();
    l.make_split(ids[0], ids[1]);
    l.set_split_ratio(ids[0], 0.3);
    assert!(l.swap_split(ids[0]));
    let s = l.split_of(ids[0]).unwrap();
    assert_eq!((s.left, s.right), (ids[1], ids[0]));
    assert!((s.ratio - 0.7).abs() < 1e-9);
    assert_eq!(urls(&l), ["https://b/", "https://a/", "https://c/"]);
    assert!(!l.swap_split(ids[2]));
}

#[test]
fn replace_in_split_keeps_side_and_ratio() {
    let (mut l, ids) = abc();
    l.make_split(ids[0], ids[1]);
    l.set_split_ratio(ids[0], 0.4);
    assert!(l.replace_in_split(ids[1], ids[2]));
    let s = l.split_of(ids[0]).unwrap();
    assert_eq!((s.left, s.right, s.ratio), (ids[0], ids[2], 0.4));
    assert!(l.split_of(ids[1]).is_none());
    // Заменить на вторую половину той же пары или на несуществующую нельзя.
    assert!(!l.replace_in_split(ids[2], ids[0]));
    assert!(!l.replace_in_split(ids[2], 999));
}

#[test]
fn halves_keep_their_own_pinning() {
    let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
    l.make_split(ids[1], ids[0]);
    let p = l.get(ids[0]).unwrap();
    assert!(p.pinned);
    assert_eq!(p.pinned_url.as_deref(), Some("https://p/"));
    assert!(!l.get(ids[1]).unwrap().pinned);
}

#[test]
fn pinning_one_half_pins_both() {
    let (mut l, ids) = abc();
    l.make_split(ids[1], ids[2]);
    l.set_pinned(ids[2], true);
    assert!(l.get(ids[1]).unwrap().pinned && l.get(ids[2]).unwrap().pinned);
    assert_eq!(urls(&l), ["https://b/", "https://c/", "https://a/"]);
}

#[test]
fn unsplit_breaks_pair() {
    let (mut l, ids) = abc();
    l.make_split(ids[0], ids[1]);
    assert!(l.unsplit(ids[1]));
    assert!(!l.unsplit(ids[1]));
    assert!(l.splits().is_empty());
}

#[test]
fn split_ratio_is_clamped() {
    let (mut l, ids) = abc();
    l.make_split(ids[0], ids[1]);
    l.set_split_ratio(ids[1], 0.65);
    assert_eq!(l.split_of(ids[0]).unwrap().ratio, 0.65);
    l.set_split_ratio(ids[0], 0.01);
    assert_eq!(l.split_of(ids[0]).unwrap().ratio, SPLIT_MIN);
    l.set_split_ratio(ids[0], 5.0);
    assert_eq!(l.split_of(ids[0]).unwrap().ratio, SPLIT_MAX);
    l.set_split_ratio(ids[0], f64::NAN);
    assert_eq!(l.split_of(ids[0]).unwrap().ratio, 0.5);
    // Вкладка не в паре — ничего не происходит.
    l.set_split_ratio(ids[2], 0.3);
}

#[test]
fn visible_includes_both_halves_of_active_pair() {
    let (mut l, ids) = abc();
    assert!(l.visible().is_empty());
    l.activate(ids[2], NOW);
    assert_eq!(l.visible(), [ids[2]]);
    l.make_split(ids[0], ids[2]);
    assert_eq!(l.visible(), [ids[0], ids[2]]);
}

#[test]
fn activating_half_refreshes_both_halves() {
    let (mut l, ids) = list(vec![st("https://a/", false, 1), st("https://b/", false, 1)]);
    l.make_split(ids[0], ids[1]);
    l.activate(ids[1], NOW);
    assert_eq!(l.get(ids[0]).unwrap().last_active, NOW);
    assert_eq!(l.get(ids[1]).unwrap().last_active, NOW);
}

#[test]
fn sleep_keeps_visible_half_awake() {
    let (mut l, ids) = list(vec![st("https://a/", false, 1), st("https://b/", false, 1), st("https://c/", false, 1)]);
    l.make_split(ids[0], ids[1]);
    l.activate(ids[0], 1);
    assert!(l.sleep_idle(NOW, HOUR));
    let asleep: Vec<bool> = ids.iter().map(|id| l.get(*id).unwrap().view.is_none()).collect();
    assert_eq!(asleep, [false, false, true]);
}

#[test]
fn closing_active_half_moves_to_partner_and_breaks_pair() {
    let (mut l, ids) = abc();
    l.make_split(ids[1], ids[2]);
    l.activate(ids[1], NOW);
    assert_eq!(l.close(ids[1], NOW), Some(AfterClose { was_active: true, next: Some(ids[2]) }));
    assert!(l.splits().is_empty());
}

#[test]
fn closing_background_half_breaks_pair() {
    let (mut l, ids) = abc();
    l.make_split(ids[1], ids[2]);
    l.activate(ids[0], NOW);
    assert_eq!(l.close(ids[2], NOW), Some(AfterClose { was_active: false, next: Some(ids[0]) }));
    assert!(l.splits().is_empty());
}

#[test]
fn clear_unpinned_drops_pairs_of_removed_tabs() {
    let (mut l, ids) =
        list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("https://b/", false, NOW)]);
    l.make_split(ids[1], ids[2]);
    l.clear_unpinned(NOW);
    assert!(l.splits().is_empty());
}

#[test]
fn insert_after_left_half_goes_after_pair() {
    let (mut l, ids) = abc();
    l.make_split(ids[0], ids[1]);
    l.activate(ids[0], NOW);
    let id = l.alloc_id();
    l.insert(id, "https://new/".into(), NOW, None, None);
    assert_eq!(urls(&l), ["https://a/", "https://b/", "https://new/", "https://c/"]);
}

#[test]
fn splits_survive_save_and_restore() {
    let (mut l, ids) = abc();
    l.make_split(ids[2], ids[0]);
    l.set_split_ratio(ids[0], 0.3);
    let (tabs, active, splits) = l.to_saved();
    assert_eq!(splits, [SavedSplit { left: 1, right: 2, ratio: 0.3 }]);

    let mut r: PageList<()> = PageList::new(vec![]);
    r.restore(tabs, active, &splits, NOW, 0);
    assert_eq!(urls(&r), ["https://b/", "https://c/", "https://a/"]);
    let s = r.splits()[0];
    assert_eq!(
        (r.get(s.left).unwrap().url.as_str(), r.get(s.right).unwrap().url.as_str(), s.ratio),
        ("https://c/", "https://a/", 0.3)
    );
}

#[test]
fn restore_drops_pair_with_archived_half_or_bad_index() {
    let mut l: PageList<()> = PageList::new(vec![]);
    let splits = [SavedSplit { left: 0, right: 1, ratio: 0.5 }, SavedSplit { left: 0, right: 9, ratio: 0.5 }];
    l.restore(vec![st("https://keep/", false, NOW), st("https://stale/", false, 1)], None, &splits, NOW, HOUR);
    assert!(l.splits().is_empty());
}
