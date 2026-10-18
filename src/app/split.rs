//! Split View: две страницы рядом.

use super::*;

impl App {
    // ---------- Split View ----------

    /// Показать `right` рядом с `left` и перейти в правую половину.
    pub(super) fn split_with(&mut self, left: u64, right: u64) {
        if self.pages.make_split(left, right) {
            self.activate(right);
        }
    }

    pub(super) fn unsplit(&mut self, id: u64) {
        if self.pages.unsplit(id) {
            self.layout();
            self.push_state();
            self.save();
        }
    }

    pub(super) fn load_in(&mut self, id: u64, url: &str) {
        let has_wv = self.pages.get(id).is_some_and(|t| t.view.is_some());
        if let Some(t) = self.pages.get_mut(id) {
            t.url = url.to_string();
        }
        if has_wv && let Some(wv) = self.pages.get(id).and_then(|t| t.view.as_ref()) {
            wv.navigate(url);
        }
        self.activate(id);
    }

    pub(super) fn with_active_wv(&self, f: impl FnOnce(&View)) {
        if let Some(wv) = self.pages.active_page().and_then(|t| t.view.as_ref()) {
            f(wv);
        }
    }
}
