//! Модульные тесты src/keymap.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn vk(c: char) -> u32 {
    c as u32
}

fn km() -> Keymap {
    Keymap::default()
}

#[test]
fn parse_and_print_roundtrip() {
    for s in
        ["Ctrl+K", "Ctrl+Shift+T", "Alt+Left", "F11", "Shift+F5", "Ctrl+Space", "Ctrl+Comma", "Ctrl+PageDown", "Ctrl+1"]
    {
        let c = Combo::parse(s).unwrap_or_else(|| panic!("{s}"));
        assert_eq!(c.to_string_opt().as_deref(), Some(s));
    }
    // Модификаторы в любом регистре и порядке, буква — любой регистр.
    assert_eq!(Combo::parse("shift+ctrl+k"), Combo::parse("Ctrl+Shift+K"));
    assert_eq!(Combo::parse(" Ctrl + l "), Combo::parse("Ctrl+L"));
}

#[test]
fn parse_rejects_bad_or_unassignable() {
    for bad in ["", "K", "Shift+K", "Ctrl+Alt+K", "Ctrl+Ctrl+K", "Meta+K", "Ctrl+", "Ctrl+Nope", "Ctrl+F25", "Space"] {
        assert_eq!(Combo::parse(bad), None, "{bad}");
    }
}

#[test]
fn default_shortcuts() {
    let k = km();
    let cases = [
        (vk('K'), true, false, false, "palette"),
        (0x20, true, false, false, "orbit"),
        (vk('L'), true, false, false, "edit_url"),
        (vk('T'), true, false, false, "new_tab"),
        (vk('T'), true, true, false, "reopen"),
        (vk('W'), true, false, false, "close"),
        (0x09, true, false, false, "next"),
        (0x09, true, true, false, "prev"),
        (0x25, false, false, true, "back"),
        (0x27, false, false, true, "forward"),
        (vk('R'), true, false, false, "reload"),
        (0x74, false, false, false, "reload"),
        (vk('R'), true, true, false, "hard_reload"),
        (vk('F'), true, false, false, "find"),
        (vk('F'), true, true, false, "focus"),
        (vk('H'), true, false, false, "history"),
        (vk('J'), true, false, false, "downloads"),
        (0x7A, false, false, false, "fullscreen"),
        (0xBC, true, false, false, "settings"),
    ];
    for (code, c, s, a, cmd) in cases {
        assert_eq!(
            k.lookup(code, c, s, a).map(|c| c.to_string()).map(|c| c.to_string()).as_deref(),
            Some(cmd),
            "{cmd}"
        );
    }
}

#[test]
fn digits_go_to_page_or_space() {
    let k = km();
    assert_eq!(k.lookup(vk('1'), true, false, false).map(|c| c.to_string()).as_deref(), Some("goto:1"));
    assert_eq!(k.lookup(vk('9'), true, true, false).map(|c| c.to_string()).as_deref(), Some("space:9"));
    assert_eq!(k.lookup(vk('0'), true, false, false), None);
    assert_eq!(k.lookup(vk('1'), false, false, false), None);
}

#[test]
fn page_keys_and_altgr_are_left_alone() {
    let k = km();
    // Ctrl+C/V/A/Z/P и т.п. браузер не перехватывает.
    for c in ['C', 'V', 'A', 'Z', 'P', 'S'] {
        assert_eq!(k.lookup(vk(c), true, false, false), None, "Ctrl+{c}");
    }
    assert_eq!(k.lookup(vk('K'), true, false, true), None);
    assert_eq!(k.lookup(vk('K'), false, false, false), None);
}

#[test]
fn overrides_replace_or_disable() {
    let mut o = BTreeMap::new();
    o.insert("orbit".to_string(), "Alt+O".to_string());
    o.insert("palette".to_string(), String::new());
    o.insert("unknown".to_string(), "Ctrl+Q".to_string());
    o.insert("find".to_string(), "garbage".to_string());
    let k = Keymap::new(&o);
    assert_eq!(k.lookup(vk('O'), false, false, true).map(|c| c.to_string()).as_deref(), Some("orbit"));
    assert_eq!(k.lookup(0x20, true, false, false), None);
    assert_eq!(k.lookup(vk('K'), true, false, false), None);
    assert_eq!(k.lookup(vk('Q'), true, false, false), None);
    // Неразборчивое — по умолчанию.
    assert_eq!(k.lookup(vk('F'), true, false, false).map(|c| c.to_string()).as_deref(), Some("find"));
}

#[test]
fn owner_finds_conflicts() {
    let k = km();
    assert_eq!(k.owner(&Combo::parse("Ctrl+K").unwrap()), Some(Command::Palette));
    assert_eq!(k.owner(&Combo::parse("Ctrl+Q").unwrap()), None);
}

#[test]
fn defaults_are_valid_and_unique() {
    let mut seen: Vec<Combo> = Vec::new();
    for (cmd, keys) in DEFAULTS {
        for key in *keys {
            let c = Combo::parse(key).unwrap_or_else(|| panic!("{cmd}: {key}"));
            assert!(!seen.contains(&c), "повтор: {key}");
            seen.push(c);
        }
        assert!(Keymap::customizable(*cmd));
    }
    let json = km().to_json();
    assert_eq!(json.as_array().unwrap().len(), DEFAULTS.len());
    assert_eq!(json[0]["cmd"], "palette");
    assert_eq!(json[0]["keys"][0], "Ctrl+K");
}
