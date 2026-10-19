//! Изменение настройки из Settings: тип вместо пары «ключ + произвольный JSON».
//!
//! Интерфейс шлёт `{"type": "setting", "key": "sleep_minutes", "value": 30}`; serde
//! разбирает это в `SettingChange` (неверный тип значения — сообщение отброшено),
//! `apply` проверяет диапазоны и меняет `Config`. Побочные эффекты (тема, уже открытые
//! WebView, клавиши) выполняет ядро — app.rs.

use crate::command::Command;
use crate::config::Config;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    System,
    Reduced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Auto,
    Ru,
    En,
    Uz,
}

/// Новое сочетание для команды ("" — без сочетания).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Shortcut {
    pub cmd: Command,
    pub keys: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "key", content = "value", rename_all = "snake_case")]
pub enum SettingChange {
    Theme(Theme),
    SearchUrl(String),
    ArchiveHours(u64),
    SleepMinutes(u64),
    CheckUpdates(bool),
    BetaUpdates(bool),
    Shield(bool),
    SelectionToolbar(bool),
    EdgeReveal(bool),
    DimChrome(bool),
    Motion(Motion),
    Ai(bool),
    Devtools(bool),
    Extensions(bool),
    CustomTitlebar(bool),
    Language(Language),
    DownloadDir(String),
    Shortcut(Shortcut),
    /// Сбросить сочетание команды к умолчанию; None — все команды.
    ShortcutReset(Option<Command>),
}

/// Что сделать после изменения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// Вступит в силу после перезапуска.
    pub restart: bool,
}

const ARCHIVE_HOURS_MAX: u64 = 24 * 365;
const SLEEP_MINUTES_MAX: u64 = 24 * 60;

impl SettingChange {
    /// Применить к настройкам. Err — ключ сообщения для пользователя (i18n.rs).
    /// Тему и проверку конфликтов сочетаний делает ядро до вызова.
    pub fn apply(self, cfg: &mut Config) -> Result<Applied, &'static str> {
        let mut restart = false;
        match self {
            // Тема хранится в сессии, не в config.json.
            SettingChange::Theme(_) => {}
            SettingChange::SearchUrl(s) => {
                let s = s.trim();
                if !s.starts_with("https://") || !s.contains("%s") {
                    return Err("The search address must start with https:// and contain %s");
                }
                cfg.search_url = s.to_string();
            }
            SettingChange::ArchiveHours(n) if n <= ARCHIVE_HOURS_MAX => cfg.archive_hours = n,
            SettingChange::SleepMinutes(n) if n <= SLEEP_MINUTES_MAX => cfg.sleep_minutes = n,
            SettingChange::ArchiveHours(_) | SettingChange::SleepMinutes(_) => return Err("Invalid value"),
            SettingChange::CheckUpdates(on) => cfg.check_updates = on,
            SettingChange::BetaUpdates(on) => cfg.beta_updates = on,
            SettingChange::Shield(on) => cfg.shield = on,
            SettingChange::SelectionToolbar(on) => cfg.selection_toolbar = on,
            SettingChange::EdgeReveal(on) => cfg.edge_reveal = on,
            SettingChange::DimChrome(on) => cfg.dim_chrome = on,
            SettingChange::Motion(m) => cfg.motion = if m == Motion::Reduced { "reduced" } else { "system" }.into(),
            SettingChange::Ai(on) => cfg.ai = on,
            SettingChange::Devtools(on) => (cfg.devtools, restart) = (on, true),
            SettingChange::Extensions(on) => (cfg.extensions, restart) = (on, true),
            SettingChange::CustomTitlebar(on) => (cfg.custom_titlebar, restart) = (on, true),
            SettingChange::Language(l) => {
                cfg.language = match l {
                    Language::Auto => "auto",
                    Language::Ru => "ru",
                    Language::En => "en",
                    Language::Uz => "uz",
                }
                .into();
                restart = true;
            }
            SettingChange::DownloadDir(d) => {
                let d = d.trim();
                if !d.is_empty() && !std::path::Path::new(d).is_dir() {
                    return Err("That folder doesn't exist");
                }
                cfg.download_dir = d.to_string();
            }
            SettingChange::Shortcut(s) => {
                if !crate::keymap::Keymap::customizable(s.cmd) {
                    return Err("Invalid value");
                }
                if !s.keys.is_empty() && crate::keymap::Combo::parse(&s.keys).is_none() {
                    return Err("This shortcut can't be used: it needs Ctrl or Alt (or F1\u{2013}F24)");
                }
                cfg.shortcuts.insert(s.cmd.to_string(), s.keys);
            }
            SettingChange::ShortcutReset(Some(cmd)) => {
                cfg.shortcuts.remove(&cmd.to_string());
            }
            SettingChange::ShortcutReset(None) => cfg.shortcuts.clear(),
        }
        Ok(Applied { restart })
    }
}

#[cfg(test)]
#[path = "../tests/unit/settings.rs"]
mod tests;
