//! Engine Abstraction: граница между логикой браузера и движком, который показывает
//! страницы.
//!
//! Ядро Jiyda (страницы, пространства, защита, разрешения, клавиши, загрузки) работает
//! только с этим модулем: создаёт страницы через [`Engine`], управляет ими через
//! [`PageView`] и получает от них [`PageEvent`]. Какой движок внутри — ядро не знает.
//!
//! Сейчас движок один — Microsoft WebView2 ([`webview2`]): `wry` и WebView2 COM видны
//! только в `src/engine/webview2/`; это проверяет тест (tests/unit/engine.rs). Другой
//! движок (например, CEF) — это ещё одна реализация [`Engine`] и [`PageView`] рядом,
//! без изменений в ядре.

use crate::hotkeys::KeyEvent;
use crate::permissions::Decision;
use crate::shield::Kind;
use std::any::Any;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub mod webview2;

/// Номер страницы, как у ядра (`page_list`).
pub type PageId = u64;

/// Прямоугольник в логических пикселях окна.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Bounds {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// Тема для страниц (CSS `prefers-color-scheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    /// Из настройки Jiyda: "light", "dark", остальное — как в системе.
    pub fn from_setting(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

/// Ход загрузки файла: адрес, сколько получено и сколько всего (0 — неизвестно).
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadProgress {
    pub url: String,
    pub received: u64,
    pub total: u64,
}

/// Что движок сообщает ядру о странице (или об интерфейсе — тогда без номера страницы).
#[derive(Debug)]
pub enum PageEvent {
    /// Сообщение из скрипта страницы (tab.js) или интерфейса (`window.ipc.postMessage`).
    Message(String),
    Title(String),
    Load {
        started: bool,
        url: String,
    },
    /// Сайт открывает окно без размера (`window.open`, target=_blank) — открыть страницей.
    Popup(String),
    DownloadStarted {
        url: String,
        path: String,
    },
    DownloadFinished {
        url: String,
        path: Option<String>,
        ok: bool,
    },
    DownloadProgress(DownloadProgress),
    /// Значок сайта, PNG.
    Favicon(Vec<u8>),
    /// Элемент страницы (видео) развернулся на весь экран или вернулся.
    Fullscreen(bool),
    /// Защита заблокировала запрос трекера или рекламы.
    Blocked(Kind),
    /// Сайт просит разрешение, запомненного решения нет. Ответ — [`Engine::answer_permission`].
    PermissionAsk {
        ask: u64,
        origin: String,
        kind: &'static str,
    },
    /// Сочетание клавиш Jiyda (до того, как нажатие дойдёт до страницы).
    Key(KeyEvent),
}

/// Куда движок отдаёт события. `None` — событие интерфейса, а не страницы.
/// Может вызываться не из главного потока — реализация сама доставляет его в цикл событий.
pub type Sink = Arc<dyn Fn(Option<PageId>, PageEvent) + Send + Sync>;

/// Запомненное решение о разрешении: (источник, вид) → разрешить / запретить.
pub type PermissionFn = Rc<dyn Fn(&str, &str) -> Option<Decision>>;

/// Файл интерфейса по пути: байты и MIME-тип; None — 404.
pub type AssetFn = fn(&str) -> Option<(&'static [u8], &'static str)>;

/// Решения ядра, которые движку нужны прямо во время работы страницы.
pub struct Policy {
    /// Сочетания клавиш и режим записи нового сочетания.
    pub keys: Rc<crate::hotkeys::Keys>,
    /// Блокировать ли трекеры на странице с этим адресом (защита и исключения сайтов).
    pub shield: Rc<dyn Fn(&str) -> bool>,
    /// Запомненное решение о разрешении: (источник, вид) → разрешить / запретить.
    pub permission: PermissionFn,
}

/// Настройки, общие для всех страниц (WebView2 требует, чтобы они совпадали у всех
/// страниц одного профиля).
#[derive(Debug, Clone)]
pub struct Options {
    pub devtools: bool,
    pub extensions: bool,
    pub extensions_dir: PathBuf,
}

/// Профиль браузера: куки, логины, кэш (у каждого пространства свой). Что внутри —
/// дело движка; ядро только хранит его и передаёт при создании страниц.
pub struct Profile(Box<dyn Any>);

impl Profile {
    pub fn new(inner: impl Any) -> Self {
        Self(Box::new(inner))
    }
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.0.downcast_mut()
    }
}

/// Что нужно, чтобы создать страницу.
pub struct PageSpec<'a> {
    pub id: PageId,
    pub url: &'a str,
    pub bounds: Bounds,
    pub visible: bool,
    pub theme: Theme,
    /// Тёмная ли тема сейчас (для подложки до загрузки страницы).
    pub dark: bool,
    /// Скрипт, который выполняется в каждом документе страницы до её собственных.
    pub init_script: String,
    pub download_dir: &'a str,
}

/// Интерфейс браузера — своя страница поверх страниц сайтов.
pub struct UiSpec {
    /// Адрес интерфейса (`<scheme>://ui/index.html`).
    pub url: String,
    /// Протокол, по которому интерфейс берёт свои файлы.
    pub scheme: &'static str,
    /// Файл интерфейса по пути: байты и MIME-тип; None — 404.
    pub asset: AssetFn,
    /// Content-Security-Policy страницы интерфейса.
    pub csp: &'static str,
    /// Куда интерфейсу можно переходить (остальное — открыть страницей, см. `on_leave`).
    pub is_own_url: fn(&str) -> bool,
    /// Интерфейс пытается уйти на сайт — открыть его страницей.
    pub on_leave: Box<dyn Fn(String) + 'static>,
    pub init_script: String,
    pub bounds: Bounds,
    pub theme: Theme,
    pub dark: bool,
}

/// Окно, в которое движок встраивает страницы.
pub type Window = tao::window::Window;

/// Показанная страница (или интерфейс).
pub trait PageView {
    /// Текущий адрес документа (после переходов внутри страницы тоже); None — неизвестен.
    fn url(&self) -> Option<String>;
    fn navigate(&self, url: &str);
    fn reload(&self);
    /// Обновить без кэша.
    fn hard_reload(&self);
    fn eval(&self, js: &str);
    /// Выполнить скрипт и получить результат как JSON (ответ может прийти из другого потока).
    fn eval_with(&self, js: &str, done: Box<dyn FnOnce(String) + Send + 'static>);
    fn set_bounds(&self, b: Bounds);
    fn set_visible(&self, visible: bool);
    fn focus(&self);
    fn set_theme(&self, theme: Theme);
    fn set_download_folder(&self, dir: &str);
    fn open_devtools(&self);
    /// Снимок видимой части страницы (JPEG) для Orbit; None — не вышло.
    fn capture_preview(&self, done: Box<dyn FnOnce(Option<Vec<u8>>) + 'static>);
    /// Стереть данные сайтов профиля этой страницы (куки, кэш, хранилища).
    fn clear_profile_data(&self) -> Result<(), String>;
    /// Системное окно страницы (HWND) — для региона и порядка окон (frame.rs).
    fn native_handle(&self) -> isize;
}

/// Страница, которой владеет ядро.
pub type Page = Box<dyn PageView>;

/// Движок: создаёт профили, страницы и интерфейс; держит то, что живёт дольше одной
/// страницы (ждущие ответа запросы разрешений, идущие загрузки).
pub trait Engine {
    fn profile(&self, dir: PathBuf) -> Profile;
    fn create_ui(&self, window: &Window, profile: &mut Profile, spec: UiSpec) -> Result<Page, String>;
    fn create_page(&self, window: &Window, profile: &mut Profile, spec: PageSpec) -> Result<Page, String>;
    /// Ответ на [`PageEvent::PermissionAsk`]: вернуть (источник, вид), чтобы ядро могло
    /// запомнить решение; None — запроса уже нет.
    fn answer_permission(&self, ask: u64, allow: bool) -> Option<(String, &'static str)>;
    /// Страницу закрыли или увели со страницы: её запросы разрешений отклоняются.
    fn drop_page(&self, id: PageId);
    /// Отменить идущую загрузку. false — уже не отменить.
    fn cancel_download(&self, url: &str) -> bool;
    /// Загрузка закончилась — больше не следить.
    fn forget_download(&self, url: &str);
}

/// Движок по умолчанию.
pub fn default_engine(options: Options, policy: Policy, sink: Sink) -> Box<dyn Engine> {
    Box::new(webview2::WebView2Engine::new(options, policy, sink))
}

#[cfg(test)]
#[path = "../../tests/unit/engine.rs"]
mod tests;
