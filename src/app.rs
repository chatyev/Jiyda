//! Ядро браузера: окно, вкладки, раскладка, обработка команд.

use crate::config::{Config, Paths};
use crate::input;
use crate::limit::RateLimit;
use crate::store::{History, Session, now};
use crate::tabs::{AfterClose, TabList};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
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
    Download { path: String, ok: bool },
}

/// Команды, которые принимаются из страницы (их присылает tab.js по горячим клавишам).
/// Страница может отправить такое сообщение и сама, поэтому команд, способных
/// навредить (закрыть все вкладки, открыть DevTools), здесь нет.
fn allowed_from_tab(cmd: &str) -> bool {
    matches!(
        cmd,
        "new_tab" | "edit_url" | "close" | "reopen" | "next" | "prev" | "toggle_sidebar" | "pin" | "copied"
    ) || cmd
        .strip_prefix("goto:")
        .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
}

pub struct App {
    window: Window,
    ctx: WebContext,
    ui: WebView,
    proxy: EventLoopProxy<UserEvent>,
    cfg: Config,
    paths: Paths,
    tabs: TabList<WebView>,
    sidebar_visible: bool,
    palette_open: bool,
    history: History,
    /// Когда сбросить сессию и историю на диск (None — изменений нет).
    save_due: Option<Instant>,
    next_sleep_check: Instant,
    page_tabs: RateLimit,
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

impl App {
    pub fn new(event_loop: &EventLoop<UserEvent>) -> wry::Result<Self> {
        let paths = Paths::new();
        let cfg = Config::load(&paths);
        let session = Session::load(&paths.session);
        let history = History::load(&paths.history);
        let proxy = event_loop.create_proxy();

        let window = WindowBuilder::new()
            .with_title("Drift")
            .with_inner_size(tao::dpi::LogicalSize::new(1360.0, 860.0))
            .with_min_inner_size(tao::dpi::LogicalSize::new(640.0, 400.0))
            .with_theme(Some(tao::window::Theme::Dark))
            .build(event_loop)
            .expect("не удалось создать окно");

        let mut ctx = WebContext::new(Some(paths.webview_data.clone()));

        // Интерфейс (сайдбар, палитра) — отдельный WebView во всё окно, под вкладками.
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        let ui_proxy = proxy.clone();
        let nav_proxy = proxy.clone();
        let ui = common(WebViewBuilder::new_with_web_context(&mut ctx), &cfg, &paths)
            .with_html(UI_HTML)
            .with_bounds(rect(0.0, 0.0, size.width, size.height))
            .with_initialization_script(format!(
                "window.DRIFT_CONFIG = {};",
                json!({ "sidebarWidth": cfg.sidebar_width, "padding": cfg.frame_padding })
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

        let mut app = Self {
            window,
            ctx,
            ui,
            proxy,
            cfg,
            paths,
            tabs: TabList::new(session.closed.clone()),
            sidebar_visible: session.sidebar_visible.unwrap_or(true),
            palette_open: false,
            history,
            save_due: None,
            next_sleep_check: Instant::now() + SLEEP_CHECK,
            page_tabs: RateLimit::new(PAGE_TABS_MAX, PAGE_TABS_WINDOW),
        };
        app.restore(session);
        Ok(app)
    }

    // ---------- Сессия ----------

    fn restore(&mut self, session: Session) {
        let target = self.tabs.restore(session.tabs, session.active, now(), self.cfg.archive_hours * 3600);
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
        let (tabs, active) = self.tabs.to_saved();
        Session {
            tabs,
            active,
            sidebar_visible: Some(self.sidebar_visible),
            closed: self.tabs.closed().to_vec(),
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

    fn content_rect(&self) -> Rect {
        let (w, h) = self.window_size();
        let p = self.cfg.frame_padding;
        let left = if self.sidebar_visible { self.cfg.sidebar_width } else { p };
        rect(left, p, w - left - p, h - 2.0 * p)
    }

    // ---------- Раскладка и отрисовка ----------

    pub fn layout(&mut self) {
        let (w, h) = self.window_size();
        let _ = self.ui.set_bounds(rect(0.0, 0.0, w, h));
        let r = self.content_rect();
        let show_id = if self.palette_open { None } else { self.tabs.active() };
        for t in self.tabs.iter() {
            if let Some(wv) = &t.view {
                if Some(t.id) == show_id {
                    let _ = wv.set_bounds(r);
                    let _ = wv.set_visible(true);
                } else {
                    let _ = wv.set_visible(false);
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
            "palette": self.palette_open,
            "closedCount": self.tabs.closed().len(),
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
        let (p1, p2, p3, p4, p5) = (p.clone(), p.clone(), p.clone(), p.clone(), p);
        let bounds = self.content_rect();
        let builder = common(WebViewBuilder::new_with_web_context(&mut self.ctx), &self.cfg, &self.paths)
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
            .with_download_completed_handler(move |_url, path, ok| {
                let path = path.map(|p| p.display().to_string()).unwrap_or_default();
                let _ = p5.send_event(UserEvent::Download { path, ok });
            });
        match builder.build_as_child(&self.window) {
            Ok(wv) => Some(wv),
            Err(e) => {
                log::error!("не удалось создать вкладку: {e}");
                self.toast(&format!("Ошибка WebView2: {e}"));
                None
            }
        }
    }

    fn open_tab(&mut self, url: String, background: bool) {
        let id = self.tabs.alloc_id();
        let webview = self.build_webview(id, &url, false);
        self.tabs.insert(id, url, now(), webview);
        if background {
            self.push_state();
        } else {
            self.activate(id);
        }
        self.save();
    }

    fn activate(&mut self, id: u64) {
        // Спящую вкладку сначала загружаем.
        if let Some(url) = self.tabs.get(id).filter(|t| t.view.is_none()).map(|t| t.url.clone()) {
            let wv = self.build_webview(id, &url, true);
            if let Some(t) = self.tabs.get_mut(id) {
                t.loading = wv.is_some();
                t.view = wv;
            }
        }
        if !self.tabs.activate(id, now()) {
            return;
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
        self.tabs.sleep_idle(now(), self.cfg.sleep_minutes * 60)
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

    fn navigate(&mut self, text: &str, new_tab: bool) {
        let Some(url) = input::resolve(text, &self.cfg.search_url) else { return };
        match self.tabs.active().filter(|_| !new_tab) {
            Some(id) => self.load_in(id, &url),
            None => self.open_tab(url, false),
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
        let current = self.tabs.active().and_then(|id| self.tabs.get(id)).map(|t| t.url.clone()).unwrap_or_default();
        let hist: Vec<Value> = self
            .history
            .recent(800)
            .into_iter()
            .map(|e| json!({ "url": e.url, "title": e.title, "visits": e.visits }))
            .collect();
        self.push_state();
        let _ = self.ui.focus();
        self.eval_ui(&format!(
            "window.drift && drift.openPalette({}, {}, {})",
            json!(mode),
            json!(current),
            Value::Array(hist)
        ));
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
            "copied" => self.toast("Ссылка скопирована"),
            "clear" => self.clear_unpinned(),
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
                let url = self.tabs.get(id).and_then(|t| t.view.as_ref()).and_then(|w| w.url().ok());
                if let Some(t) = self.tabs.get_mut(id) {
                    t.title = title.clone();
                    if let Some(u) = url.filter(|u| !u.is_empty()) {
                        t.url = u;
                    }
                    let u = t.url.clone();
                    self.history.set_title(&u, &title);
                }
                self.push_state();
            }
            UserEvent::Load { id, started, url } => {
                let mut visited = None;
                if let Some(t) = self.tabs.get_mut(id) {
                    t.loading = started;
                    if !url.is_empty() {
                        t.url = url.clone();
                    }
                    if !started {
                        visited = Some((t.url.clone(), t.title.clone()));
                    }
                }
                if let Some((u, title)) = visited {
                    self.history.visit(&u, &title);
                    self.save();
                }
                self.push_state();
            }
            UserEvent::OpenTab { url, background } => self.open_tab(url, background),
            UserEvent::Popup { url } => self.open_tab_from_page(url, false),
            UserEvent::Download { path, ok } => {
                if ok {
                    self.toast(&format!("Загружено: {path}"));
                } else {
                    self.toast("Загрузка не удалась");
                }
            }
        }
    }

    fn handle_tab_msg(&mut self, id: u64, msg: &str) {
        let Ok(v) = serde_json::from_str::<Value>(msg) else { return };
        match v["t"].as_str().unwrap_or("") {
            // Страница может прислать что угодно, поэтому команды из вкладки
            // принимаем только от активной вкладки.
            "key" if self.tabs.active() == Some(id) => {
                match v["k"].as_str() {
                    Some(k) if allowed_from_tab(k) => self.command(k),
                    Some(k) => log::warn!("вкладка {id} прислала запрещённую команду {k:?}"),
                    None => {}
                }
            }
            // Ctrl/средний клик возможен только во вкладке, которая сейчас на экране.
            "open_bg" if self.tabs.active() == Some(id) => {
                if let Some(u) = v["u"].as_str().filter(|u| u.starts_with("http")) {
                    self.open_tab_from_page(u.to_string(), true);
                }
            }
            "icon" => {
                let u = v["u"].as_str().unwrap_or("").to_string();
                if u.starts_with("http") || u.starts_with("data:image/") {
                    if let Some(t) = self.tabs.get_mut(id) {
                        t.favicon = u;
                    }
                    self.push_state();
                }
            }
            "media" => {
                if let Some(t) = self.tabs.get_mut(id) {
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
                let new_tab = v["mode"].as_str() == Some("new");
                self.palette_open = false;
                self.navigate(&text, new_tab);
                self.layout();
                self.push_state();
            }
            "palette_close" => self.close_palette(),
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
    use super::allowed_from_tab;

    #[test]
    fn tab_hotkey_commands_are_allowed() {
        for cmd in ["new_tab", "edit_url", "close", "reopen", "next", "prev", "toggle_sidebar", "pin", "copied"] {
            assert!(allowed_from_tab(cmd), "{cmd}");
        }
        for n in 1..=9 {
            assert!(allowed_from_tab(&format!("goto:{n}")));
        }
    }

    #[test]
    fn dangerous_or_unknown_commands_are_rejected() {
        for cmd in ["clear", "devtools", "back", "forward", "reload", "", "goto:", "goto:0", "goto:10", "goto:x", "NEW_TAB"] {
            assert!(!allowed_from_tab(cmd), "{cmd}");
        }
    }
}
