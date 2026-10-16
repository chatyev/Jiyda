//! Настройки браузера и пути к файлам данных.
//!
//! Файл настроек: %LOCALAPPDATA%\Seyro\config.json — создаётся при первом запуске,
//! его можно править руками (браузер нужно перезапустить).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Папка данных в %LOCALAPPDATA%.
const DATA_DIR: &str = "Seyro";
/// Так она называлась до ребрендинга (браузер назывался Drift).
const OLD_DATA_DIR: &str = "Drift";

fn local_data() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Перенести данные из старой папки Drift в Seyro (вкладки, пространства, логины,
/// настройки). Ok(true) — перенесли, Ok(false) — нечего переносить. Вызывается до
/// `Paths::new`, пока в папках ничего не открыто.
pub fn migrate_old_data() -> std::io::Result<bool> {
    migrate_in(&local_data())
}

fn migrate_in(local: &Path) -> std::io::Result<bool> {
    let (new, old) = (local.join(DATA_DIR), local.join(OLD_DATA_DIR));
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    // Переименование внутри одного диска мгновенное и ничего не копирует.
    std::fs::rename(&old, &new)?;
    Ok(true)
}

/// Если перенести не удалось (старая папка занята, например работает drift.exe),
/// работаем в старой папке, чтобы ничего не потерять; перенос повторится при
/// следующем запуске.
fn data_dir_in(local: &Path) -> PathBuf {
    let (new, old) = (local.join(DATA_DIR), local.join(OLD_DATA_DIR));
    if !new.exists() && old.is_dir() { old } else { new }
}

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
    /// Своя полоса заголовка вместо рамки Windows: кнопки окна в сайдбаре, как в Arc.
    /// false — обычное окно с системной рамкой.
    pub custom_titlebar: bool,
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
            custom_titlebar: true,
        }
    }
}

pub struct Paths {
    pub config: PathBuf,
    pub session: PathBuf,
    pub history: PathBuf,
    pub downloads: PathBuf,
    /// Профиль WebView2 пространства «Основное» (куки, кэш, логины).
    pub webview_data: PathBuf,
    /// Папки остальных пространств: spaces/<id>/webview.
    pub spaces: PathBuf,
    pub extensions: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        let base = data_dir_in(&local_data());
        let p = Self {
            config: base.join("config.json"),
            session: base.join("session.json"),
            history: base.join("history.json"),
            downloads: base.join("downloads.json"),
            webview_data: base.join("webview"),
            spaces: base.join("spaces"),
            extensions: base.join("extensions"),
            log: base.join("seyro.log"),
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
            downloads: dir.join("downloads.json"),
            webview_data: dir.join("webview"),
            spaces: dir.join("spaces"),
            extensions: dir.join("extensions"),
            log: dir.join("seyro.log"),
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
        assert!(cfg.custom_titlebar);
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

    // ---------- Перенос данных из Drift ----------

    fn old_drift_data(local: &std::path::Path) {
        let old = local.join(OLD_DATA_DIR);
        std::fs::create_dir_all(old.join("webview").join("EBWebView")).unwrap();
        std::fs::write(old.join("session.json"), "{\"spaces\":[]}").unwrap();
        std::fs::write(old.join("webview").join("EBWebView").join("Cookies"), "c").unwrap();
    }

    #[test]
    fn migration_moves_old_folder_with_everything_inside() {
        let local = tempfile::tempdir().unwrap();
        old_drift_data(local.path());
        assert!(migrate_in(local.path()).unwrap());
        let new = local.path().join(DATA_DIR);
        assert_eq!(std::fs::read_to_string(new.join("session.json")).unwrap(), "{\"spaces\":[]}");
        assert!(new.join("webview").join("EBWebView").join("Cookies").exists());
        assert!(!local.path().join(OLD_DATA_DIR).exists());
        assert_eq!(data_dir_in(local.path()), new);
        // Второй раз переносить нечего.
        assert!(!migrate_in(local.path()).unwrap());
    }

    #[test]
    fn migration_never_overwrites_existing_seyro_data() {
        let local = tempfile::tempdir().unwrap();
        old_drift_data(local.path());
        let new = local.path().join(DATA_DIR);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("session.json"), "new").unwrap();
        assert!(!migrate_in(local.path()).unwrap());
        assert_eq!(std::fs::read_to_string(new.join("session.json")).unwrap(), "new");
        assert!(local.path().join(OLD_DATA_DIR).exists());
        assert_eq!(data_dir_in(local.path()), new);
    }

    #[test]
    fn fresh_install_uses_seyro_folder() {
        let local = tempfile::tempdir().unwrap();
        assert!(!migrate_in(local.path()).unwrap());
        assert_eq!(data_dir_in(local.path()), local.path().join(DATA_DIR));
    }

    #[test]
    fn falls_back_to_old_folder_when_not_migrated() {
        // Перенос не удался (папка занята) — работаем со старыми данными, а не с пустыми.
        let local = tempfile::tempdir().unwrap();
        old_drift_data(local.path());
        assert_eq!(data_dir_in(local.path()), local.path().join(OLD_DATA_DIR));
    }

    #[test]
    fn old_name_as_a_file_is_ignored() {
        let local = tempfile::tempdir().unwrap();
        std::fs::write(local.path().join(OLD_DATA_DIR), "not a folder").unwrap();
        assert!(!migrate_in(local.path()).unwrap());
        assert_eq!(data_dir_in(local.path()), local.path().join(DATA_DIR));
    }
}
