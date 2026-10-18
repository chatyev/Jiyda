//! Загрузки, история и очистка данных.

use super::*;

impl App {
    // ---------- Загрузки, история, данные ----------

    pub(super) fn downloads_json(&self) -> Value {
        Value::Array(
            self.downloads
                .recent()
                .take(100)
                .map(|d| {
                    json!({
                        "id": d.id, "name": d.name(), "path": d.path, "url": d.url, "state": d.state,
                        "ts": d.ts, "received": d.received, "total": d.total,
                    })
                })
                .collect(),
        )
    }

    pub(super) fn push_downloads(&self) {
        self.ui_data("downloads", self.downloads_json());
    }

    /// Показать файл в Проводнике.
    pub(super) fn reveal_download(&self, id: u64) {
        let Some(d) = self.downloads.get(id) else { return };
        if d.path.is_empty() || !std::path::Path::new(&d.path).exists() {
            self.toast(i18n::tr("File not found. It may have been moved or deleted"));
            return;
        }
        let mut cmd = std::process::Command::new("explorer");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // explorer разбирает /select сам, поэтому путь передаём одной строкой в кавычках.
            cmd.raw_arg(format!("/select,\"{}\"", d.path));
        }
        if let Err(e) = cmd.spawn() {
            log::error!("не удалось открыть Проводник: {e}");
        }
    }

    /// Открыть скачанный файл программой по умолчанию.
    pub(super) fn open_download(&self, id: u64) {
        let Some(d) = self.downloads.get(id) else { return };
        if d.path.is_empty() || !std::path::Path::new(&d.path).exists() {
            self.toast(i18n::tr("File not found. It may have been moved or deleted"));
            return;
        }
        // explorer открывает файл так же, как двойной клик в Проводнике.
        if let Err(e) = std::process::Command::new("explorer").arg(&d.path).spawn() {
            log::error!("не удалось открыть файл: {e}");
        }
    }

    pub(super) fn open_folder(&self, dir: &std::path::Path) {
        if let Err(e) = std::process::Command::new("explorer").arg(dir).spawn() {
            log::error!("не удалось открыть папку {}: {e}", dir.display());
        }
    }

    pub(super) fn download_dir(&self) -> Option<std::path::PathBuf> {
        if self.cfg.download_dir.is_empty() { dirs::download_dir() } else { Some(self.cfg.download_dir.clone().into()) }
    }

    pub(super) fn clear_history(&mut self) {
        self.history.clear();
        self.save_now();
        log::info!("история очищена");
        self.toast(i18n::tr("History cleared"));
    }

    /// Куки, кэш, хранилища и сохранённые пароли всех сайтов пространства.
    /// Очистка идёт через любой WebView этого профиля: у «Основного» это интерфейс,
    /// у остальных — открытая страница пространства.
    pub(super) fn clear_site_data(&self) {
        let space = &self.spaces[self.active_space];
        let wv =
            if space.id == spaces::MAIN_ID { Some(&self.ui) } else { self.pages.iter().find_map(|t| t.view.as_ref()) };
        let Some(wv) = wv else {
            self.toast(i18n::tr("Open any page in this Space and try again"));
            return;
        };
        match wv.clear_profile_data() {
            Ok(()) => {
                log::info!("данные сайтов пространства «{}» очищены", space.name);
                self.toast(&i18n::trf("Site data in \u{201c}{}\u{201d} cleared", &[&space.name]));
            }
            Err(e) => {
                log::error!("не удалось очистить данные сайтов: {e}");
                self.toast(i18n::tr("Couldn't clear site data"));
            }
        }
    }

    pub(super) fn history_json(&self) -> Value {
        Value::Array(
            self.history
                .recent(3000)
                .into_iter()
                .map(|e| json!({ "url": e.url, "title": e.title, "ts": e.ts, "visits": e.visits, "favicon": self.icon_for(&e.url, "") }))
                .collect(),
        )
    }

    pub(super) fn remove_history(&mut self, url: &str) {
        if let Some(e) = self.history.remove(url) {
            self.removed_history.push(e);
            self.save();
            self.ui_data("history", self.history_json());
            self.toast_action(i18n::tr("Removed from history"), i18n::tr("Undo"), json!({ "type": "history_undo" }));
        }
    }

    pub(super) fn undo_history_remove(&mut self) {
        if let Some(e) = self.removed_history.pop() {
            self.history.restore(e);
            self.save();
            self.ui_data("history", self.history_json());
        }
    }

    /// Restore context: открыть страницы, которые смотрели примерно тогда же.
    pub(super) fn history_context(&mut self, url: &str, ts: u64) {
        let around: Vec<String> = self.history.around(url, ts, 15 * 60, 6).into_iter().map(|e| e.url.clone()).collect();
        for u in &around {
            self.open_tab(u.clone(), true, None);
        }
        self.open_tab(url.to_string(), false, None);
        if !around.is_empty() {
            self.toast(&i18n::trf("Opened {} pages from the same time", &[&(around.len() + 1)]));
        }
    }
}
