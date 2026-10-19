//! «Помочь улучшить Jiyda» — анонимная статистика использования, только если пользователь
//! сам её включил (по умолчанию выключена; Настройки → Приватность или знакомство).
//!
//! Что уходит — раз в сутки один отчёт за прошедший день ([`Report`]): случайный номер
//! установки (не связан ни с чем, при выключении стирается), версии Jiyda и Windows,
//! сколько раз за день Jiyda запускался и падал, удались ли обновления, пользовались ли
//! Spaces и палитрой, закончены ли перенос и знакомство, примерное число страниц
//! (диапазоном) и выбран ли Jiyda браузером по умолчанию. Адресов, заголовков, поисковых
//! запросов, текста и всего, что на страницах, здесь нет и быть не может: [`Event`] —
//! закрытый список, у событий нет данных.
//!
//! Куда — адрес задаётся при сборке (`JIYDA_TELEMETRY_URL`, в CI — переменная репозитория
//! TELEMETRY_URL; сервер — telemetry/ в репозитории). Нет адреса — нет и функции: в
//! Настройках её не видно, ничего не считается и не отправляется.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Адрес приёма отчётов, заданный при сборке.
const BUILT_ENDPOINT: Option<&str> = match option_env!("JIYDA_TELEMETRY_URL") {
    Some(u) if !u.is_empty() => Some(u),
    _ => None,
};

/// Сколько неотправленных отчётов хранить (нет сети неделю — старые не нужны).
const MAX_PENDING: usize = 7;

/// Куда отправлять; None — статистики в этой сборке нет. Отладочной сборке адрес можно
/// задать переменной окружения (проверка интерфейса без своего сервера).
pub fn endpoint() -> Option<String> {
    #[cfg(debug_assertions)]
    if let Some(u) = std::env::var("JIYDA_TELEMETRY_URL").ok().filter(|u| !u.is_empty()) {
        return Some(u);
    }
    BUILT_ENDPOINT.filter(|u| u.starts_with("https://")).map(str::to_string)
}

/// Что можно посчитать. Других событий нет; данных у них нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    AppStarted,
    AppCrashed,
    UpdateSuccess,
    UpdateFailed,
    WorkspaceUsed,
    CommandPaletteUsed,
    ImportCompleted,
    OnboardingCompleted,
}

impl Event {
    /// Событие, о котором сообщает интерфейс (ui/js), по имени; остальные считает ядро.
    pub fn from_ui(name: &str) -> Option<Event> {
        match name {
            "command_palette_used" => Some(Event::CommandPaletteUsed),
            _ => None,
        }
    }
}

/// Счётчики за день.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Counts {
    pub app_started: u32,
    pub app_crashed: u32,
    pub update_success: u32,
    pub update_failed: u32,
    pub workspace_used: u32,
    pub command_palette_used: u32,
    pub import_completed: u32,
    pub onboarding_completed: u32,
}

impl Counts {
    fn add(&mut self, ev: Event) {
        let c = match ev {
            Event::AppStarted => &mut self.app_started,
            Event::AppCrashed => &mut self.app_crashed,
            Event::UpdateSuccess => &mut self.update_success,
            Event::UpdateFailed => &mut self.update_failed,
            Event::WorkspaceUsed => &mut self.workspace_used,
            Event::CommandPaletteUsed => &mut self.command_palette_used,
            Event::ImportCompleted => &mut self.import_completed,
            Event::OnboardingCompleted => &mut self.onboarding_completed,
        };
        *c = c.saturating_add(1);
    }
}

/// Отчёт за день — ровно то, что уходит на сервер (и что показывает «Что отправляется»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Случайный номер установки.
    pub id: String,
    /// День ГГГГ-ММ-ДД (местное время).
    pub day: String,
    pub version: String,
    /// «10.0.26100» — версия и сборка Windows.
    pub windows: String,
    pub counts: Counts,
    /// Открытые страницы во всех Spaces — диапазоном: «1», «2-5», «6-10», «11-20», «21-50», «51+».
    pub tabs: String,
    pub default_browser: bool,
}

/// Что сейчас: для полей отчёта, которые не считаются, а смотрятся в момент отправки.
pub struct Snapshot {
    pub version: String,
    pub windows: String,
    pub pages: usize,
    pub default_browser: bool,
}

/// Диапазон числа страниц.
pub fn tabs_bucket(n: usize) -> &'static str {
    match n {
        0 => "0",
        1 => "1",
        2..=5 => "2-5",
        6..=10 => "6-10",
        11..=20 => "11-20",
        21..=50 => "21-50",
        _ => "51+",
    }
}

/// Что хранится на диске (telemetry.json в папке данных).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    id: String,
    /// Версия при прошлом запуске: сменилась на более новую — обновление удалось.
    version: String,
    day: String,
    counts: Counts,
    pending: Vec<Report>,
}

pub struct Telemetry {
    path: PathBuf,
    on: bool,
    state: State,
    dirty: bool,
}

impl Telemetry {
    /// `on` — пользователь включил статистику и адрес есть. Выключено — с диска всё стирается.
    pub fn load(path: &Path, on: bool) -> Self {
        let mut t = Self { path: path.to_path_buf(), on: false, state: State::default(), dirty: false };
        if on {
            t.state =
                std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            t.enable();
        } else {
            let _ = std::fs::remove_file(path);
        }
        t
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Включить: новый случайный номер установки, если его ещё нет.
    pub fn enable(&mut self) {
        self.on = true;
        if self.state.id.len() != 32 {
            self.state.id = random_id();
            self.dirty = true;
        }
    }

    /// Выключить: забыть номер, счётчики и неотправленное — и стереть файл.
    pub fn disable(&mut self) {
        self.on = false;
        self.state = State::default();
        self.dirty = false;
        let _ = std::fs::remove_file(&self.path);
    }

    /// Учесть событие сегодняшнего дня `today`.
    pub fn record(&mut self, ev: Event, today: &str) {
        if !self.on {
            return;
        }
        self.start_day(today, None);
        self.state.counts.add(ev);
        self.dirty = true;
    }

    /// Запуск версии `version`: сменилась на более новую с прошлого запуска — обновление удалось.
    pub fn started(&mut self, version: &str, crashed: bool, today: &str) {
        if !self.on {
            return;
        }
        let prev = std::mem::replace(&mut self.state.version, version.to_string());
        self.record(Event::AppStarted, today);
        if crashed {
            self.record(Event::AppCrashed, today);
        }
        if !prev.is_empty() && crate::update::is_newer(version, &prev) {
            self.record(Event::UpdateSuccess, today);
        }
    }

    /// Начался новый день: счётчики прошлого дня становятся отчётом в очереди отправки.
    /// `snap` — как сейчас (для отчёта); None — только сменить день (событие из `record`,
    /// отчёт соберёт следующий `roll`).
    fn start_day(&mut self, today: &str, snap: Option<&Snapshot>) {
        if self.state.day == today {
            return;
        }
        if !self.state.day.is_empty() && self.state.counts != Counts::default() {
            let report = self.report_with(&self.state.day.clone(), snap);
            self.state.pending.push(report);
            let extra = self.state.pending.len().saturating_sub(MAX_PENDING);
            self.state.pending.drain(..extra);
        }
        self.state.day = today.to_string();
        self.state.counts = Counts::default();
        self.dirty = true;
    }

    fn report_with(&self, day: &str, snap: Option<&Snapshot>) -> Report {
        Report {
            id: self.state.id.clone(),
            day: day.to_string(),
            version: snap.map_or_else(|| self.state.version.clone(), |s| s.version.clone()),
            windows: snap.map(|s| s.windows.clone()).unwrap_or_default(),
            counts: self.state.counts.clone(),
            tabs: snap.map_or("", |s| tabs_bucket(s.pages)).to_string(),
            default_browser: snap.is_some_and(|s| s.default_browser),
        }
    }

    /// Раз в тик: день сменился — закрыть прошлый. Вернуть отчёты, которые пора отправить
    /// (они остаются в очереди, пока не придёт `sent`).
    pub fn due(&mut self, today: &str, snap: &Snapshot) -> Vec<Report> {
        if !self.on {
            return Vec::new();
        }
        self.start_day(today, Some(snap));
        // Отчёты, собранные без снимка (день сменился в `record`), дополняем сейчас.
        for r in &mut self.state.pending {
            if r.windows.is_empty() {
                r.windows = snap.windows.clone();
                r.tabs = tabs_bucket(snap.pages).to_string();
                r.default_browser = snap.default_browser;
            }
        }
        self.state.pending.clone()
    }

    /// Сервер принял отчёт за день `day` — убрать его из очереди.
    pub fn sent(&mut self, day: &str) {
        self.state.pending.retain(|r| r.day != day);
        self.dirty = true;
    }

    /// Каким будет отчёт за сегодня (кнопка «Что отправляется»).
    pub fn preview(&self, snap: &Snapshot) -> Option<Report> {
        self.on.then(|| self.report_with(&self.state.day, Some(snap)))
    }

    /// Для записи в фоне: данные, если были изменения.
    pub fn take_changes(&mut self) -> Option<String> {
        if !self.on || !std::mem::take(&mut self.dirty) {
            return None;
        }
        serde_json::to_string(&self.state).ok()
    }
}

/// Случайный номер установки: 128 бит из генератора, которым std засевает хеш-таблицы.
fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// Версия Windows: «10.0.26100» (у Windows 11 тоже 10.0 — различает номер сборки).
#[cfg(windows)]
pub fn windows_version() -> String {
    let build = crate::instance::reg_string(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuild", true);
    format!("10.0.{}", build.unwrap_or_default())
}

#[cfg(not(windows))]
pub fn windows_version() -> String {
    String::new()
}

/// Отправить отчёт. Ok — сервер принял (2xx).
pub fn send(endpoint: &str, report: &Report) -> Result<(), String> {
    let body = serde_json::to_vec(report).map_err(|e| e.to_string())?;
    let r = crate::net::https(
        "POST",
        endpoint,
        &[("Content-Type", "application/json")],
        Some(&body),
        16 * 1024,
        std::time::Duration::from_secs(30),
    )?;
    if (200..300).contains(&r.status) { Ok(()) } else { Err(format!("сервер ответил {}", r.status)) }
}

#[cfg(test)]
#[path = "../tests/unit/telemetry.rs"]
mod tests;
