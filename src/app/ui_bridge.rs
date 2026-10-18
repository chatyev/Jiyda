//! Связь с интерфейсом: состояние браузера для ui/js и вызовы jiyda.*.

use super::*;

impl App {
    // ---------- Состояние для интерфейса ----------

    pub(super) fn space_pages(&self, i: usize) -> &PageList<View> {
        if i == self.active_space { &self.pages } else { &self.spaces[i].pages }
    }

    /// Отметить, что состояние изменилось. Отправится интерфейсу один раз за итерацию
    /// цикла событий (`tick`) — или раньше, перед любым вызовом интерфейса, чтобы он
    /// видел изменения в том же порядке, в каком они сделаны.
    pub(super) fn push_state(&self) {
        self.state_dirty.set(true);
    }

    pub(super) fn flush_state(&self) {
        if self.state_dirty.replace(false) {
            self.send_state();
        }
    }

    /// Всё состояние браузера — интерфейсу (`jiyda.render`).
    pub(super) fn send_state(&self) {
        // Группы Orbit — по страницам Now активного пространства.
        let now_pages: Vec<(u64, &str)> =
            self.pages.ordered().iter().filter(|t| t.is_now()).map(|t| (t.id, t.url.as_str())).collect();
        let groups = orbit::group(&now_pages);
        let group_of = |id: u64| {
            groups
                .iter()
                .find(|g| g.ids.contains(&id))
                .map(|g| if g.key == "site" { format!("site:{}", g.label) } else { g.key.to_string() })
                .unwrap_or_default()
        };
        let mut pages = Vec::new();
        for (i, s) in self.spaces.iter().enumerate() {
            for t in self.space_pages(i).ordered() {
                let g = if i == self.active_space { group_of(t.id) } else { String::new() };
                pages.push(page_json(t, s.id, &g, self.icon_for(&t.url, &t.favicon)));
            }
        }
        let closed: Vec<Value> = self
            .pages
            .closed()
            .iter()
            .enumerate()
            .rev()
            .map(|(i, c)| json!({ "index": i, "title": c.title, "url": c.url, "favicon": self.icon_for(&c.url, &c.favicon), "closedAt": c.closed_at }))
            .collect();
        let today = shield::today();
        let (trackers_today, ads_today) = self.daily.for_day(&today);
        let state = json!({
            "pages": pages,
            "groups": groups.iter().map(|g| json!({ "key": g.key, "label": g.label, "ids": g.ids })).collect::<Vec<_>>(),
            "active": self.pages.active(),
            "visible": self.pages.visible(),
            "splits": self.pages.splits().iter()
                .map(|s| json!({ "left": s.left, "right": s.right, "ratio": s.ratio }))
                .collect::<Vec<_>>(),
            "spaces": self.spaces.iter().enumerate()
                .map(|(i, s)| {
                    let list = self.space_pages(i);
                    json!({
                        "id": s.id, "name": s.name, "color": s.color, "main": s.id == spaces::MAIN_ID,
                        "pages": list.iter().filter(|t| t.is_now()).count(),
                    })
                })
                .collect::<Vec<_>>(),
            "activeSpace": self.active_space,
            "closed": closed,
            "theme": self.theme,
            "maximized": self.maximized,
            "fullscreen": self.fullscreen || self.page_fullscreen.is_some(),
            "pageFullscreen": self.page_fullscreen.is_some(),
            "find": self.find_open,
            "update": self.updates.visible().map(|u| json!({ "version": u.version })),
            "updating": self.updates.downloading,
            "recovery": self.recovery.is_some(),
            "importOffer": self.import_offer(),
            "onboarding": self.onboarding,
            "bookmarked": self.pages.active_page().is_some_and(|t| self.bookmarks.contains(&t.url)),
            "home": self.home_data(),
            "shield": { "on": self.cfg.shield, "trackersToday": trackers_today, "adsToday": ads_today },
            "keys": self.keys.map.borrow().to_json(),
            "prefs": {
                "edgeReveal": self.cfg.edge_reveal,
                "dimChrome": self.cfg.dim_chrome,
                "selectionToolbar": self.cfg.selection_toolbar,
                "motion": self.cfg.motion,
                "ai": self.cfg.ai,
                "aiKey": self.ai_key.is_some(),
            },
            "peek": self.peek.as_ref().map(|p| json!({ "url": p.url, "rect": [p.rect.0, p.rect.1, p.rect.2, p.rect.3] })),
        });
        self.eval_ui(&format!("window.jiyda && jiyda.render({state})"));

        let title = self.pages.active_page().map(|t| if t.title.is_empty() { t.url.clone() } else { t.title.clone() });
        self.window.set_title(&match title {
            Some(t) => format!("{t} — Jiyda"),
            None => "Jiyda".into(),
        });
    }

    /// Значок для интерфейса: свой у страницы, иначе из кэша по сайту. Только data: —
    /// интерфейс не должен сам ходить в сеть за значками (это раскрыло бы, какие сайты
    /// открыты, и шло бы с куки «Основного» Space).
    pub(super) fn icon_for<'a>(&'a self, url: &str, own: &'a str) -> &'a str {
        if own.starts_with("data:image/") {
            return own;
        }
        self.favicons.get(&orbit::domain(url)).unwrap_or("")
    }

    pub(super) fn eval_ui(&self, js: &str) {
        self.ui.eval(js);
    }

    /// Команда, которую выполняет интерфейс (открыть палитру, Orbit, панель…).
    pub(super) fn ui_command(&self, cmd: &str, arg: Value) {
        self.flush_state();
        self.eval_ui(&format!("window.jiyda && jiyda.command({}, {arg})", json!(cmd)));
    }

    pub(super) fn ui_data(&self, kind: &str, data: Value) {
        self.flush_state();
        self.eval_ui(&format!("window.jiyda && jiyda.data({}, {data})", json!(kind)));
    }

    pub(super) fn toast(&self, text: &str) {
        self.flush_state();
        self.eval_ui(&format!("window.jiyda && jiyda.toast({})", json!({ "text": text })));
    }

    /// Тост с кнопкой (Undo): по нажатию интерфейс пришлёт сообщение `msg`.
    pub(super) fn toast_action(&self, text: &str, label: &str, msg: Value) {
        self.flush_state();
        self.eval_ui(&format!(
            "window.jiyda && jiyda.toast({})",
            json!({ "text": text, "action": { "label": label, "msg": msg } })
        ));
    }
}
