//! Настройки (Settings): изменение, тема, защита сайтов.

use super::*;

impl App {
    // ---------- Настройки ----------

    pub(super) fn settings_json(&self) -> Value {
        let c = &self.cfg;
        json!({
            "search_url": c.search_url,
            "archive_hours": c.archive_hours,
            "sleep_minutes": c.sleep_minutes,
            "extensions": c.extensions,
            "devtools": c.devtools,
            "custom_titlebar": c.custom_titlebar,
            "check_updates": c.check_updates,
            "language": c.language,
            "shield": c.shield,
            "shield_allow": c.shield_allow,
            "selection_toolbar": c.selection_toolbar,
            "edge_reveal": c.edge_reveal,
            "dim_chrome": c.dim_chrome,
            "motion": c.motion,
            "ai": c.ai,
            "ai_key": self.ai_key.is_some(),
            "download_dir": self.download_dir().map(|d| d.display().to_string()).unwrap_or_default(),
            "theme": self.theme,
            "keys": self.keys.map.borrow().to_json(),
            "permissions": self.perms.borrow().to_value(),
            "version": update::CURRENT,
            "data_dir": self.paths.base().display().to_string(),
            "default_browser": instance::registered_as_browser(),
        })
    }

    /// Изменить настройку из Settings. Значения проверяет `SettingChange::apply`;
    /// здесь — то, что затрагивает окно и уже открытые WebView.
    pub(super) fn set_setting(&mut self, change: SettingChange) {
        // Тема — в сессии, а сочетание сначала проверяем на занятость другой командой.
        match &change {
            SettingChange::Theme(t) => {
                self.set_theme(t.as_str());
                return;
            }
            SettingChange::Shortcut(s) if !s.keys.is_empty() => {
                let owner = Combo::parse(&s.keys).and_then(|c| self.keys.map.borrow().owner(&c));
                if let Some(other) = owner.filter(|o| *o != s.cmd) {
                    let conflict = json!({ "cmd": s.cmd.to_string(), "keys": s.keys, "owner": other.to_string() });
                    self.ui_data("shortcut_conflict", conflict);
                    return;
                }
            }
            _ => {}
        }
        let applied = match change.clone().apply(&mut self.cfg) {
            Ok(a) => a,
            Err(message) => {
                self.toast(i18n::tr(message));
                return;
            }
        };
        match change {
            SettingChange::Shield(on) => self.shield.borrow_mut().enabled = on,
            SettingChange::SelectionToolbar(on) => {
                // Уже открытым страницам — сразу, новым — через скрипт при создании.
                let js = format!("window.__jiydaSel && window.__jiydaSel({on})");
                for wv in self.all_views() {
                    wv.eval(&js);
                }
            }
            SettingChange::DownloadDir(_) => {
                for wv in self.all_views() {
                    wv.set_download_folder(&self.cfg.download_dir);
                }
            }
            SettingChange::Shortcut(_) | SettingChange::ShortcutReset(_) => {
                *self.keys.map.borrow_mut() = Keymap::new(&self.cfg.shortcuts);
            }
            _ => {}
        }
        self.save_config();
        if applied.restart {
            self.toast(i18n::tr("This change takes effect after restarting Jiyda"));
        }
        self.ui_data("settings", self.settings_json());
        self.push_state();
    }

    /// Все загруженные WebView страниц во всех пространствах.
    pub(super) fn all_views(&self) -> impl Iterator<Item = &View> {
        self.pages.iter().chain(self.spaces.iter().flat_map(|s| s.pages.iter())).filter_map(|t| t.view.as_ref())
    }

    pub(super) fn set_theme(&mut self, theme: &str) {
        self.theme = theme.to_string();
        self.window.set_theme(window_theme(&self.theme));
        // Уже открытые страницы во всех пространствах — тоже.
        let theme = engine::Theme::from_setting(&self.theme);
        self.ui.set_theme(theme);
        let views =
            self.pages.iter().chain(self.spaces.iter().flat_map(|s| s.pages.iter())).filter_map(|t| t.view.as_ref());
        for wv in views {
            wv.set_theme(theme);
        }
        self.push_state();
        self.save();
    }

    /// «Сменить тему»: как в Windows → светлая → тёмная → как в Windows.
    pub(super) fn cycle_theme(&mut self) {
        let t = next_theme(&self.theme);
        self.set_theme(t);
        self.toast(i18n::tr(match t {
            "light" => "Theme: light",
            "dark" => "Theme: dark",
            _ => "Theme: same as Windows",
        }));
    }

    pub(super) fn security_json(&self) -> Value {
        let Some(t) = self.pages.active_page() else { return Value::Null };
        let origin = permissions::origin_of(&t.url).unwrap_or_default();
        let site = shield::site_of(&t.url).unwrap_or_default();
        let (trackers, ads) = self.page_blocks.get(&t.id).copied().unwrap_or((0, 0));
        let perms = self.perms.borrow();
        json!({
            "id": t.id,
            "url": t.url,
            "site": site,
            "origin": origin,
            "favicon": self.icon_for(&t.url, &t.favicon),
            "https": t.url.starts_with("https://"),
            "web": t.url.starts_with("http"),
            "trackers": trackers,
            "ads": ads,
            "shield": self.cfg.shield,
            "siteShield": !self.cfg.shield_allow.contains(&site),
            "space": self.spaces[self.active_space].name,
            "allowed": perms.allowed_count(&origin),
            "permissions": perms.sites.get(&origin).map(|k| serde_json::to_value(k).unwrap_or_default()).unwrap_or(json!({})),
        })
    }

    pub(super) fn set_site_shield(&mut self, site: &str, on: bool) {
        let site = site.trim().to_ascii_lowercase();
        if site.is_empty() {
            return;
        }
        self.cfg.shield_allow.retain(|s| *s != site);
        if !on {
            self.cfg.shield_allow.push(site);
        }
        self.shield.borrow_mut().allow = self.cfg.shield_allow.clone();
        self.save_config();
        self.with_active_wv(|wv| {
            wv.reload();
        });
        self.ui_data("security", self.security_json());
    }
}
