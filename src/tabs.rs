//! Модель вкладок: порядок, закрепление, закрытие, сон и архив, Split View.
//!
//! Здесь нет окна и WebView: `V` — то, что держит загруженную страницу
//! (в браузере `wry::WebView`, в тестах `()`). Создаёт и показывает их `App`.

use crate::store::{SavedSplit, SavedTab};

const CLOSED_LIMIT: usize = 50;
/// Пределы доли ширины левой половины в Split View.
const SPLIT_MIN: f64 = 0.2;
const SPLIT_MAX: f64 = 0.8;

/// Две вкладки рядом (Split View). В списке правая всегда стоит сразу за левой
/// и закреплена так же, как левая.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Split {
    pub left: u64,
    pub right: u64,
    /// Доля ширины левой половины.
    pub ratio: f64,
}

impl Split {
    pub fn partner(&self, id: u64) -> Option<u64> {
        if id == self.left {
            Some(self.right)
        } else if id == self.right {
            Some(self.left)
        } else {
            None
        }
    }
}

fn clamp_ratio(r: f64) -> f64 {
    if r.is_finite() { r.clamp(SPLIT_MIN, SPLIT_MAX) } else { 0.5 }
}

pub struct Tab<V> {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub favicon: String,
    pub pinned: bool,
    pub pinned_url: Option<String>,
    pub loading: bool,
    pub media: bool,
    pub last_active: u64,
    /// None — вкладка спит (не загружена в память).
    pub view: Option<V>,
}

impl<V> Tab<V> {
    pub fn saved(&self) -> SavedTab {
        SavedTab {
            url: self.url.clone(),
            title: self.title.clone(),
            pinned: self.pinned,
            pinned_url: self.pinned_url.clone(),
            favicon: self.favicon.clone(),
            last_active: self.last_active,
        }
    }

    /// Закреплённая вкладка ушла со своего адреса.
    pub fn drifted(&self) -> bool {
        self.pinned && self.pinned_url.as_deref().is_some_and(|u| u != self.url)
    }

    fn set_pinned(&mut self, pinned: bool) {
        self.pinned = pinned;
        self.pinned_url = if pinned { Some(self.url.clone()) } else { None };
    }
}

/// Что стало после закрытия вкладок.
#[derive(Debug, PartialEq, Eq)]
pub struct AfterClose {
    /// Среди закрытых была активная вкладка (теперь активной нет).
    pub was_active: bool,
    /// Какую вкладку сделать активной.
    pub next: Option<u64>,
}

pub struct TabList<V> {
    /// Порядок хранения; в сайдбаре закреплённые идут первыми (см. `order`).
    tabs: Vec<Tab<V>>,
    active: Option<u64>,
    next_id: u64,
    /// Недавно закрытые и архивированные вкладки, последняя — в конце.
    closed: Vec<SavedTab>,
    splits: Vec<Split>,
}

impl<V> TabList<V> {
    pub fn new(closed: Vec<SavedTab>) -> Self {
        Self::with_id_base(closed, 0)
    }

    /// Список, у которого номера вкладок начинаются после `base` (у каждого
    /// пространства свой диапазон, см. spaces::tab_id_base).
    pub fn with_id_base(closed: Vec<SavedTab>, base: u64) -> Self {
        let mut list = Self { tabs: Vec::new(), active: None, next_id: base + 1, closed, splits: Vec::new() };
        list.trim_closed();
        list
    }

    /// Восстановить вкладки из сессии. Незакреплённые вкладки, которые не открывали
    /// дольше `archive_after` секунд (0 — не архивировать), уходят в закрытые;
    /// пара Split View, где одну из вкладок архивировали, распадается.
    /// Возвращает вкладку, которую нужно сделать активной.
    pub fn restore(
        &mut self,
        saved: Vec<SavedTab>,
        active: Option<usize>,
        splits: &[SavedSplit],
        now: u64,
        archive_after: u64,
    ) -> Option<u64> {
        let mut active_id = None;
        let mut ids = Vec::with_capacity(saved.len());
        for (i, st) in saved.into_iter().enumerate() {
            let stale = archive_after > 0 && !st.pinned && st.last_active > 0
                && now.saturating_sub(st.last_active) > archive_after;
            if stale {
                self.closed.push(st);
                ids.push(None);
                continue;
            }
            let id = self.alloc_id();
            ids.push(Some(id));
            if active == Some(i) {
                active_id = Some(id);
            }
            self.tabs.push(Tab {
                id,
                url: st.url,
                title: st.title,
                favicon: st.favicon,
                pinned: st.pinned,
                pinned_url: st.pinned_url,
                loading: false,
                media: false,
                last_active: if st.last_active == 0 { now } else { st.last_active },
                view: None,
            });
        }
        for s in splits {
            let id_at = |i: usize| ids.get(i).copied().flatten();
            if let (Some(left), Some(right)) = (id_at(s.left), id_at(s.right)) {
                self.make_split(left, right);
                self.set_split_ratio(left, s.ratio);
            }
        }
        self.trim_closed();
        active_id.or_else(|| self.order().first().copied())
    }

    /// Вкладки для сессии в порядке сайдбара, индекс активной и пары Split View
    /// (индексы в том же списке).
    pub fn to_saved(&self) -> (Vec<SavedTab>, Option<usize>, Vec<SavedSplit>) {
        let order = self.order();
        let tabs = order.iter().filter_map(|id| self.get(*id)).map(Tab::saved).collect();
        let index = |id: u64| order.iter().position(|x| *x == id);
        let active = self.active.and_then(index);
        let splits = self
            .splits
            .iter()
            .filter_map(|s| Some(SavedSplit { left: index(s.left)?, right: index(s.right)?, ratio: s.ratio }))
            .collect();
        (tabs, active, splits)
    }

    // ---------- Split View ----------

    pub fn splits(&self) -> &[Split] {
        &self.splits
    }

    /// Пара, в которую входит вкладка.
    pub fn split_of(&self, id: u64) -> Option<Split> {
        self.splits.iter().copied().find(|s| s.partner(id).is_some())
    }

    /// Поставить `right` рядом с `left`. Если какая-то из них уже была в паре,
    /// старая пара распадается. false — вкладок нет или это одна и та же вкладка.
    pub fn make_split(&mut self, left: u64, right: u64) -> bool {
        if left == right || self.get(left).is_none() || self.get(right).is_none() {
            return false;
        }
        self.splits.retain(|s| s.partner(left).is_none() && s.partner(right).is_none());
        self.splits.push(Split { left, right, ratio: 0.5 });
        self.keep_splits_together();
        true
    }

    /// Разъединить пару, в которую входит вкладка. false — она не в паре.
    pub fn unsplit(&mut self, id: u64) -> bool {
        let before = self.splits.len();
        self.splits.retain(|s| s.partner(id).is_none());
        self.splits.len() != before
    }

    pub fn set_split_ratio(&mut self, id: u64, ratio: f64) {
        if let Some(s) = self.splits.iter_mut().find(|s| s.partner(id).is_some()) {
            s.ratio = clamp_ratio(ratio);
        }
    }

    /// Правая вкладка пары — сразу за левой. Закрепление у половин своё
    /// (пара показывается в разделе левой); общим его делают только явные
    /// действия — закрепить пару или перетащить её в другой раздел.
    fn keep_splits_together(&mut self) {
        for s in self.splits.clone() {
            let Some(ri) = self.tabs.iter().position(|t| t.id == s.right) else { continue };
            let right = self.tabs.remove(ri);
            let li = self.tabs.iter().position(|t| t.id == s.left).map_or(ri, |li| li + 1);
            self.tabs.insert(li, right);
        }
    }

    pub fn closed(&self) -> &[SavedTab] {
        &self.closed
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn get(&self, id: u64) -> Option<&Tab<V>> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Tab<V>> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tab<V>> {
        self.tabs.iter()
    }

    pub fn active(&self) -> Option<u64> {
        self.active
    }

    pub fn active_tab(&self) -> Option<&Tab<V>> {
        self.active.and_then(|id| self.get(id))
    }

    /// Порядок в сайдбаре: сначала закреплённые, потом остальные.
    pub fn order(&self) -> Vec<u64> {
        let pinned = self.tabs.iter().filter(|t| t.pinned).map(|t| t.id);
        let rest = self.tabs.iter().filter(|t| !t.pinned).map(|t| t.id);
        pinned.chain(rest).collect()
    }

    /// Вкладки в порядке сайдбара.
    pub fn ordered(&self) -> Vec<&Tab<V>> {
        self.order().into_iter().filter_map(|id| self.get(id)).collect()
    }

    /// Добавить новую вкладку (id взят из `alloc_id`). Она встаёт сразу после
    /// активной незакреплённой (как в Arc), иначе в конец.
    pub fn insert(&mut self, id: u64, url: String, now: u64, view: Option<V>) {
        let tab = Tab {
            id,
            url,
            title: String::new(),
            favicon: String::new(),
            pinned: false,
            pinned_url: None,
            loading: true,
            media: false,
            last_active: now,
            view,
        };
        let pos = self
            .active
            .and_then(|a| self.tabs.iter().position(|t| t.id == a && !t.pinned))
            .map(|i| i + 1)
            .unwrap_or(self.tabs.len());
        self.tabs.insert(pos, tab);
        // Если активная — левая половина пары, новая вкладка встанет после пары.
        self.keep_splits_together();
    }

    /// Сделать вкладку активной. Вторая половина пары тоже на экране, поэтому
    /// время открытия обновляется у обеих. false — такой вкладки нет.
    /// Ни одной вкладки на экране — показывается домашняя страница.
    pub fn deactivate(&mut self) {
        self.active = None;
    }

    pub fn activate(&mut self, id: u64, now: u64) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.active = Some(id);
        for v in self.visible() {
            if let Some(t) = self.get_mut(v) {
                t.last_active = now;
            }
        }
        true
    }

    /// Выгрузить фоновые вкладки, которые не открывали дольше `limit` секунд
    /// (0 — никогда). Вкладки со звуком и вторая половина активной пары не трогаются.
    /// true — хоть одна уснула.
    pub fn sleep_idle(&mut self, now: u64, limit: u64) -> bool {
        let visible = self.visible();
        self.sleep_idle_except(now, limit, &visible)
    }

    /// То же для пространства, которого нет на экране: усыпить можно любую вкладку.
    pub fn sleep_idle_offscreen(&mut self, now: u64, limit: u64) -> bool {
        self.sleep_idle_except(now, limit, &[])
    }

    fn sleep_idle_except(&mut self, now: u64, limit: u64, visible: &[u64]) -> bool {
        if limit == 0 {
            return false;
        }
        let mut any = false;
        for t in &mut self.tabs {
            if !visible.contains(&t.id) && !t.media && t.view.is_some()
                && now.saturating_sub(t.last_active) > limit
            {
                t.view = None;
                t.loading = false;
                any = true;
            }
        }
        any
    }

    /// Вкладки на экране: активная и, если она в паре, вторая половина.
    pub fn visible(&self) -> Vec<u64> {
        let Some(a) = self.active else { return Vec::new() };
        match self.split_of(a) {
            Some(s) => vec![s.left, s.right],
            None => vec![a],
        }
    }

    /// Закрыть вкладку. Закреплённая не удаляется: выгружается и возвращается
    /// к своему адресу. Пара, в которую входила вкладка, распадается, и активной
    /// становится вторая половина. None — такой вкладки нет.
    pub fn close(&mut self, id: u64) -> Option<AfterClose> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let was_active = self.active == Some(id);
        let partner = self.split_of(id).and_then(|s| s.partner(id));
        let next = if was_active { partner.or_else(|| self.neighbor(id)) } else { self.active };
        self.unsplit(id);

        if self.tabs[idx].pinned {
            let t = &mut self.tabs[idx];
            t.view = None;
            t.loading = false;
            t.media = false;
            if let Some(u) = t.pinned_url.clone() {
                t.url = u;
            }
        } else {
            let t = self.tabs.remove(idx);
            if t.url.starts_with("http") {
                self.closed.push(t.saved());
                self.trim_closed();
            }
        }
        if was_active {
            self.active = None;
        }
        Some(AfterClose { was_active, next })
    }

    /// Закрыть все незакреплённые вкладки.
    pub fn clear_unpinned(&mut self) -> AfterClose {
        let was_active = self.active_tab().is_some_and(|t| !t.pinned);
        let (keep, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut self.tabs).into_iter().partition(|t| t.pinned);
        self.tabs = keep;
        self.splits.retain(|s| !gone.iter().any(|t| s.partner(t.id).is_some()));
        self.closed.extend(gone.iter().filter(|t| t.url.starts_with("http")).map(Tab::saved));
        self.trim_closed();
        if was_active {
            self.active = None;
        }
        let next = if was_active { self.order().first().copied() } else { self.active };
        AfterClose { was_active, next }
    }

    /// Достать последнюю закрытую вкладку.
    pub fn pop_closed(&mut self) -> Option<SavedTab> {
        self.closed.pop()
    }

    /// Соседняя вкладка, на которую переходим после закрытия: следующая в той же
    /// группе (закреплённые / обычные), иначе предыдущая, иначе любая другая.
    fn neighbor(&self, id: u64) -> Option<u64> {
        let order = self.order();
        let i = order.iter().position(|x| *x == id)?;
        let is_pinned = self.get(id).is_some_and(|t| t.pinned);
        let same_group = |x: &u64| self.get(*x).is_some_and(|t| t.pinned == is_pinned);
        order[i + 1..].iter().find(|x| same_group(x))
            .or_else(|| order[..i].iter().rev().find(|x| same_group(x)))
            .or_else(|| order.iter().find(|x| **x != id))
            .copied()
    }

    /// Вкладка на `delta` позиций от активной по кругу (Ctrl+Tab).
    pub fn cycle_target(&self, delta: isize) -> Option<u64> {
        let order = self.order();
        if order.is_empty() {
            return None;
        }
        let cur = self.active.and_then(|a| order.iter().position(|x| *x == a)).unwrap_or(0) as isize;
        let n = order.len() as isize;
        Some(order[(cur + delta).rem_euclid(n) as usize])
    }

    /// Вкладка по номеру (Ctrl+1…8), 9 — последняя.
    pub fn goto_target(&self, n: usize) -> Option<u64> {
        let order = self.order();
        if n == 9 { order.last().copied() } else { order.get(n.saturating_sub(1)).copied() }
    }

    /// Закрепить или открепить вкладку (пару — обе половины сразу).
    pub fn set_pinned(&mut self, id: u64, pinned: bool) {
        let partner = self.split_of(id).and_then(|s| s.partner(id));
        for x in std::iter::once(id).chain(partner) {
            if let Some(t) = self.get_mut(x) {
                t.set_pinned(pinned);
            }
        }
    }

    /// Перетаскивание в сайдбаре: поставить вкладку перед `before` (None — в конец)
    /// в разделе закреплённых или обычных. Пара переносится целиком — за левую половину.
    pub fn move_tab(&mut self, id: u64, before: Option<u64>, pinned: bool) {
        let id = self.split_of(id).map_or(id, |s| s.left);
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else { return };
        let mut t = self.tabs.remove(idx);
        if t.pinned != pinned {
            t.set_pinned(pinned);
        }
        // Пару перетащили в другой раздел — вторая половина переезжает туда же.
        if let Some(p) = self.split_of(id).and_then(|s| s.partner(id))
            && let Some(other) = self.get_mut(p)
            && other.pinned != pinned
        {
            other.set_pinned(pinned);
        }
        // Нельзя встать перед правой половиной пары — это разорвало бы её.
        let before = before.map(|b| self.split_of(b).filter(|s| s.right == b).map_or(b, |s| s.left));
        let pos = before
            .and_then(|b| self.tabs.iter().position(|x| x.id == b))
            .unwrap_or(self.tabs.len());
        self.tabs.insert(pos, t);
        self.keep_splits_together();
    }

    fn trim_closed(&mut self) {
        if self.closed.len() > CLOSED_LIMIT {
            let extra = self.closed.len() - CLOSED_LIMIT;
            self.closed.drain(..extra);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;
    const HOUR: u64 = 3600;

    fn st(url: &str, pinned: bool, last_active: u64) -> SavedTab {
        SavedTab {
            url: url.into(),
            pinned,
            pinned_url: pinned.then(|| url.to_string()),
            last_active,
            ..Default::default()
        }
    }

    /// Список из сохранённых вкладок (без архивации); у всех вкладок есть «страница».
    fn list(tabs: Vec<SavedTab>) -> (TabList<()>, Vec<u64>) {
        let mut l = TabList::new(vec![]);
        l.restore(tabs, None, &[], NOW, 0);
        for t in &mut l.tabs {
            t.view = Some(());
        }
        let ids = l.tabs.iter().map(|t| t.id).collect();
        (l, ids)
    }

    fn urls(l: &TabList<()>) -> Vec<&str> {
        l.ordered().iter().map(|t| t.url.as_str()).collect()
    }

    fn closed_urls(l: &TabList<()>) -> Vec<&str> {
        l.closed().iter().map(|t| t.url.as_str()).collect()
    }

    // ---------- Восстановление и сохранение ----------

    #[test]
    fn restore_keeps_order_and_active() {
        let mut l: TabList<()> = TabList::new(vec![]);
        let target = l.restore(vec![st("https://a/", true, NOW), st("https://b/", false, NOW)], Some(1), &[], NOW, 0);
        assert_eq!(urls(&l), ["https://a/", "https://b/"]);
        assert_eq!(target, l.ordered().get(1).map(|t| t.id));
        assert!(l.iter().all(|t| t.view.is_none() && !t.loading));
    }

    #[test]
    fn restore_without_active_targets_first_tab() {
        let mut l: TabList<()> = TabList::new(vec![]);
        let target = l.restore(vec![st("https://a/", false, NOW)], None, &[], NOW, 0);
        assert_eq!(target, l.ordered().first().map(|t| t.id));

        let mut empty: TabList<()> = TabList::new(vec![]);
        assert_eq!(empty.restore(vec![], None, &[], NOW, 0), None);
    }

    #[test]
    fn restore_archives_stale_unpinned_tabs() {
        let mut l: TabList<()> = TabList::new(vec![st("https://old-closed/", false, 1)]);
        l.restore(
            vec![
                st("https://pinned-old/", true, NOW - 100 * HOUR),
                st("https://stale/", false, NOW - 13 * HOUR),
                st("https://fresh/", false, NOW - 11 * HOUR),
                st("https://unknown-time/", false, 0),
            ],
            None,
            &[],
            NOW,
            12 * HOUR,
        );
        assert_eq!(urls(&l), ["https://pinned-old/", "https://fresh/", "https://unknown-time/"]);
        // Архивированные встают после уже закрытых.
        assert_eq!(closed_urls(&l), ["https://old-closed/", "https://stale/"]);
        // Вкладка без времени считается открытой сейчас.
        assert_eq!(l.ordered()[2].last_active, NOW);
    }

    #[test]
    fn restore_with_archive_disabled_keeps_everything() {
        let mut l: TabList<()> = TabList::new(vec![]);
        l.restore(vec![st("https://ancient/", false, 1)], None, &[], NOW, 0);
        assert_eq!(urls(&l), ["https://ancient/"]);
    }

    #[test]
    fn restore_falls_back_when_active_tab_was_archived() {
        let mut l: TabList<()> = TabList::new(vec![]);
        let target = l.restore(vec![st("https://keep/", false, NOW), st("https://stale/", false, 1)], Some(1), &[], NOW, HOUR);
        assert_eq!(target, l.ordered().first().map(|t| t.id));
        assert_eq!(urls(&l), ["https://keep/"]);
    }

    #[test]
    fn to_saved_uses_sidebar_order_and_active_index() {
        let (mut l, ids) = list(vec![st("https://u1/", false, NOW), st("https://p/", true, NOW), st("https://u2/", false, NOW)]);
        l.activate(ids[2], NOW);
        let (saved, active, _) = l.to_saved();
        let saved_urls: Vec<_> = saved.iter().map(|t| t.url.as_str()).collect();
        assert_eq!(saved_urls, ["https://p/", "https://u1/", "https://u2/"]);
        assert_eq!(active, Some(2));
        assert_eq!(saved[0].pinned_url.as_deref(), Some("https://p/"));
    }

    #[test]
    fn new_trims_closed_to_limit() {
        let closed = (0..CLOSED_LIMIT + 5).map(|i| st(&format!("https://c/{i}"), false, 1)).collect();
        let l: TabList<()> = TabList::new(closed);
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
        l.insert(id, "https://new/".into(), NOW, None);
        assert_eq!(urls(&l), ["https://a/", "https://new/", "https://b/"]);
        let t = l.get(id).unwrap();
        assert!(t.loading && !t.pinned && t.title.is_empty());
    }

    #[test]
    fn insert_goes_to_end_when_active_is_pinned_or_none() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
        let id = l.alloc_id();
        l.insert(id, "https://x/".into(), NOW, None);
        l.activate(ids[0], NOW);
        let id = l.alloc_id();
        l.insert(id, "https://y/".into(), NOW, None);
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
        let mut a: TabList<()> = TabList::with_id_base(vec![], 1_000);
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
        assert_eq!(l.close(ids[2]), Some(AfterClose { was_active: true, next: Some(ids[3]) }));
        assert_eq!(l.active(), None);
        assert_eq!(urls(&l), ["https://p/", "https://a/", "https://c/"]);
        assert_eq!(closed_urls(&l), ["https://b/"]);
    }

    #[test]
    fn close_last_in_group_moves_to_previous_in_group() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("https://b/", false, NOW)]);
        l.activate(ids[2], NOW);
        assert_eq!(l.close(ids[2]).unwrap().next, Some(ids[1]));
    }

    #[test]
    fn close_only_unpinned_moves_to_pinned() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
        l.activate(ids[1], NOW);
        assert_eq!(l.close(ids[1]).unwrap().next, Some(ids[0]));
    }

    #[test]
    fn close_the_only_tab_leaves_nothing_active() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
        l.activate(ids[0], NOW);
        assert_eq!(l.close(ids[0]), Some(AfterClose { was_active: true, next: None }));
        assert!(l.ordered().is_empty());
    }

    #[test]
    fn close_background_tab_keeps_active() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
        l.activate(ids[0], NOW);
        assert_eq!(l.close(ids[1]), Some(AfterClose { was_active: false, next: Some(ids[0]) }));
        assert_eq!(l.active(), Some(ids[0]));
    }

    #[test]
    fn close_unknown_tab_does_nothing() {
        let (mut l, _) = list(vec![st("https://a/", false, NOW)]);
        assert_eq!(l.close(999), None);
        assert_eq!(urls(&l), ["https://a/"]);
    }

    #[test]
    fn close_pinned_tab_unloads_and_resets_url() {
        let (mut l, ids) = list(vec![st("https://home/", true, NOW), st("https://a/", false, NOW)]);
        l.activate(ids[0], NOW);
        let t = l.get_mut(ids[0]).unwrap();
        t.url = "https://home/elsewhere".into();
        t.media = true;
        assert!(t.drifted());

        assert_eq!(l.close(ids[0]).unwrap().next, Some(ids[1]));
        let t = l.get(ids[0]).unwrap();
        assert_eq!(t.url, "https://home/");
        assert!(t.view.is_none() && !t.media && !t.drifted());
        assert!(l.closed().is_empty());
    }

    #[test]
    fn close_non_http_tab_is_not_remembered() {
        let (mut l, ids) = list(vec![st("about:blank", false, NOW)]);
        l.close(ids[0]);
        assert!(l.closed().is_empty());
    }

    #[test]
    fn closed_list_is_capped() {
        let tabs = (0..CLOSED_LIMIT + 3).map(|i| st(&format!("https://t/{i}"), false, NOW)).collect();
        let (mut l, ids) = list(tabs);
        for id in ids {
            l.close(id);
        }
        assert_eq!(l.closed().len(), CLOSED_LIMIT);
        assert_eq!(l.closed()[0].url, "https://t/3");
    }

    #[test]
    fn pop_closed_returns_last_closed_first() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
        l.close(ids[0]);
        l.close(ids[1]);
        assert_eq!(l.pop_closed().unwrap().url, "https://b/");
        assert_eq!(l.pop_closed().unwrap().url, "https://a/");
        assert!(l.pop_closed().is_none());
    }

    #[test]
    fn clear_unpinned_with_active_unpinned_moves_to_first_pinned() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("about:blank", false, NOW)]);
        l.activate(ids[1], NOW);
        assert_eq!(l.clear_unpinned(), AfterClose { was_active: true, next: Some(ids[0]) });
        assert_eq!(urls(&l), ["https://p/"]);
        assert_eq!(closed_urls(&l), ["https://a/"]);
    }

    #[test]
    fn clear_unpinned_with_active_pinned_keeps_it() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
        l.activate(ids[0], NOW);
        assert_eq!(l.clear_unpinned(), AfterClose { was_active: false, next: Some(ids[0]) });
        assert_eq!(l.active(), Some(ids[0]));
    }

    #[test]
    fn clear_unpinned_without_pinned_leaves_nothing() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW)]);
        l.activate(ids[0], NOW);
        assert_eq!(l.clear_unpinned(), AfterClose { was_active: true, next: None });
    }

    // ---------- Переключение ----------

    #[test]
    fn cycle_wraps_in_both_directions() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)]);
        l.activate(ids[2], NOW);
        assert_eq!(l.cycle_target(1), Some(ids[0]));
        assert_eq!(l.cycle_target(-1), Some(ids[1]));
        l.activate(ids[0], NOW);
        assert_eq!(l.cycle_target(-1), Some(ids[2]));
        assert_eq!(l.cycle_target(4), Some(ids[1]));
    }

    #[test]
    fn cycle_follows_sidebar_order() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://p/", true, NOW)]);
        l.activate(ids[1], NOW);
        assert_eq!(l.cycle_target(1), Some(ids[0]));
    }

    #[test]
    fn cycle_without_active_starts_from_first() {
        let (l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW)]);
        assert_eq!(l.cycle_target(1), Some(ids[1]));
        let empty: TabList<()> = TabList::new(vec![]);
        assert_eq!(empty.cycle_target(1), None);
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

    // ---------- Закрепление и перетаскивание ----------

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

    #[test]
    fn move_tab_before_another_and_to_end() {
        let (mut l, ids) = list(vec![st("https://a/", false, NOW), st("https://b/", false, NOW), st("https://c/", false, NOW)]);
        l.move_tab(ids[2], Some(ids[0]), false);
        assert_eq!(urls(&l), ["https://c/", "https://a/", "https://b/"]);
        l.move_tab(ids[2], None, false);
        assert_eq!(urls(&l), ["https://a/", "https://b/", "https://c/"]);
        l.move_tab(999, None, false);
        assert_eq!(urls(&l).len(), 3);
    }

    #[test]
    fn move_tab_between_sections_changes_pin() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW)]);
        l.move_tab(ids[1], Some(ids[0]), true);
        assert_eq!(urls(&l), ["https://a/", "https://p/"]);
        assert_eq!(l.get(ids[1]).unwrap().pinned_url.as_deref(), Some("https://a/"));

        l.move_tab(ids[0], None, false);
        let t = l.get(ids[0]).unwrap();
        assert!(!t.pinned && t.pinned_url.is_none());
        assert_eq!(urls(&l), ["https://a/", "https://p/"]);
    }

    #[test]
    fn move_tab_within_section_keeps_pinned_url() {
        let (mut l, ids) = list(vec![st("https://p1/", true, NOW), st("https://p2/", true, NOW)]);
        l.get_mut(ids[0]).unwrap().url = "https://p1/away".into();
        l.move_tab(ids[0], None, true);
        assert_eq!(l.get(ids[0]).unwrap().pinned_url.as_deref(), Some("https://p1/"));
    }

    // ---------- Split View ----------

    fn abc() -> (TabList<()>, Vec<u64>) {
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
    fn halves_keep_their_own_pinning() {
        // Открыть закреплённую вкладку рядом с обычной — она остаётся закреплённой.
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
        assert_eq!(l.close(ids[1]), Some(AfterClose { was_active: true, next: Some(ids[2]) }));
        assert!(l.splits().is_empty());
    }

    #[test]
    fn closing_background_half_breaks_pair() {
        let (mut l, ids) = abc();
        l.make_split(ids[1], ids[2]);
        l.activate(ids[0], NOW);
        assert_eq!(l.close(ids[2]), Some(AfterClose { was_active: false, next: Some(ids[0]) }));
        assert!(l.splits().is_empty());
    }

    #[test]
    fn clear_unpinned_drops_pairs_of_removed_tabs() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("https://b/", false, NOW)]);
        l.make_split(ids[1], ids[2]);
        l.clear_unpinned();
        assert!(l.splits().is_empty());
    }

    #[test]
    fn insert_after_left_half_goes_after_pair() {
        let (mut l, ids) = abc();
        l.make_split(ids[0], ids[1]);
        l.activate(ids[0], NOW);
        let id = l.alloc_id();
        l.insert(id, "https://new/".into(), NOW, None);
        assert_eq!(urls(&l), ["https://a/", "https://b/", "https://new/", "https://c/"]);
    }

    #[test]
    fn moving_either_half_moves_the_pair() {
        let (mut l, ids) = abc();
        l.make_split(ids[0], ids[1]);
        l.move_tab(ids[1], None, false);
        assert_eq!(urls(&l), ["https://c/", "https://a/", "https://b/"]);
        l.move_tab(ids[0], Some(ids[2]), false);
        assert_eq!(urls(&l), ["https://a/", "https://b/", "https://c/"]);
    }

    #[test]
    fn dropping_before_right_half_goes_before_pair() {
        let (mut l, ids) = abc();
        l.make_split(ids[0], ids[1]);
        l.move_tab(ids[2], Some(ids[1]), false);
        assert_eq!(urls(&l), ["https://c/", "https://a/", "https://b/"]);
    }

    #[test]
    fn moving_pair_to_pinned_pins_both() {
        let (mut l, ids) = list(vec![st("https://p/", true, NOW), st("https://a/", false, NOW), st("https://b/", false, NOW)]);
        l.make_split(ids[1], ids[2]);
        l.move_tab(ids[1], Some(ids[0]), true);
        assert_eq!(urls(&l), ["https://a/", "https://b/", "https://p/"]);
        assert!(l.get(ids[2]).unwrap().pinned);
    }

    #[test]
    fn splits_survive_save_and_restore() {
        let (mut l, ids) = abc();
        l.make_split(ids[2], ids[0]);
        l.set_split_ratio(ids[0], 0.3);
        let (tabs, active, splits) = l.to_saved();
        assert_eq!(splits, [SavedSplit { left: 1, right: 2, ratio: 0.3 }]);

        let mut r: TabList<()> = TabList::new(vec![]);
        r.restore(tabs, active, &splits, NOW, 0);
        assert_eq!(urls(&r), ["https://b/", "https://c/", "https://a/"]);
        let s = r.splits()[0];
        assert_eq!((r.get(s.left).unwrap().url.as_str(), r.get(s.right).unwrap().url.as_str(), s.ratio), ("https://c/", "https://a/", 0.3));
    }

    #[test]
    fn restore_drops_pair_with_archived_half_or_bad_index() {
        let mut l: TabList<()> = TabList::new(vec![]);
        let splits = [SavedSplit { left: 0, right: 1, ratio: 0.5 }, SavedSplit { left: 0, right: 9, ratio: 0.5 }];
        l.restore(vec![st("https://keep/", false, NOW), st("https://stale/", false, 1)], None, &splits, NOW, HOUR);
        assert!(l.splits().is_empty());
    }
}
