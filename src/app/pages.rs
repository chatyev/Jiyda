//! Страницы: создание WebView, открытие, активация, закрытие, Later, снимки, поиск по странице.

use super::*;

impl App {
    // ---------- Страницы ----------

    /// Создать страницу движка для страницы `id` (в профиле активного пространства).
    pub(super) fn build_webview(&mut self, id: u64, url: &str, visible: bool) -> Option<View> {
        let b = self.content_box();
        let spec = engine::PageSpec {
            id,
            url,
            bounds: rect(b.0, b.1, b.2, b.3),
            visible,
            theme: engine::Theme::from_setting(&self.theme),
            dark: theme_is_dark(&self.theme, &self.window),
            init_script: format!(
                "window.__JIYDA_PAGE = {};\n{TAB_JS}",
                json!({ "selection": self.cfg.selection_toolbar, "lang": i18n::code() })
            ),
            download_dir: &self.cfg.download_dir,
        };
        match self.engine.create_page(&self.window, &mut self.spaces[self.active_space].ctx, spec) {
            Ok(view) => Some(view),
            Err(e) => {
                log::error!("не удалось создать страницу: {e}");
                self.toast(&i18n::trf("WebView2 error: {}", &[&e]));
                None
            }
        }
    }
    pub(super) fn open_tab(&mut self, url: String, background: bool, parent: Option<u64>) -> u64 {
        let id = self.pages.alloc_id();
        let webview = self.build_webview(id, &url, false);
        self.pages.insert(id, url, now(), parent, webview);
        if background {
            self.push_state();
        } else {
            self.activate(id);
        }
        self.save();
        id
    }

    /// Загрузить спящую страницу.
    pub(super) fn wake(&mut self, id: u64) {
        if let Some(url) = self.pages.get(id).filter(|t| t.view.is_none()).map(|t| t.url.clone()) {
            let wv = self.build_webview(id, &url, true);
            if let Some(t) = self.pages.get_mut(id) {
                t.loading = wv.is_some();
                t.view = wv;
            }
        }
    }

    pub(super) fn activate(&mut self, id: u64) {
        let before = self.pages.active();
        // Поиск относится к странице, при переходе на другую он закрывается.
        if self.find_open && before != Some(id) {
            self.find_open = false;
            self.run_find("", "clear");
        }
        // Уходящую страницу снимаем для Orbit, пока она ещё на экране.
        if let Some(prev) = before.filter(|p| *p != id) {
            self.capture(prev);
        }
        if !self.pages.activate(id, now()) {
            return;
        }
        self.home_return = None;
        // Спящие страницы на экране (в том числе вторую половину пары) загружаем.
        for v in self.pages.visible() {
            self.wake(v);
        }
        self.layout();
        if !self.ui_modal
            && let Some(wv) = self.pages.get(id).and_then(|t| t.view.as_ref())
        {
            wv.focus();
        }
        self.sleep_idle_tabs();
        self.push_state();
        self.save();
    }

    pub(super) fn activate_opt(&mut self, id: Option<u64>) {
        if let Some(id) = id {
            self.activate(id);
        }
    }

    /// Выгрузить из памяти давно не открывавшиеся фоновые страницы.
    pub(super) fn sleep_idle_tabs(&mut self) -> bool {
        let (t, limit) = (now(), self.cfg.sleep_minutes * 60);
        for s in &mut self.spaces {
            s.pages.sleep_idle_offscreen(t, limit);
        }
        self.pages.sleep_idle(t, limit)
    }

    /// Открыть страницу по просьбе сайта, если он не превысил лимит.
    pub(super) fn open_tab_from_page(&mut self, url: String, background: bool, parent: u64) {
        if self.page_tabs.allow(Instant::now()) {
            self.open_tab(url, background, Some(parent));
        } else {
            log::warn!("страница открывает слишком много страниц, заблокировано: {url}");
            self.toast(i18n::tr("Blocked: the page is opening too many pages"));
        }
    }

    /// window.open со страницы `opener`. Новая страница открывается в пространстве этой
    /// страницы: иначе ссылка из «Работы» загрузилась бы с куки «Личного».
    pub(super) fn popup(&mut self, opener: u64, url: String) {
        if self.pages.get(opener).is_some() {
            self.open_tab_from_page(url, false, opener);
            return;
        }
        let Some(space) = self.spaces.iter_mut().find(|s| s.pages.get(opener).is_some()) else { return };
        if !self.page_tabs.allow(Instant::now()) {
            log::warn!("страница открывает слишком много страниц, заблокировано: {url}");
            return;
        }
        // Пространство не на экране: страница спит и загрузится, когда в него перейдут.
        let id = space.pages.alloc_id();
        space.pages.insert(id, url, now(), Some(opener), None);
        if let Some(t) = space.pages.get_mut(id) {
            t.loading = false;
        }
        let name = space.name.clone();
        self.toast(&i18n::trf("New page in \u{201c}{}\u{201d}", &[&name]));
        self.push_state();
        self.save();
    }

    /// Закрыть страницу. `undo` — показать «Page closed · Undo».
    pub(super) fn close_tab(&mut self, id: u64, undo: bool) {
        let http = self.pages.get(id).is_some_and(|t| !t.pinned && t.url.starts_with("http"));
        self.engine.drop_page(id);
        self.page_blocks.remove(&id);
        if let Some(after) = self.pages.close(id, now()) {
            self.after_close(after);
            if undo && http {
                self.toast_action(i18n::tr("Page closed"), i18n::tr("Undo"), json!({ "type": "cmd", "k": "reopen" }));
            }
        }
    }

    pub(super) fn clear_unpinned(&mut self) {
        let after = self.pages.clear_unpinned(now());
        self.after_close(after);
    }

    /// Обновить экран после закрытия страниц: если закрыли активную — перейти на `next`.
    pub(super) fn after_close(&mut self, after: AfterClose) {
        if after.was_active
            && let Some(n) = after.next
        {
            self.activate(n);
            return;
        }
        self.focus_ui_if_empty();
        self.layout();
        self.push_state();
        self.save();
    }

    /// Move to Later.
    pub(super) fn move_to_later(&mut self, id: u64) {
        let Some(after) = self.pages.move_to_later(id) else { return };
        self.engine.drop_page(id);
        let was_active = after.was_active;
        self.after_close(after);
        self.toast_action(
            i18n::tr("Moved to Later"),
            i18n::tr("Undo"),
            json!({ "type": "later_undo", "id": id, "activate": was_active }),
        );
    }

    /// Окно получило фокус (запуск, Alt+Tab, клик по панели задач). Windows отдаёт его
    /// самому окну, а не WebView внутри — передаём туда, где ждут ввода с клавиатуры.
    pub fn on_focus(&self) {
        if self.ui_modal {
            self.ui.focus();
            return;
        }
        match self.pages.active_page().and_then(|t| t.view.as_ref()) {
            Some(wv) => {
                wv.focus();
            }
            None => {
                self.ui.focus();
            }
        }
    }

    /// Когда страниц на экране нет, клавиатура должна попадать в интерфейс —
    /// иначе фокус ни у кого, и Ctrl+T и другие клавиши не работают до клика мышью.
    pub(super) fn focus_ui_if_empty(&self) {
        if self.pages.active().is_none() {
            self.ui.focus();
        }
    }

    pub(super) fn reopen_closed(&mut self) {
        match self.pages.pop_closed() {
            Some(st) => {
                self.open_tab(st.url, false, None);
            }
            None => self.toast(i18n::tr("No closed pages")),
        }
    }

    pub(super) fn restore_closed(&mut self, index: usize) {
        if let Some(st) = self.pages.take_closed(index) {
            self.open_tab(st.url, false, None);
        }
    }

    pub(super) fn restore_all_closed(&mut self) {
        let mut all = Vec::new();
        while let Some(st) = self.pages.pop_closed() {
            all.push(st);
        }
        // Последняя закрытая станет активной, остальные откроются спящими.
        let Some(last) = all.first().cloned() else { return };
        for st in all.into_iter().skip(1).rev() {
            self.pages.insert_saved(st, now());
        }
        self.open_tab(last.url, false, None);
    }

    pub(super) fn set_pinned(&mut self, id: u64, pinned: bool) {
        self.pages.set_pinned(id, pinned);
        self.push_state();
        self.save();
    }

    /// Перейти по тексту из адресной строки или палитры.
    pub(super) fn navigate(&mut self, text: &str, mode: NavMode) {
        let Some(url) = input::resolve(text, &self.cfg.search_url) else { return };
        match (self.pages.active(), mode) {
            (Some(active), NavMode::Split) => {
                let id = self.open_tab(url, true, None);
                self.split_with(active, id);
            }
            // Введённый адрес — новый контекст: родителя (Context) у такой страницы нет.
            (_, NavMode::Background) => {
                self.open_tab(url, true, None);
            }
            (Some(id), NavMode::Current) => self.load_in(id, &url),
            _ => {
                self.open_tab(url, false, None);
            }
        }
    }

    // ---------- Снимки для Orbit ----------

    pub(super) fn capture(&self, id: u64) {
        let Some(wv) = self.pages.get(id).and_then(|t| t.view.as_ref()) else { return };
        let p = self.proxy.clone();
        wv.capture_preview(Box::new(move |jpeg| {
            if let Some(jpeg) = jpeg {
                let _ = p.send_event(UserEvent::Preview { id, jpeg });
            }
        }));
    }

    // ---------- Поиск по странице ----------

    pub(super) fn open_find(&mut self) {
        if self.pages.active_page().is_none_or(|t| t.view.is_none()) {
            return;
        }
        self.find_open = true;
        self.push_state();
        self.ui_command("find", Value::Null);
    }

    pub(super) fn close_find(&mut self) {
        if !self.find_open {
            return;
        }
        self.find_open = false;
        self.run_find("", "clear");
        self.push_state();
    }

    pub(super) fn run_find(&self, query: &str, mode: &str) {
        let Some(id) = self.pages.active() else { return };
        let proxy = self.proxy.clone();
        self.with_active_wv(|wv| {
            wv.eval_with(
                &find_script(query, mode),
                Box::new(move |json| {
                    let _ = proxy.send_event(UserEvent::FindResult { id, json });
                }),
            );
        });
    }
}
