//! Настройки браузера и пути к файлам данных.
//!
//! Файл настроек: %LOCALAPPDATA%\Drift\config.json — создаётся при первом запуске,
//! его можно править руками (браузер нужно перезапустить).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Шаблон поиска, `%s` заменяется на запрос.
    pub search_url: String,
    /// Ширина боковой панели в логических пикселях.
    pub sidebar_width: f64,
    /// Отступ вокруг страницы (рамка в стиле Arc). 0 — страница во всё окно.
    pub frame_padding: f64,
    /// Через сколько часов без использования незакреплённая вкладка уходит в архив
    /// (проверяется при запуске). 0 — никогда.
    pub archive_hours: u64,
    /// Через сколько минут фоновая вкладка «засыпает» (выгружается из памяти). 0 — никогда.
    pub sleep_minutes: u64,
    /// Загружать распакованные расширения из папки extensions.
    pub extensions: bool,
    /// Включить инструменты разработчика (F12).
    pub devtools: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            search_url: "https://www.google.com/search?q=%s".into(),
            sidebar_width: 260.0,
            frame_padding: 8.0,
            archive_hours: 12,
            sleep_minutes: 60,
            extensions: true,
            devtools: true,
        }
    }
}

pub struct Paths {
    pub config: PathBuf,
    pub session: PathBuf,
    pub history: PathBuf,
    pub webview_data: PathBuf,
    pub extensions: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .join("Drift");
        let p = Self {
            config: base.join("config.json"),
            session: base.join("session.json"),
            history: base.join("history.json"),
            webview_data: base.join("webview"),
            extensions: base.join("extensions"),
            log: base.join("drift.log"),
        };
        let _ = std::fs::create_dir_all(&p.webview_data);
        let _ = std::fs::create_dir_all(&p.extensions);
        p
    }
}

impl Config {
    pub fn load(paths: &Paths) -> Self {
        match std::fs::read_to_string(&paths.config) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                log::warn!("config.json повреждён ({e}), использую настройки по умолчанию");
                Self::default()
            }),
            Err(_) => {
                let cfg = Self::default();
                if let Ok(s) = serde_json::to_string_pretty(&cfg) {
                    let _ = std::fs::write(&paths.config, s);
                }
                cfg
            }
        }
    }
}

/// Атомарная запись файла: сначала во временный, потом переименование.
pub fn write_atomic(path: &std::path::Path, data: &str) {
    let tmp = path.with_extension("tmp");
    let res = std::fs::write(&tmp, data).and_then(|_| std::fs::rename(&tmp, path));
    if let Err(e) = res {
        log::warn!("не удалось сохранить {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &std::path::Path) -> Paths {
        Paths {
            config: dir.join("config.json"),
            session: dir.join("session.json"),
            history: dir.join("history.json"),
            webview_data: dir.join("webview"),
            extensions: dir.join("extensions"),
            log: dir.join("drift.log"),
        }
    }

    #[test]
    fn missing_config_creates_default_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        let cfg = Config::load(&p);
        assert_eq!(cfg.search_url, Config::default().search_url);

        let written: Config = serde_json::from_str(&std::fs::read_to_string(&p.config).unwrap()).unwrap();
        assert_eq!(written.sidebar_width, 260.0);
        assert_eq!(written.archive_hours, 12);
    }

    #[test]
    fn partial_config_fills_in_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        std::fs::write(&p.config, r#"{"search_url":"https://s.test/?q=%s","devtools":false}"#).unwrap();
        let cfg = Config::load(&p);
        assert_eq!(cfg.search_url, "https://s.test/?q=%s");
        assert!(!cfg.devtools);
        assert_eq!(cfg.sleep_minutes, 60);
        assert_eq!(cfg.frame_padding, 8.0);
    }

    #[test]
    fn corrupt_config_falls_back_to_default_and_keeps_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        std::fs::write(&p.config, "{oops").unwrap();
        let cfg = Config::load(&p);
        assert_eq!(cfg.search_url, Config::default().search_url);
        // Повреждённый файл не перезаписывается — его можно поправить руками.
        assert_eq!(std::fs::read_to_string(&p.config).unwrap(), "{oops");
    }

    #[test]
    fn write_atomic_creates_and_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_atomic(&path, "one");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
        write_atomic(&path, "two");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn write_atomic_into_missing_dir_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no/such/dir/data.json");
        write_atomic(&path, "x");
        assert!(!path.exists());
    }
}
