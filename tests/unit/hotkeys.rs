//! Модульные тесты src/hotkeys.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn cmd(ev: Option<KeyEvent>) -> Option<String> {
    match ev {
        Some(KeyEvent::Command(c)) => Some(c.to_string()),
        _ => None,
    }
}

fn rec(ev: Option<KeyEvent>) -> Option<String> {
    match ev {
        Some(KeyEvent::Recorded(c)) => Some(c),
        _ => None,
    }
}

#[test]
fn commands_from_keymap() {
    let keys = Keys::default();
    assert_eq!(cmd(decide(&keys, 'K' as u32, true, false, false)).as_deref(), Some("palette"));
    assert_eq!(cmd(decide(&keys, 'C' as u32, true, false, false)), None);
}

#[test]
fn recording_returns_combo_instead_of_command() {
    let keys = Keys::default();
    keys.recording.set(true);
    assert_eq!(rec(decide(&keys, 'K' as u32, true, false, false)).as_deref(), Some("Ctrl+K"));
    assert_eq!(rec(decide(&keys, 0x7A, false, false, false)).as_deref(), Some("F11"));
    // Отдельно нажатый Ctrl, обычная буква и AltGr не записываются.
    assert!(decide(&keys, 0x11, true, false, false).is_none());
    assert!(decide(&keys, 'K' as u32, false, false, false).is_none());
    assert!(decide(&keys, 'K' as u32, true, false, true).is_none());
}
