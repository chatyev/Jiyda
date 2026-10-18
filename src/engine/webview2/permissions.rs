//! Запросы разрешений сайтов в WebView2: событие `PermissionRequested`. Есть запомненное
//! решение (Policy::permission, хранилище — permissions.rs) — отвечаем сразу; нет —
//! откладываем ответ и сообщаем ядру (PageEvent::PermissionAsk), ответ придёт через
//! Engine::answer_permission. WebView2 сам решения не хранит (SetSavesInProfile(false)).

use crate::permissions::{Decision, origin_of};

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
