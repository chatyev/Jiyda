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

#[cfg(test)]
#[path = "../tests/unit/permissions.rs"]
mod tests;
