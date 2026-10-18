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
    /// Метка «Jiyda работает» (recovery.rs): осталась после запуска — тот оборвался.
    pub running: PathBuf,
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
            running: base.join("running"),
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
#[path = "../tests/unit/config.rs"]
mod tests;
