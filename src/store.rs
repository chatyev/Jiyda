//! Сохранение сессии (вкладки) и истории посещений на диск.

#[cfg(test)]
use crate::config::write_atomic;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedTab {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub pinned_url: Option<String>,
    pub favicon: String,
    pub last_active: u64,
    /// Страница отложена (Orbit → Later). В сессии пишется только true.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub later: bool,
    /// Когда страницу открыли (0 — неизвестно, старая сессия).
    #[serde(skip_serializing_if = "is_zero")]
    pub created_at: u64,
    /// Страница, с которой открыли эту: индекс в том же списке сессии.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    /// Когда закрыли (только у закрытых и архивированных).
    #[serde(skip_serializing_if = "is_zero")]
    pub closed_at: u64,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    /// Пространства (Spaces) со своими вкладками.
    pub spaces: Vec<SavedSpace>,
    /// Индекс активного пространства в `spaces`.
    pub active_space: usize,
    pub sidebar_visible: Option<bool>,
    /// Тема интерфейса: "system" (как в Windows), "light" или "dark".
    pub theme: Option<String>,
    /// Удалённые пространства: их папки данных стираются при следующем запуске
    /// (пока браузер работает, файлы заняты).
    pub deleted_spaces: Vec<u32>,

    // Старый формат — одно пространство прямо в сессии. Читается, чтобы перенести
    // его в «Основное» (см. `take_spaces`), и больше не пишется.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<SavedTab>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub closed: Vec<SavedTab>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<SavedSplit>,
}

/// Пространство в сессии.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedSpace {
    /// 1 — «Основное»: его профиль — старая папка webview\, остальные — spaces\<id>\.
    pub id: u32,
    pub name: String,
    /// Ключ цвета из палитры (см. spaces.rs и ui/index.html).
    pub color: String,
    pub tabs: Vec<SavedTab>,
    pub active: Option<usize>,
    /// Недавно закрытые и архивированные вкладки (для Ctrl+Shift+T).
    pub closed: Vec<SavedTab>,
    /// Пары Split View: индексы в `tabs`.
    pub splits: Vec<SavedSplit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SavedSplit {
    pub left: usize,
    pub right: usize,
    pub ratio: f64,
}

impl Session {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&self, path: &Path) {
        if let Some(s) = self.to_json() {
            write_atomic(path, &s);
        }
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(self).ok()
    }

    /// Забрать пространства. Сессия старого формата (без пространств)
    /// становится одним пространством «Основное» с её вкладками.
    pub fn take_spaces(&mut self) -> Vec<SavedSpace> {
        if !self.spaces.is_empty() {
            return std::mem::take(&mut self.spaces);
        }
        vec![SavedSpace {
            id: 1,
            name: String::new(),
            color: String::new(),
            tabs: std::mem::take(&mut self.tabs),
            active: self.active.take(),
            closed: std::mem::take(&mut self.closed),
            splits: std::mem::take(&mut self.splits),
        }]
    }
}

/// Сайт для плитки домашней страницы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopSite {
    /// Домен без «www.».
    pub host: String,
    pub url: String,
    pub visits: u32,
    pub last: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistEntry {
    pub url: String,
    pub title: String,
    pub ts: u64,
    pub visits: u32,
}

pub struct History {
    pub entries: Vec<HistEntry>,
    /// Есть изменения, ещё не записанные на диск.
    dirty: bool,
}

const HISTORY_LIMIT: usize = 5000;

impl History {
    pub fn load(path: &Path) -> Self {
        let entries =
            std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { entries, dirty: false }
    }

    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&mut self, path: &Path) {
        if let Some(s) = self.to_json() {
            write_atomic(path, &s);
        }
        self.dirty = false;
    }

    /// Для записи в фоне: данные и отметка «записано».
    pub fn take_changes(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.to_json()
    }

    fn to_json(&self) -> Option<String> {
        serde_json::to_string(&self.entries).ok()
    }

    #[cfg(test)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Записать посещение.
    pub fn visit(&mut self, url: &str, title: &str) {
        if !url.starts_with("http") {
            return;
        }
        let ts = now();
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url) {
            e.ts = ts;
            e.visits += 1;
            if !title.is_empty() {
                e.title = title.to_string();
            }
        } else {
            self.entries.push(HistEntry { url: url.into(), title: title.into(), ts, visits: 1 });
            if self.entries.len() > HISTORY_LIMIT {
                self.entries.sort_by_key(|e| std::cmp::Reverse(e.ts));
                self.entries.truncate(HISTORY_LIMIT);
            }
        }
        self.dirty = true;
    }

    pub fn set_title(&mut self, url: &str, title: &str) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url && e.title != title) {
            e.title = title.to_string();
            self.dirty = true;
        }
    }

    /// Последние записи для командной палитры.
    pub fn recent(&self, n: usize) -> Vec<&HistEntry> {
        let mut v: Vec<_> = self.entries.iter().collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.ts));
        v.truncate(n);
        v
    }

    /// Самые посещаемые сайты для плиток домашней страницы: посещения страниц
    /// одного сайта складываются; адрес плитки — главная страница сайта.
    pub fn top_sites(&self, n: usize) -> Vec<TopSite> {
        let mut sites: Vec<TopSite> = Vec::new();
        for e in &self.entries {
            let Ok(u) = url::Url::parse(&e.url) else { continue };
            let (Some(host), "http" | "https") = (u.host_str(), u.scheme()) else { continue };
            let host = host.strip_prefix("www.").unwrap_or(host);
            match sites.iter_mut().find(|s| s.host == host) {
                Some(s) => {
                    s.visits += e.visits;
                    s.last = s.last.max(e.ts);
                }
                None => sites.push(TopSite {
                    host: host.to_string(),
                    url: format!("{}://{}/", u.scheme(), u.host_str().unwrap_or(host)),
                    visits: e.visits,
                    last: e.ts,
                }),
            }
        }
        // Чаще — выше; при равенстве — то, что открывали недавно.
        sites.sort_by(|a, b| b.visits.cmp(&a.visits).then(b.last.cmp(&a.last)));
        sites.truncate(n);
        sites
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.dirty = true;
    }

    /// Удалить запись (из панели «История»); вернуть её, чтобы можно было отменить.
    pub fn remove(&mut self, url: &str) -> Option<HistEntry> {
        let i = self.entries.iter().position(|e| e.url == url)?;
        self.dirty = true;
        Some(self.entries.remove(i))
    }

    /// Вернуть удалённую запись (Undo). Если адрес за это время открыли снова,
    /// посещения складываются.
    pub fn restore(&mut self, entry: HistEntry) {
        match self.entries.iter_mut().find(|e| e.url == entry.url) {
            Some(e) => {
                e.visits += entry.visits;
                if e.title.is_empty() {
                    e.title = entry.title;
                }
            }
            None => self.entries.push(entry),
        }
        self.dirty = true;
    }

    /// Перенести историю другого браузера. Адрес уже есть — берётся больше посещений и
    /// более позднее время; всего остаётся не больше HISTORY_LIMIT, свежие. Возвращает,
    /// сколько адресов добавлено.
    pub fn import(&mut self, entries: Vec<HistEntry>) -> usize {
        let mut index: std::collections::HashMap<String, usize> =
            self.entries.iter().enumerate().map(|(i, e)| (e.url.clone(), i)).collect();
        let mut added = 0;
        for e in entries {
            if !e.url.starts_with("http") {
                continue;
            }
            match index.get(&e.url) {
                Some(&i) => {
                    let mine = &mut self.entries[i];
                    mine.visits = mine.visits.max(e.visits);
                    mine.ts = mine.ts.max(e.ts);
                    if mine.title.is_empty() {
                        mine.title = e.title;
                    }
                }
                None => {
                    index.insert(e.url.clone(), self.entries.len());
                    self.entries.push(e);
                    added += 1;
                }
            }
        }
        if self.entries.len() > HISTORY_LIMIT {
            self.entries.sort_by_key(|e| std::cmp::Reverse(e.ts));
            self.entries.truncate(HISTORY_LIMIT);
        }
        self.dirty = true;
        added
    }

    /// Записи, открытые в пределах `window` секунд от `ts` (кроме самой `url`),
    /// новые первыми: «Restore context» в истории.
    pub fn around(&self, url: &str, ts: u64, window: u64, n: usize) -> Vec<&HistEntry> {
        let mut v: Vec<_> = self.entries.iter().filter(|e| e.url != url && e.ts.abs_diff(ts) <= window).collect();
        v.sort_by_key(|e| e.ts.abs_diff(ts));
        v.truncate(n);
        v
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: u64,
    pub url: String,
    /// Куда сохраняется файл (может быть пустым, если WebView2 не сообщил).
    pub path: String,
    pub state: DownloadState,
    pub ts: u64,
    /// Ход загрузки: получено и всего байт (0 — неизвестно).
    #[serde(default)]
    pub received: u64,
    #[serde(default)]
    pub total: u64,
}

impl Download {
    /// Имя файла для списка: из пути, иначе из адреса.
    pub fn name(&self) -> String {
        let from_path = Path::new(&self.path).file_name().map(|n| n.to_string_lossy().into_owned());
        from_path
            .filter(|n| !n.is_empty())
            .or_else(|| {
                let no_query = self.url.split(['?', '#']).next().unwrap_or("");
                no_query.rsplit('/').find(|s| !s.is_empty()).map(str::to_string)
            })
            .unwrap_or_else(|| self.url.clone())
    }
}

/// Список загрузок для палитры (Ctrl+J).
pub struct Downloads {
    pub items: Vec<Download>,
    next_id: u64,
}

const DOWNLOADS_LIMIT: usize = 200;

impl Downloads {
    pub fn load(path: &Path) -> Self {
        let mut items: Vec<Download> =
            std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        // Браузер закрыли посреди загрузки — она уже не завершится.
        for d in &mut items {
            if d.state == DownloadState::Running {
                d.state = DownloadState::Failed;
            }
        }
        let next_id = items.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        Self { items, next_id }
    }

    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&self, path: &Path) {
        if let Some(s) = self.to_json() {
            write_atomic(path, &s);
        }
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(&self.items).ok()
    }

    pub fn started(&mut self, url: &str, path: &str, ts: u64) {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Download {
            id,
            url: url.into(),
            path: path.into(),
            state: DownloadState::Running,
            ts,
            received: 0,
            total: 0,
        });
        if self.items.len() > DOWNLOADS_LIMIT {
            let extra = self.items.len() - DOWNLOADS_LIMIT;
            self.items.drain(..extra);
        }
    }

    /// Отметить окончание загрузки с этого адреса (последней незавершённой).
    /// Если начало не было замечено, загрузка добавляется сразу завершённой.
    pub fn finished(&mut self, url: &str, path: Option<&str>, ok: bool, ts: u64) -> &Download {
        let state = if ok { DownloadState::Done } else { DownloadState::Failed };
        let idx = match self.items.iter().rposition(|d| d.url == url && d.state == DownloadState::Running) {
            Some(i) => i,
            None => {
                self.started(url, "", ts);
                self.items.len() - 1
            }
        };
        let d = &mut self.items[idx];
        d.state = state;
        if ok && d.total > 0 {
            d.received = d.total;
        }
        if let Some(p) = path.filter(|p| !p.is_empty()) {
            d.path = p.into();
        }
        d
    }

    pub fn get(&self, id: u64) -> Option<&Download> {
        self.items.iter().find(|d| d.id == id)
    }

    /// Ход идущей загрузки с этого адреса. true — нашлась.
    pub fn progress(&mut self, url: &str, received: u64, total: u64) -> bool {
        match self.items.iter_mut().rev().find(|d| d.url == url && d.state == DownloadState::Running) {
            Some(d) => {
                d.received = received;
                d.total = total;
                true
            }
            None => false,
        }
    }

    /// Убрать из списка (файл не трогается).
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|d| d.id != id);
        before != self.items.len()
    }

    /// Новые сверху.
    pub fn recent(&self) -> impl Iterator<Item = &Download> {
        self.items.iter().rev()
    }
}

/// Значки сайтов по домену (без «www.»): data: URI с PNG, который отдал сам WebView.
/// Нужны там, где страницы нет (история, New Page, закрытые) — интерфейс не грузит
/// значки из сети сам. Недавно обновлённые — в конце; самые старые вытесняются.
pub struct Favicons {
    items: Vec<(String, String)>,
    dirty: bool,
}

const FAVICONS_LIMIT: usize = 600;

impl Favicons {
    pub fn load(path: &Path) -> Self {
        let items: Vec<(String, String)> =
            std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let items = items.into_iter().filter(|(_, u)| u.starts_with("data:image/")).collect();
        Self { items, dirty: false }
    }

    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&mut self, path: &Path) {
        if let Some(s) = self.take_changes() {
            write_atomic(path, &s);
        }
    }

    /// Для записи в фоне: данные, если были изменения, и отметка «записано».
    pub fn take_changes(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        serde_json::to_string(&self.items).ok()
    }

    pub fn get(&self, host: &str) -> Option<&str> {
        self.items.iter().find(|(h, _)| h == host).map(|(_, u)| u.as_str())
    }

    /// Значок из другого браузера: только если своего нет и есть место. Встаёт как самый
    /// старый — свои значки он не вытесняет, а сам уходит первым.
    pub fn set_if_missing(&mut self, host: &str, uri: &str) {
        if host.is_empty() || !uri.starts_with("data:image/") || self.items.len() >= FAVICONS_LIMIT {
            return;
        }
        if self.get(host).is_none() {
            self.items.insert(0, (host.to_string(), uri.to_string()));
            self.dirty = true;
        }
    }

    /// Запомнить значок сайта. Принимается только data:image/ — никаких адресов в сети.
    pub fn set(&mut self, host: &str, uri: &str) {
        if host.is_empty() || !uri.starts_with("data:image/") {
            return;
        }
        if self.get(host) == Some(uri) {
            return;
        }
        self.items.retain(|(h, _)| h != host);
        self.items.push((host.to_string(), uri.to_string()));
        if self.items.len() > FAVICONS_LIMIT {
            let extra = self.items.len() - FAVICONS_LIMIT;
            self.items.drain(..extra);
        }
        self.dirty = true;
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
#[path = "../tests/unit/store.rs"]
mod tests;
