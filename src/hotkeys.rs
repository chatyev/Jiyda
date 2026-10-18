//! Горячие клавиши браузера во вкладках и в интерфейсе: что делать с нажатием.
//!
//! Нажатия ловит движок до того, как они попадут на страницу (WebView2 —
//! src/engine/webview2/keys.rs), и спрашивает здесь `decide`. Поэтому клавиши работают
//! на любой странице, а страница не может их ни перехватить, ни подделать. Какое
//! сочетание что делает — решает keymap.rs.

use crate::command::Command;
use crate::keymap::{Combo, Keymap};
use std::cell::{Cell, RefCell};

/// Общие для всех страниц: сочетания и режим записи нового сочетания
/// (Settings → Shortcuts: следующее нажатие не выполняется, а отдаётся интерфейсу).
#[derive(Default)]
pub struct Keys {
    pub map: RefCell<Keymap>,
    pub recording: Cell<bool>,
}

/// Что получил обработчик: команду или (в режиме записи) нажатое сочетание.
#[derive(Debug)]
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
