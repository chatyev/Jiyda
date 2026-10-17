//! Ask Jiyda: сбор текста страниц и запрос к Claude в фоновом потоке.

use super::*;

impl App {
    // ---------- Ask Jiyda ----------

    pub(super) fn ask(&mut self, req: u64, prompt: String, context: &str, pages: Vec<u64>, selection: String) {
        let Some(_) = &self.ai_key else {
            self.ui_data("ai", json!({ "req": req, "error": "no_key" }));
            return;
        };
        let mut job = AiJob { prompt, selection, want: 0, pages: Vec::new() };
        let ids: Vec<u64> = match context {
            "page" => self.pages.visible(),
            "pages" => pages,
            _ => Vec::new(),
        };
        if context == "history" {
            let list: String = self.history.recent(300).iter().map(|e| format!("{} — {}\n", e.title, e.url)).collect();
            job.pages.push((i18n::tr("History").to_string(), "jiyda://history".into(), list));
        }
        // Только заголовки и адреса открытых страниц (Organize pages) — без их текста.
        if context == "titles" {
            let list: String = self
                .pages
                .ordered()
                .iter()
                .filter(|t| t.is_now())
                .map(|t| format!("{} — {}\n", t.title, t.url))
                .collect();
            job.pages.push(("Open pages".to_string(), "jiyda://pages".into(), list));
        }
        for id in ids {
            let Some(t) = self.page_any(id) else { continue };
            match &t.view {
                Some(wv) => {
                    job.want += 1;
                    let p = self.proxy.clone();
                    wv.evaluate_script_with_callback(PAGE_TEXT_JS, move |json| {
                        let _ = p.send_event(UserEvent::PageText { req, id, text: js_string_result(&json) });
                    })
                    .or_log("page text");
                }
                // Спящая страница: текста нет, есть заголовок и адрес.
                None => job.pages.push((t.title.clone(), t.url.clone(), String::new())),
            }
        }
        let ready = job.want == 0;
        self.ai_jobs.insert(req, job);
        if ready {
            self.run_ai(req);
        }
    }

    pub(super) fn on_page_text(&mut self, req: u64, id: u64, text: String) {
        let (title, url) = self.page_any(id).map(|t| (t.title.clone(), t.url.clone())).unwrap_or_default();
        let Some(job) = self.ai_jobs.get_mut(&req) else { return };
        job.pages.push((title, url, text));
        job.want = job.want.saturating_sub(1);
        if job.want == 0 {
            self.run_ai(req);
        }
    }

    pub(super) fn run_ai(&mut self, req: u64) {
        let (Some(job), Some(key)) = (self.ai_jobs.remove(&req), self.ai_key.clone()) else { return };
        let lang = i18n::code();
        let p = self.proxy.clone();
        std::thread::spawn(move || {
            let r = ai::Request { prompt: &job.prompt, lang, selection: &job.selection, pages: &job.pages };
            let result = ai::ask(&key, &r);
            if let Err(e) = &result {
                log::warn!("Ask Jiyda: {e}");
            }
            let _ = p.send_event(UserEvent::AiAnswer { req, result });
        });
    }

    pub(super) fn set_ai_key(&mut self, key: &str) {
        let key = key.trim();
        match ai::save_key(key) {
            Ok(()) => {
                self.ai_key = (!key.is_empty()).then(|| key.to_string());
                if self.ai_key.is_some() {
                    self.cfg.ai = true;
                    self.save_config();
                }
                self.toast(i18n::tr(if key.is_empty() {
                    "Key removed"
                } else {
                    "Key saved in Windows Credential Manager"
                }));
            }
            Err(e) => {
                log::error!("не удалось сохранить ключ: {e}");
                self.toast(i18n::tr("Couldn't save the key"));
            }
        }
        self.ui_data("settings", self.settings_json());
        self.push_state();
    }
}
