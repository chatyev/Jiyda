//! Граница между интерфейсом (ui/*) и ядром браузера: сообщения интерфейса → ядро.
//!
//! Интерфейс шлёт JSON вида `{"type": "activate", "id": 5}` (window.ipc.postMessage),
//! ядро разбирает его в `UiMsg`. Обратно ядро вызывает функции `window.jiyda.*`
//! (render, command, data, toast, preview, …) — см. `App::push_state` и ui/js/core.js.
//! Неизвестные и неполные сообщения отбрасываются: интерфейс не может сломать ядро.

use serde::Deserialize;

/// Где открыть введённый адрес или запрос.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NavMode {
    /// В текущей странице (адресная строка).
    #[default]
    Current,
    /// Новой страницей.
    New,
    /// Новой страницей рядом с текущей (Split View).
    Split,
    /// Новой страницей, не переходя на неё.
    Background,
}

/// Прямоугольник интерфейса в логических пикселях: x, y, ширина, высота.
pub type Rect4 = [f64; 4];

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiMsg {
    /// Интерфейс загрузился и ждёт состояние.
    Ready,
    /// Команда браузера — те же имена, что у горячих клавиш (keymap.rs).
    Cmd {
        k: crate::command::Command,
    },

    // ----- Страницы -----
    Activate {
        id: u64,
    },
    Close {
        id: u64,
    },
    /// Move to Later.
    Later {
        id: u64,
    },
    /// Undo для Move to Later: вернуть в Now (и на экран, если была активной).
    LaterUndo {
        id: u64,
        activate: bool,
    },
    Pin {
        id: u64,
        on: bool,
    },
    Duplicate {
        id: u64,
    },
    ResetPinned {
        id: u64,
    },
    MoveToSpace {
        id: u64,
        space: usize,
    },
    Navigate {
        text: String,
        #[serde(default)]
        mode: NavMode,
    },
    /// Esc на New Page — вернуться на страницу, с которой ушли.
    HomeCancel,

    // ----- Split View -----
    /// Открыть страницу `id` рядом с текущей; `side` — "left" или "right" (по умолчанию справа).
    Split {
        id: u64,
        #[serde(default)]
        side: Option<String>,
    },
    Unsplit {
        #[serde(default)]
        id: Option<u64>,
    },
    SplitSwap,
    SplitReplace {
        old: u64,
        new: u64,
    },
    SplitRatio {
        ratio: f64,
        #[serde(default)]
        done: bool,
    },

    // ----- Пространства -----
    Space {
        index: usize,
    },
    SpaceNew,
    SpaceRename {
        index: usize,
        name: String,
    },
    SpaceColor {
        index: usize,
        color: String,
    },
    SpaceDelete {
        index: usize,
    },

    // ----- Закрытые и история -----
    RestoreClosed {
        index: usize,
    },
    RestoreAllClosed,
    ClearClosed,
    HistoryRemove {
        url: String,
    },
    HistoryUndo,
    HistoryContext {
        url: String,
        ts: u64,
    },

    // ----- Окно и интерфейс -----
    /// Где интерфейс должен получать мышь (см. frame::UiRegion).
    Region {
        #[serde(default)]
        full: bool,
        #[serde(default)]
        rects: Vec<Rect4>,
        #[serde(default)]
        holes: Vec<Rect4>,
    },
    /// Открыт слой, которому нужна клавиатура (палитра, Orbit, панель); false — вернуть её странице.
    Modal {
        on: bool,
    },
    /// Цвет верхней панели — под него рисуются кнопки окна.
    FrameColors {
        bg: String,
        fg: String,
    },

    // ----- Поиск по странице -----
    Find {
        text: String,
        #[serde(default)]
        mode: String,
    },
    FindClose,

    // ----- Загрузки -----
    Download {
        id: u64,
        action: String,
    },

    // ----- Данные для панелей и настройки -----
    Get {
        what: String,
    },
    Setting(crate::settings::SettingChange),
    KeysRecord {
        on: bool,
    },
    PermissionAnswer {
        id: u64,
        decision: crate::permissions::Answer,
    },
    SiteShield {
        host: String,
        on: bool,
    },
    PermissionRevoke {
        origin: String,
        kind: String,
    },
    /// Свежий снимок страницы для Orbit.
    Capture {
        id: u64,
    },

    // ----- Ask Jiyda -----
    Ask {
        id: u64,
        prompt: String,
        #[serde(default)]
        context: String,
        #[serde(default)]
        pages: Vec<u64>,
        #[serde(default)]
        selection: String,
    },
    AiKey {
        key: String,
    },

    // ----- Peek -----
    PeekOpen,
    PeekClose,
    PeekBox {
        rect: Rect4,
    },

    /// Интерфейс потерял фокус: ядро решает, ушёл ли пользователь (тогда закрыть панели).
    UiBlur,

    /// Нажатие на пустое место верхней панели: перетаскивать окно (с прилипанием к краям).
    WindowDrag,

    /// Правый щелчок по пустому месту верхней панели: системное меню окна у курсора.
    WindowMenu,

    /// Ошибка в скрипте интерфейса — в журнал jiyda.log.
    Log {
        msg: String,
    },

    /// Только для тестов E2E (отладочная сборка с JIYDA_AUTOMATION): выполнить `code` в
    /// странице `id` (0 — активная) и вернуть результат интерфейсу как data:e2e {req, json}.
    #[cfg(debug_assertions)]
    E2eEval {
        req: u64,
        id: u64,
        code: String,
    },

    #[serde(other)]
    Unknown,
}

impl UiMsg {
    pub fn parse(body: &str) -> Option<UiMsg> {
        match serde_json::from_str(body) {
            Ok(UiMsg::Unknown) => None,
            Ok(m) => Some(m),
            Err(e) => {
                log::debug!("сообщение интерфейса не разобрано ({e}): {body:.200}");
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/ipc.rs"]
mod tests;
