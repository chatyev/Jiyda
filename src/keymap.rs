//! Горячие клавиши Jiyda: какие сочетания какую команду вызывают.
//!
//! Сочетание записывается строкой «Ctrl+Shift+K» и сравнивается по виртуальному коду
//! клавиши Windows, поэтому работает на любой раскладке (в том числе русской).
//! Пользователь может переназначить любую команду (Settings → Shortcuts); переназначения
//! хранятся в config.json (`shortcuts`: команда → сочетание, "" — без сочетания).
//!
//! Ctrl+1…9 (страница по номеру) и Ctrl+Shift+1…9 (пространство по номеру) не
//! переназначаются.

use crate::command::Command;
use std::collections::BTreeMap;

/// Команды, которые можно переназначить, и их сочетания по умолчанию.
/// Порядок — как в Settings → Shortcuts.
const DEFAULTS: &[(Command, &[&str])] = &[
    (Command::Palette, &["Ctrl+K"]),
    (Command::Orbit, &["Ctrl+Space"]),
    (Command::EditUrl, &["Ctrl+L", "Alt+D", "F6"]),
    (Command::NewTab, &["Ctrl+T"]),
    (Command::Close, &["Ctrl+W", "Ctrl+F4"]),
    (Command::Reopen, &["Ctrl+Shift+T"]),
    (Command::Next, &["Ctrl+Tab"]),
    (Command::Prev, &["Ctrl+Shift+Tab"]),
    (Command::NextSeq, &["Ctrl+PageDown"]),
    (Command::PrevSeq, &["Ctrl+PageUp"]),
    (Command::Back, &["Alt+Left"]),
    (Command::Forward, &["Alt+Right"]),
    (Command::Reload, &["Ctrl+R", "F5"]),
    (Command::HardReload, &["Ctrl+Shift+R", "Ctrl+F5", "Shift+F5"]),
    (Command::Find, &["Ctrl+F"]),
    (Command::Focus, &["Ctrl+Shift+F"]),
    (Command::Spaces, &["Ctrl+Shift+Space"]),
    (Command::History, &["Ctrl+H"]),
    (Command::Closed, &["Ctrl+Shift+H"]),
    (Command::Downloads, &["Ctrl+J"]),
    (Command::Later, &["Ctrl+Shift+L"]),
    (Command::Split, &["Ctrl+Shift+S"]),
    (Command::Ask, &["Ctrl+Shift+A"]),
    (Command::Fullscreen, &["F11"]),
    (Command::Pin, &["Ctrl+D"]),
    (Command::CopyUrl, &["Ctrl+Shift+U"]),
    (Command::Settings, &["Ctrl+Comma"]),
    (Command::Shortcuts, &["Ctrl+Slash"]),
    (Command::Devtools, &["F12", "Ctrl+Shift+I"]),
];

/// Сочетание клавиш.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Виртуальный код клавиши Windows.
    pub vk: u32,
}

/// Имена клавиш (как в строке сочетания) и их виртуальные коды, кроме букв, цифр и F1–F24.
const NAMED: &[(&str, u32)] = &[
    ("Space", 0x20),
    ("Tab", 0x09),
    ("Enter", 0x0D),
    ("Esc", 0x1B),
    ("Backspace", 0x08),
    ("Delete", 0x2E),
    ("Insert", 0x2D),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("Semicolon", 0xBA),
    ("Equal", 0xBB),
    ("Comma", 0xBC),
    ("Minus", 0xBD),
    ("Period", 0xBE),
    ("Slash", 0xBF),
    ("Backquote", 0xC0),
    ("BracketLeft", 0xDB),
    ("Backslash", 0xDC),
    ("BracketRight", 0xDD),
    ("Quote", 0xDE),
];

fn key_vk(name: &str) -> Option<u32> {
    let b = name.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_uppercase() || b[0].is_ascii_digit()) {
        return Some(u32::from(b[0]));
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
        && (1..=24).contains(&n)
    {
        return Some(0x70 + n - 1);
    }
    NAMED.iter().find(|(n, _)| *n == name).map(|(_, vk)| *vk)
}

fn key_name(vk: u32) -> Option<String> {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => Some(char::from(vk as u8).to_string()),
        0x70..=0x87 => Some(format!("F{}", vk - 0x70 + 1)),
        _ => NAMED.iter().find(|(_, v)| *v == vk).map(|(n, _)| n.to_string()),
    }
}

fn is_function_key(vk: u32) -> bool {
    (0x70..=0x87).contains(&vk)
}

impl Combo {
    /// «Ctrl+Shift+K» → сочетание. Регистр модификаторов не важен, буквы — заглавные.
    /// None — не разобрали или сочетание нельзя назначить (см. `assignable`).
    pub fn parse(s: &str) -> Option<Combo> {
        let mut c = Combo { ctrl: false, shift: false, alt: false, vk: 0 };
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let (key, mods) = parts.split_last()?;
        for m in mods {
            let flag = match m.to_ascii_lowercase().as_str() {
                "ctrl" => &mut c.ctrl,
                "shift" => &mut c.shift,
                "alt" => &mut c.alt,
                _ => return None,
            };
            if *flag {
                return None;
            }
            *flag = true;
        }
        let key = if key.len() == 1 { key.to_ascii_uppercase() } else { key.to_string() };
        c.vk = key_vk(&key)?;
        c.assignable().then_some(c)
    }

    /// Можно ли назначить: нужен Ctrl или Alt (иначе это обычный ввод текста), но не
    /// оба сразу — Ctrl+Alt это AltGr на многих раскладках. Функциональные клавиши — можно без них.
    pub fn assignable(&self) -> bool {
        if self.ctrl && self.alt {
            return false;
        }
        self.ctrl || self.alt || is_function_key(self.vk)
    }

    /// Строка в том же виде, что принимает `parse`.
    pub fn to_string_opt(self) -> Option<String> {
        let key = key_name(self.vk)?;
        let mut s = String::new();
        for (on, name) in [(self.ctrl, "Ctrl+"), (self.shift, "Shift+"), (self.alt, "Alt+")] {
            if on {
                s.push_str(name);
            }
        }
        s.push_str(&key);
        Some(s)
    }
}

pub struct Keymap {
    /// Команда → её сочетания (пустой список — команда без клавиш).
    bindings: Vec<(Command, Vec<Combo>)>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new(&BTreeMap::new())
    }
}

impl Keymap {
    /// Сочетания по умолчанию с переназначениями из настроек. Переназначение
    /// неизвестной команды или неразборчивое сочетание пропускается.
    pub fn new(overrides: &BTreeMap<String, String>) -> Self {
        let bindings = DEFAULTS
            .iter()
            .map(|(cmd, keys)| {
                let combos = match overrides.get(&cmd.to_string()) {
                    Some(s) if s.trim().is_empty() => Vec::new(),
                    Some(s) => match Combo::parse(s) {
                        Some(c) => vec![c],
                        None => {
                            log::warn!("сочетание «{s}» для {cmd} не разобрано, оставлено по умолчанию");
                            keys.iter().filter_map(|k| Combo::parse(k)).collect()
                        }
                    },
                    None => keys.iter().filter_map(|k| Combo::parse(k)).collect(),
                };
                (*cmd, combos)
            })
            .collect();
        Self { bindings }
    }

    /// Можно ли назначить команде сочетание в Settings (Space и Goto — нельзя).
    pub fn customizable(cmd: Command) -> bool {
        DEFAULTS.iter().any(|(c, _)| *c == cmd)
    }

    /// Команда для нажатия. `vk` — виртуальный код клавиши Windows.
    pub fn lookup(&self, vk: u32, ctrl: bool, shift: bool, alt: bool) -> Option<Command> {
        // Ctrl+Alt — это AltGr: ввод символов, не наши клавиши.
        if ctrl && alt {
            return None;
        }
        let pressed = Combo { ctrl, shift, alt, vk };
        if let Some((cmd, _)) = self.bindings.iter().find(|(_, combos)| combos.contains(&pressed)) {
            return Some(*cmd);
        }
        // Ctrl+1…9 — страница по номеру, Ctrl+Shift+1…9 — пространство по номеру.
        if ctrl && !alt && (u32::from(b'1')..=u32::from(b'9')).contains(&vk) {
            let n = (vk - u32::from(b'0')) as usize;
            return Some(if shift { Command::Space(n) } else { Command::Goto(n) });
        }
        None
    }

    /// Какая команда уже занимает сочетание (для предупреждения о конфликте).
    pub fn owner(&self, combo: &Combo) -> Option<Command> {
        self.bindings.iter().find(|(_, c)| c.contains(combo)).map(|(cmd, _)| *cmd)
    }

    /// Для интерфейса: команда, её сочетания сейчас и по умолчанию.
    pub fn to_json(&self) -> serde_json::Value {
        let names = |combos: &[Combo]| combos.iter().copied().filter_map(Combo::to_string_opt).collect::<Vec<_>>();
        serde_json::Value::Array(
            self.bindings
                .iter()
                .map(|(cmd, combos)| {
                    let default: Vec<Combo> = DEFAULTS
                        .iter()
                        .find(|(c, _)| c == cmd)
                        .map(|(_, keys)| keys.iter().filter_map(|k| Combo::parse(k)).collect())
                        .unwrap_or_default();
                    serde_json::json!({ "cmd": cmd.to_string(), "keys": names(combos), "default": names(&default) })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vk(c: char) -> u32 {
        c as u32
    }

    fn km() -> Keymap {
        Keymap::default()
    }

    #[test]
    fn parse_and_print_roundtrip() {
        for s in [
            "Ctrl+K",
            "Ctrl+Shift+T",
            "Alt+Left",
            "F11",
            "Shift+F5",
            "Ctrl+Space",
            "Ctrl+Comma",
            "Ctrl+PageDown",
            "Ctrl+1",
        ] {
            let c = Combo::parse(s).unwrap_or_else(|| panic!("{s}"));
            assert_eq!(c.to_string_opt().as_deref(), Some(s));
        }
        // Модификаторы в любом регистре и порядке, буква — любой регистр.
        assert_eq!(Combo::parse("shift+ctrl+k"), Combo::parse("Ctrl+Shift+K"));
        assert_eq!(Combo::parse(" Ctrl + l "), Combo::parse("Ctrl+L"));
    }

    #[test]
    fn parse_rejects_bad_or_unassignable() {
        for bad in
            ["", "K", "Shift+K", "Ctrl+Alt+K", "Ctrl+Ctrl+K", "Meta+K", "Ctrl+", "Ctrl+Nope", "Ctrl+F25", "Space"]
        {
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
}
