//! Сочетания клавиш в WebView2: событие `AcceleratorKeyPressed` приходит до того, как
//! нажатие попадёт на страницу, — поэтому клавиши Jiyda работают на любой странице, а
//! страница не может их ни перехватить, ни подделать. Что делает сочетание — решает
//! `hotkeys::decide` (без движка).

use crate::hotkeys::{KeyEvent, Keys, decide};
use std::rc::Rc;

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
