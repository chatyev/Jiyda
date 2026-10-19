//! Раскладка окна: где страницы и интерфейс, регион, полноэкранный режим.

use super::*;

impl App {
    // ---------- Раскладка ----------

    pub(super) fn window_size(&self) -> (f64, f64) {
        let s = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        (s.width, s.height)
    }

    /// Видна ли верхняя панель над страницей (в полноэкранном режиме — нет).
    pub(super) fn chrome_visible(&self) -> bool {
        !self.fullscreen && self.page_fullscreen.is_none()
    }

    /// Область страницы: (x, y, ширина, высота) в логических пикселях.
    pub(super) fn content_box(&self) -> Box4 {
        let (w, h) = self.window_size();
        // Закреплённая панель закладок справа — страница рядом с ней, а не под ней (кроме
        // полноэкранного видео со страницы).
        let w = if self.page_fullscreen.is_none() { (w - self.sidebar).max(200.0) } else { w };
        if self.chrome_visible() { (0.0, CHROME_H, w, h - CHROME_H) } else { (0.0, 0.0, w, h) }
    }

    /// Окно изменило размер (в том числе развернули или восстановили).
    pub fn on_resized(&mut self) {
        self.layout();
        let maximized = self.window.is_maximized();
        if maximized != self.maximized {
            self.maximized = maximized;
            self.push_state();
        }
        self.apply_region();
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// Где страницы: активная или обе половины пары; Peek — поверх.
    pub fn layout(&mut self) {
        let (w, h) = self.window_size();
        self.ui.set_bounds(rect(0.0, 0.0, w, h));
        let content = self.content_box();
        let mut shown: Vec<(u64, Box4)> = Vec::new();
        if let Some(id) = self.page_fullscreen {
            shown.push((id, (0.0, 0.0, w, h)));
        } else if let Some(active) = self.pages.active() {
            match self.pages.split_of(active) {
                Some(s) => {
                    // Над половинами — строка с сайтом и действиями (кроме полноэкранного режима).
                    let (x, y, cw, ch) = content;
                    let pane = if self.chrome_visible() { PANE_H } else { 0.0 };
                    let [l, r] = split_boxes((x, y + pane, cw, ch - pane), s.ratio);
                    shown.push((s.left, l));
                    shown.push((s.right, r));
                }
                None => shown.push((active, content)),
            }
        }
        for t in self.pages.iter() {
            if let Some(wv) = &t.view {
                match shown.iter().find(|(id, _)| *id == t.id) {
                    Some((_, b)) => {
                        wv.set_bounds(rect(b.0, b.1, b.2, b.3));
                        wv.set_visible(true);
                    }
                    None => {
                        wv.set_visible(false);
                    }
                }
            }
        }
        if let Some(p) = &self.peek {
            p.view.set_bounds(rect(p.rect.0, p.rect.1, p.rect.2, p.rect.3));
        }
    }

    /// Обрезать интерфейс по тому, что он сейчас рисует.
    pub(super) fn apply_region(&self) {
        frame::set_ui_region(
            &self.window,
            self.ui.native_handle(),
            &self.region,
            true,
            !self.chrome_visible(),
            self.cfg.custom_titlebar,
        );
    }

    pub(super) fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.window.set_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)));
        if let Some(f) = &self.frame {
            f.set_buttons_visible(self.chrome_visible());
        }
        self.layout();
        self.apply_region();
        self.push_state();
    }

    pub(super) fn on_page_fullscreen(&mut self, id: u64, on: bool) {
        if on && self.pages.visible().contains(&id) {
            self.page_fullscreen = Some(id);
            self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else if !on && self.page_fullscreen == Some(id) {
            self.page_fullscreen = None;
            if !self.fullscreen {
                self.window.set_fullscreen(None);
            }
        } else {
            return;
        }
        if let Some(f) = &self.frame {
            f.set_buttons_visible(self.chrome_visible());
        }
        self.layout();
        self.apply_region();
        self.push_state();
    }
}
