//! Peek: предпросмотр ссылки поверх страницы.

use super::*;

impl App {
    // ---------- Peek ----------

    /// Предпросмотр ссылки со страницы `from` (Shift+клик).
    pub(super) fn open_peek(&mut self, url: String) {
        self.close_peek();
        let id = self.pages.alloc_id();
        let (x, y, w, h) = self.content_box();
        let pw = (w * 0.78).clamp(320.0, 1100.0);
        let ph = (h - 96.0).max(240.0);
        let rect = (x + (w - pw) / 2.0, y + 56.0, pw, ph);
        let Some(view) = self.build_webview(id, &url, true) else { return };
        view.set_bounds(wry::Rect {
            position: LogicalPosition::new(rect.0, rect.1).into(),
            size: LogicalSize::new(rect.2, rect.3).into(),
        })
        .or_log("set_bounds");
        view.focus().or_log("focus");
        self.peek = Some(Peek { id, url, view, rect });
        self.push_state();
    }

    pub(super) fn close_peek(&mut self) {
        if self.peek.take().is_some() {
            self.push_state();
            self.with_active_wv(|wv| {
                wv.focus().or_log("focus");
            });
        }
    }

    /// С предпросмотром начали работать — он становится настоящей страницей.
    pub(super) fn promote_peek(&mut self) {
        let Some(p) = self.peek.take() else { return };
        let url = p.view.url().unwrap_or(p.url);
        let parent = self.pages.active();
        self.pages.insert(p.id, url, now(), parent, Some(p.view));
        if let Some(t) = self.pages.get_mut(p.id) {
            t.loading = false;
        }
        self.activate(p.id);
    }
}
