//! Граница между интерфейсом (ui/*) и ядром браузера: сообщения интерфейса → ядро.
//!
//! Интерфейс шлёт JSON вида `{"type": "activate", "id": 5}` (window.ipc.postMessage),
//! ядро разбирает его в `UiMsg`. Обратно ядро вызывает функции `window.seyro.*`
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

    // ----- Ask Seyro -----
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

    /// Ошибка в скрипте интерфейса — в журнал seyro.log.
    Log {
        msg: String,
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
mod tests {
    use super::*;

    #[test]
    fn parses_known_messages() {
        assert_eq!(UiMsg::parse(r#"{"type":"activate","id":5}"#), Some(UiMsg::Activate { id: 5 }));
        assert_eq!(
            UiMsg::parse(r#"{"type":"cmd","k":"orbit"}"#),
            Some(UiMsg::Cmd { k: crate::command::Command::Orbit })
        );
        assert_eq!(
            UiMsg::parse(r#"{"type":"navigate","text":"rust","mode":"split"}"#),
            Some(UiMsg::Navigate { text: "rust".into(), mode: NavMode::Split })
        );
        assert_eq!(
            UiMsg::parse(r#"{"type":"navigate","text":"rust"}"#),
            Some(UiMsg::Navigate { text: "rust".into(), mode: NavMode::Current })
        );
        assert_eq!(
            UiMsg::parse(r#"{"type":"region","full":false,"rects":[[0,0,10,10]]}"#),
            Some(UiMsg::Region { full: false, rects: vec![[0.0, 0.0, 10.0, 10.0]], holes: vec![] })
        );
        assert_eq!(UiMsg::parse(r#"{"type":"split_swap"}"#), Some(UiMsg::SplitSwap));
        assert_eq!(UiMsg::parse(r#"{"type":"window_drag"}"#), Some(UiMsg::WindowDrag));
        assert_eq!(UiMsg::parse(r#"{"type":"window_menu"}"#), Some(UiMsg::WindowMenu));
        assert_eq!(
            UiMsg::parse(r#"{"type":"setting","key":"sleep_minutes","value":30}"#),
            Some(UiMsg::Setting(crate::settings::SettingChange::SleepMinutes(30)))
        );
        assert_eq!(UiMsg::parse(r#"{"type":"setting","key":"sleep_minutes","value":"30"}"#), None);
        assert_eq!(
            UiMsg::parse(r#"{"type":"permission_answer","id":3,"decision":"always"}"#),
            Some(UiMsg::PermissionAnswer { id: 3, decision: crate::permissions::Answer::Always })
        );
    }

    #[test]
    fn rejects_unknown_or_malformed() {
        for bad in [
            r#"{"type":"nope"}"#,
            r#"{"type":"activate"}"#,
            r#"{"type":"activate","id":"5"}"#,
            r#"{"id":5}"#,
            r#"{"type":"navigate","text":"x","mode":"weird"}"#,
            // Старый формат: сайт в ответе интерфейса больше не принимается.
            r#"{"type":"permission_answer","id":3,"decision":"always|https://evil.test|camera"}"#,
            "not json",
            "null",
        ] {
            assert_eq!(UiMsg::parse(bad), None, "{bad}");
        }
    }
}
