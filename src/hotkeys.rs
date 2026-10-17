//! Горячие клавиши браузера во вкладках и в интерфейсе.
//!
//! Ловятся событием WebView2 `AcceleratorKeyPressed` — до того, как нажатие
//! попадёт на страницу. Поэтому они работают на любой странице (в том числе
//! about:blank, куда не встраивается tab.js), а страница не может их ни
//! перехватить, ни подделать. Какое сочетание что делает — решает keymap.rs.

use crate::command::Command;
use crate::keymap::{Combo, Keymap};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Общие для всех WebView: сочетания и режим записи нового сочетания
/// (Settings → Shortcuts: следующее нажатие не выполняется, а отдаётся интерфейсу).
#[derive(Default)]
pub struct Keys {
    pub map: RefCell<Keymap>,
    pub recording: Cell<bool>,
}

/// Что получил обработчик: команду или (в режиме записи) нажатое сочетание.
pub enum KeyEvent {
    Command(Command),
    Recorded(String),
}

/// Решить, что делать с нажатием. None — пропустить на страницу.
pub fn decide(keys: &Keys, vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<KeyEvent> {
    if keys.recording.get() {
        let c = Combo { ctrl, shift, alt, vk };
        // Сами модификаторы и не назначаемые сочетания пропускаем — ждём настоящую клавишу.
        let modifier = matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5 | 0x5B | 0x5C);
        return (!modifier && c.assignable()).then(|| c.to_string_opt()).flatten().map(KeyEvent::Recorded);
    }
    keys.map.borrow().lookup(vk, ctrl, shift, alt).map(KeyEvent::Command)
}

/// Подписаться на нажатия в WebView. `on_key` вызывается в главном потоке.
#[cfg(windows)]
pub fn install(wv: &wry::WebView, keys: Rc<Keys>, on_key: impl Fn(KeyEvent) + 'static) {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
        COREWEBVIEW2_PHYSICAL_KEY_STATUS,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_SHIFT};
    use wry::WebViewExtWindows;

    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        // SAFETY: обычные вызовы COM-интерфейса, который WebView2 передал в обработчик.
        unsafe {
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
            args.KeyEventKind(&mut kind)?;
            if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN {
                return Ok(());
            }
            let mut vk = 0u32;
            args.VirtualKey(&mut vk)?;
            let down = |k: VIRTUAL_KEY| GetKeyState(i32::from(k.0)) < 0;
            let Some(ev) = decide(&keys, vk, down(VK_CONTROL), down(VK_SHIFT), down(VK_MENU)) else { return Ok(()) };
            args.SetHandled(true)?;
            let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
            args.PhysicalKeyStatus(&mut status)?;
            let repeat = status.WasKeyDown.as_bool();
            match ev {
                KeyEvent::Command(cmd) if repeat && !cmd.repeats() => {}
                KeyEvent::Recorded(_) if repeat => {}
                ev => on_key(ev),
            }
        }
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: регистрация обработчика на контроллере живого WebView.
    if let Err(e) = unsafe { wv.controller().add_AcceleratorKeyPressed(&handler, &mut token) } {
        log::error!("не удалось подключить горячие клавиши: {e}");
    }
}

#[cfg(not(windows))]
pub fn install(_wv: &wry::WebView, _keys: Rc<Keys>, _on_key: impl Fn(KeyEvent) + 'static) {}
/// Положить текст в буфер обмена Windows.
#[cfg(windows)]
pub fn copy_to_clipboard(text: &str) -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: стандартная последовательность Win32: память выделяется GlobalAlloc,
    // заполняется под GlobalLock и передаётся буферу обмена, который ею владеет дальше.
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let result = (|| -> windows::core::Result<()> {
            EmptyClipboard()?;
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2)?;
            let ptr = GlobalLock(mem).cast::<u16>();
            if ptr.is_null() {
                return Err(windows::core::Error::from_thread());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(mem);
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0)))?;
            Ok(())
        })();
        let _ = CloseClipboard();
        if let Err(e) = &result {
            log::warn!("не удалось скопировать в буфер обмена: {e}");
        }
        result.is_ok()
    }
}

#[cfg(not(windows))]
pub fn copy_to_clipboard(_text: &str) -> bool {
    false
}

#[cfg(test)]
#[path = "../tests/unit/hotkeys.rs"]
mod tests;
