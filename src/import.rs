//! Перенос из Google Chrome и Microsoft Edge: закладки, история, значки сайтов.
//!
//! Оба браузера — Chromium и хранят профиль одинаково (`<LOCALAPPDATA>\…\User Data\<профиль>`):
//! закладки — JSON `Bookmarks`, история — SQLite `History`, значки — SQLite `Favicons`.
//! Базы, открытые работающим браузером, читаются из копии во временной папке.
//!
//! Пароли и входы на сайты (куки) не переносятся: Chrome шифрует их ключом, привязанным
//! к себе (App-Bound Encryption), а положить их в хранилище WebView2 нечем.

use crate::bookmarks::Node;
use crate::store::HistEntry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Браузеры, из которых переносим: ключ, название, папка данных внутри %LOCALAPPDATA%.
const BROWSERS: &[(&str, &str, &str)] =
    &[("chrome", "Google Chrome", r"Google\Chrome\User Data"), ("edge", "Microsoft Edge", r"Microsoft\Edge\User Data")];

/// Сколько записей истории переносим (столько же держит своя история, store.rs).
pub const HISTORY_MAX: usize = 5000;
/// Значков сайтов — не больше (своё хранилище держит 600, оставляем место своим).
const FAVICONS_MAX: usize = 300;

/// Профиль браузера, из которого можно перенести данные.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// "chrome" или "edge".
    pub browser: &'static str,
    pub browser_name: &'static str,
    /// Папка профиля внутри User Data ("Default", "Profile 1").
    pub profile: String,
    /// Имя профиля, как его видит пользователь.
    pub profile_name: String,
    pub dir: PathBuf,
}

impl Source {
    /// Ключ для интерфейса: "chrome:Default".
    pub fn id(&self) -> String {
        format!("{}:{}", self.browser, self.profile)
    }
}

/// Что переносить.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct What {
    pub bookmarks: bool,
    pub history: bool,
}

/// Прочитанное из профиля.
#[derive(Debug, Default)]
pub struct Imported {
    /// Панель закладок, «Другие» и «Мобильные» (папки Chrome верхнего уровня).
    pub bar: Vec<Node>,
    pub other: Vec<Node>,
    pub mobile: Vec<Node>,
    /// Новые первыми.
    pub history: Vec<HistEntry>,
    /// Значки сайтов: домен (как orbit::domain) → PNG.
    pub favicons: Vec<(String, Vec<u8>)>,
    /// Что не удалось прочитать (для журнала и сообщения пользователю).
    pub errors: Vec<String>,
}

/// Где искать браузеры: %LOCALAPPDATA%. В отладочной сборке JIYDA_IMPORT_ROOT подменяет
/// её (тесты E2E подкладывают свой «Chrome»).
pub fn default_root() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("JIYDA_IMPORT_ROOT") {
        return Some(PathBuf::from(p));
    }
    dirs::data_local_dir()
}

/// Профили установленных браузеров, в которых есть закладки или история.
pub fn sources(root: &Path) -> Vec<Source> {
    let mut out = Vec::new();
    for (browser, browser_name, rel) in BROWSERS {
        let user_data = root.join(rel);
        for (profile, profile_name) in profiles(&user_data) {
            let dir = user_data.join(&profile);
            if dir.join("Bookmarks").is_file() || dir.join("History").is_file() {
                out.push(Source { browser, browser_name, profile, profile_name, dir });
            }
        }
    }
    out
}

/// Профили из `Local State` (в порядке, как их показывает браузер); нет файла — "Default".
fn profiles(user_data: &Path) -> Vec<(String, String)> {
    let state: serde_json::Value = std::fs::read_to_string(user_data.join("Local State"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let cache = state["profile"]["info_cache"].as_object();
    let mut names: Vec<String> = state["profile"]["profiles_order"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if let Some(cache) = cache {
        let mut rest: Vec<&String> = cache.keys().filter(|k| !names.contains(k)).collect();
        rest.sort();
        names.extend(rest.into_iter().cloned());
    }
    if names.is_empty() {
        names.push("Default".into());
    }
    names
        .into_iter()
        // Только имя папки: Local State мог бы указать путь за пределы User Data.
        .filter(|p| !p.is_empty() && !p.contains(['/', '\\']) && p != "." && p != "..")
        .map(|p| {
            let name = cache
                .and_then(|c| c.get(&p))
                .and_then(|v| v["name"].as_str())
                .filter(|n| !n.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| p.clone());
            (p, name)
        })
        .collect()
}

/// Прочитать профиль. Ошибки отдельных частей не мешают остальным — они в `errors`.
pub fn read(src: &Source, what: What) -> Imported {
    let mut out = Imported::default();
    if what.bookmarks {
        match std::fs::read_to_string(src.dir.join("Bookmarks")) {
            Ok(s) => match parse_bookmarks(&s) {
                Ok((bar, other, mobile)) => (out.bar, out.other, out.mobile) = (bar, other, mobile),
                Err(e) => out.errors.push(format!("закладки: {e}")),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => out.errors.push(format!("закладки: {e}")),
        }
    }
    // Своя временная папка на каждое чтение: два импорта подряд не мешают друг другу.
    static RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("jiyda-import-{}-{run}", std::process::id()));
    if what.history && src.dir.join("History").is_file() {
        match snapshot(&src.dir.join("History"), &tmp).and_then(|db| read_history(&db, HISTORY_MAX)) {
            Ok(h) => out.history = h,
            Err(e) => out.errors.push(format!("история: {e}")),
        }
    }
    // Значки — для сайтов из закладок и самых посещаемых из истории.
    let wanted = wanted_domains(&out);
    if !wanted.is_empty() && src.dir.join("Favicons").is_file() {
        match snapshot(&src.dir.join("Favicons"), &tmp).and_then(|db| read_favicons(&db, &wanted)) {
            Ok(f) => out.favicons = f,
            // Без значков перенос всё равно удался: только в журнал.
            Err(e) => log::warn!("импорт: значки не прочитаны: {e}"),
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    out
}

/// Время Chromium (микросекунды с 1601-01-01) → секунды Unix.
fn chrome_time(us: i64) -> u64 {
    const EPOCH_DIFF: i64 = 11_644_473_600;
    (us / 1_000_000 - EPOCH_DIFF).max(0) as u64
}

/// Корни закладок Chromium: панель закладок, другие, мобильные.
pub type Roots = (Vec<Node>, Vec<Node>, Vec<Node>);

/// Разобрать файл Bookmarks.
pub fn parse_bookmarks(json: &str) -> Result<Roots, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let roots = v.get("roots").filter(|r| r.is_object()).ok_or("нет roots")?;
    let children = |key: &str| roots[key]["children"].as_array().map(|c| nodes(c, 0)).unwrap_or_default();
    Ok((children("bookmark_bar"), children("other"), children("synced")))
}

fn nodes(items: &[serde_json::Value], depth: usize) -> Vec<Node> {
    // Глубже — скорее порченый файл, чем чьи-то папки.
    if depth > 32 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for it in items {
        let title = it["name"].as_str().unwrap_or("").trim();
        let added = it["date_added"].as_str().and_then(|s| s.parse::<i64>().ok()).map(chrome_time).unwrap_or(0);
        match it["type"].as_str() {
            Some("url") => {
                let url = it["url"].as_str().unwrap_or("");
                // Только сайты: javascript:-закладки и служебные chrome:// здесь не работают.
                if url.starts_with("https://") || url.starts_with("http://") {
                    out.push(Node::link(title, url, added));
                }
            }
            Some("folder") => {
                let c = it["children"].as_array().map(|c| nodes(c, depth + 1)).unwrap_or_default();
                if !c.is_empty() {
                    out.push(Node::folder(title, c));
                }
            }
            _ => {}
        }
    }
    out
}

/// Скопировать базу (и её журнал) во временную папку: работающий браузер держит файл.
fn snapshot(db: &Path, tmp: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let name = db.file_name().ok_or("нет имени файла")?;
    let to = tmp.join(name);
    std::fs::copy(db, &to).map_err(|e| format!("не удалось скопировать {}: {e}", db.display()))?;
    for suffix in ["-journal", "-wal"] {
        let mut side = db.as_os_str().to_owned();
        side.push(suffix);
        let mut side_to = to.as_os_str().to_owned();
        side_to.push(suffix);
        let _ = std::fs::remove_file(&side_to);
        if Path::new(&side).is_file() {
            let _ = std::fs::copy(&side, &side_to);
        }
    }
    Ok(to)
}

/// История из базы History: посещённые адреса сайтов, новые первыми.
pub fn read_history(db: &Path, limit: usize) -> Result<Vec<HistEntry>, String> {
    let conn = rusqlite::Connection::open(db).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT url, title, visit_count, last_visit_time FROM urls \
             WHERE hidden = 0 AND last_visit_time > 0 ORDER BY last_visit_time DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let Ok((url, title, visits, last)) = row else { continue };
        if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() > 4096 {
            continue;
        }
        out.push(HistEntry {
            url,
            title: title.unwrap_or_default(),
            ts: chrome_time(last),
            visits: visits.clamp(1, u32::MAX as i64) as u32,
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// Домены, для которых нужны значки: из закладок и самые посещаемые из истории.
fn wanted_domains(imp: &Imported) -> HashSet<String> {
    let mut out = HashSet::new();
    fn links(nodes: &[Node], out: &mut HashSet<String>) {
        for n in nodes {
            match &n.children {
                Some(c) => links(c, out),
                None => {
                    out.insert(crate::orbit::domain(&n.url));
                }
            }
        }
    }
    for list in [&imp.bar, &imp.other, &imp.mobile] {
        links(list, &mut out);
    }
    let mut hist: Vec<&HistEntry> = imp.history.iter().collect();
    hist.sort_by_key(|e| std::cmp::Reverse(e.visits));
    for e in hist {
        if out.len() >= FAVICONS_MAX {
            break;
        }
        out.insert(crate::orbit::domain(&e.url));
    }
    out.remove("");
    out
}

/// Значки из базы Favicons: на домен — PNG размера ближе всего к 32 px.
pub fn read_favicons(db: &Path, wanted: &HashSet<String>) -> Result<Vec<(String, Vec<u8>)>, String> {
    let conn = rusqlite::Connection::open(db).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT m.page_url, b.image_data, b.width FROM icon_mapping m \
             JOIN favicon_bitmaps b ON b.icon_id = m.icon_id \
             WHERE b.width BETWEEN 16 AND 64 AND length(b.image_data) BETWEEN 1 AND 65536",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, i64>(2)?)))
        .map_err(|e| e.to_string())?;
    let mut best: HashMap<String, (i64, Vec<u8>)> = HashMap::new();
    for row in rows {
        let Ok((page, png, width)) = row else { continue };
        let domain = crate::orbit::domain(&page);
        if !wanted.contains(&domain) || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            continue;
        }
        let score = (width - 32).abs();
        if best.get(&domain).is_none_or(|(s, _)| score < *s) {
            best.insert(domain, (score, png));
        }
    }
    let mut out: Vec<(String, Vec<u8>)> = best.into_iter().map(|(d, (_, png))| (d, png)).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.truncate(FAVICONS_MAX);
    Ok(out)
}

#[cfg(test)]
#[path = "../tests/unit/import.rs"]
mod tests;
