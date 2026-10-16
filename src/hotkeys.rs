//! Горячие клавиши браузера во вкладках.
//!
//! Ловятся событием WebView2 `AcceleratorKeyPressed` — до того, как нажатие
//! попадёт на страницу. Поэтому они работают на любой странице (в том числе
//! about:blank, куда не встраивается tab.js), а страница не может их ни
//! перехватить, ни подделать.

const VK_TAB: u32 = 0x09;

/// Команда для нажатия. `vk` — виртуальный код клавиши Windows: для букв и цифр
/// он совпадает с латинской буквой на клавише при любой раскладке, в том числе русской.
pub fn command(vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<String> {
    // Ctrl+Alt — это AltGr на многих раскладках, его не трогаем.
    if !ctrl || alt {
        return None;
    }
    let key = |c: u8| vk == u32::from(c);
    let cmd = if vk == VK_TAB {
        if shift { "prev" } else { "next" }
    } else if key(b'T') {
        if shift { "reopen" } else { "new_tab" }
    } else if key(b'L') {
        "edit_url"
    } else if key(b'W') {
        "close"
    } else if key(b'U') && shift {
        "copy_url"
    } else if shift {
        return None;
    } else if key(b'S') {
        "toggle_sidebar"
    } else if key(b'D') {
        "pin"
    } else if key(b'F') {
        "find"
    } else if key(b'J') {
        "downloads"
    } else if (u32::from(b'1')..=u32::from(b'9')).contains(&vk) {
        return Some(format!("goto:{}", vk - u32::from(b'0')));
    } else {
        return None;
    };
    Some(cmd.to_string())
}

/// При удержании клавиши повторяем только переключение вкладок.
pub fn repeats(cmd: &str) -> bool {
    matches!(cmd, "next" | "prev")
}

/// Подписаться на нажатия во вкладке. `on_command` вызывается в главном потоке.
#[cfg(windows)]
pub fn install(wv: &wry::WebView, on_command: impl Fn(String) + 'static) {
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
            let Some(cmd) = command(vk, down(VK_CONTROL), down(VK_SHIFT), down(VK_MENU)) else { return Ok(()) };
            args.SetHandled(true)?;
            let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
            args.PhysicalKeyStatus(&mut status)?;
            if !status.WasKeyDown.as_bool() || repeats(&cmd) {
                on_command(cmd);
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
pub fn install(_wv: &wry::WebView, _on_command: impl Fn(String) + 'static) {}

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
mod tests {
    use super::*;

    fn vk(c: char) -> u32 {
        c as u32
    }

    fn cmd(vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<String> {
        command(vk, ctrl, shift, alt)
    }

    #[test]
    fn ctrl_letters() {
        let cases = [('T', "new_tab"), ('L', "edit_url"), ('W', "close"), ('S', "toggle_sidebar"), ('D', "pin"), ('F', "find"), ('J', "downloads")];
        for (c, expected) in cases {
            assert_eq!(cmd(vk(c), true, false, false).as_deref(), Some(expected), "Ctrl+{c}");
        }
    }

    #[test]
    fn ctrl_shift_combinations() {
        assert_eq!(cmd(vk('T'), true, true, false).as_deref(), Some("reopen"));
        assert_eq!(cmd(vk('U'), true, true, false).as_deref(), Some("copy_url"));
        assert_eq!(cmd(VK_TAB, true, true, false).as_deref(), Some("prev"));
        // Как и раньше в tab.js, Shift не мешает Ctrl+L и Ctrl+W.
        assert_eq!(cmd(vk('L'), true, true, false).as_deref(), Some("edit_url"));
        assert_eq!(cmd(vk('W'), true, true, false).as_deref(), Some("close"));
    }

    #[test]
    fn shift_disables_other_shortcuts() {
        for c in ['S', 'D', 'F', 'J', '1', '9'] {
            assert_eq!(cmd(vk(c), true, true, false), None, "Ctrl+Shift+{c}");
        }
    }

    #[test]
    fn tab_switching_and_digits() {
        assert_eq!(cmd(VK_TAB, true, false, false).as_deref(), Some("next"));
        assert_eq!(cmd(vk('1'), true, false, false).as_deref(), Some("goto:1"));
        assert_eq!(cmd(vk('9'), true, false, false).as_deref(), Some("goto:9"));
        assert_eq!(cmd(vk('0'), true, false, false), None);
    }

    #[test]
    fn needs_ctrl_and_no_alt() {
        assert_eq!(cmd(vk('T'), false, false, false), None);
        assert_eq!(cmd(vk('T'), false, true, false), None);
        // Ctrl+Alt (AltGr) — ввод символов на некоторых раскладках.
        assert_eq!(cmd(vk('T'), true, false, true), None);
        assert_eq!(cmd(vk('U'), true, false, false), None);
    }

    #[test]
    fn page_shortcuts_are_left_to_the_page() {
        // Ctrl+C/V/A/Z/R и т.п. браузер не перехватывает.
        for c in ['C', 'V', 'A', 'Z', 'R', 'P', 'K'] {
            assert_eq!(cmd(vk(c), true, false, false), None, "Ctrl+{c}");
        }
    }

    #[test]
    fn only_tab_switching_repeats() {
        assert!(repeats("next") && repeats("prev"));
        assert!(!repeats("close") && !repeats("new_tab"));
    }
}
