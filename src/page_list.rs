//! Модель страниц пространства: порядок, закрепление, Now / Later, закрытие,
//! сон и архив, Split View, цепочки переходов (parent) и последние открытые.
//!
//! Здесь нет окна и WebView: `V` — то, что держит загруженную страницу
//! (в браузере — страница движка `engine::Page`, в тестах `()`). Создаёт и показывает их `App`.
//!
//! Слово «вкладка» в коде осталось с прежнего интерфейса; в интерфейсе это «страница».

use crate::store::{SavedSplit, SavedTab};

const CLOSED_LIMIT: usize = 50;
/// Пределы доли ширины левой половины в Split View.
const SPLIT_MIN: f64 = 0.2;
const SPLIT_MAX: f64 = 0.8;

/// Две вкладки рядом (Split View). В списке правая всегда стоит сразу за левой.
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

/// Ctrl+Tab как Alt+Tab в Windows: нажатия подряд (не дольше `window` между ними) идут
/// дальше по списку последних страниц, замороженному при первом нажатии.
#[derive(Debug, Default)]
pub struct MruWalk {
    list: Vec<u64>,
    pos: usize,
    last: Option<std::time::Instant>,
}

impl MruWalk {
    /// Следующая страница. `fresh` — список последних (текущая первой), если обход
    /// начинается заново; `alive` — есть ли ещё страница (закрытые пропускаются).
    pub fn step(
        &mut self,
        now: std::time::Instant,
        window: std::time::Duration,
        forward: bool,
        fresh: impl FnOnce() -> Vec<u64>,
        alive: impl Fn(u64) -> bool,
    ) -> Option<u64> {
        if self.last.is_none_or(|t| now.duration_since(t) >= window) {
            self.list = fresh();
            self.pos = 0;
        }
        self.last = Some(now);
        let n = self.list.len();
        if n < 2 {
            return None;
        }
        // Как Alt+Tab: по кругу, включая страницу, с которой начали (Ctrl+Tab, затем
        // Ctrl+Shift+Tab возвращает на неё). Сразу на неё же шаг не ведёт: с позиции 0
        // первый шаг всегда уходит на соседнюю.
        for _ in 0..n {
            self.pos = if forward { (self.pos + 1) % n } else { (self.pos + n - 1) % n };
            let id = self.list[self.pos];
            if alive(id) {
                return Some(id);
            }
        }
        None
    }
}

/// Состояние страницы в Orbit. Закрытые и архивированные (Archive) живут
/// отдельно — в `PageList::closed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
    /// Страницы, с которыми работают сейчас.
    Now,
    /// Отложенные: выгружены из памяти, не участвуют в Ctrl+Tab и не архивируются.
    Later,
}

impl PageState {
    pub fn as_str(self) -> &'static str {
        match self {
            PageState::Now => "now",
            PageState::Later => "later",
        }
    }
}

pub struct Page<V> {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub favicon: String,
    pub pinned: bool,
    pub pinned_url: Option<String>,
    pub loading: bool,
    pub media: bool,
    pub last_active: u64,
    pub created_at: u64,
    pub state: PageState,
    /// Страница, с которой открыли эту (ссылка, window.open) — основа Context.
    pub parent: Option<u64>,
    /// None — вкладка спит (не загружена в память).
    pub view: Option<V>,
}

impl<V> Page<V> {
    /// Выгрузить из памяти: WebView закрывается, страница остаётся в списке.
    fn unload(&mut self) {
        self.view = None;
        self.loading = false;
        self.media = false;
    }

    /// Для сессии и списка закрытых. `parent` — индекс в сохраняемом списке,
    /// его проставляет `PageList::to_saved`.
    pub fn saved(&self) -> SavedTab {
        SavedTab {
            url: self.url.clone(),
            title: self.title.clone(),
            pinned: self.pinned,
            pinned_url: self.pinned_url.clone(),
            favicon: self.favicon.clone(),
            last_active: self.last_active,
            later: self.state == PageState::Later,
            created_at: self.created_at,
            parent: None,
            closed_at: 0,
        }
    }

    fn new(id: u64, url: String, now: u64, parent: Option<u64>, view: Option<V>) -> Self {
        Page {
            id,
            url,
            title: String::new(),
            favicon: String::new(),
            pinned: false,
            pinned_url: None,
            loading: true,
            media: false,
            last_active: now,
            created_at: now,
            state: PageState::Now,
            parent,
            view,
        }
    }

    fn from_saved(id: u64, st: SavedTab, now: u64) -> Self {
        Page {
            id,
            url: st.url,
            title: st.title,
            favicon: st.favicon,
            pinned: st.pinned,
            pinned_url: st.pinned_url,
            loading: false,
            media: false,
            last_active: if st.last_active == 0 { now } else { st.last_active },
            created_at: if st.created_at == 0 { st.last_active.min(now) } else { st.created_at },
            state: if st.later && !st.pinned { PageState::Later } else { PageState::Now },
            parent: None,
            view: None,
        }
    }

    /// Закреплённая вкладка ушла со своего адреса.
    pub fn drifted(&self) -> bool {
        self.pinned && self.pinned_url.as_deref().is_some_and(|u| u != self.url)
    }

    pub fn is_now(&self) -> bool {
        self.state == PageState::Now
    }

    fn set_pinned(&mut self, pinned: bool) {
        self.pinned = pinned;
        self.pinned_url = if pinned { Some(self.url.clone()) } else { None };
        if pinned {
            self.state = PageState::Now;
        }
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

pub struct PageList<V> {
    /// Порядок хранения; закреплённые показываются первыми (см. `order`).
    pages: Vec<Page<V>>,
    active: Option<u64>,
    next_id: u64,
    /// Недавно закрытые и архивированные вкладки, последняя — в конце.
    closed: Vec<SavedTab>,
    splits: Vec<Split>,
}

impl<V> PageList<V> {
    pub fn new(closed: Vec<SavedTab>) -> Self {
        Self::with_id_base(closed, 0)
    }

    /// Список, у которого номера вкладок начинаются после `base` (у каждого
    /// пространства свой диапазон, см. spaces::page_id_base).
    pub fn with_id_base(closed: Vec<SavedTab>, base: u64) -> Self {
        let mut list = Self { pages: Vec::new(), active: None, next_id: base + 1, closed, splits: Vec::new() };
        list.trim_closed();
        list
    }

    /// Восстановить вкладки из сессии. Незакреплённые страницы Now, которые не открывали
    /// дольше `archive_after` секунд (0 — не архивировать), уходят в закрытые (Archive);
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
        let mut parents = Vec::with_capacity(saved.len());
        for (i, mut st) in saved.into_iter().enumerate() {
            let stale = archive_after > 0
                && !st.pinned
                && !st.later
                && st.last_active > 0
                && now.saturating_sub(st.last_active) > archive_after;
            if stale {
                st.closed_at = now;
                st.parent = None;
                self.closed.push(st);
                ids.push(None);
                parents.push(None);
                continue;
            }
            let id = self.alloc_id();
            ids.push(Some(id));
            parents.push(st.parent);
            if active == Some(i) {
                active_id = Some(id);
            }
            self.pages.push(Page::from_saved(id, st, now));
        }
        // Родители — по индексам в сохранённом списке; архивированный родитель теряется.
        for (i, p) in parents.into_iter().enumerate() {
            if let (Some(id), Some(parent)) = (ids[i], p.and_then(|p| ids.get(p).copied().flatten()))
                && parent != id
                && let Some(t) = self.get_mut(id)
            {
                t.parent = Some(parent);
            }
        }
        for s in splits {
            let id_at = |i: usize| ids.get(i).copied().flatten();
            if let (Some(left), Some(right)) = (id_at(s.left), id_at(s.right)) {
                self.make_split(left, right);
                self.set_split_ratio(left, s.ratio);
            }
        }
        self.trim_closed();
        // Отложенную страницу активной не делаем.
        active_id.filter(|id| self.get(*id).is_some_and(Page::is_now)).or_else(|| self.now_order().first().copied())
    }

    /// Вкладки для сессии в порядке `order`, индекс активной и пары Split View
    /// (индексы в том же списке).
    pub fn to_saved(&self) -> (Vec<SavedTab>, Option<usize>, Vec<SavedSplit>) {
        let order = self.order();
        let index = |id: u64| order.iter().position(|x| *x == id);
        let tabs = order
            .iter()
            .filter_map(|id| self.get(*id))
            .map(|t| SavedTab { parent: t.parent.and_then(index), ..t.saved() })
            .collect();
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
    /// старая пара распадается. Отложенная страница возвращается в Now.
    /// false — вкладок нет или это одна и та же вкладка.
    pub fn make_split(&mut self, left: u64, right: u64) -> bool {
        if left == right || self.get(left).is_none() || self.get(right).is_none() {
            return false;
        }
        self.splits.retain(|s| s.partner(left).is_none() && s.partner(right).is_none());
        self.splits.push(Split { left, right, ratio: 0.5 });
        for id in [left, right] {
            if let Some(t) = self.get_mut(id) {
                t.state = PageState::Now;
            }
        }
        self.keep_splits_together();
        true
    }

    /// Разъединить пару, в которую входит вкладка. false — она не в паре.
    pub fn unsplit(&mut self, id: u64) -> bool {
        let before = self.splits.len();
        self.splits.retain(|s| s.partner(id).is_none());
        self.splits.len() != before
    }

    /// Поменять половины пары местами. false — вкладка не в паре.
    pub fn swap_split(&mut self, id: u64) -> bool {
        let Some(s) = self.splits.iter_mut().find(|s| s.partner(id).is_some()) else { return false };
        std::mem::swap(&mut s.left, &mut s.right);
        s.ratio = clamp_ratio(1.0 - s.ratio);
        self.keep_splits_together();
        true
    }

    /// Заменить половину пары `old` другой страницей `new` (которая уходит из своей пары).
    pub fn replace_in_split(&mut self, old: u64, new: u64) -> bool {
        if old == new || self.get(new).is_none() {
            return false;
        }
        let Some(s) = self.split_of(old) else { return false };
        let other = s.partner(old).unwrap_or(old);
        if other == new {
            return false;
        }
        self.unsplit(new);
        let (l, r) = if s.left == old { (new, other) } else { (other, new) };
        if !self.make_split(l, r) {
            return false;
        }
        self.set_split_ratio(l, s.ratio);
        true
    }

    pub fn set_split_ratio(&mut self, id: u64, ratio: f64) {
        if let Some(s) = self.splits.iter_mut().find(|s| s.partner(id).is_some()) {
            s.ratio = clamp_ratio(ratio);
        }
    }

    /// Правая вкладка пары — сразу за левой. Закрепление у половин своё.
    fn keep_splits_together(&mut self) {
        for s in self.splits.clone() {
            let Some(ri) = self.pages.iter().position(|t| t.id == s.right) else { continue };
            let right = self.pages.remove(ri);
            let li = self.pages.iter().position(|t| t.id == s.left).map_or(ri, |li| li + 1);
            self.pages.insert(li, right);
        }
    }

    // ---------- Доступ ----------

    pub fn closed(&self) -> &[SavedTab] {
        &self.closed
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn get(&self, id: u64) -> Option<&Page<V>> {
        self.pages.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Page<V>> {
        self.pages.iter_mut().find(|t| t.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Page<V>> {
        self.pages.iter()
    }

    pub fn active(&self) -> Option<u64> {
        self.active
    }

    pub fn active_page(&self) -> Option<&Page<V>> {
        self.active.and_then(|id| self.get(id))
    }

    /// Все страницы: сначала закреплённые, потом остальные.
    pub fn order(&self) -> Vec<u64> {
        let pinned = self.pages.iter().filter(|t| t.pinned).map(|t| t.id);
        let rest = self.pages.iter().filter(|t| !t.pinned).map(|t| t.id);
        pinned.chain(rest).collect()
    }

    /// Страницы Now в том же порядке — по ним ходят Ctrl+1…9 и выбирается соседняя.
    pub fn now_order(&self) -> Vec<u64> {
        self.order().into_iter().filter(|id| self.get(*id).is_some_and(Page::is_now)).collect()
    }

    /// Вкладки в порядке `order`.
    pub fn ordered(&self) -> Vec<&Page<V>> {
        self.order().into_iter().filter_map(|id| self.get(id)).collect()
    }

    // ---------- Добавление и активация ----------

    /// Добавить новую вкладку (id взят из `alloc_id`). Она встаёт сразу после
    /// активной незакреплённой, иначе в конец. `parent` — страница, с которой её открыли.
    pub fn insert(&mut self, id: u64, url: String, now: u64, parent: Option<u64>, view: Option<V>) {
        let parent = parent.filter(|p| self.get(*p).is_some());
        let tab = Page::new(id, url, now, parent, view);
        let pos = self
            .active
            .and_then(|a| self.pages.iter().position(|t| t.id == a && !t.pinned))
            .map(|i| i + 1)
            .unwrap_or(self.pages.len());
        self.pages.insert(pos, tab);
        // Если активная — левая половина пары, новая вкладка встанет после пары.
        self.keep_splits_together();
    }

    /// Добавить страницу из сессии или из другого пространства (спящей, в конец).
    pub fn insert_saved(&mut self, st: SavedTab, now: u64) -> u64 {
        let id = self.alloc_id();
        self.pages.push(Page::from_saved(id, st, now));
        id
    }

    /// Ни одной вкладки на экране — показывается New Page.
    pub fn deactivate(&mut self) {
        self.active = None;
    }

    /// Сделать вкладку активной. Вторая половина пары тоже на экране, поэтому
    /// время открытия обновляется у обеих; отложенная страница возвращается в Now.
    /// false — такой вкладки нет.
    pub fn activate(&mut self, id: u64, now: u64) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.active = Some(id);
        for v in self.visible() {
            if let Some(t) = self.get_mut(v) {
                t.last_active = now;
                t.state = PageState::Now;
            }
        }
        true
    }

    // ---------- Now / Later ----------

    /// Отложить страницу (Move to Later): выгрузить, убрать из пары. Если она была
    /// на экране — `next` подскажет, куда перейти. None — такой страницы нет или
    /// она закреплена (закреплённые всегда в Now).
    pub fn move_to_later(&mut self, id: u64) -> Option<AfterClose> {
        if self.get(id).is_none_or(|t| t.pinned) {
            return None;
        }
        let after = self.detach(id);
        let t = self.get_mut(id)?;
        t.state = PageState::Later;
        t.unload();
        Some(after)
    }

    /// Вернуть отложенную страницу в Now, не открывая её.
    pub fn move_to_now(&mut self, id: u64) -> bool {
        match self.get_mut(id) {
            Some(t) if t.state == PageState::Later => {
                t.state = PageState::Now;
                true
            }
            _ => false,
        }
    }

    // ---------- Сон ----------

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
        for t in &mut self.pages {
            if !visible.contains(&t.id) && !t.media && t.view.is_some() && now.saturating_sub(t.last_active) > limit {
                t.unload();
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

    // ---------- Закрытие ----------

    /// Закрыть вкладку. Закреплённая не удаляется: выгружается и возвращается
    /// к своему адресу. Пара, в которую входила вкладка, распадается, и активной
    /// становится вторая половина. `now` — время закрытия для Recently closed.
    /// None — такой вкладки нет.
    pub fn close(&mut self, id: u64, now: u64) -> Option<AfterClose> {
        let idx = self.pages.iter().position(|t| t.id == id)?;
        let after = self.detach(id);
        if self.pages[idx].pinned {
            let t = &mut self.pages[idx];
            t.unload();
            if let Some(u) = t.pinned_url.clone() {
                t.url = u;
            }
        } else {
            let t = self.pages.remove(idx);
            self.forget_parent(id);
            if t.url.starts_with("http") {
                self.closed.push(SavedTab { closed_at: now, ..t.saved() });
                self.trim_closed();
            }
        }
        Some(after)
    }

    /// Убрать страницу из списка совсем (перенос в другое пространство):
    /// вернуть её для сессии. Закреплённая теряет закрепление.
    pub fn take(&mut self, id: u64) -> Option<(SavedTab, AfterClose)> {
        let idx = self.pages.iter().position(|t| t.id == id)?;
        let after = self.detach(id);
        let t = self.pages.remove(idx);
        self.forget_parent(id);
        let st = SavedTab { pinned: false, pinned_url: None, ..t.saved() };
        Some((st, after))
    }

    /// Убрать страницу с экрана перед закрытием, переносом или откладыванием:
    /// пара распадается, а если страница была активной — активной нет, и `next`
    /// подсказывает, куда перейти (вторая половина пары или соседняя страница).
    fn detach(&mut self, id: u64) -> AfterClose {
        let was_active = self.active == Some(id);
        let partner = self.split_of(id).and_then(|s| s.partner(id));
        let next = if was_active { partner.or_else(|| self.neighbor(id)) } else { self.active };
        self.unsplit(id);
        if was_active {
            self.active = None;
        }
        AfterClose { was_active, next }
    }

    /// Закрыть все незакреплённые страницы Now.
    pub fn clear_unpinned(&mut self, now: u64) -> AfterClose {
        let was_active = self.active_page().is_some_and(|t| !t.pinned);
        let (keep, gone): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.pages).into_iter().partition(|t| t.pinned || t.state == PageState::Later);
        self.pages = keep;
        self.splits.retain(|s| !gone.iter().any(|t| s.partner(t.id).is_some()));
        for t in &gone {
            self.forget_parent(t.id);
        }
        self.closed.extend(
            gone.iter().filter(|t| t.url.starts_with("http")).map(|t| SavedTab { closed_at: now, ..t.saved() }),
        );
        self.trim_closed();
        if was_active {
            self.active = None;
        }
        let next = if was_active { self.now_order().first().copied() } else { self.active };
        AfterClose { was_active, next }
    }

    /// Достать последнюю закрытую вкладку.
    pub fn pop_closed(&mut self) -> Option<SavedTab> {
        self.closed.pop()
    }

    /// Достать закрытую вкладку по индексу в `closed` (Recently closed, Archive).
    pub fn take_closed(&mut self, index: usize) -> Option<SavedTab> {
        (index < self.closed.len()).then(|| self.closed.remove(index))
    }

    /// Забыть все закрытые (кнопка в Archive).
    pub fn clear_closed(&mut self) {
        self.closed.clear();
    }

    /// У детей закрытой страницы родителя больше нет.
    fn forget_parent(&mut self, id: u64) {
        for t in &mut self.pages {
            if t.parent == Some(id) {
                t.parent = None;
            }
        }
    }

    /// Соседняя страница Now, на которую переходим после закрытия: следующая в той же
    /// группе (закреплённые / обычные), иначе предыдущая, иначе любая другая.
    fn neighbor(&self, id: u64) -> Option<u64> {
        let order = self.order();
        let i = order.iter().position(|x| *x == id)?;
        let is_pinned = self.get(id).is_some_and(|t| t.pinned);
        let ok = |x: &u64| *x != id && self.get(*x).is_some_and(Page::is_now);
        let same_group = |x: &u64| ok(x) && self.get(*x).is_some_and(|t| t.pinned == is_pinned);
        order[i + 1..]
            .iter()
            .find(|x| same_group(x))
            .or_else(|| order[..i].iter().rev().find(|x| same_group(x)))
            .or_else(|| order.iter().find(|x| ok(x)))
            .copied()
    }

    // ---------- Переключение ----------

    /// Страницы Now от недавно открытых к давно открытым (Ctrl+Tab).
    pub fn mru(&self) -> Vec<u64> {
        let mut v: Vec<&Page<V>> = self.pages.iter().filter(|t| t.is_now()).collect();
        let active = self.active;
        // Активная — первой, даже если время совпадает.
        v.sort_by(|a, b| (Some(b.id) == active).cmp(&(Some(a.id) == active)).then(b.last_active.cmp(&a.last_active)));
        v.into_iter().map(|t| t.id).collect()
    }

    /// Вкладка на `delta` позиций от активной по кругу в порядке `now_order`.
    pub fn cycle_target(&self, delta: isize) -> Option<u64> {
        let order = self.now_order();
        if order.is_empty() {
            return None;
        }
        let cur = self.active.and_then(|a| order.iter().position(|x| *x == a)).unwrap_or(0) as isize;
        let n = order.len() as isize;
        Some(order[(cur + delta).rem_euclid(n) as usize])
    }

    /// Вкладка по номеру (Ctrl+1…8), 9 — последняя.
    pub fn goto_target(&self, n: usize) -> Option<u64> {
        let order = self.now_order();
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

    fn trim_closed(&mut self) {
        if self.closed.len() > CLOSED_LIMIT {
            let extra = self.closed.len() - CLOSED_LIMIT;
            self.closed.drain(..extra);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/page_list.rs"]
mod tests;
