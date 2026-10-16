//! Ядро браузера: окно, вкладки, раскладка, обработка команд.

use crate::config::{Config, Paths};
use crate::hotkeys;
use crate::input;
use crate::limit::RateLimit;
use crate::spaces;
use crate::store::{DownloadState, Downloads, History, SavedSpace, Session, now};
use crate::tabs::{AfterClose, Tab, TabList};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::window::{ResizeDirection, Window, WindowBuilder};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

const UI_HTML: &str = include_str!("../ui/index.html");
const TAB_JS: &str = include_str!("tab.js");
/// Сессия и история пишутся на диск не сразу, а через эту паузу после изменения.
const SAVE_DELAY: Duration = Duration::from_secs(2);
/// Как часто проверять, не пора ли усыпить фоновые вкладки.
const SLEEP_CHECK: Duration = Duration::from_secs(60);
/// Сколько вкладок страницы могут открыть (window.open, Ctrl/средний клик) за окно времени.
const PAGE_TABS_MAX: usize = 10;
const PAGE_TABS_WINDOW: Duration = Duration::from_secs(5);
/// Высота панели поиска по странице над вкладкой (та же, что `--find-h` в ui/index.html).
const FIND_BAR_H: f64 = 40.0;
/// Промежуток между половинами Split View, в нём разделитель (`--split-gap` в ui/index.html).
const SPLIT_GAP: f64 = 8.0;

type Box4 = (f64, f64, f64, f64);

/// Разделить область страницы (x, y, ширина, высота) на две половины: левая
/// занимает долю `ratio` ширины без промежутка. Та же формула — в index.html.
fn split_boxes((x, y, w, h): Box4, ratio: f64) -> [Box4; 2] {
    let inner = (w - SPLIT_GAP).max(0.0);
    let lw = (inner * ratio).round();
    [(x, y, lw, h), (x + lw + SPLIT_GAP, y, inner - lw, h)]
}

/// События, которые приходят в главный поток из обработчиков WebView.
#[derive(Debug)]
pub enum UserEvent {
    FromUi(String),
    FromTab { id: u64, msg: String },
    Title { id: u64, title: String },
    Load { id: u64, started: bool, url: String },
    OpenTab { url: String, background: bool },
    /// window.open со страницы — в отличие от OpenTab, ограничивается по частоте.
    Popup { url: String },
    DownloadStarted { url: String, path: String },
    /// Загрузка закончилась; `id` — вкладка, которая её начала.
    Download { id: u64, url: String, path: Option<String>, ok: bool },
    /// Результат поиска по странице (JSON от скрипта во вкладке).
    FindResult { id: u64, json: String },
    /// Горячая клавиша во вкладке (см. hotkeys.rs).
    Hotkey { id: u64, cmd: String },
}

const FIND_JS: &str = include_str!("find.js");

/// Скрипт поиска по странице (см. find.js): `mode` — "type", "next", "prev" или "clear".
fn find_script(query: &str, mode: &str) -> String {
    format!("({FIND_JS})({}, {})", json!(query), json!(mode))
}

/// Разобрать ответ скрипта поиска: (число совпадений, номер текущего с нуля).
/// Страница может подменить что угодно, поэтому проверяем типы и диапазон.
fn parse_find_result(json: &str) -> Option<(u64, Option<u64>)> {
    let v: Value = serde_json::from_str(json).ok()?;
    let n = v["n"].as_u64()?;
    match v["i"].as_i64()? {
        -1 if n == 0 => Some((0, None)),
        i if i >= 0 && (i as u64) < n => Some((n, Some(i as u64))),
        _ => None,
    }
}

/// Пространство: свой профиль WebView2 (куки, логины, кэш) и свои вкладки.
struct Space {
    id: u32,
    name: String,
    color: String,
    /// Вкладки. У активного пространства здесь пустой список: его вкладки — в `App::tabs`.
    /// Стоит раньше `ctx`, чтобы при удалении пространства WebView закрылись до профиля.
    tabs: TabList<WebView>,
    ctx: WebContext,
}

pub struct App {
    window: Window,
    ui: WebView,
    proxy: EventLoopProxy<UserEvent>,
    cfg: Config,
    paths: Paths,
    /// Вкладки активного пространства.
    tabs: TabList<WebView>,
    spaces: Vec<Space>,
    active_space: usize,
    /// Удалённые пространства, чьи папки данных ещё не стёрты (см. Session::deleted_spaces).
    deleted_spaces: Vec<u32>,
    sidebar_visible: bool,
    palette_open: bool,
    find_open: bool,
    history: History,
    downloads: Downloads,
    /// Когда сбросить сессию и историю на диск (None — изменений нет).
    save_due: Option<Instant>,
    next_sleep_check: Instant,
    page_tabs: RateLimit,
    /// Пользователь закрыл окно кнопкой в интерфейсе — main.rs завершит цикл событий.
    exit_requested: bool,
    /// Окно развёрнуто — интерфейс прячет зоны изменения размера и меняет значок кнопки.
    maximized: bool,
}

/// Направление изменения размера окна из сообщения интерфейса (края рамки в index.html).
fn resize_direction(dir: &str) -> Option<ResizeDirection> {
    Some(match dir {
        "n" => ResizeDirection::North,
        "s" => ResizeDirection::South,
        "e" => ResizeDirection::East,
        "w" => ResizeDirection::West,
        "ne" => ResizeDirection::NorthEast,
        "nw" => ResizeDirection::NorthWest,
        "se" => ResizeDirection::SouthEast,
        "sw" => ResizeDirection::SouthWest,
        _ => return None,
    })
}

/// Общие настройки для всех WebView. Они обязаны совпадать, иначе WebView2
/// откажется использовать одну папку данных для разных вкладок.
fn common<'a>(b: WebViewBuilder<'a>, cfg: &Config, paths: &Paths) -> WebViewBuilder<'a> {
    let b = b
        .with_devtools(cfg.devtools)
        .with_hotkeys_zoom(true)
        .with_clipboard(true)
        .with_general_autofill_enabled(true)
        .with_background_color((24, 24, 32, 255));
    #[cfg(windows)]
    let b = {
        use wry::WebViewBuilderExtWindows;
        let b = b.with_browser_extensions_enabled(cfg.extensions);
        if cfg.extensions { b.with_extensions_path(paths.extensions.clone()) } else { b }
    };
    #[cfg(not(windows))]
    let _ = paths;
    b
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(1.0), h.max(1.0)).into(),
    }
}

/// Стереть папки пространств, удалённых в прошлый раз. Возвращает те, что стереть
/// не удалось (попробуем при следующем запуске). «Основное» не трогается никогда,
/// а стирается только папка spaces\<id> — путь строится из номера, не из сессии.
fn remove_deleted_spaces(paths: &Paths, ids: Vec<u32>) -> Vec<u32> {
    ids.into_iter()
        .filter(|&id| id != spaces::MAIN_ID)
        .filter(|&id| {
            let dir = spaces::space_dir(paths, id);
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => {
                    log::info!("данные удалённого пространства {id} стёрты");
                    false
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => {
                    log::warn!("не удалось стереть {}: {e}", dir.display());
                    true
                }
            }
        })
        .collect()
}

impl App {
    pub fn new(event_loop: &EventLoop<UserEvent>) -> wry::Result<Self> {
        let paths = Paths::new();
        let cfg = Config::load(&paths);
        let mut session = Session::load(&paths.session);
        let deleted_spaces = remove_deleted_spaces(&paths, std::mem::take(&mut session.deleted_spaces));
        let saved_spaces: Vec<SavedSpace> = spaces::normalize(session.take_spaces())
            .into_iter()
            .filter(|s| !deleted_spaces.contains(&s.id))
            .collect();
        let mut space_slots: Vec<Space> = saved_spaces
            .iter()
            .map(|s| Space {
                id: s.id,
                name: s.name.clone(),
                color: s.color.clone(),
                ctx: WebContext::new(Some(spaces::profile_dir(&paths, s.id))),
                tabs: TabList::with_id_base(s.closed.clone(), spaces::tab_id_base(s.id)),
            })
            .collect();
        let history = History::load(&paths.history);
        let downloads = Downloads::load(&paths.downloads);
        let proxy = event_loop.create_proxy();

        let builder = WindowBuilder::new()
            .with_title("Drift")
            .with_inner_size(tao::dpi::LogicalSize::new(1360.0, 860.0))
            .with_min_inner_size(tao::dpi::LogicalSize::new(640.0, 400.0))
            .with_theme(Some(tao::window::Theme::Dark))
            .with_decorations(!cfg.custom_titlebar);
        // Без рамки Windows всё равно рисует тень и скругляет углы, если попросить.
        #[cfg(windows)]
        let builder = {
            use tao::platform::windows::WindowBuilderExtWindows;
            builder.with_undecorated_shadow(true)
        };
        let window = builder.build(event_loop).expect("не удалось создать окно");

        // Интерфейс (сайдбар, палитра) — отдельный WebView во всё окно, под вкладками.
        // Он живёт в профиле «Основного» (normalize ставит его первым).
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        let ui_proxy = proxy.clone();
        let nav_proxy = proxy.clone();
        let ui = common(WebViewBuilder::new_with_web_context(&mut space_slots[0].ctx), &cfg, &paths)
            .with_html(UI_HTML)
            .with_bounds(rect(0.0, 0.0, size.width, size.height))
            .with_initialization_script(format!(
                "window.DRIFT_CONFIG = {};",
                json!({ "sidebarWidth": cfg.sidebar_width, "padding": cfg.frame_padding, "frameless": cfg.custom_titlebar })
            ))
            .with_ipc_handler(move |req| {
                let _ = ui_proxy.send_event(UserEvent::FromUi(req.body().clone()));
            })
            .with_navigation_handler(move |url| {
                // Интерфейс никуда не уходит: внешние ссылки открываем во вкладке.
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = nav_proxy.send_event(UserEvent::OpenTab { url, background: false });
                    false
                } else {
                    true
                }
            })
            .build_as_child(&window)?;

        let active_space = session.active_space.min(space_slots.len() - 1);
        let mut app = Self {
            window,
            ui,
            proxy,
            cfg,
            paths,
            tabs: TabList::new(vec![]),
            spaces: space_slots,
            active_space,
            deleted_spaces,
            sidebar_visible: session.sidebar_visible.unwrap_or(true),
            palette_open: false,
            find_open: false,
            history,
            downloads,
            save_due: None,
            next_sleep_check: Instant::now() + SLEEP_CHECK,
            page_tabs: RateLimit::new(PAGE_TABS_MAX, PAGE_TABS_WINDOW),
            exit_requested: false,
            maximized: false,
        };
        app.restore(saved_spaces);
        Ok(app)
    }

    // ---------- Сессия ----------

    fn restore(&mut self, saved: Vec<SavedSpace>) {
        let archive_after = self.cfg.archive_hours * 3600;
        let mut target = None;
        for (i, s) in saved.into_iter().enumerate() {
            let t = self.spaces[i].tabs.restore(s.tabs, s.active, &s.splits, now(), archive_after);
            if i == self.active_space {
                target = t;
            }
        }
        std::mem::swap(&mut self.tabs, &mut self.spaces[self.active_space].tabs);
        match target {
            Some(id) => self.activate(id),
            None => {
                self.layout();
                self.push_state();
            }
        }
    }

    /// Запланировать запись сессии и истории на диск.
    fn save(&mut self) {
        if self.save_due.is_none() {
            self.save_due = Some(Instant::now() + SAVE_DELAY);
        }
    }

    fn save_now(&mut self) {
        self.save_due = None;
        if self.history.is_dirty() {
            self.history.save(&self.paths.history);
        }
        let spaces = self
            .spaces
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let list = if i == self.active_space { &self.tabs } else { &s.tabs };
                let (tabs, active, splits) = list.to_saved();
                SavedSpace {
                    id: s.id,
                    name: s.name.clone(),
                    color: s.color.clone(),
                    tabs,
                    active,
                    closed: list.closed().to_vec(),
                    splits,
                }
            })
            .collect();
        Session {
            spaces,
            active_space: self.active_space,
            sidebar_visible: Some(self.sidebar_visible),
            deleted_spaces: self.deleted_spaces.clone(),
            ..Default::default()
        }
        .save(&self.paths.session);
    }

    pub fn shutdown(&mut self) {
        self.save_now();
    }

    /// Отложенные дела: запись на диск и усыпление вкладок. Вызывается из цикла событий.
    pub fn tick(&mut self) {
        let t = Instant::now();
        if self.save_due.is_some_and(|due| t >= due) {
            self.save_now();
        }
        if t >= self.next_sleep_check {
            self.next_sleep_check = t + SLEEP_CHECK;
            if self.sleep_idle_tabs() {
                self.push_state();
            }
        }
    }

    /// Когда циклу событий проснуться, чтобы вызвать `tick`.
    pub fn next_wakeup(&self) -> Instant {
        self.save_due.map_or(self.next_sleep_check, |due| due.min(self.next_sleep_check))
    }

    // ---------- Вспомогательное ----------

    fn window_size(&self) -> (f64, f64) {
        let s = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        (s.width, s.height)
    }

    /// Область страницы: (x, y, ширина, высота) в логических пикселях.
    fn content_box(&self) -> (f64, f64, f64, f64) {
        let (w, h) = self.window_size();
        let p = self.cfg.frame_padding;
        let left = if self.sidebar_visible { self.cfg.sidebar_width } else { p };
        // Панель поиска занимает полосу над страницей.
        let top = if self.find_open { p + FIND_BAR_H } else { p };
        (left, top, w - left - p, h - top - p)
    }

    fn content_rect(&self) -> Rect {
        let (x, y, w, h) = self.content_box();
        rect(x, y, w, h)
    }

    // ---------- Раскладка и отрисовка ----------

    /// Окно изменило размер (в том числе развернули или восстановили).
    pub fn on_resized(&mut self) {
        self.layout();
        let maximized = self.window.is_maximized();
        if maximized != self.maximized {
            self.maximized = maximized;
            self.push_state();
        }
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    pub fn layout(&mut self) {
        let (w, h) = self.window_size();
        let _ = self.ui.set_bounds(rect(0.0, 0.0, w, h));
        // На экране активная вкладка или обе половины её пары; под палитрой — ничего.
        let content = self.content_box();
        let mut shown: Vec<(u64, Rect)> = Vec::new();
        if !self.palette_open
            && let Some(active) = self.tabs.active()
        {
            match self.tabs.split_of(active) {
                Some(s) => {
                    let [l, r] = split_boxes(content, s.ratio);
                    shown.push((s.left, rect(l.0, l.1, l.2, l.3)));
                    shown.push((s.right, rect(r.0, r.1, r.2, r.3)));
                }
                None => shown.push((active, self.content_rect())),
            }
        }
        for t in self.tabs.iter() {
            if let Some(wv) = &t.view {
                match shown.iter().find(|(id, _)| *id == t.id) {
                    Some((_, r)) => {
                        let _ = wv.set_bounds(*r);
                        let _ = wv.set_visible(true);
                    }
                    None => {
                        let _ = wv.set_visible(false);
                    }
                }
            }
        }
    }

    fn push_state(&mut self) {
        let tabs: Vec<Value> = self
            .tabs
            .ordered()
            .into_iter()
            .map(|t| {
                json!({
                    "id": t.id,
                    "title": t.title,
                    "url": t.url,
                    "favicon": t.favicon,
                    "pinned": t.pinned,
                    "drifted": t.drifted(),
                    "loading": t.loading,
                    "media": t.media,
                    "asleep": t.view.is_none(),
                })
            })
            .collect();
        let state = json!({
            "tabs": tabs,
            "active": self.tabs.active(),
            "sidebar": self.sidebar_visible,
            "maximized": self.maximized,
            "palette": self.palette_open,
            "find": self.find_open,
            "closedCount": self.tabs.closed().len(),
            "splits": self.tabs.splits().iter()
                .map(|s| json!({ "left": s.left, "right": s.right, "ratio": s.ratio }))
                .collect::<Vec<_>>(),
            "splitGap": SPLIT_GAP,
            "spaces": self.spaces.iter().enumerate()
                .map(|(i, s)| {
                    let n = if i == self.active_space { self.tabs.iter().count() } else { s.tabs.iter().count() };
                    json!({ "id": s.id, "name": s.name, "color": s.color, "tabs": n, "main": s.id == spaces::MAIN_ID })
                })
                .collect::<Vec<_>>(),
            "activeSpace": self.active_space,
        });
        self.eval_ui(&format!("window.drift && drift.render({state})"));

        let title = self
            .tabs
            .active_tab()
            .map(|t| if t.title.is_empty() { t.url.clone() } else { t.title.clone() });
        self.window.set_title(&match title {
            Some(t) => format!("{t} — Drift"),
            None => "Drift".into(),
        });
    }

    fn eval_ui(&self, js: &str) {
        let _ = self.ui.evaluate_script(js);
    }

    fn toast(&self, text: &str) {
        self.eval_ui(&format!("window.drift && drift.toast({})", json!(text)));
    }

    // ---------- Вкладки ----------

    fn build_webview(&mut self, id: u64, url: &str, visible: bool) -> Option<WebView> {
        let p = self.proxy.clone();
        let (p1, p2, p3, p4, p5, p6) = (p.clone(), p.clone(), p.clone(), p.clone(), p.clone(), p);
        let bounds = self.content_rect();
        let builder = common(WebViewBuilder::new_with_web_context(&mut self.spaces[self.active_space].ctx), &self.cfg, &self.paths)
            .with_url(url)
            .with_bounds(bounds)
            .with_visible(visible)
            .with_initialization_script(TAB_JS)
            .with_ipc_handler(move |req| {
                let _ = p1.send_event(UserEvent::FromTab { id, msg: req.body().clone() });
            })
            .with_document_title_changed_handler(move |title| {
                let _ = p2.send_event(UserEvent::Title { id, title });
            })
            .with_on_page_load_handler(move |ev, url| {
                let started = matches!(ev, PageLoadEvent::Started);
                let _ = p3.send_event(UserEvent::Load { id, started, url });
            })
            .with_new_window_req_handler(move |url, features| {
                // Всплывающие окна с заданным размером (вход через Google и т.п.)
                // оставляем окнами, иначе ломается window.opener. Остальное — во вкладку.
                if features.size.is_some() {
                    NewWindowResponse::Allow
                } else {
                    let _ = p4.send_event(UserEvent::Popup { url });
                    NewWindowResponse::Deny
                }
            })
            .with_download_started_handler(move |url, path| {
                let path = path.display().to_string();
                let _ = p5.send_event(UserEvent::DownloadStarted { url, path });
                true
            })
            .with_download_completed_handler(move |url, path, ok| {
                let path = path.map(|p| p.display().to_string());
                let _ = p6.send_event(UserEvent::Download { id, url, path, ok });
            });
        match builder.build_as_child(&self.window) {
            Ok(wv) => {
                let p = self.proxy.clone();
                hotkeys::install(&wv, move |cmd| {
                    let _ = p.send_event(UserEvent::Hotkey { id, cmd });
                });
                Some(wv)
            }
            Err(e) => {
                log::error!("не удалось создать вкладку: {e}");
                self.toast(&format!("Ошибка WebView2: {e}"));
                None
            }
        }
    }

    fn open_tab(&mut self, url: String, background: bool) -> u64 {
        let id = self.tabs.alloc_id();
        let webview = self.build_webview(id, &url, false);
        self.tabs.insert(id, url, now(), webview);
        if background {
            self.push_state();
        } else {
            self.activate(id);
        }
        self.save();
        id
    }

    /// Загрузить спящую вкладку.
    fn wake(&mut self, id: u64) {
        if let Some(url) = self.tabs.get(id).filter(|t| t.view.is_none()).map(|t| t.url.clone()) {
            let wv = self.build_webview(id, &url, true);
            if let Some(t) = self.tabs.get_mut(id) {
                t.loading = wv.is_some();
                t.view = wv;
            }
        }
    }

    fn activate(&mut self, id: u64) {
        // Поиск относится к странице, при переходе на другую вкладку он закрывается.
        if self.find_open && self.tabs.active() != Some(id) {
            self.find_open = false;
            self.run_find("", "clear");
        }
        if !self.tabs.activate(id, now()) {
            return;
        }
        // Спящие вкладки на экране (в том числе вторую половину пары) загружаем.
        for v in self.tabs.visible() {
            self.wake(v);
        }
        self.palette_open = false;
        self.layout();
        if let Some(wv) = self.tabs.get(id).and_then(|t| t.view.as_ref()) {
            let _ = wv.focus();
        }
        self.sleep_idle_tabs();
        self.push_state();
        self.save();
    }

    fn activate_opt(&mut self, id: Option<u64>) {
        if let Some(id) = id {
            self.activate(id);
        }
    }

    /// Выгрузить из памяти давно не открывавшиеся фоновые вкладки.
    /// Возвращает true, если хоть одна вкладка уснула.
    fn sleep_idle_tabs(&mut self) -> bool {
        let (t, limit) = (now(), self.cfg.sleep_minutes * 60);
        // В других пространствах на экране ничего нет — усыпить можно любую вкладку.
        for s in &mut self.spaces {
            s.tabs.sleep_idle_offscreen(t, limit);
        }
        self.tabs.sleep_idle(t, limit)
    }

    /// Открыть вкладку по просьбе страницы, если она не превысила лимит.
    fn open_tab_from_page(&mut self, url: String, background: bool) {
        if self.page_tabs.allow(Instant::now()) {
            self.open_tab(url, background);
        } else {
            log::warn!("страница открывает слишком много вкладок, заблокировано: {url}");
            self.toast("Заблокировано: страница открывает слишком много вкладок");
        }
    }

    fn close_tab(&mut self, id: u64) {
        if let Some(after) = self.tabs.close(id) {
            self.after_close(after);
        }
    }

    fn clear_unpinned(&mut self) {
        let after = self.tabs.clear_unpinned();
        self.after_close(after);
    }

    /// Обновить экран после закрытия вкладок: если закрыли активную — перейти на `next`.
    fn after_close(&mut self, after: AfterClose) {
        if after.was_active
            && let Some(n) = after.next
        {
            self.activate(n);
            return;
        }
        self.layout();
        self.push_state();
        self.save();
    }

    fn reopen_closed(&mut self) {
        if let Some(st) = self.tabs.pop_closed() {
            self.open_tab(st.url, false);
        } else {
            self.toast("Нет закрытых вкладок");
        }
    }

    fn set_pinned(&mut self, id: u64, pinned: bool) {
        self.tabs.set_pinned(id, pinned);
        self.push_state();
        self.save();
    }

    fn move_tab(&mut self, id: u64, before: Option<u64>, pinned: bool) {
        self.tabs.move_tab(id, before, pinned);
        self.push_state();
        self.save();
    }

    /// Перейти по тексту из палитры. `mode`: "new" — в новой вкладке, "split" —
    /// в новой вкладке рядом с текущей, иначе — в текущей.
    fn navigate(&mut self, text: &str, mode: &str) {
        let Some(url) = input::resolve(text, &self.cfg.search_url) else { return };
        match (self.tabs.active(), mode) {
            (Some(active), "split") => {
                let id = self.open_tab(url, true);
                self.split_with(active, id);
            }
            (Some(id), m) if m != "new" => self.load_in(id, &url),
            _ => {
                self.open_tab(url, false);
            }
        }
    }

    // ---------- Пространства ----------

    /// Вкладка в любом пространстве: события приходят и от вкладок, которых сейчас не видно.
    fn tab_any(&self, id: u64) -> Option<&Tab<WebView>> {
        self.tabs.get(id).or_else(|| self.spaces.iter().find_map(|s| s.tabs.get(id)))
    }

    fn tab_mut_any(&mut self, id: u64) -> Option<&mut Tab<WebView>> {
        if self.tabs.get(id).is_some() {
            return self.tabs.get_mut(id);
        }
        self.spaces.iter_mut().find_map(|s| s.tabs.get_mut(id))
    }

    fn switch_space(&mut self, idx: usize) {
        if idx == self.active_space || idx >= self.spaces.len() {
            return;
        }
        if self.find_open {
            self.find_open = false;
            self.run_find("", "clear");
        }
        self.palette_open = false;
        for t in self.tabs.iter() {
            if let Some(wv) = &t.view {
                let _ = wv.set_visible(false);
            }
        }
        // Вкладки активного пространства живут в self.tabs, остальные — в своих слотах.
        std::mem::swap(&mut self.tabs, &mut self.spaces[self.active_space].tabs);
        self.active_space = idx;
        std::mem::swap(&mut self.tabs, &mut self.spaces[idx].tabs);
        match self.tabs.active().or_else(|| self.tabs.order().first().copied()) {
            Some(id) => self.activate(id),
            None => {
                self.layout();
                self.push_state();
                self.save();
            }
        }
    }

    /// Новое пространство: свой профиль (пустые куки), сразу переходим в него.
    fn new_space(&mut self) {
        let mut used: Vec<(u32, &str)> = self.spaces.iter().map(|s| (s.id, s.color.as_str())).collect();
        // Номера удалённых пространств не берём: их папки, возможно, ещё не стёрты.
        used.extend(self.deleted_spaces.iter().map(|id| (*id, "")));
        let Some(mut meta) = spaces::new_space(&used) else {
            self.toast(&format!("Больше {} пространств создать нельзя", spaces::MAX_SPACES));
            return;
        };
        // На диске может остаться папка от старых данных — новому пространству нужен чистый профиль.
        while spaces::space_dir(&self.paths, meta.id).exists() {
            meta.id += 1;
            meta.name = spaces::default_name(meta.id);
        }
        log::info!("создано пространство {} «{}»", meta.id, meta.name);
        self.spaces.push(Space {
            id: meta.id,
            name: meta.name,
            color: meta.color,
            ctx: WebContext::new(Some(spaces::profile_dir(&self.paths, meta.id))),
            tabs: TabList::with_id_base(vec![], spaces::tab_id_base(meta.id)),
        });
        self.switch_space(self.spaces.len() - 1);
        self.open_palette("new");
    }

    fn rename_space(&mut self, name: &str) {
        if let Some(name) = spaces::clean_name(name) {
            self.spaces[self.active_space].name = name;
            self.push_state();
            self.save();
        }
    }

    fn set_space_color(&mut self, idx: usize, color: &str) {
        if spaces::is_color(color)
            && let Some(s) = self.spaces.get_mut(idx)
        {
            s.color = color.to_string();
            self.push_state();
            self.save();
        }
    }

    /// Удалить активное пространство: вкладки закрываются, папка профиля
    /// стирается при следующем запуске. «Основное» удалить нельзя.
    fn delete_space(&mut self) {
        let idx = self.active_space;
        let id = self.spaces[idx].id;
        if id == spaces::MAIN_ID {
            self.toast("Пространство «Основное» удалить нельзя");
            return;
        }
        let name = self.spaces[idx].name.clone();
        let target = if idx > 0 { idx - 1 } else { 1 };
        self.switch_space(target);
        let removed = self.spaces.remove(idx);
        if self.active_space > idx {
            self.active_space -= 1;
        }
        drop(removed);
        self.deleted_spaces.push(id);
        log::info!("удалено пространство {id} «{name}», данные будут стёрты при следующем запуске");
        self.toast(&format!("Пространство «{name}» удалено"));
        self.push_state();
        self.save_now();
    }

    // ---------- Split View ----------

    /// Показать `right` рядом с `left` и перейти в правую половину.
    fn split_with(&mut self, left: u64, right: u64) {
        if self.tabs.make_split(left, right) {
            self.activate(right);
        }
    }

    fn unsplit(&mut self, id: u64) {
        if self.tabs.unsplit(id) {
            self.layout();
            self.push_state();
            self.save();
        }
    }

    fn load_in(&mut self, id: u64, url: &str) {
        let has_wv = self.tabs.get(id).is_some_and(|t| t.view.is_some());
        if let Some(t) = self.tabs.get_mut(id) {
            t.url = url.to_string();
        }
        if has_wv
            && let Some(wv) = self.tabs.get(id).and_then(|t| t.view.as_ref()) {
                let _ = wv.load_url(url);
            }
        self.activate(id);
    }

    fn with_active_wv(&self, f: impl FnOnce(&WebView)) {
        if let Some(wv) = self.tabs.active().and_then(|id| self.tabs.get(id)).and_then(|t| t.view.as_ref()) {
            f(wv);
        }
    }

    fn open_palette(&mut self, mode: &str) {
        self.palette_open = true;
        self.layout();
        // В поле ввода: адрес текущей вкладки, а при переименовании — имя пространства.
        let current = if mode == "rename_space" {
            self.spaces[self.active_space].name.clone()
        } else {
            self.tabs.active_tab().map(|t| t.url.clone()).unwrap_or_default()
        };
        // Третий аргумент — список для поиска: загрузки в режиме "downloads", иначе история.
        let items: Vec<Value> = if mode == "downloads" {
            self.downloads
                .recent()
                .map(|d| {
                    json!({ "id": d.id, "name": d.name(), "path": d.path, "url": d.url, "state": d.state })
                })
                .collect()
        } else {
            self.history
                .recent(800)
                .into_iter()
                .map(|e| json!({ "url": e.url, "title": e.title, "visits": e.visits }))
                .collect()
        };
        self.push_state();
        let _ = self.ui.focus();
        self.eval_ui(&format!(
            "window.drift && drift.openPalette({}, {}, {})",
            json!(mode),
            json!(current),
            Value::Array(items)
        ));
    }

    // ---------- Поиск по странице ----------

    fn open_find(&mut self) {
        if self.tabs.active_tab().is_none_or(|t| t.view.is_none()) {
            return;
        }
        self.palette_open = false;
        self.find_open = true;
        self.layout();
        self.push_state();
        let _ = self.ui.focus();
        self.eval_ui("window.drift && drift.openFind()");
    }

    fn close_find(&mut self) {
        if !self.find_open {
            return;
        }
        self.find_open = false;
        self.run_find("", "clear");
        self.layout();
        self.with_active_wv(|wv| {
            let _ = wv.focus();
        });
        self.push_state();
    }

    fn run_find(&self, query: &str, mode: &str) {
        let Some(id) = self.tabs.active() else { return };
        let proxy = self.proxy.clone();
        self.with_active_wv(|wv| {
            let _ = wv.evaluate_script_with_callback(&find_script(query, mode), move |json| {
                let _ = proxy.send_event(UserEvent::FindResult { id, json });
            });
        });
    }

    // ---------- Загрузки и очистка ----------

    /// Показать файл в Проводнике (или папку загрузок, если файла уже нет).
    fn reveal_download(&self, id: u64) {
        let Some(d) = self.downloads.get(id) else { return };
        let path = std::path::Path::new(&d.path);
        if d.path.is_empty() || !path.exists() {
            self.toast("Файл не найден: возможно, его удалили или переместили");
            return;
        }
        let mut cmd = std::process::Command::new("explorer");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // explorer разбирает /select сам, поэтому путь передаём одной строкой в кавычках.
            cmd.raw_arg(format!("/select,\"{}\"", d.path));
        }
        if let Err(e) = cmd.spawn() {
            log::error!("не удалось открыть Проводник: {e}");
        }
    }

    fn open_downloads_folder(&self) {
        let Some(dir) = dirs::download_dir() else { return };
        if let Err(e) = std::process::Command::new("explorer").arg(dir).spawn() {
            log::error!("не удалось открыть папку загрузок: {e}");
        }
    }

    fn clear_history(&mut self) {
        self.history.clear();
        self.save_now();
        log::info!("история очищена");
        self.toast("История очищена");
    }

    /// Куки, кэш, хранилища и сохранённые пароли всех сайтов: профиль общий для всех WebView.
    /// Очистка идёт через любой WebView этого профиля: у «Основного» это интерфейс,
    /// у остальных — открытая вкладка пространства.
    fn clear_site_data(&self) {
        let space = &self.spaces[self.active_space];
        let wv = if space.id == spaces::MAIN_ID {
            Some(&self.ui)
        } else {
            self.tabs.iter().find_map(|t| t.view.as_ref())
        };
        let Some(wv) = wv else {
            self.toast("Откройте любую вкладку в этом пространстве и повторите");
            return;
        };
        match wv.clear_all_browsing_data() {
            Ok(()) => {
                log::info!("данные сайтов пространства «{}» очищены", space.name);
                self.toast(&format!("Данные сайтов в «{}» очищены", space.name));
            }
            Err(e) => {
                log::error!("не удалось очистить данные сайтов: {e}");
                self.toast("Не удалось очистить данные сайтов");
            }
        }
    }

    fn close_palette(&mut self) {
        if !self.palette_open {
            return;
        }
        self.palette_open = false;
        self.layout();
        self.with_active_wv(|wv| {
            let _ = wv.focus();
        });
        self.push_state();
    }

    // ---------- Команды ----------

    fn command(&mut self, cmd: &str) {
        match cmd {
            "new_tab" => self.open_palette("new"),
            "edit_url" => {
                if self.tabs.active().is_some() { self.open_palette("edit") } else { self.open_palette("new") }
            }
            "close" => {
                if let Some(id) = self.tabs.active() {
                    self.close_tab(id);
                }
            }
            "reopen" => self.reopen_closed(),
            "next" => self.activate_opt(self.tabs.cycle_target(1)),
            "prev" => self.activate_opt(self.tabs.cycle_target(-1)),
            "toggle_sidebar" => {
                self.sidebar_visible = !self.sidebar_visible;
                self.layout();
                self.push_state();
                self.save();
            }
            "pin" => {
                if let Some(id) = self.tabs.active() {
                    let p = self.tabs.get(id).is_some_and(|t| t.pinned);
                    self.set_pinned(id, !p);
                }
            }
            "back" => self.with_active_wv(|wv| {
                let _ = wv.evaluate_script("history.back()");
            }),
            "forward" => self.with_active_wv(|wv| {
                let _ = wv.evaluate_script("history.forward()");
            }),
            "reload" => self.with_active_wv(|wv| {
                let _ = wv.reload();
            }),
            "devtools" => self.with_active_wv(|wv| wv.open_devtools()),
            "win_min" => self.window.set_minimized(true),
            "win_max" => self.window.set_maximized(!self.window.is_maximized()),
            "win_close" => self.exit_requested = true,
            "copy_url" => {
                if let Some(url) = self.tabs.active_tab().map(|t| t.url.clone()) {
                    if hotkeys::copy_to_clipboard(&url) {
                        self.toast("Ссылка скопирована");
                    } else {
                        self.toast("Не удалось скопировать ссылку");
                    }
                }
            }
            "clear" => self.clear_unpinned(),
            "space_new" => self.new_space(),
            "rename_space" => self.open_palette("rename_space"),
            // Удаление необратимо: палитра сначала спрашивает подтверждение.
            "delete_space_confirm" => self.open_palette("delete_space"),
            "delete_space" => self.delete_space(),
            c if c.starts_with("space:") => {
                if let Ok(n) = c[6..].parse::<usize>()
                    && n >= 1
                {
                    self.switch_space(n - 1);
                }
            }
            // Разделить можно только при открытой вкладке: она станет левой половиной.
            "split" if self.tabs.active().is_some() => self.open_palette("split"),
            "unsplit" => {
                if let Some(id) = self.tabs.active() {
                    self.unsplit(id);
                }
            }
            "find" => self.open_find(),
            "downloads" => self.open_palette("downloads"),
            "open_downloads_folder" => self.open_downloads_folder(),
            "clear_history" => self.clear_history(),
            "clear_data" => self.clear_site_data(),
            c if c.starts_with("goto:") => {
                if let Ok(n) = c[5..].parse() {
                    self.activate_opt(self.tabs.goto_target(n));
                }
            }
            _ => {}
        }
    }

    pub fn handle(&mut self, ev: UserEvent) {
        match ev {
            UserEvent::FromUi(body) => self.handle_ui(&body),
            UserEvent::FromTab { id, msg } => self.handle_tab_msg(id, &msg),
            UserEvent::Title { id, title } => {
                let url = self.tab_any(id).and_then(|t| t.view.as_ref()).and_then(|w| w.url().ok());
                let tab_url = self.tab_mut_any(id).map(|t| {
                    t.title = title.clone();
                    if let Some(u) = url.filter(|u| !u.is_empty()) {
                        t.url = u;
                    }
                    t.url.clone()
                });
                if let Some(u) = tab_url {
                    self.history.set_title(&u, &title);
                }
                self.push_state();
            }
            UserEvent::Load { id, started, url } => {
                let visited = self.tab_mut_any(id).and_then(|t| {
                    t.loading = started;
                    if !url.is_empty() {
                        t.url = url.clone();
                    }
                    (!started).then(|| (t.url.clone(), t.title.clone()))
                });
                if let Some((u, title)) = visited {
                    self.history.visit(&u, &title);
                    self.save();
                }
                self.push_state();
            }
            UserEvent::OpenTab { url, background } => {
                self.open_tab(url, background);
            }
            UserEvent::Popup { url } => self.open_tab_from_page(url, false),
            UserEvent::DownloadStarted { url, path } => {
                self.downloads.started(&url, &path, now());
                self.downloads.save(&self.paths.downloads);
                if let Some(d) = self.downloads.recent().next() {
                    self.toast(&format!("Загрузка: {} (Ctrl+J — список)", d.name()));
                }
            }
            UserEvent::Download { id, url, path, ok } => {
                let d = self.downloads.finished(&url, path.as_deref(), ok, now());
                let text = match d.state {
                    DownloadState::Done => format!("Загружено: {}", d.name()),
                    _ => format!("Загрузка не удалась: {}", d.name()),
                };
                self.downloads.save(&self.paths.downloads);
                // Вкладка, открытая только ради загрузки, остаётся пустой — закрываем её, как Chrome.
                if self.tabs.get(id).is_some_and(|t| !t.pinned && t.url == "about:blank") {
                    self.close_tab(id);
                }
                self.toast(&text);
            }
            UserEvent::FindResult { id, json } => {
                if self.find_open && self.tabs.active() == Some(id)
                    && let Some((n, i)) = parse_find_result(&json)
                {
                    self.eval_ui(&format!("window.drift && drift.findResult({})", json!({ "n": n, "i": i })));
                }
            }
            // Клавиши приходят от WebView2, а не от страницы, но фокус может остаться
            // в скрытой вкладке — слушаем только ту, что на экране.
            UserEvent::Hotkey { id, cmd } => {
                if self.tabs.active() == Some(id) {
                    self.command(&cmd);
                }
            }
        }
    }

    fn handle_tab_msg(&mut self, id: u64, msg: &str) {
        let Ok(v) = serde_json::from_str::<Value>(msg) else { return };
        // Страница может прислать что угодно: команд браузера отсюда не принимаем
        // (горячие клавиши ловит hotkeys.rs), только сведения о ней самой.
        match v["t"].as_str().unwrap_or("") {
            // Ctrl/средний клик возможен только во вкладке, которая сейчас на экране.
            "open_bg" if self.tabs.active() == Some(id) => {
                if let Some(u) = v["u"].as_str().filter(|u| u.starts_with("http")) {
                    self.open_tab_from_page(u.to_string(), true);
                }
            }
            "icon" => {
                let u = v["u"].as_str().unwrap_or("").to_string();
                if u.starts_with("http") || u.starts_with("data:image/") {
                    if let Some(t) = self.tab_mut_any(id) {
                        t.favicon = u;
                    }
                    self.push_state();
                }
            }
            "media" => {
                if let Some(t) = self.tab_mut_any(id) {
                    t.media = v["on"].as_bool().unwrap_or(false);
                }
                self.push_state();
            }
            _ => {}
        }
    }

    fn handle_ui(&mut self, body: &str) {
        let Ok(v) = serde_json::from_str::<Value>(body) else { return };
        let id = v["id"].as_u64();
        match v["type"].as_str().unwrap_or("") {
            "ready" => {
                self.layout();
                self.push_state();
            }
            "activate" => {
                if let Some(id) = id {
                    self.activate(id);
                }
            }
            "close" => {
                if let Some(id) = id {
                    self.close_tab(id);
                }
            }
            "pin" => {
                if let Some(id) = id {
                    self.set_pinned(id, true);
                }
            }
            "unpin" => {
                if let Some(id) = id {
                    self.set_pinned(id, false);
                }
            }
            "reset_pinned" => {
                if let Some(id) = id
                    && let Some(u) = self.tabs.get(id).and_then(|t| t.pinned_url.clone()) {
                        self.load_in(id, &u);
                    }
            }
            "move" => {
                if let Some(id) = id {
                    let pinned = v["pinned"].as_bool().unwrap_or(false);
                    self.move_tab(id, v["before"].as_u64(), pinned);
                }
            }
            "navigate" => {
                let text = v["text"].as_str().unwrap_or("").to_string();
                self.palette_open = false;
                self.navigate(&text, v["mode"].as_str().unwrap_or(""));
                self.layout();
                self.push_state();
            }
            // «Открыть рядом»: вкладка `id` встаёт справа от текущей.
            "split" => {
                if let Some(id) = id {
                    match self.tabs.active() {
                        Some(active) if active != id => self.split_with(active, id),
                        _ => self.activate(id),
                    }
                }
            }
            "unsplit" => {
                if let Some(id) = id.or(self.tabs.active()) {
                    self.unsplit(id);
                }
            }
            // Разделитель тянут: двигаем половины сразу, а сохраняем, когда отпустили.
            "split_ratio" => {
                if let (Some(active), Some(r)) = (self.tabs.active(), v["ratio"].as_f64()) {
                    self.tabs.set_split_ratio(active, r);
                    self.layout();
                    if v["done"].as_bool() == Some(true) {
                        self.push_state();
                        self.save();
                    }
                }
            }
            "palette_close" => self.close_palette(),
            "space" => {
                if let Some(i) = v["index"].as_u64() {
                    self.switch_space(i as usize);
                }
            }
            "space_rename" => {
                self.palette_open = false;
                self.rename_space(v["name"].as_str().unwrap_or(""));
                self.layout();
                self.push_state();
            }
            "space_color" => {
                if let (Some(i), Some(c)) = (v["index"].as_u64(), v["color"].as_str()) {
                    self.set_space_color(i as usize, c);
                }
            }
            // Перетаскивание и изменение размера отдаём Windows: так работают
            // прилипание к краям экрана и разворот при перетаскивании к верху.
            "drag" => {
                let _ = self.window.drag_window();
            }
            "resize" => {
                if let Some(dir) = v["dir"].as_str().and_then(resize_direction) {
                    let _ = self.window.drag_resize_window(dir);
                }
            }
            "find" => {
                let mode = v["mode"].as_str().filter(|m| matches!(*m, "next" | "prev")).unwrap_or("type");
                self.run_find(v["text"].as_str().unwrap_or(""), mode);
            }
            "find_close" => self.close_find(),
            "open_download" => {
                if let Some(id) = id {
                    self.reveal_download(id);
                }
            }
            "key" => {
                if let Some(k) = v["k"].as_str() {
                    self.command(k);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SPLIT_GAP, find_script, parse_find_result, resize_direction, split_boxes};

    #[test]
    fn split_boxes_fill_area_around_gap() {
        let [l, r] = split_boxes((100.0, 8.0, 808.0, 500.0), 0.5);
        assert_eq!(l, (100.0, 8.0, 400.0, 500.0));
        assert_eq!(r, (100.0 + 400.0 + SPLIT_GAP, 8.0, 400.0, 500.0));
        // Половины и промежуток занимают ровно всю ширину.
        let [l, r] = split_boxes((0.0, 0.0, 1001.0, 10.0), 0.3);
        assert_eq!(l.2 + SPLIT_GAP + r.2, 1001.0);
        assert_eq!(r.0, l.2 + SPLIT_GAP);
    }

    #[test]
    fn split_boxes_never_negative() {
        let [l, r] = split_boxes((0.0, 0.0, 4.0, 10.0), 0.5);
        assert!(l.2 >= 0.0 && r.2 >= 0.0);
    }
    use tao::window::ResizeDirection;

    #[test]
    fn resize_directions_from_ui() {
        assert_eq!(resize_direction("n"), Some(ResizeDirection::North));
        assert_eq!(resize_direction("se"), Some(ResizeDirection::SouthEast));
        assert_eq!(resize_direction("sw"), Some(ResizeDirection::SouthWest));
        for bad in ["", "N", "north", "x", "nse"] {
            assert_eq!(resize_direction(bad), None, "{bad}");
        }
    }

    #[test]
    fn find_result_with_matches() {
        assert_eq!(parse_find_result(r#"{"n":7,"i":2}"#), Some((7, Some(2))));
        assert_eq!(parse_find_result(r#"{"n":1,"i":0,"extra":"<script>"}"#), Some((1, Some(0))));
    }

    #[test]
    fn find_result_without_matches() {
        assert_eq!(parse_find_result(r#"{"n":0,"i":-1}"#), Some((0, None)));
    }

    #[test]
    fn find_result_rejects_bad_values() {
        for bad in [
            r#"{"n":3,"i":3}"#,
            r#"{"n":3,"i":-1}"#,
            r#"{"n":0,"i":0}"#,
            r#"{"n":-1,"i":-1}"#,
            r#"{"n":"3","i":0}"#,
            r#"{"n":3}"#,
            r#""{\"n\":1,\"i\":0}""#,
            "null",
            "not json",
        ] {
            assert_eq!(parse_find_result(bad), None, "{bad}");
        }
    }

    #[test]
    fn find_script_calls_function_with_json_arguments() {
        let js = find_script("a\"b'</script>\n", "next");
        assert!(js.starts_with('(') && js.ends_with(r#")("a\"b'</script>\n", "next")"#), "{js}");
    }
}
