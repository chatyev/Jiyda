//! Движок Microsoft WebView2 (через `wry` и WebView2 COM). Единственное место в Jiyda,
//! где видны `wry` и `webview2_com`: остальной код работает с [`crate::engine`].
//!
//! - mod.rs — создание страниц и интерфейса, [`PageView`] поверх `wry::WebView`;
//! - hooks.rs — события WebView2, которых нет в wry (загрузки, значок, полный экран, снимок);
//! - keys.rs — сочетания клавиш до страницы (`AcceleratorKeyPressed`);
//! - shield.rs — блокировка трекеров (`WebResourceRequested`);
//! - permissions.rs — запросы разрешений (`PermissionRequested`) и ответы на них.

mod hooks;
mod keys;
mod permissions;
mod shield;

use super::{
    AssetFn, Bounds, Engine, Options, Page, PageEvent, PageId, PageSpec, PageView, Policy, Profile, Sink, Theme,
    UiSpec, Window,
};
use crate::logging::OrLog;
use std::borrow::Cow;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::http::{Request, Response, StatusCode, header::CONTENT_TYPE};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

/// Движок WebView2: настройки, решения ядра, куда слать события и состояние, которое
/// живёт дольше одной страницы.
pub struct WebView2Engine {
    options: Options,
    policy: Policy,
    sink: Sink,
    requests: Rc<permissions::Requests>,
    downloads: Rc<hooks::ActiveDownloads>,
    /// Окно интерфейса: новые страницы ложатся поверх, интерфейс поднимаем обратно.
    ui_hwnd: Cell<Option<isize>>,
}

impl WebView2Engine {
    pub fn new(options: Options, policy: Policy, sink: Sink) -> Self {
        Self { options, policy, sink, requests: Rc::default(), downloads: Rc::default(), ui_hwnd: Cell::new(None) }
    }

    /// Обработчик для wry, который шлёт событие страницы `id` (None — интерфейса).
    fn emit(&self, id: Option<PageId>) -> impl Fn(PageEvent) + Clone + 'static {
        let sink = self.sink.clone();
        move |ev| sink(id, ev)
    }

    /// Общие настройки всех WebView. Они обязаны совпадать, иначе WebView2 откажется
    /// использовать одну папку данных для разных страниц.
    fn common<'a>(&self, b: WebViewBuilder<'a>, theme: Theme, dark: bool) -> WebViewBuilder<'a> {
        let b = b
            .with_devtools(self.options.devtools)
            .with_hotkeys_zoom(true)
            .with_clipboard(true)
            .with_general_autofill_enabled(true)
            // Подложка до загрузки страницы — под тему, чтобы не мелькал чужой цвет.
            .with_background_color(if dark { (11, 15, 20, 255) } else { (255, 255, 255, 255) });
        #[cfg(windows)]
        let b = {
            use wry::WebViewBuilderExtWindows;
            // Сайты, которые умеют светлую и тёмную тему, показываются в теме Jiyda.
            let b = b.with_browser_extensions_enabled(self.options.extensions).with_theme(wry_theme(theme));
            if self.options.extensions { b.with_extensions_path(self.options.extensions_dir.clone()) } else { b }
        };
        #[cfg(not(windows))]
        let _ = theme;
        b
    }

    fn raise_ui(&self) {
        if let Some(h) = self.ui_hwnd.get() {
            crate::frame::raise(h);
        }
    }
}

fn context(profile: &mut Profile) -> Result<&mut WebContext, String> {
    profile.downcast_mut::<WebContext>().ok_or_else(|| "профиль не от движка WebView2".to_string())
}

fn rect(b: Bounds) -> Rect {
    Rect { position: LogicalPosition::new(b.x, b.y).into(), size: LogicalSize::new(b.w, b.h).into() }
}

#[cfg(windows)]
fn wry_theme(t: Theme) -> wry::Theme {
    match t {
        Theme::Light => wry::Theme::Light,
        Theme::Dark => wry::Theme::Dark,
        Theme::System => wry::Theme::Auto,
    }
}

/// Ответ на запрос интерфейса к своему протоколу (`<scheme>://ui/…`).
fn respond(asset: AssetFn, csp: &str, req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let response = match asset(req.uri().path()) {
        Some((data, mime)) => Response::builder()
            .header(CONTENT_TYPE, mime)
            .header("Content-Security-Policy", csp)
            .header("X-Content-Type-Options", "nosniff")
            .body(Cow::Borrowed(data)),
        None => {
            log::warn!("интерфейс запросил несуществующий файл: {}", req.uri());
            Response::builder().status(StatusCode::NOT_FOUND).body(Cow::Borrowed(&b""[..]))
        }
    };
    response.unwrap_or_else(|_| Response::new(Cow::Borrowed(&b""[..])))
}

/// Куда интерфейсу можно перейти: только на свои страницы. Сайт (http, https) открывается
/// страницей Jiyda (`leave`), всё остальное (file:, data:, javascript:, about: …) запрещено —
/// иначе чужой документ занял бы место интерфейса вместе с его правами.
fn ui_navigation(is_own: fn(&str) -> bool, url: &str, leave: impl FnOnce(String)) -> bool {
    if is_own(url) {
        return true;
    }
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        leave(url.to_string());
    } else {
        log::warn!("интерфейс не переходит на {url}");
    }
    false
}

/// Настройки WebView интерфейса: без своих клавиш WebView (Ctrl+P, F5, Ctrl+F…) и
/// масштаба — у интерфейса они не нужны, а клавиши Jiyda по-прежнему приходят в
/// AcceleratorKeyPressed.
///
/// CSS `app-region: drag` (SetIsNonClientRegionSupportEnabled) не включаем: пересчитывая
/// области перетаскивания, WebView2 синхронно двигает своё окно (UpdateDraggableRegions →
/// SetWindowPos), и это может навсегда остановить главный поток — так зависал Jiyda на CI.
/// Окно тянет ядро по сообщению интерфейса (UiMsg::WindowDrag, titlebar.js).
#[cfg(windows)]
fn configure_ui(ui: &WebView) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
    use windows::core::Interface;
    use wry::WebViewExtWindows;
    // SAFETY: настройки живого WebView.
    unsafe {
        if let Ok(s) = ui.webview().Settings() {
            let _ = s.SetIsZoomControlEnabled(false);
            if let Ok(s3) = s.cast::<ICoreWebView2Settings3>() {
                let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
            }
        }
    }
}
#[cfg(not(windows))]
fn configure_ui(_ui: &WebView) {}

impl Engine for WebView2Engine {
    fn profile(&self, dir: PathBuf) -> Profile {
        Profile::new(WebContext::new(Some(dir)))
    }

    fn create_ui(&self, window: &Window, profile: &mut Profile, spec: UiSpec) -> Result<Page, String> {
        let builder = self.common(WebViewBuilder::new_with_web_context(context(profile)?), spec.theme, spec.dark);
        // Интерфейс — с https-адреса (https://<scheme>.ui/): защищённый источник.
        #[cfg(windows)]
        let builder = {
            use wry::WebViewBuilderExtWindows;
            builder.with_https_scheme(true)
        };
        let (asset, csp) = (spec.asset, spec.csp);
        let is_own = spec.is_own_url;
        let on_leave = Rc::new(spec.on_leave);
        let leave_popup = on_leave.clone();
        let emit = self.emit(None);
        let ui = builder
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_custom_protocol(spec.scheme.into(), move |_, req| respond(asset, csp, &req))
            .with_url(&spec.url)
            .with_bounds(rect(spec.bounds))
            .with_initialization_script(&spec.init_script)
            // Команды браузеру принимаем только от самого интерфейса: окажись в его WebView
            // чужой документ, он не должен управлять Jiyda через window.ipc.
            .with_ipc_handler(move |req| {
                if is_own(&req.uri().to_string()) {
                    emit(PageEvent::Message(req.body().clone()));
                } else {
                    log::warn!("сообщение интерфейсу не из интерфейса отклонено: {}", req.uri());
                }
            })
            .with_navigation_handler(move |url| ui_navigation(is_own, &url, |u| on_leave(u)))
            // Ссылка с target=_blank или window.open из интерфейса — обычная страница Jiyda,
            // а не отдельное окно WebView2.
            .with_new_window_req_handler(move |url, _| {
                ui_navigation(is_own, &url, |u| leave_popup(u));
                NewWindowResponse::Deny
            })
            .build_as_child(window)
            .map_err(|e| e.to_string())?;
        configure_ui(&ui);
        let emit = self.emit(None);
        keys::install(&ui, self.policy.keys.clone(), move |ev| emit(PageEvent::Key(ev)));
        let page = Wv2Page(ui);
        self.ui_hwnd.set(Some(page.native_handle()));
        Ok(Box::new(page))
    }

    fn create_page(&self, window: &Window, profile: &mut Profile, spec: PageSpec) -> Result<Page, String> {
        let id = spec.id;
        let emit = self.emit(Some(id));
        let (e1, e2, e3, e4, e5, e6) =
            (emit.clone(), emit.clone(), emit.clone(), emit.clone(), emit.clone(), emit.clone());
        let wv = self
            .common(WebViewBuilder::new_with_web_context(context(profile)?), spec.theme, spec.dark)
            .with_url(spec.url)
            .with_bounds(rect(spec.bounds))
            .with_visible(spec.visible)
            .with_initialization_script(&spec.init_script)
            .with_ipc_handler(move |req| e1(PageEvent::Message(req.body().clone())))
            .with_document_title_changed_handler(move |title| e2(PageEvent::Title(title)))
            .with_on_page_load_handler(move |ev, url| {
                e3(PageEvent::Load { started: matches!(ev, PageLoadEvent::Started), url })
            })
            .with_new_window_req_handler(move |url, features| {
                // Всплывающие окна с заданным размером (вход через Google и т.п.)
                // оставляем окнами, иначе ломается window.opener. Остальное — страницей.
                if features.size.is_some() {
                    NewWindowResponse::Allow
                } else {
                    e4(PageEvent::Popup(url));
                    NewWindowResponse::Deny
                }
            })
            .with_download_started_handler(move |url, path| {
                e5(PageEvent::DownloadStarted { url, path: path.display().to_string() });
                true
            })
            .with_download_completed_handler(move |url, path, ok| {
                e6(PageEvent::DownloadFinished { url, path: path.map(|p| p.display().to_string()), ok })
            })
            .build_as_child(window)
            .map_err(|e| e.to_string())?;

        // Клавиши, защита, разрешения, загрузки, значок, полный экран.
        let e = emit.clone();
        keys::install(&wv, self.policy.keys.clone(), move |ev| e(PageEvent::Key(ev)));
        let blocks = self.policy.shield.clone();
        let e = emit.clone();
        shield::install(&wv, move |page| blocks(page), move |kind| e(PageEvent::Blocked(kind)));
        let decide = self.policy.permission.clone();
        let e = emit.clone();
        self.requests.install(
            &wv,
            id,
            move |origin, kind| decide(origin, kind),
            move |ask| e(PageEvent::PermissionAsk { ask: ask.id, origin: ask.origin, kind: ask.kind }),
        );
        let e = emit.clone();
        hooks::watch_downloads(&wv, self.downloads.clone(), move |p| e(PageEvent::DownloadProgress(p)));
        let e = emit.clone();
        hooks::watch_favicon(&wv, move |png| e(PageEvent::Favicon(png)));
        let e = emit;
        hooks::watch_fullscreen(&wv, move |on| e(PageEvent::Fullscreen(on)));
        hooks::set_download_folder(&wv, spec.download_dir);
        // Новые WebView ложатся поверх — интерфейс должен остаться выше.
        self.raise_ui();
        Ok(Box::new(Wv2Page(wv)))
    }

    fn answer_permission(&self, ask: u64, allow: bool) -> Option<(String, &'static str)> {
        self.requests.answer(ask, allow).map(|a| (a.origin, a.kind))
    }

    fn drop_page(&self, id: PageId) {
        self.requests.drop_tab(id);
    }

    fn cancel_download(&self, url: &str) -> bool {
        self.downloads.cancel(url)
    }

    fn forget_download(&self, url: &str) {
        self.downloads.forget(url);
    }
}

/// Страница WebView2.
struct Wv2Page(WebView);

impl PageView for Wv2Page {
    fn url(&self) -> Option<String> {
        self.0.url().ok()
    }
    fn navigate(&self, url: &str) {
        self.0.load_url(url).or_log("load_url");
    }
    fn reload(&self) {
        self.0.reload().or_log("reload");
    }
    fn hard_reload(&self) {
        hooks::hard_reload(&self.0);
    }
    fn eval(&self, js: &str) {
        self.0.evaluate_script(js).or_log("evaluate_script");
    }
    fn eval_with(&self, js: &str, done: Box<dyn FnOnce(String) + Send + 'static>) {
        // wry зовёт обработчик как Fn — отдаём результат один раз.
        let done = Mutex::new(Some(done));
        self.0
            .evaluate_script_with_callback(js, move |json| {
                if let Some(f) = done.lock().ok().and_then(|mut d| d.take()) {
                    f(json);
                }
            })
            .or_log("evaluate_script_with_callback");
    }
    fn set_bounds(&self, b: Bounds) {
        self.0.set_bounds(rect(b)).or_log("set_bounds");
    }
    fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible).or_log("set_visible");
    }
    fn focus(&self) {
        self.0.focus().or_log("focus");
    }
    fn set_theme(&self, theme: Theme) {
        #[cfg(windows)]
        {
            use wry::WebViewExtWindows;
            self.0.set_theme(wry_theme(theme)).or_log("set_theme");
        }
        #[cfg(not(windows))]
        let _ = theme;
    }
    fn set_download_folder(&self, dir: &str) {
        hooks::set_download_folder(&self.0, dir);
    }
    fn open_devtools(&self) {
        self.0.open_devtools();
    }
    fn capture_preview(&self, done: Box<dyn FnOnce(Option<Vec<u8>>) + 'static>) {
        hooks::capture(&self.0, done);
    }
    fn clear_profile_data(&self) -> Result<(), String> {
        self.0.clear_all_browsing_data().map_err(|e| e.to_string())
    }
    fn native_handle(&self) -> isize {
        #[cfg(windows)]
        {
            use wry::WebViewExtWindows;
            self.0.hwnd().0 as isize
        }
        #[cfg(not(windows))]
        0
    }
}
