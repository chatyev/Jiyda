//! События: от WebView, потоков и интерфейса (сообщения ipc.rs).

use super::*;

impl App {
    pub fn handle(&mut self, ev: UserEvent) {
        match ev {
            // Только разбудить цикл: дела сделает tick() на MainEventsCleared.
            UserEvent::Wake => {}
            UserEvent::FromUi(body) => {
                if let Some(msg) = UiMsg::parse(&body) {
                    self.handle_ui(msg);
                }
            }
            UserEvent::FromTab { id, msg } => self.handle_tab_msg(id, &msg),
            UserEvent::Title { id, title } => {
                let url = self.page_any(id).and_then(|t| t.view.as_ref()).and_then(|w| w.url());
                let tab_url = self.page_mut_any(id).map(|t| {
                    t.title = title.clone();
                    if let Some(u) = url.filter(|u| !u.is_empty()) {
                        t.url = u;
                    }
                    t.url.clone()
                });
                if let Some(u) = tab_url {
                    self.history.set_title(&u, &title);
                }
                self.push_state();
            }
            UserEvent::Load { id, started, url } => self.on_load(id, started, url),
            UserEvent::OpenTab { url, background } => {
                self.open_tab(url, background, None);
            }
            UserEvent::Popup { id, url } => self.popup(id, url),
            UserEvent::DownloadStarted { url, path } => {
                self.downloads.started(&url, &path, now());
                self.save_downloads();
                self.push_downloads();
            }
            UserEvent::DownloadProgress(p) => {
                if self.downloads.progress(&p.url, p.received, p.total) {
                    self.push_downloads();
                }
            }
            UserEvent::Download { id, url, path, ok } => {
                self.engine.forget_download(&url);
                self.downloads.finished(&url, path.as_deref(), ok, now());
                self.save_downloads();
                // Страница, открытая только ради загрузки, остаётся пустой — закрываем её.
                if self.pages.get(id).is_some_and(|t| !t.pinned && t.url == "about:blank") {
                    self.close_tab(id, false);
                }
                self.push_downloads();
            }
            UserEvent::FindResult { id, json } => {
                if self.find_open
                    && self.pages.active() == Some(id)
                    && let Some((n, i)) = parse_find_result(&json)
                {
                    self.eval_ui(&format!("window.jiyda && jiyda.findResult({})", json!({ "n": n, "i": i })));
                }
            }
            // Клавиши приходят от WebView2, а не от страницы, но фокус может остаться
            // в скрытой странице — слушаем только видимые и интерфейс.
            UserEvent::Key { id, cmd } => {
                if id.is_none_or(|id| {
                    self.pages.visible().contains(&id) || self.peek.as_ref().is_some_and(|p| p.id == id)
                }) {
                    self.command(cmd);
                }
            }
            UserEvent::KeyRecorded(combo) => {
                self.keys.recording.set(false);
                self.ui_data("recorded", json!(combo));
            }
            UserEvent::External { urls } => self.open_external(urls),
            UserEvent::UpdateChecked { result, manual } => self.on_update_checked(result, manual),
            UserEvent::UpdateDownloaded { result } => self.on_update_downloaded(result),
            UserEvent::Imported { label, data } => self.on_imported(label, *data),
            UserEvent::TelemetrySent { days } => self.on_telemetry_sent(days),
            UserEvent::Favicon { id, png } => {
                let uri = format!("data:image/png;base64,{}", base64(&png));
                let url = self.page_mut_any(id).map(|t| {
                    t.favicon.clone_from(&uri);
                    t.url.clone()
                });
                if let Some(url) = url {
                    self.favicons.set(&orbit::domain(&url), &uri);
                    self.save();
                    self.push_state();
                }
            }
            UserEvent::Preview { id, jpeg } => {
                self.eval_ui(&format!(
                    "window.jiyda && jiyda.preview({id}, 'data:image/jpeg;base64,{}')",
                    base64(&jpeg)
                ));
            }
            UserEvent::Blocked { id, kind } => {
                let e = self.page_blocks.entry(id).or_default();
                match kind {
                    shield::Kind::Tracker => e.0 += 1,
                    shield::Kind::Ad => e.1 += 1,
                }
                self.daily.add(&shield::today(), kind);
                self.save();
            }
            UserEvent::PermissionAsk { tab, id, origin, kind } => {
                let space = self.spaces.iter().position(|s| s.pages.get(tab).is_some()).unwrap_or(self.active_space);
                self.ui_command(
                    "permission",
                    json!({ "id": id, "tab": tab, "origin": origin, "kind": kind, "space": space }),
                );
            }
            UserEvent::PageFullscreen { id, on } => self.on_page_fullscreen(id, on),
            UserEvent::PageText { req, id, text } => self.on_page_text(req, id, text),
            #[cfg(debug_assertions)]
            UserEvent::E2eResult { req, json } => {
                // JSON от WebView кладём как есть: это уже значение (или null).
                let value: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
                self.ui_data("e2e", json!({ "req": req, "value": value }));
            }
            UserEvent::AiAnswer { req, result } => match result {
                Ok(text) => self.ui_data("ai", json!({ "req": req, "text": text })),
                Err(e) => self.ui_data("ai", json!({ "req": req, "error": e })),
            },
        }
    }

    pub(super) fn on_load(&mut self, id: u64, started: bool, url: String) {
        // Peek: переход внутри предпросмотра — значит, с ним начали работать.
        if let Some(p) = &self.peek
            && p.id == id
        {
            if !started && !url.is_empty() && url != p.url && url != "about:blank" {
                self.promote_peek();
            }
            return;
        }
        if started {
            self.page_blocks.remove(&id);
        }
        let visited = self.page_mut_any(id).and_then(|t| {
            t.loading = started;
            if !url.is_empty() {
                t.url = url.clone();
            }
            (!started).then(|| (t.url.clone(), t.title.clone()))
        });
        if let Some((u, title)) = visited {
            self.history.visit(&u, &title);
            self.save();
            if self.pages.visible().contains(&id) {
                self.capture_due.retain(|(c, _)| *c != id);
                self.capture_due.push((id, Instant::now() + CAPTURE_AFTER_LOAD));
            }
        }
        self.push_state();
    }

    /// Ссылки из других программ: каждая — новой страницей в текущем пространстве,
    /// окно выходит на передний план (и без ссылок — повторный запуск ярлыком).
    pub(super) fn open_external(&mut self, urls: Vec<String>) {
        log::info!("ссылки из других программ: {}", urls.len());
        for url in urls {
            self.open_tab(url, false, None);
        }
        self.window.set_minimized(false);
        self.window.set_visible(true);
        self.window.set_focus();
    }

    pub(super) fn handle_tab_msg(&mut self, id: u64, msg: &str) {
        let Ok(v) = serde_json::from_str::<Value>(msg) else { return };
        let visible = self.pages.visible().contains(&id);
        // Страница может прислать что угодно: команд браузера отсюда не принимаем
        // (горячие клавиши ловит hotkeys.rs), только сведения о ней самой и жесты
        // пользователя на видимой странице.
        match v["t"].as_str().unwrap_or("") {
            "esc" if self.peek.as_ref().is_some_and(|p| p.id == id) => self.close_peek(),
            "open_bg" if visible => {
                if let Some(u) = v["u"].as_str().filter(|u| u.starts_with("http")) {
                    self.open_tab_from_page(u.to_string(), true, id);
                }
            }
            // Сообщение может прислать и сам сайт, без щелчка пользователя, — поэтому
            // Peek ограничен тем же лимитом, что и страницы, которые открывает сайт.
            "peek" if visible && self.peek.is_none() => {
                if let Some(u) = v["u"].as_str().filter(|u| u.starts_with("http")) {
                    if self.page_tabs.allow(Instant::now()) {
                        self.open_peek(u.to_string());
                    } else {
                        log::warn!("страница слишком часто открывает предпросмотр, заблокировано: {u}");
                    }
                }
            }
            // Панель над выделенным текстом.
            "sel" if visible => {
                let text: String = v["s"].as_str().unwrap_or("").chars().take(20_000).collect();
                if text.trim().is_empty() {
                    return;
                }
                match v["a"].as_str().unwrap_or("") {
                    "search" => {
                        if self.page_tabs.allow(Instant::now())
                            && let Some(url) = input::resolve(&text, &self.cfg.search_url)
                        {
                            self.open_tab(url, false, Some(id));
                        }
                    }
                    a @ ("explain" | "translate" | "ask") => {
                        if self.page_ai.allow(Instant::now()) {
                            self.ui_command("ask_selection", json!({ "action": a, "text": text }))
                        } else {
                            log::warn!("страница слишком часто открывает Ask Jiyda, заблокировано");
                        }
                    }
                    _ => {}
                }
            }
            "media" => {
                if let Some(t) = self.page_mut_any(id) {
                    t.media = v["on"].as_bool().unwrap_or(false);
                }
                self.push_state();
            }
            _ => {}
        }
    }

    pub(super) fn handle_ui(&mut self, msg: UiMsg) {
        match msg {
            UiMsg::Metric { name } => {
                if let Some(ev) = crate::telemetry::Event::from_ui(&name) {
                    self.metric(ev);
                }
            }
            UiMsg::Ready => {
                self.layout();
                self.push_state();
            }
            UiMsg::Cmd { k } => self.command(k),
            UiMsg::Activate { id } => {
                if self.pages.get(id).is_some() {
                    self.activate(id);
                } else if let Some(idx) = self.spaces.iter().position(|s| s.pages.get(id).is_some()) {
                    // Страница из другого пространства (поиск в Orbit).
                    self.switch_space(idx);
                    self.activate(id);
                }
            }
            UiMsg::Close { id } => self.close_tab(id, true),
            UiMsg::Later { id } => self.move_to_later(id),
            UiMsg::LaterUndo { id, activate } => {
                if activate {
                    self.activate(id);
                } else if self.pages.move_to_now(id) {
                    self.push_state();
                    self.save();
                }
            }
            UiMsg::Pin { id, on } => self.set_pinned(id, on),
            UiMsg::Duplicate { id } => {
                if let Some(u) = self.pages.get(id).map(|t| t.url.clone()) {
                    self.open_tab(u, false, Some(id));
                }
            }
            UiMsg::ResetPinned { id } => {
                if let Some(u) = self.pages.get(id).and_then(|t| t.pinned_url.clone()) {
                    self.load_in(id, &u);
                }
            }
            UiMsg::MoveToSpace { id, space } => self.move_to_space(id, space),
            UiMsg::Navigate { text, mode } => self.navigate(&text, mode),
            UiMsg::HomeCancel => {
                if let Some(id) = self.home_return.take() {
                    self.activate(id);
                }
            }
            UiMsg::Split { id, side } => match self.pages.active() {
                Some(active) if active != id => {
                    if side.as_deref() == Some("left") {
                        self.split_with(id, active)
                    } else {
                        self.split_with(active, id)
                    }
                }
                _ => self.activate(id),
            },
            UiMsg::Unsplit { id } => {
                if let Some(id) = id.or(self.pages.active()) {
                    self.unsplit(id);
                }
            }
            UiMsg::SplitSwap => self.command(Command::SplitSwap),
            UiMsg::SplitReplace { old, new } => {
                if self.pages.replace_in_split(old, new) {
                    self.activate(new);
                }
            }
            // Разделитель тянут: двигаем половины сразу, а сохраняем, когда отпустили.
            UiMsg::SplitRatio { ratio, done } => {
                if let Some(active) = self.pages.active() {
                    self.pages.set_split_ratio(active, ratio);
                    self.layout();
                    if done {
                        self.push_state();
                        self.save();
                    }
                }
            }
            UiMsg::Space { index } => self.switch_space(index),
            UiMsg::SpaceNew => self.new_space(),
            UiMsg::SpaceRename { index, name } => self.rename_space(index, &name),
            UiMsg::SpaceColor { index, color } => self.set_space_color(index, &color),
            UiMsg::SpaceDelete { index } => self.delete_space(index),
            UiMsg::RestoreClosed { index } => self.restore_closed(index),
            UiMsg::RestoreAllClosed => self.restore_all_closed(),
            UiMsg::ClearClosed => self.command(Command::ClearClosed),
            UiMsg::HistoryRemove { url } => self.remove_history(&url),
            UiMsg::HistoryUndo => self.undo_history_remove(),
            UiMsg::BookmarkRemove { id } => self.remove_bookmark(id),
            UiMsg::BookmarkUndo => self.undo_bookmark_remove(),
            UiMsg::BookmarkEdit { id, title, url } => self.edit_bookmark(id, &title, url.as_deref()),
            UiMsg::BookmarkMove { id, parent, index } => self.move_bookmark(id, parent, index),
            UiMsg::BookmarkFolder { parent, title } => self.new_bookmark_folder(parent, &title),
            UiMsg::BookmarkOpenAll { id } => self.open_bookmark_folder(id),
            UiMsg::BookmarkAdd { url, title, parent } => self.add_bookmark(&url, &title, parent),
            UiMsg::Import { source, bookmarks, history } => {
                self.start_import(&source, crate::import::What { bookmarks, history })
            }
            UiMsg::HistoryContext { url, ts } => self.history_context(&url, ts),
            UiMsg::Region { full, rects, holes } => {
                let conv = |v: Vec<[f64; 4]>| {
                    v.into_iter()
                        .filter(|r| r.iter().all(|x| x.is_finite()))
                        .map(|r| (r[0], r[1], r[2], r[3]))
                        .collect()
                };
                let region = UiRegion { full, rects: conv(rects), holes: conv(holes) };
                if region != self.region {
                    self.region = region;
                    self.apply_region();
                }
            }
            UiMsg::Modal { on } => {
                self.ui_modal = on;
                if on {
                    self.ui.focus();
                } else if let Some(p) = &self.peek {
                    p.view.focus();
                } else {
                    self.on_focus();
                }
            }
            UiMsg::FrameColors { bg, fg } => {
                if let (Some(bg), Some(fg), Some(f)) = (frame::parse_hex(&bg), frame::parse_hex(&fg), &self.frame) {
                    f.set_colors(bg, fg);
                }
            }
            UiMsg::Find { text, mode } => {
                let mode = if matches!(mode.as_str(), "next" | "prev") { mode.as_str() } else { "type" };
                self.run_find(&text, mode);
            }
            UiMsg::FindClose => self.close_find(),
            UiMsg::Download { id, action } => match action.as_str() {
                "open" => self.open_download(id),
                "reveal" => self.reveal_download(id),
                "cancel" => {
                    if let Some(url) = self.downloads.get(id).map(|d| d.url.clone())
                        && !self.engine.cancel_download(&url)
                    {
                        self.toast(i18n::tr("This download can't be cancelled anymore"));
                    }
                }
                "remove"
                    if self.downloads.get(id).is_some_and(|d| d.state != DownloadState::Running)
                        && self.downloads.remove(id) =>
                {
                    self.save_downloads();
                    self.push_downloads();
                }
                _ => {}
            },
            UiMsg::Get { what } => match what.as_str() {
                "history" => self.ui_data("history", self.history_json()),
                "bookmarks" => self.push_bookmarks(),
                "import" => {
                    if !self.import.busy {
                        self.import = bookmark_ops::ImportState::scan();
                    }
                    self.ui_data("import", self.import_json());
                }
                "downloads" => self.push_downloads(),
                "settings" => self.ui_data("settings", self.settings_json()),
                "security" => self.ui_data("security", self.security_json()),
                "previews" => {
                    for id in self.pages.visible() {
                        self.capture(id);
                    }
                }
                _ => {}
            },
            UiMsg::Setting(change) => self.set_setting(change),
            UiMsg::KeysRecord { on } => self.keys.recording.set(on),
            UiMsg::PermissionAnswer { id, decision } => {
                // Сайт и вид разрешения — из самого запроса, а не от интерфейса.
                if let Some((origin, kind)) = self.engine.answer_permission(id, decision.allows())
                    && let Some(d) = decision.remembered()
                {
                    self.perms.borrow_mut().set(&origin, kind, d);
                    self.save_permissions();
                }
            }
            UiMsg::SiteShield { host, on } => self.set_site_shield(&host, on),
            UiMsg::PermissionRevoke { origin, kind } => {
                let removed = self.perms.borrow_mut().revoke(&origin, &kind);
                if removed {
                    self.save_permissions();
                    self.ui_data("settings", self.settings_json());
                    self.ui_data("security", self.security_json());
                }
            }
            UiMsg::Capture { id } => self.capture(id),
            UiMsg::Ask { id, prompt, context, pages, selection } => self.ask(id, prompt, &context, pages, selection),
            UiMsg::AiKey { key } => self.set_ai_key(&key),
            UiMsg::PeekOpen => self.promote_peek(),
            UiMsg::PeekClose => self.close_peek(),
            UiMsg::PeekBox { rect } => {
                if let Some(p) = &mut self.peek
                    && rect.iter().all(|x| x.is_finite())
                {
                    p.rect = (rect[0], rect[1], rect[2].max(200.0), rect[3].max(150.0));
                    self.layout();
                }
            }
            UiMsg::UiBlur => {
                let user = frame::user_moved_focus(&self.window);
                log::debug!("интерфейс потерял фокус, пользователь ушёл: {user}");
                // Тесты E2E (JIYDA_AUTOMATION): окно может быть не впереди, а мышь — нажата в
                // другой программе (проверка кнопок системная); панели они закрывают сами.
                #[cfg(debug_assertions)]
                let user = user && std::env::var_os("JIYDA_AUTOMATION").is_none();
                if user {
                    self.ui_command("close_pops", Value::Null);
                }
            }
            // Развёрнутое окно тоже тянется: Windows восстановит его под курсором, как обычное.
            UiMsg::WindowDrag if !self.fullscreen => self.window.drag_window().or_log("drag_window"),
            UiMsg::WindowDrag => {}
            UiMsg::WindowMenu => frame::show_system_menu(&self.window),
            UiMsg::Log { msg } => log::warn!("интерфейс: {}", msg.chars().take(500).collect::<String>()),
            #[cfg(debug_assertions)]
            UiMsg::E2eEval { req, id, code } => {
                if std::env::var_os("JIYDA_AUTOMATION").is_none() {
                    return;
                }
                let id = if id == 0 { self.pages.active() } else { Some(id) };
                let p = self.proxy.clone();
                match id.and_then(|id| self.page_any(id)).and_then(|t| t.view.as_ref()) {
                    Some(wv) => wv.eval_with(
                        &code,
                        Box::new(move |json| {
                            let _ = p.send_event(UserEvent::E2eResult { req, json });
                        }),
                    ),
                    None => self.ui_data("e2e", json!({ "req": req, "error": "нет загруженной страницы" })),
                }
            }
            UiMsg::Unknown => {}
        }
    }
}
