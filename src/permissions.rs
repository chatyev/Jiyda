//! Разрешения сайтов: камера, микрофон, местоположение, уведомления…
//!
//! Запрос сайта WebView2 передаёт в `PermissionRequested`; Jiyda откладывает ответ
//! и показывает свою подсказку под адресной строкой: «Allow once» (только сейчас),
//! «Always allow» и «Block» (запоминаются в permissions.json). WebView2 решения
//! сам не хранит — всё в одном месте, его видно и можно отозвать в настройках.

#[cfg(test)]
use crate::config::write_atomic;
use std::collections::BTreeMap;
use std::path::Path;

/// Запомненные решения: источник (https://meet.google.com) → вид → allow / block.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Store {
    pub sites: BTreeMap<String, BTreeMap<String, Decision>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Block,
}

/// Источник адреса: схема, хост и нестандартный порт. None — не веб-адрес.
pub fn origin_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    Some(u.origin().ascii_serialization())
}

impl Store {
    pub fn load(path: &Path) -> Self {
        let sites = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { sites }
    }

    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&self, path: &Path) {
        if let Some(s) = self.to_json() {
            write_atomic(path, &s);
        }
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(&self.sites).ok()
    }

    pub fn get(&self, origin: &str, kind: &str) -> Option<Decision> {
        self.sites.get(origin).and_then(|k| k.get(kind)).copied()
    }

    pub fn set(&mut self, origin: &str, kind: &str, d: Decision) {
        self.sites.entry(origin.to_string()).or_default().insert(kind.to_string(), d);
    }

    pub fn revoke(&mut self, origin: &str, kind: &str) -> bool {
        let Some(kinds) = self.sites.get_mut(origin) else { return false };
        let removed = kinds.remove(kind).is_some();
        if kinds.is_empty() {
            self.sites.remove(origin);
        }
        removed
    }

    /// Сколько разрешений сайту выдано навсегда (Security panel: «2 allowed»).
    pub fn allowed_count(&self, origin: &str) -> usize {
        self.sites.get(origin).map_or(0, |k| k.values().filter(|d| **d == Decision::Allow).count())
    }

    /// Для интерфейса (Settings).
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.sites).unwrap_or_default()
    }
}

/// Ответ пользователя на запрос разрешения (из подсказки интерфейса). Сайт и вид
/// разрешения интерфейс не присылает — их помнит ядро вместе с запросом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Разрешить только сейчас, ничего не запоминать.
    Once,
    /// Разрешать всегда.
    Always,
    /// Запретить и запомнить.
    Block,
    /// Закрыли без ответа: отказ только на этот раз.
    Dismiss,
}

impl Answer {
    pub fn allows(self) -> bool {
        matches!(self, Answer::Once | Answer::Always)
    }

    /// Что запомнить для сайта. None — ничего (разовый ответ).
    pub fn remembered(self) -> Option<Decision> {
        match self {
            Answer::Always => Some(Decision::Allow),
            Answer::Block => Some(Decision::Block),
            Answer::Once | Answer::Dismiss => None,
        }
    }
}

/// Запрос, ждущий ответа пользователя.
#[derive(Debug, Clone)]
pub struct Ask {
    pub id: u64,
    pub origin: String,
    pub kind: &'static str,
}

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use super::{Ask, Decision, origin_of};
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::{PermissionRequestedEventHandler, take_pwstr};
    use windows::core::{Interface, PWSTR};
    use wry::WebViewExtWindows;

    struct Pending {
        args: ICoreWebView2PermissionRequestedEventArgs,
        deferral: ICoreWebView2Deferral,
        tab: u64,
        ask: Ask,
    }

    /// Запросы разрешений, ждущие ответа пользователя. Один на браузер, принадлежит
    /// ядру (`App`); обработчики WebView держат на него `Rc`.
    #[derive(Default)]
    pub struct Requests {
        pending: RefCell<HashMap<u64, Pending>>,
        next: Cell<u64>,
    }

    fn kind_name(k: COREWEBVIEW2_PERMISSION_KIND) -> Option<&'static str> {
        Some(match k {
            COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => "microphone",
            COREWEBVIEW2_PERMISSION_KIND_CAMERA => "camera",
            COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => "location",
            COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => "notifications",
            COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS => "sensors",
            COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ => "clipboard",
            COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS => "downloads",
            COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE => "files",
            COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS => "fonts",
            COREWEBVIEW2_PERMISSION_KIND_WINDOW_MANAGEMENT => "windows",
            COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES => "midi",
            _ => return None,
        })
    }

    unsafe fn finish(args: &ICoreWebView2PermissionRequestedEventArgs, state: COREWEBVIEW2_PERMISSION_STATE) {
        unsafe {
            let _ = args.SetState(state);
            // Решения хранит Jiyda, а не профиль WebView2.
            if let Ok(a3) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
                let _ = a3.SetSavesInProfile(false);
            }
        }
    }

    impl Requests {
        /// Подключить запросы разрешений вкладки `tab`. `decide(источник, вид)` — запомненное
        /// решение; если его нет — `ask` показывает подсказку, ответ придёт в `answer`.
        pub fn install(
            self: &Rc<Self>,
            wv: &wry::WebView,
            tab: u64,
            decide: impl Fn(&str, &str) -> Option<Decision> + 'static,
            ask: impl Fn(Ask) + 'static,
        ) {
            let requests = Rc::clone(self);
            let handler = PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                // SAFETY: объекты, которые WebView2 передал в обработчик.
                unsafe {
                    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut kind)?;
                    // Автовоспроизведение и неизвестное — на усмотрение WebView2.
                    let Some(kind) = kind_name(kind) else { return Ok(()) };
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let Some(origin) = origin_of(&take_pwstr(uri)) else {
                        finish(&args, COREWEBVIEW2_PERMISSION_STATE_DENY);
                        return Ok(());
                    };
                    match decide(&origin, kind) {
                        Some(Decision::Allow) => finish(&args, COREWEBVIEW2_PERMISSION_STATE_ALLOW),
                        Some(Decision::Block) => finish(&args, COREWEBVIEW2_PERMISSION_STATE_DENY),
                        None => {
                            let deferral = args.GetDeferral()?;
                            let id = requests.next.get() + 1;
                            requests.next.set(id);
                            let request = Ask { id, origin, kind };
                            let pending = Pending { args: args.clone(), deferral, tab, ask: request.clone() };
                            requests.pending.borrow_mut().insert(id, pending);
                            ask(request);
                        }
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            // SAFETY: подписка на событие живого WebView.
            if let Err(e) = unsafe { wv.webview().add_PermissionRequested(&handler, &mut token) } {
                log::warn!("запросы разрешений не подключены: {e}");
            }
        }

        /// Ответить на запрос: true — разрешить. Вернуть сам запрос (сайт и вид) —
        /// чтобы запомнить решение; None — такого запроса уже нет.
        pub fn answer(&self, id: u64, allow: bool) -> Option<Ask> {
            let p = self.pending.borrow_mut().remove(&id)?;
            // SAFETY: отложенный ответ на живой запрос.
            unsafe {
                finish(
                    &p.args,
                    if allow { COREWEBVIEW2_PERMISSION_STATE_ALLOW } else { COREWEBVIEW2_PERMISSION_STATE_DENY },
                );
                let _ = p.deferral.Complete();
            }
            Some(p.ask)
        }

        /// Вкладку закрыли или увели со страницы — её запросы отклоняются. Вернуть их id.
        pub fn drop_tab(&self, tab: u64) -> Vec<u64> {
            let ids: Vec<u64> = self.pending.borrow().iter().filter(|(_, v)| v.tab == tab).map(|(k, _)| *k).collect();
            for id in &ids {
                self.answer(*id, false);
            }
            ids
        }
    }
}

#[cfg(not(windows))]
#[derive(Default)]
pub struct Requests;

#[cfg(not(windows))]
impl Requests {
    pub fn install(
        self: &std::rc::Rc<Self>,
        _wv: &wry::WebView,
        _tab: u64,
        _decide: impl Fn(&str, &str) -> Option<Decision> + 'static,
        _ask: impl Fn(Ask) + 'static,
    ) {
    }
    pub fn answer(&self, _id: u64, _allow: bool) -> Option<Ask> {
        None
    }
    pub fn drop_tab(&self, _tab: u64) -> Vec<u64> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        assert_eq!(origin_of("https://meet.google.com/abc-def?x=1").as_deref(), Some("https://meet.google.com"));
        assert_eq!(origin_of("http://localhost:3000/a").as_deref(), Some("http://localhost:3000"));
        assert_eq!(origin_of("about:blank"), None);
        assert_eq!(origin_of("file:///C:/a.html"), None);
    }

    #[test]
    fn set_get_revoke_count() {
        let mut s = Store::default();
        s.set("https://a.test", "camera", Decision::Allow);
        s.set("https://a.test", "microphone", Decision::Allow);
        s.set("https://a.test", "location", Decision::Block);
        assert_eq!(s.get("https://a.test", "camera"), Some(Decision::Allow));
        assert_eq!(s.get("https://b.test", "camera"), None);
        assert_eq!(s.allowed_count("https://a.test"), 2);
        assert!(s.revoke("https://a.test", "camera"));
        assert!(!s.revoke("https://a.test", "camera"));
        assert!(s.revoke("https://a.test", "microphone"));
        assert!(s.revoke("https://a.test", "location"));
        assert!(s.sites.is_empty());
    }

    #[test]
    fn answers_allow_and_remember() {
        assert!(Answer::Once.allows() && Answer::Always.allows());
        assert!(!Answer::Block.allows() && !Answer::Dismiss.allows());
        assert_eq!(Answer::Always.remembered(), Some(Decision::Allow));
        assert_eq!(Answer::Block.remembered(), Some(Decision::Block));
        assert_eq!(Answer::Once.remembered(), None);
        assert_eq!(Answer::Dismiss.remembered(), None);
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("permissions.json");
        let mut s = Store::default();
        s.set("https://a.test", "notifications", Decision::Block);
        s.save(&p);
        assert_eq!(Store::load(&p), s);
        assert_eq!(Store::load(&dir.path().join("none.json")), Store::default());
    }
}
