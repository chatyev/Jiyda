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

    /// Плитки New Page — только когда она видна.
    pub(super) fn home_data(&self) -> Value {
        if self.pages.active().is_some() {
            return Value::Null;
        }
        let top: Vec<Value> = self
            .history
            .top_sites(HOME_TILES)
            .into_iter()
            .map(|s| json!({ "host": s.host, "url": s.url, "favicon": self.favicons.get(&s.host).unwrap_or("") }))
            .collect();
        json!({ "top": top, "canReturn": self.home_return.is_some() })
    }

    pub(super) fn make_default_browser(&self) {
        if !instance::registered_as_browser() {
            self.toast(i18n::tr("Install Seyro with its installer first: it adds Seyro to Windows' list of browsers"));
            return;
        }
        if !instance::open_default_apps_settings() {
            log::error!("не удалось открыть «Параметры» → «Приложения по умолчанию»");
            self.toast(i18n::tr("Couldn't open Settings"));
        }
    }
}
