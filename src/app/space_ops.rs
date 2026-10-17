//! Пространства (Spaces): переключение, создание, переименование, удаление, перенос страниц.

use super::*;

impl App {
    // ---------- Пространства ----------

    /// Страница в любом пространстве: события приходят и от страниц, которых сейчас не видно.
    pub(super) fn page_any(&self, id: u64) -> Option<&Page<WebView>> {
        self.pages.get(id).or_else(|| self.spaces.iter().find_map(|s| s.pages.get(id)))
    }

    pub(super) fn page_mut_any(&mut self, id: u64) -> Option<&mut Page<WebView>> {
        if self.pages.get(id).is_some() {
            return self.pages.get_mut(id);
        }
        self.spaces.iter_mut().find_map(|s| s.pages.get_mut(id))
    }

    pub(super) fn switch_space(&mut self, idx: usize) {
        if idx == self.active_space || idx >= self.spaces.len() {
            return;
        }
        if self.find_open {
            self.find_open = false;
            self.run_find("", "clear");
        }
        self.close_peek();
        if let Some(a) = self.pages.active() {
            self.capture(a);
        }
        for t in self.pages.iter() {
            if let Some(wv) = &t.view {
                wv.set_visible(false).or_log("set_visible");
            }
        }
        // Страницы активного пространства живут в self.pages, остальные — в своих слотах.
        std::mem::swap(&mut self.pages, &mut self.spaces[self.active_space].pages);
        self.active_space = idx;
        std::mem::swap(&mut self.pages, &mut self.spaces[idx].pages);
        self.home_return = None;
        match self.pages.active().or_else(|| self.pages.now_order().first().copied()) {
            Some(id) => self.activate(id),
            None => {
                self.focus_ui_if_empty();
                self.layout();
                self.push_state();
                self.save();
            }
        }
    }

    /// Новое пространство: свой профиль (пустые куки), сразу переходим в него.
    pub(super) fn new_space(&mut self) {
        let mut used: Vec<(u32, &str)> = self.spaces.iter().map(|s| (s.id, s.color.as_str())).collect();
        // Номера удалённых пространств не берём: их папки, возможно, ещё не стёрты.
        used.extend(self.deleted_spaces.iter().map(|id| (*id, "")));
        let Some(mut meta) = spaces::new_space(&used) else {
            self.toast(&i18n::trf("You can't have more than {} Spaces", &[&spaces::MAX_SPACES]));
            return;
        };
        // На диске может остаться папка от старых данных — новому пространству нужен чистый профиль.
        while spaces::space_dir(&self.paths, meta.id).exists() {
            meta.id += 1;
            meta.name = spaces::default_name(meta.id);
        }
        log::info!("создано пространство {} «{}»", meta.id, meta.name);
        self.spaces.push(Space {
            id: meta.id,
            name: meta.name,
            color: meta.color,
            ctx: WebContext::new(Some(spaces::profile_dir(&self.paths, meta.id))),
            pages: PageList::with_id_base(vec![], spaces::page_id_base(meta.id)),
        });
        self.switch_space(self.spaces.len() - 1);
        self.ui_command("space_created", json!(self.spaces.len() - 1));
    }

    pub(super) fn rename_space(&mut self, idx: usize, name: &str) {
        if let (Some(name), Some(s)) = (spaces::clean_name(name), self.spaces.get_mut(idx)) {
            s.name = name;
            self.push_state();
            self.save();
        }
    }

    pub(super) fn set_space_color(&mut self, idx: usize, color: &str) {
        if spaces::is_color(color)
            && let Some(s) = self.spaces.get_mut(idx)
        {
            s.color = color.to_string();
            self.push_state();
            self.save();
        }
    }

    /// Удалить пространство: страницы закрываются, папка профиля стирается при
    /// следующем запуске. «Основное» удалить нельзя.
    pub(super) fn delete_space(&mut self, idx: usize) {
        let Some(id) = self.spaces.get(idx).map(|s| s.id) else { return };
        if id == spaces::MAIN_ID {
            self.toast(i18n::tr("The main Space can't be deleted"));
            return;
        }
        let name = self.spaces[idx].name.clone();
        if idx == self.active_space {
            let target = if idx > 0 { idx - 1 } else { 1 };
            self.switch_space(target);
        }
        let removed = self.spaces.remove(idx);
        if self.active_space > idx {
            self.active_space -= 1;
        }
        drop(removed);
        self.deleted_spaces.push(id);
        log::info!("удалено пространство {id} «{name}», данные будут стёрты при следующем запуске");
        self.toast(&i18n::trf("Space \u{201c}{}\u{201d} deleted", &[&name]));
        self.push_state();
        self.save_now();
    }

    /// Перенести страницу в другое пространство. Куки там свои, поэтому страница
    /// откроется заново (спящей) с тем же адресом.
    pub(super) fn move_to_space(&mut self, id: u64, idx: usize) {
        if idx == self.active_space || idx >= self.spaces.len() {
            return;
        }
        self.permission_requests.drop_tab(id);
        let Some((st, after)) = self.pages.take(id) else { return };
        self.spaces[idx].pages.insert_saved(st, now());
        let name = self.spaces[idx].name.clone();
        self.after_close(after);
        self.toast(&i18n::trf("Moved to \u{201c}{}\u{201d}", &[&name]));
    }
}
