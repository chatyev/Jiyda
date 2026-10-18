//! Закладки (панель Ctrl+Shift+O, звёздочка в адресной строке) и перенос из Chrome и Edge.

use super::*;
use crate::bookmarks::Node;
use crate::import::{self, Source, What};

/// Перенос из других браузеров: найденные профили и идёт ли чтение.
#[derive(Default)]
pub(super) struct ImportState {
    pub sources: Vec<Source>,
    pub busy: bool,
}

impl ImportState {
    pub fn scan() -> Self {
        let sources = import::default_root().map(|r| import::sources(&r)).unwrap_or_default();
        Self { sources, busy: false }
    }
}

impl App {
    // ---------- Закладки ----------

    pub(super) fn bookmarks_json(&self) -> Value {
        json!({ "items": self.bookmarks.items() })
    }

    pub(super) fn push_bookmarks(&self) {
        self.ui_data("bookmarks", self.bookmarks_json());
    }

    fn bookmarks_changed(&mut self) {
        self.save();
        self.push_bookmarks();
        self.push_state();
    }

    /// Ctrl+D и звёздочка: окно закладки для текущей страницы — новой (название, папка;
    /// сохраняется только по «Сохранить») или уже сохранённой (изменить, удалить).
    pub(super) fn bookmark_page(&mut self) {
        let Some(t) = self.pages.active_page() else { return };
        if !is_web_url(&t.url) {
            return;
        }
        let page = json!({ "url": t.url, "title": if t.title.is_empty() { &t.url } else { &t.title } });
        // Окну нужны папки и сама закладка, если она есть, — данные раньше команды.
        self.push_bookmarks();
        self.ui_command("bookmark_page", page);
    }

    /// Сохранить страницу в закладки (окно Ctrl+D, «Добавить текущую страницу» в панели).
    /// Адрес уже в закладках — второй не появится: у прежней меняются название и папка.
    pub(super) fn add_bookmark(&mut self, url: &str, title: &str, parent: Option<u64>) {
        let url = url.trim();
        if !is_web_url(url) {
            self.toast(i18n::tr("Bookmark address must start with http:// or https://"));
            return;
        }
        let (_, new) = self.bookmarks.save_page(parent, title, url, now());
        self.bookmarks_changed();
        self.toast(i18n::tr(if new { "Added to bookmarks" } else { "Bookmark saved" }));
    }

    pub(super) fn remove_bookmark(&mut self, id: u64) {
        let Some(r) = self.bookmarks.remove(id) else { return };
        let text = if r.node.is_folder() { i18n::tr("Folder deleted") } else { i18n::tr("Removed from bookmarks") };
        self.removed_bookmarks.push(r);
        self.bookmarks_changed();
        self.toast_action(text, i18n::tr("Undo"), json!({ "type": "bookmark_undo" }));
    }

    pub(super) fn undo_bookmark_remove(&mut self) {
        if let Some(r) = self.removed_bookmarks.pop() {
            self.bookmarks.restore(r);
            self.bookmarks_changed();
        }
    }

    pub(super) fn edit_bookmark(&mut self, id: u64, title: &str, url: Option<&str>) {
        // Адрес — только сайта: закладка не должна открывать javascript: или файлы.
        if url.is_some_and(|u| !u.trim().is_empty() && !is_web_url(u.trim())) {
            self.toast(i18n::tr("Bookmark address must start with http:// or https://"));
            self.push_bookmarks();
            return;
        }
        if self.bookmarks.edit(id, title, url) {
            self.bookmarks_changed();
        }
    }

    pub(super) fn move_bookmark(&mut self, id: u64, parent: Option<u64>, index: Option<usize>) {
        if self.bookmarks.move_to(id, parent, index) {
            self.bookmarks_changed();
        }
    }

    pub(super) fn new_bookmark_folder(&mut self, parent: Option<u64>, title: &str) {
        let title = if title.trim().is_empty() { i18n::tr("New folder") } else { title.trim() };
        if self.bookmarks.add_folder(parent, title).is_some() {
            self.bookmarks_changed();
        }
    }

    /// Открыть все ссылки папки (без вложенных) фоновыми страницами.
    pub(super) fn open_bookmark_folder(&mut self, id: u64) {
        let Some(urls) = self
            .bookmarks
            .get(id)
            .and_then(|n| n.children.as_ref())
            .map(|c| c.iter().filter(|n| !n.is_folder()).map(|n| n.url.clone()).collect::<Vec<_>>())
        else {
            return;
        };
        // Сайтам нельзя открыть много страниц разом (limit.rs), а здесь просит пользователь —
        // но и ему больше 20 за раз не открываем: это скорее промах, чем желание.
        const MAX: usize = 20;
        for url in urls.iter().take(MAX) {
            self.open_tab(url.clone(), true, None);
        }
        if urls.len() > MAX {
            self.toast(&i18n::trf("Opened the first {} pages", &[&MAX]));
        }
    }

    // ---------- Перенос из Chrome и Edge ----------

    pub(super) fn import_json(&self) -> Value {
        let multi = |browser: &str| self.import.sources.iter().filter(|s| s.browser == browser).count() > 1;
        json!({
            "busy": self.import.busy,
            "sources": self.import.sources.iter().map(|s| json!({
                "id": s.id(),
                "browser": s.browser,
                "name": s.browser_name,
                "profile": if multi(s.browser) { s.profile_name.as_str() } else { "" },
            })).collect::<Vec<_>>(),
        })
    }

    /// Предложить перенос на New Page: ещё не предлагали и есть откуда.
    pub(super) fn import_offer(&self) -> Option<Value> {
        if self.cfg.import_offered || self.import.sources.is_empty() {
            return None;
        }
        let mut names: Vec<&str> = self.import.sources.iter().map(|s| s.browser_name).collect();
        names.dedup();
        Some(json!({ "browsers": names }))
    }

    pub(super) fn dismiss_import_offer(&mut self) {
        if !self.cfg.import_offered {
            self.cfg.import_offered = true;
            self.save_config();
            self.push_state();
        }
    }

    /// Начать перенос из профиля `source` ("chrome:Default"): чтение — в фоне.
    pub(super) fn start_import(&mut self, source: &str, what: What) {
        if self.import.busy || !(what.bookmarks || what.history) {
            return;
        }
        // Список профилей мог измениться (поставили Chrome, пока Jiyda работал).
        self.import = ImportState::scan();
        let Some(src) = self.import.sources.iter().find(|s| s.id() == source).cloned() else {
            self.toast(i18n::tr("That browser profile is no longer available"));
            self.ui_data("import", self.import_json());
            return;
        };
        self.import.busy = true;
        self.ui_data("import", self.import_json());
        let label = self.import_label(&src);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let data = import::read(&src, what);
            let _ = proxy.send_event(UserEvent::Imported { label, data: Box::new(data) });
        });
    }

    /// Имя папки для закладок профиля: «Google Chrome» или «Google Chrome · Work».
    fn import_label(&self, src: &Source) -> String {
        let several = self.import.sources.iter().filter(|s| s.browser == src.browser).count() > 1;
        if several { format!("{} · {}", src.browser_name, src.profile_name) } else { src.browser_name.to_string() }
    }

    pub(super) fn on_imported(&mut self, label: String, data: import::Imported) {
        self.import.busy = false;
        for e in &data.errors {
            log::warn!("импорт из {label}: {e}");
        }
        let mut nodes = data.bar;
        if !data.other.is_empty() {
            nodes.push(Node::folder(i18n::tr("Other bookmarks"), data.other));
        }
        if !data.mobile.is_empty() {
            nodes.push(Node::folder(i18n::tr("Mobile bookmarks"), data.mobile));
        }
        let bookmarks = self.bookmarks.import(&label, nodes);
        let history = data.history.len();
        if history > 0 {
            self.history.import(data.history);
        }
        for (domain, png) in &data.favicons {
            self.favicons.set_if_missing(domain, &format!("data:image/png;base64,{}", base64(png)));
        }
        log::info!(
            "импорт из {label}: закладок {bookmarks}, записей истории {history}, значков {}",
            data.favicons.len()
        );
        self.cfg.import_offered = true;
        self.save_config();
        self.save();
        if bookmarks == 0 && history == 0 {
            self.toast(&if data.errors.is_empty() {
                i18n::trf("Nothing new to import from {}", &[&label])
            } else {
                i18n::trf("Couldn't read data from {}. Close it and try again", &[&label])
            });
        } else {
            self.toast(&i18n::trf(
                "Imported from {}: {} bookmarks, {} history entries",
                &[&label, &bookmarks, &history],
            ));
        }
        let mut info = self.import_json();
        info["done"] = json!({ "bookmarks": bookmarks, "history": history });
        self.ui_data("import", info);
        self.push_bookmarks();
        self.ui_data("history", self.history_json());
        self.push_state();
    }
}

/// Закладка — только на сайт: не javascript:, не файл и не служебная страница.
fn is_web_url(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}
