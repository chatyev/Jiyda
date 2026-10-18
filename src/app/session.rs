//! Сессия: восстановление, запись на диск, отложенные дела (tick).

use super::*;

impl App {
    // ---------- Сессия ----------

    /// Вернуть страницы прошлого запуска. `crashed` — тот запуск оборвался: страницы
    /// возвращаются в список, но на экран ничего не загружается, пока не нажмут
    /// «Открыть снова» (`recover`) или не выберут страницу сами.
    pub(super) fn restore(&mut self, saved: Vec<SavedSpace>, crashed: bool) {
        let archive_after = self.cfg.archive_hours * 3600;
        let mut target = None;
        for (i, s) in saved.into_iter().enumerate() {
            let t = self.spaces[i].pages.restore(s.tabs, s.active, &s.splits, now(), archive_after);
            if i == self.active_space {
                target = t;
            }
        }
        std::mem::swap(&mut self.pages, &mut self.spaces[self.active_space].pages);
        if crashed {
            self.recovery = target.take();
        }
        match target {
            Some(id) => self.activate(id),
            None => {
                self.focus_ui_if_empty();
                self.layout();
                self.push_state();
            }
        }
    }

    /// Запланировать запись сессии и истории на диск.
    pub(super) fn save(&mut self) {
        if self.save_due.is_none() {
            self.save_due = Some(Instant::now() + SAVE_DELAY);
        }
    }

    pub(super) fn save_now(&mut self) {
        self.save_due = None;
        // Данные готовим здесь, а пишет их фоновый поток (`writer`).
        if let Some(s) = self.history.take_changes() {
            self.writer.write(&self.paths.history, s);
        }
        if let Some(s) = self.bookmarks.take_changes() {
            self.writer.write(&self.paths.bookmarks, s);
        }
        if let Some(s) = self.favicons.take_changes() {
            self.writer.write(&self.paths.favicons, s);
        }
        if let Some(s) = self.daily.to_json() {
            self.writer.write(&self.paths.shield, s);
        }
        let spaces = self
            .spaces
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let list = if i == self.active_space { &self.pages } else { &s.pages };
                let (tabs, active, splits) = list.to_saved();
                SavedSpace {
                    id: s.id,
                    name: s.name.clone(),
                    color: s.color.clone(),
                    tabs,
                    active,
                    closed: list.closed().to_vec(),
                    splits,
                }
            })
            .collect();
        let session = Session {
            spaces,
            active_space: self.active_space,
            theme: Some(self.theme.clone()),
            deleted_spaces: self.deleted_spaces.clone(),
            ..Default::default()
        };
        if let Some(s) = session.to_json() {
            self.writer.write(&self.paths.session, s);
        }
    }

    pub(super) fn save_config(&self) {
        if let Some(s) = self.cfg.to_json() {
            self.writer.write(&self.paths.config, s);
        }
    }

    pub(super) fn save_downloads(&self) {
        if let Some(s) = self.downloads.to_json() {
            self.writer.write(&self.paths.downloads, s);
        }
    }

    pub(super) fn save_permissions(&self) {
        if let Some(s) = self.perms.borrow().to_json() {
            self.writer.write(&self.paths.permissions, s);
        }
    }

    /// «Открыть снова» после сбоя: страница, что была на экране (или первая, если её
    /// уже закрыли).
    pub(super) fn recover(&mut self) {
        let Some(id) = self.recovery.take() else { return };
        let id = Some(id)
            .filter(|id| self.pages.get(*id).is_some_and(Page::is_now))
            .or_else(|| self.pages.now_order().first().copied());
        match id {
            Some(id) => self.activate(id),
            None => self.push_state(),
        }
    }

    /// Выход: всё на диск и снять метку запуска. Вызывается один раз (main.rs,
    /// `Event::LoopDestroyed`).
    pub fn shutdown(&mut self) {
        self.flush_state();
        self.save_now();
        // Дождаться, пока всё ляжет на диск: дальше процесс завершится.
        self.writer.finish();
        // Только после записи: оборвись выход раньше — следующий запуск это заметит.
        self.run_marker.finish();
    }

    /// Отложенные дела: запись на диск, сон страниц, снимки, обновления.
    pub fn tick(&mut self) {
        let t = Instant::now();
        #[cfg(debug_assertions)]
        self.automation();
        if self.save_due.is_some_and(|due| t >= due) {
            self.save_now();
        }
        if t >= self.next_sleep_check {
            self.next_sleep_check = t + SLEEP_CHECK;
            if self.sleep_idle_tabs() {
                self.push_state();
            }
        }
        if self.capture_due.iter().any(|(_, at)| *at <= t) {
            let (due, rest): (Vec<_>, Vec<_>) =
                std::mem::take(&mut self.capture_due).into_iter().partition(|(_, at)| *at <= t);
            self.capture_due = rest;
            for (id, _) in due {
                self.capture(id);
            }
        }
        if self.auto_update_checks() && t >= self.updates.next_check {
            self.updates.next_check = t + UPDATE_CHECK_EVERY;
            self.check_updates(false);
        }
        // Все события пачки обработаны — одно обновление интерфейса вместо десятков.
        self.flush_state();
    }

    /// Отладочная сборка с JIYDA_AUTOMATION: команды из файла automation.txt в папке
    /// данных (по строке: `ui {json}`, `cmd имя`, `js код`) — для проверки интерфейса
    /// без мыши и клавиатуры. В релизной сборке этого нет.
    #[cfg(debug_assertions)]
    pub(super) fn automation(&mut self) {
        if std::env::var_os("JIYDA_AUTOMATION").is_none() {
            return;
        }
        // Пульс раз в 5 с: по журналу видно, крутится ли цикл событий, если интерфейс завис.
        thread_local!(static PULSE: Cell<Option<Instant>> = const { Cell::new(None) });
        PULSE.with(|p| {
            if p.get().is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
                p.set(Some(Instant::now()));
                log::debug!("automation: цикл событий жив");
            }
        });
        let path = self.paths.base().join("automation.txt");
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let _ = std::fs::remove_file(&path);
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
            log::debug!("automation: {line}");
            match kind {
                "ui" => self.handle(UserEvent::FromUi(rest.to_string())),
                "cmd" => match rest.parse::<Command>() {
                    Ok(c) => self.command(c),
                    Err(e) => log::warn!("automation: {e}"),
                },
                "js" => self.eval_ui(rest),
                // Целый файл JS — сценарии E2E (tests/e2e): в одну строку они не помещаются.
                "jsfile" => match std::fs::read_to_string(rest) {
                    // Каждый файл — в своей области (косвенный eval): одинаковые имена
                    // констант в разных сценариях не мешают друг другу. Не загрузился файл
                    // (синтаксическая ошибка) — это ошибка интерфейса в журнале, прогон провален,
                    // а не тихо пропущенные тесты.
                    Ok(code) => self.eval_ui(&format!(
                        "try {{ (0, eval)({code}) }} catch (e) {{ window.ipc.postMessage(JSON.stringify({{ type: 'log', msg: 'jsfile ' + {name} + ': ' + e }})) }}",
                        code = json!(code),
                        name = json!(rest),
                    )),
                    Err(e) => log::warn!("automation: {rest}: {e}"),
                },
                "size" => {
                    let mut it = rest.split_whitespace().filter_map(|n| n.parse::<f64>().ok());
                    if let (Some(w), Some(h)) = (it.next(), it.next()) {
                        self.window.set_maximized(false);
                        self.window.set_inner_size(tao::dpi::LogicalSize::new(w, h));
                    }
                }
                _ => {}
            }
        }
    }

    /// Проверять обновления по расписанию. Запуски для разработки (отладочная сборка, своя
    /// папка данных) сами не проверяют: десятки таких запусков подряд тратят лимит GitHub
    /// на запросы с этого адреса.
    fn auto_update_checks(&self) -> bool {
        self.cfg.check_updates && !cfg!(debug_assertions) && crate::config::data_dir_override().is_none()
    }

    /// Когда циклу событий проснуться, чтобы вызвать `tick`.
    /// Каждый срок здесь `tick` сдвигает или снимает, когда он наступил: срок, который
    /// остаётся в прошлом, будил бы цикл впустую, а Waker (main.rs) повтор того же срока
    /// не пересылает — и цикл засыпал бы до первого события.
    pub fn next_wakeup(&self) -> Instant {
        let mut at = self.next_sleep_check;
        #[cfg(debug_assertions)]
        if std::env::var_os("JIYDA_AUTOMATION").is_some() {
            at = at.min(Instant::now() + Duration::from_millis(250));
        }
        if let Some(due) = self.save_due {
            at = at.min(due);
        }
        if let Some(c) = self.capture_due.iter().map(|(_, at)| *at).min() {
            at = at.min(c);
        }
        if self.auto_update_checks() {
            at = at.min(self.updates.next_check);
        }
        at
    }
}
