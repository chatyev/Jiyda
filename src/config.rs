//! Настройки браузера и пути к файлам данных.
//!
//! Файл настроек: %LOCALAPPDATA%\Jiyda\config.json — создаётся при первом запуске,
//! его можно править руками (браузер нужно перезапустить).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Папка данных в %LOCALAPPDATA%.
const DATA_DIR: &str = "Jiyda";
/// Так она называлась раньше: браузер был Seyro (до 0.6.0), а ещё раньше — Drift.
/// Переносится самая новая из найденных.
const OLD_DATA_DIRS: &[&str] = &["Seyro", "Drift"];

fn local_data() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Старая папка данных, если она есть.
fn old_data_dir(local: &Path) -> Option<PathBuf> {
    OLD_DATA_DIRS.iter().map(|d| local.join(d)).find(|d| d.is_dir())
}

/// Перенести данные из старой папки (Seyro или Drift) в Jiyda: вкладки, пространства,
/// логины, настройки. Ok(true) — перенесли, Ok(false) — нечего переносить. Вызывается
/// до `Paths::new`, пока в папках ничего не открыто.
pub fn migrate_old_data() -> std::io::Result<bool> {
    if data_dir_override().is_some() {
        return Ok(false);
    }
    migrate_in(&local_data())
}

/// Своя папка данных из переменной JIYDA_DATA_DIR — для разработки и проверки, чтобы
/// не трогать настоящие вкладки и логины. Такой Jiyda не мешает основному (см. instance.rs).
pub fn data_dir_override() -> Option<PathBuf> {
    std::env::var_os("JIYDA_DATA_DIR").filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn migrate_in(local: &Path) -> std::io::Result<bool> {
    let new = local.join(DATA_DIR);
    let Some(old) = old_data_dir(local).filter(|_| !new.exists()) else { return Ok(false) };
    // Переименование внутри одного диска мгновенное и ничего не копирует.
    std::fs::rename(&old, &new)?;
    Ok(true)
}

/// Если перенести не удалось (старая папка занята, например ещё работает seyro.exe),
/// работаем в старой папке, чтобы ничего не потерять; перенос повторится при
/// следующем запуске.
fn data_dir_in(local: &Path) -> PathBuf {
    let new = local.join(DATA_DIR);
    if new.exists() { new } else { old_data_dir(local).unwrap_or(new) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Шаблон поиска, `%s` заменяется на запрос.
    pub search_url: String,
    /// Через сколько часов без использования незакреплённая страница уходит в Archive
    /// (проверяется при запуске). 0 — никогда.
    pub archive_hours: u64,
    /// Через сколько минут фоновая страница «засыпает» (выгружается из памяти). 0 — никогда.
    pub sleep_minutes: u64,
    /// Загружать распакованные расширения из папки extensions.
    pub extensions: bool,
    /// Включить инструменты разработчика (F12).
    pub devtools: bool,
    /// Своя верхняя панель вместо рамки Windows. false — обычное окно с системной рамкой.
    pub custom_titlebar: bool,
    /// Проверять новые версии на GitHub (при запуске и раз в 12 часов) и предлагать обновиться.
    pub check_updates: bool,
    /// Язык интерфейса: "auto" (как в Windows), "ru" или "en".
    pub language: String,
    /// Переназначенные горячие клавиши: команда → «Ctrl+Shift+K» ("" — без клавиш).
    pub shortcuts: BTreeMap<String, String>,
    /// Блокировать трекеры и рекламу (shield.rs).
    pub shield: bool,
    /// Сайты, на которых защита выключена.
    pub shield_allow: Vec<String>,
    /// Панель действий над выделенным текстом.
    pub selection_toolbar: bool,
    /// Навигация у левого края окна (Edge Reveal).
    pub edge_reveal: bool,
    /// Приглушать верхнюю панель, пока читаешь страницу.
    pub dim_chrome: bool,
    /// Анимации: "system" (как в Windows) или "reduced".
    pub motion: String,
    /// Ask Jiyda включён (ключ — в диспетчере учётных данных Windows, не здесь).
    pub ai: bool,
    /// Папка загрузок; пусто — как в Windows.
    pub download_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            search_url: "https://www.google.com/search?q=%s".into(),
            archive_hours: 12,
            sleep_minutes: 60,
            extensions: true,
            devtools: true,
            custom_titlebar: true,
            check_updates: true,
            language: "auto".into(),
            shortcuts: BTreeMap::new(),
            shield: true,
            shield_allow: Vec::new(),
            selection_toolbar: true,
            edge_reveal: true,
            dim_chrome: true,
            motion: "system".into(),
            ai: false,
            download_dir: String::new(),
        }
    }
}

pub struct Paths {
    pub config: PathBuf,
    pub session: PathBuf,
    pub history: PathBuf,
    pub downloads: PathBuf,
    /// Значки сайтов (store::Favicons).
    pub favicons: PathBuf,
    /// Решения о разрешениях сайтов.
    pub permissions: PathBuf,
    /// Счётчик заблокированного за день.
    pub shield: PathBuf,
    /// Профиль WebView2 пространства «Основное» (куки, кэш, логины).
    pub webview_data: PathBuf,
    /// Папки остальных пространств: spaces/<id>/webview.
    pub spaces: PathBuf,
    pub extensions: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        Self::in_dir(&data_dir_override().unwrap_or_else(|| data_dir_in(&local_data())))
    }

    pub fn in_dir(base: &Path) -> Self {
        let p = Self {
            config: base.join("config.json"),
            session: base.join("session.json"),
            history: base.join("history.json"),
            downloads: base.join("downloads.json"),
            favicons: base.join("favicons.json"),
            permissions: base.join("permissions.json"),
            shield: base.join("shield.json"),
            webview_data: base.join("webview"),
            spaces: base.join("spaces"),
            extensions: base.join("extensions"),
            log: base.join("jiyda.log"),
        };
        let _ = std::fs::create_dir_all(&p.webview_data);
        let _ = std::fs::create_dir_all(&p.extensions);
        p
    }

    /// Папка данных (Settings → Advanced: «Открыть папку»).
    pub fn base(&self) -> &Path {
        self.config.parent().unwrap_or(Path::new("."))
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
                cfg.save(paths);
                cfg
            }
        }
    }

    /// Записать настройки (их меняют в Settings).
    pub fn save(&self, paths: &Paths) {
        if let Some(s) = self.to_json() {
            write_atomic(&paths.config, &s);
        }
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(self).ok()
    }
}

/// Запись файлов в фоновом потоке, чтобы окно не замирало на диске. Записи идут по
/// очереди в порядке вызова; `finish` (и Drop) дожидается, пока всё будет записано.
pub struct Writer {
    tx: Option<std::sync::mpsc::Sender<(PathBuf, String)>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Writer {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, String)>();
        let thread = std::thread::Builder::new()
            .name("jiyda-writer".into())
            .spawn(move || {
                for (path, data) in rx {
                    write_atomic(&path, &data);
                }
            })
            .map_err(|e| log::error!("поток записи не запущен, пишу сразу: {e}"))
            .ok();
        Self { tx: thread.as_ref().map(|_| tx), thread }
    }

    /// Записать файл (атомарно). Если поток недоступен — сразу, в этом потоке.
    pub fn write(&self, path: &Path, data: String) {
        match &self.tx {
            Some(tx) => {
                if let Err(e) = tx.send((path.to_path_buf(), data)) {
                    let (path, data) = e.0;
                    write_atomic(&path, &data);
                }
            }
            None => write_atomic(path, &data),
        }
    }

    /// Дождаться, пока очередь будет записана. После этого запись идёт сразу.
    pub fn finish(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.finish();
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
        Paths::in_dir(dir)
    }

    #[test]
    fn missing_config_creates_default_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        let cfg = Config::load(&p);
        assert_eq!(cfg.search_url, Config::default().search_url);

        let written: Config = serde_json::from_str(&std::fs::read_to_string(&p.config).unwrap()).unwrap();
        assert!(written.shield && written.shortcuts.is_empty());
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
        assert!(cfg.edge_reveal);
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
    fn writer_writes_in_order_and_finishes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.json");
        let mut w = Writer::new();
        for i in 0..50 {
            w.write(&path, i.to_string());
        }
        w.finish();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "49");
        // После finish запись — сразу.
        w.write(&path, "late".into());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "late");
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

    // ---------- Перенос данных из Seyro и Drift ----------

    fn old_data(local: &std::path::Path, name: &str) {
        let old = local.join(name);
        std::fs::create_dir_all(old.join("webview").join("EBWebView")).unwrap();
        std::fs::write(old.join("session.json"), "{\"spaces\":[]}").unwrap();
        std::fs::write(old.join("webview").join("EBWebView").join("Cookies"), "c").unwrap();
    }

    #[test]
    fn migration_moves_old_folder_with_everything_inside() {
        for name in OLD_DATA_DIRS {
            let local = tempfile::tempdir().unwrap();
            old_data(local.path(), name);
            assert!(migrate_in(local.path()).unwrap(), "{name}");
            let new = local.path().join(DATA_DIR);
            assert_eq!(std::fs::read_to_string(new.join("session.json")).unwrap(), "{\"spaces\":[]}");
            assert!(new.join("webview").join("EBWebView").join("Cookies").exists());
            assert!(!local.path().join(name).exists());
            assert_eq!(data_dir_in(local.path()), new);
            // Второй раз переносить нечего.
            assert!(!migrate_in(local.path()).unwrap());
        }
    }

    #[test]
    fn newest_old_folder_wins() {
        // Есть и Seyro, и совсем старый Drift — переносится Seyro, Drift не трогается.
        let local = tempfile::tempdir().unwrap();
        old_data(local.path(), "Drift");
        old_data(local.path(), "Seyro");
        assert!(migrate_in(local.path()).unwrap());
        assert!(!local.path().join("Seyro").exists());
        assert!(local.path().join("Drift").exists());
    }

    #[test]
    fn migration_never_overwrites_existing_jiyda_data() {
        let local = tempfile::tempdir().unwrap();
        old_data(local.path(), "Seyro");
        let new = local.path().join(DATA_DIR);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("session.json"), "new").unwrap();
        assert!(!migrate_in(local.path()).unwrap());
        assert_eq!(std::fs::read_to_string(new.join("session.json")).unwrap(), "new");
        assert!(local.path().join("Seyro").exists());
        assert_eq!(data_dir_in(local.path()), new);
    }

    #[test]
    fn fresh_install_uses_jiyda_folder() {
        let local = tempfile::tempdir().unwrap();
        assert!(!migrate_in(local.path()).unwrap());
        assert_eq!(data_dir_in(local.path()), local.path().join(DATA_DIR));
    }

    #[test]
    fn falls_back_to_old_folder_when_not_migrated() {
        // Перенос не удался (папка занята) — работаем со старыми данными, а не с пустыми.
        let local = tempfile::tempdir().unwrap();
        old_data(local.path(), "Seyro");
        assert_eq!(data_dir_in(local.path()), local.path().join("Seyro"));
    }

    #[test]
    fn old_name_as_a_file_is_ignored() {
        let local = tempfile::tempdir().unwrap();
        std::fs::write(local.path().join("Seyro"), "not a folder").unwrap();
        assert!(!migrate_in(local.path()).unwrap());
        assert_eq!(data_dir_in(local.path()), local.path().join(DATA_DIR));
    }
}
