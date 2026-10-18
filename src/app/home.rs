//! New Page и Windows: домашняя страница, браузер по умолчанию.

use super::*;

impl App {
    // ---------- New Page ----------

    /// New Page: убрать страницу с экрана — под ней интерфейс рисует New Page.
    pub(super) fn show_home(&mut self) {
        if self.find_open {
            self.find_open = false;
            self.run_find("", "clear");
        }
        if let Some(a) = self.pages.active() {
            self.capture(a);
            self.home_return = Some(a);
        }
        self.pages.deactivate();
        self.layout();
        self.focus_ui_if_empty();
        self.push_state();
        self.save();
    }

    /// Данные карточек New Page — только когда она видна. «Недавние» — последние сайты из
    /// истории, кроме открытых страниц (их показывает «Продолжить») и недавно закрытых
    /// (они в `closed` и открываются вместе со своей историей).
    pub(super) fn home_data(&self) -> Value {
        if self.pages.active().is_some() {
            return Value::Null;
        }
        let mut skip: std::collections::HashSet<&str> = self.pages.closed().iter().map(|c| c.url.as_str()).collect();
        for i in 0..self.spaces.len() {
            skip.extend(self.space_pages(i).iter().map(|t| t.url.as_str()));
        }
        let recent: Vec<Value> = self
            .history
            .recent_where(HOME_RECENT, |e| !skip.contains(e.url.as_str()))
            .into_iter()
            .map(|e| json!({ "title": e.title, "url": e.url, "ts": e.ts, "favicon": self.icon_for(&e.url, "") }))
            .collect();
        json!({ "recent": recent, "canReturn": self.home_return.is_some(), "returnTo": self.home_return })
    }

    pub(super) fn make_default_browser(&self) {
        if !instance::registered_as_browser() {
            self.toast(i18n::tr("Install Jiyda with its installer first: it adds Jiyda to Windows' list of browsers"));
            return;
        }
        if !instance::open_default_apps_settings() {
            log::error!("не удалось открыть «Параметры» → «Приложения по умолчанию»");
            self.toast(i18n::tr("Couldn't open Settings"));
        }
    }
}
