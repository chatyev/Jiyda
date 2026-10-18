//! Ядро браузера: окно, страницы, раскладка, обработка команд.
//!
//! Окно устроено слоями: внизу — страницы (активная или две половины Split View), над
//! ними — прозрачный интерфейс (ui/*), обрезанный регионом (frame.rs): он получает мышь
//! только там, где что-то рисует. Интерфейс и ядро общаются сообщениями — см. ipc.rs.
//! Страницы показывает движок (src/engine): ядро не знает, что это WebView2.

use crate::ai;
use crate::command::Command;
use crate::config::{Config, Paths};
use crate::engine::{self, Bounds, Engine, PageEvent, PageId, Profile};
use crate::frame::{self, CHROME_H, UiRegion};
use crate::hotkeys::{self, KeyEvent, Keys};
use crate::i18n;
use crate::input;
use crate::instance;
use crate::ipc::{NavMode, UiMsg};
use crate::keymap::{Combo, Keymap};
use crate::limit::RateLimit;
use crate::logging::OrLog;
use crate::orbit;
use crate::page_list::{AfterClose, MruWalk, Page, PageList};
use crate::permissions;
use crate::settings::SettingChange;
use crate::shield;
use crate::spaces;
use crate::store::{DownloadState, Downloads, Favicons, HistEntry, History, SavedSpace, Session, now};
use crate::update;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::window::{Fullscreen, Window, WindowBuilder};

// Методы `App` по темам. Структура и общие помощники — здесь, в mod.rs.
mod ask;
mod commands;
mod data;
mod events;
mod home;
mod init;
mod layout;
mod pages;
mod peek;
mod prefs;
mod session;
mod space_ops;
mod split;
mod ui_assets;
mod ui_bridge;
mod updates;

const TAB_JS: &str = include_str!("../tab.js");
const FIND_JS: &str = include_str!("../find.js");
/// Знак Jiyda для интерфейса (передаётся как data: URI).
const LOGO_PNG: &[u8] = include_bytes!("../../assets/jiyda-mark-128.png");

/// Сессия и история пишутся на диск не сразу, а через эту паузу после изменения.
const SAVE_DELAY: Duration = Duration::from_secs(2);
/// Как часто проверять, не пора ли усыпить фоновые страницы.
const SLEEP_CHECK: Duration = Duration::from_secs(60);
/// Сколько страниц сайты могут открыть (window.open, Ctrl/средний клик) за окно времени.
const PAGE_TABS_MAX: usize = 10;
const PAGE_TABS_WINDOW: Duration = Duration::from_secs(5);
/// Промежуток между половинами Split View, в нём разделитель (`--split-gap` в ui/styles.css).
const SPLIT_GAP: f64 = 6.0;
/// Строка над половинами Split View: значок, сайт, действия (`--pane-h`).
const PANE_H: f64 = 30.0;
/// Первая проверка обновлений — чуть позже запуска, чтобы не мешать загрузке страниц.
const UPDATE_FIRST_CHECK: Duration = Duration::from_secs(10);
const UPDATE_CHECK_EVERY: Duration = Duration::from_secs(12 * 3600);
/// Плитки New Page.
const HOME_TILES: usize = 6;
/// Ctrl+Tab, нажатый вскоре после предыдущего, идёт дальше по списку последних страниц.
const MRU_WALK: Duration = Duration::from_millis(1500);
/// Снимок страницы для Orbit — через столько после загрузки.
const CAPTURE_AFTER_LOAD: Duration = Duration::from_millis(1200);

type Box4 = (f64, f64, f64, f64);

/// Разделить область страницы (x, y, ширина, высота) на две половины: левая
/// занимает долю `ratio` ширины без промежутка. Та же формула — в ui/js/split.js.
fn split_boxes((x, y, w, h): Box4, ratio: f64) -> [Box4; 2] {
    let inner = (w - SPLIT_GAP).max(0.0);
    let lw = (inner * ratio).round();
    [(x, y, lw, h), (x + lw + SPLIT_GAP, y, inner - lw, h)]
}

/// События, которые приходят в главный поток из обработчиков WebView и потоков.
#[derive(Debug)]
pub enum UserEvent {
    /// Будильник цикла событий (main.rs, Waker): пора выполнить отложенные дела в tick().
    Wake,
    FromUi(String),
    /// Ответ страницы на UiMsg::E2eEval (только тесты E2E в отладочной сборке).
    #[cfg(debug_assertions)]
    E2eResult {
        req: u64,
        json: String,
    },
    FromTab {
        id: u64,
        msg: String,
    },
    Title {
        id: u64,
        title: String,
    },
    Load {
        id: u64,
        started: bool,
        url: String,
    },
    OpenTab {
        url: String,
        background: bool,
    },
    /// window.open со страницы — в отличие от OpenTab, ограничивается по частоте.
    Popup {
        id: u64,
        url: String,
    },
    DownloadStarted {
        url: String,
        path: String,
    },
    /// Загрузка закончилась; `id` — страница, которая её начала.
    Download {
        id: u64,
        url: String,
        path: Option<String>,
        ok: bool,
    },
    DownloadProgress(engine::DownloadProgress),
    /// Результат поиска по странице (JSON от скрипта в странице).
    FindResult {
        id: u64,
        json: String,
    },
    /// Горячая клавиша в странице (`Some(id)`) или в интерфейсе (`None`), см. hotkeys.rs.
    Key {
        id: Option<u64>,
        cmd: Command,
    },
    /// Settings → Shortcuts: нажатое сочетание для записи.
    KeyRecorded(String),
    /// Адреса из командной строки или от нового запуска jiyda.exe (см. instance.rs).
    External {
        urls: Vec<String>,
    },
    UpdateChecked {
        result: Result<Option<update::Release>, update::CheckError>,
        manual: bool,
    },
    UpdateDownloaded {
        result: Result<std::path::PathBuf, String>,
    },
    /// Снимок страницы для Orbit.
    Preview {
        id: u64,
        jpeg: Vec<u8>,
    },
    /// Значок сайта (PNG) от самого WebView.
    Favicon {
        id: u64,
        png: Vec<u8>,
    },
    /// Защита заблокировала запрос страницы.
    Blocked {
        id: u64,
        kind: shield::Kind,
    },
    /// Сайт просит разрешение — показать подсказку.
    PermissionAsk {
        tab: u64,
        id: u64,
        origin: String,
        kind: &'static str,
    },
    /// Страница развернула элемент на весь экран (видео) или вернула обратно.
    PageFullscreen {
        id: u64,
        on: bool,
    },
    /// Текст страницы для Ask Jiyda.
    PageText {
        req: u64,
        id: u64,
        text: String,
    },
    AiAnswer {
        req: u64,
        result: Result<String, String>,
    },
}

/// Страница (или интерфейс), которую показывает движок.
type View = engine::Page;

/// Событие движка → событие ядра. Одно место, где события страниц превращаются в
/// UserEvent; None — событию нужна страница, а пришло от интерфейса (такого не бывает).
fn page_event(id: Option<PageId>, ev: PageEvent) -> Option<UserEvent> {
    Some(match (id, ev) {
        (None, PageEvent::Message(body)) => UserEvent::FromUi(body),
        (Some(id), PageEvent::Message(msg)) => UserEvent::FromTab { id, msg },
        (Some(id), PageEvent::Title(title)) => UserEvent::Title { id, title },
        (Some(id), PageEvent::Load { started, url }) => UserEvent::Load { id, started, url },
        (Some(id), PageEvent::Popup(url)) => UserEvent::Popup { id, url },
        (_, PageEvent::DownloadStarted { url, path }) => UserEvent::DownloadStarted { url, path },
        (Some(id), PageEvent::DownloadFinished { url, path, ok }) => UserEvent::Download { id, url, path, ok },
        (_, PageEvent::DownloadProgress(p)) => UserEvent::DownloadProgress(p),
        (Some(id), PageEvent::Favicon(png)) => UserEvent::Favicon { id, png },
        (Some(id), PageEvent::Fullscreen(on)) => UserEvent::PageFullscreen { id, on },
        (Some(id), PageEvent::Blocked(kind)) => UserEvent::Blocked { id, kind },
        (Some(tab), PageEvent::PermissionAsk { ask, origin, kind }) => {
            UserEvent::PermissionAsk { tab, id: ask, origin, kind }
        }
        (id, PageEvent::Key(KeyEvent::Command(cmd))) => UserEvent::Key { id, cmd },
        (_, PageEvent::Key(KeyEvent::Recorded(combo))) => UserEvent::KeyRecorded(combo),
        (None, _) => return None,
    })
}

/// Base64 для data: URI (своя функция, чтобы не тянуть зависимость ради картинок).
fn base64(data: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= chunk.len() { ABC[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

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

/// Пространство: свой профиль браузера (куки, логины, кэш) и свои страницы.
struct Space {
    id: u32,
    name: String,
    color: String,
    /// Страницы. У активного пространства здесь пустой список: его страницы — в `App::tabs`.
    /// Стоит раньше `ctx`, чтобы при удалении пространства страницы закрылись до профиля.
    pages: PageList<View>,
    ctx: Profile,
}

/// Защита, общая для обработчиков всех WebView.
#[derive(Default)]
struct ShieldState {
    enabled: bool,
    allow: Vec<String>,
}

impl ShieldState {
    fn blocks_on(&self, page_url: &str) -> bool {
        self.enabled && shield::site_of(page_url).is_none_or(|s| !self.allow.contains(&s))
    }
}

/// Peek: предпросмотр ссылки, которого нет в Orbit, пока с ним не начали работать.
struct Peek {
    /// Номер, под которым страница станет настоящей (выделен заранее).
    id: u64,
    url: String,
    view: View,
    rect: Box4,
}

/// Автообновление: найденная версия и что с ней происходит.
struct Updates {
    /// Когда снова спросить GitHub.
    next_check: Instant,
    /// Найденная новая версия.
    release: Option<update::Release>,
    /// Версия, которую пользователь скрыл, — до следующего запуска не показываем.
    dismissed: Option<String>,
    /// Идёт проверка или скачивание — второй раз не начинаем.
    busy: bool,
    /// Скачивается установщик.
    downloading: bool,
}

impl Updates {
    fn new() -> Self {
        Self {
            next_check: Instant::now() + UPDATE_FIRST_CHECK,
            release: None,
            dismissed: None,
            busy: false,
            downloading: false,
        }
    }

    /// Версия для показа: найденная и не скрытая.
    fn visible(&self) -> Option<&update::Release> {
        self.release.as_ref().filter(|r| self.dismissed.as_deref() != Some(r.version.as_str()))
    }
}

/// Вопрос к Ask Jiyda, для которого собираются тексты страниц.
struct AiJob {
    prompt: String,
    selection: String,
    want: usize,
    pages: Vec<(String, String, String)>,
}

pub struct App {
    /// Запись файлов в фоне.
    writer: crate::config::Writer,
    /// Метка «Jiyda работает»: снимается при выходе (recovery.rs).
    run_marker: crate::recovery::RunMarker,
    /// Прошлый запуск оборвался: страница, что была на экране, — её откроет «Открыть
    /// снова» на New Page. None — открывать нечего или уже открыли.
    recovery: Option<u64>,
    /// Кнопки окна, которые рисует само окно (нет — системная рамка). Стоит раньше
    /// `window`: подкласс снимается до того, как окно закроется.
    frame: Option<frame::Frame>,
    window: Window,
    /// Интерфейс — тоже страница движка (прозрачная, поверх страниц сайтов).
    ui: View,
    proxy: EventLoopProxy<UserEvent>,
    cfg: Config,
    paths: Paths,
    /// Страницы активного пространства.
    pages: PageList<View>,
    spaces: Vec<Space>,
    active_space: usize,
    /// Удалённые пространства, чьи папки данных ещё не стёрты (см. Session::deleted_spaces).
    deleted_spaces: Vec<u32>,
    /// Тема интерфейса: "system", "light" или "dark".
    theme: String,
    find_open: bool,
    history: History,
    downloads: Downloads,
    /// Значки сайтов для мест, где страницы нет (история, New Page, Archive).
    favicons: Favicons,
    /// Когда сбросить сессию и историю на диск (None — изменений нет).
    save_due: Option<Instant>,
    next_sleep_check: Instant,
    page_tabs: RateLimit,
    /// Окно закрывают — main.rs завершит цикл событий.
    exit_requested: bool,
    maximized: bool,
    /// Полноэкранный режим окна (F11).
    fullscreen: bool,
    /// Страница развернула на весь экран свой элемент (видео).
    page_fullscreen: Option<u64>,
    updates: Updates,
    keys: Rc<Keys>,
    shield: Rc<RefCell<ShieldState>>,
    daily: shield::Daily,
    /// Заблокировано на странице с последнего перехода: (трекеры, реклама).
    page_blocks: HashMap<u64, (u64, u64)>,
    perms: Rc<RefCell<permissions::Store>>,
    /// Движок страниц (src/engine): создаёт страницы, держит запросы разрешений и
    /// идущие загрузки.
    engine: Box<dyn Engine>,
    /// Что интерфейс просил оставить ему (регион окна).
    region: UiRegion,
    /// Интерфейсу нужна клавиатура (открыт слой).
    ui_modal: bool,
    /// Состояние изменилось, но ещё не отправлено интерфейсу (см. `push_state`).
    state_dirty: Cell<bool>,
    /// Ctrl+Tab: замороженный список последних страниц, позиция и время нажатия.
    mru_walk: MruWalk,
    /// Куда вернуться, если закрыть New Page клавишей Esc.
    home_return: Option<u64>,
    /// Удалённые из истории записи — для Undo.
    removed_history: Vec<HistEntry>,
    /// Снимки страниц, которые нужно сделать: (страница, когда).
    capture_due: Vec<(u64, Instant)>,
    ai_key: Option<String>,
    ai_jobs: HashMap<u64, AiJob>,
    peek: Option<Peek>,
}

/// Тема окна Windows для настройки темы; None — как в системе.
fn window_theme(theme: &str) -> Option<tao::window::Theme> {
    match theme {
        "light" => Some(tao::window::Theme::Light),
        "dark" => Some(tao::window::Theme::Dark),
        _ => None,
    }
}

/// Тёмная ли тема сейчас: выбранная явно или системная.
fn theme_is_dark(theme: &str, window: &Window) -> bool {
    match theme {
        "light" => false,
        "dark" => true,
        _ => window.theme() == tao::window::Theme::Dark,
    }
}

/// Следующая тема по кругу для команды «Сменить тему».
fn next_theme(theme: &str) -> &'static str {
    match theme {
        "system" => "light",
        "light" => "dark",
        _ => "system",
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds::new(x, y, w.max(1.0), h.max(1.0))
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

/// Страница для интерфейса (Page model: id, title, url, domain, favicon, spaceId, group,
/// state, lastActiveAt, createdAt, parentPageId).
fn page_json<V>(t: &Page<V>, space: u32, group: &str, favicon: &str) -> Value {
    json!({
        "id": t.id,
        "title": t.title,
        "url": t.url,
        "domain": orbit::domain(&t.url),
        "favicon": favicon,
        "spaceId": space,
        "group": group,
        "state": t.state.as_str(),
        "lastActiveAt": t.last_active,
        "createdAt": t.created_at,
        "parentPageId": t.parent,
        "pinned": t.pinned,
        "drifted": t.drifted(),
        "loading": t.loading,
        "media": t.media,
        "asleep": t.view.is_none(),
    })
}

/// Источник для текста, который ушёл со страницы: `JSON.stringify` строки.
fn js_string_result(json: &str) -> String {
    serde_json::from_str::<String>(json).unwrap_or_default()
}

/// Скрипт, который отдаёт видимый текст страницы для Ask Jiyda.
const PAGE_TEXT_JS: &str = "(() => { try { return (document.body ? document.body.innerText : '').slice(0, 400000); } catch (e) { return ''; } })()";

#[cfg(test)]
#[path = "../../tests/unit/app.rs"]
mod tests;
