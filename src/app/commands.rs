//! Команды браузера (горячие клавиши, меню, палитра) и Ctrl+Tab.

use super::*;

/// Лицензии сторонних библиотек на сайте (если рядом с jiyda.exe их нет).
const LICENSES_URL: &str = "https://chatyev.github.io/jiyda/licenses.html";

impl App {
    // ---------- Команды ----------

    pub(super) fn command(&mut self, cmd: Command) {
        match cmd {
            // Слои интерфейса.
            Command::Palette
            | Command::Orbit
            | Command::Spaces
            | Command::History
            | Command::Closed
            | Command::Ask
            | Command::Settings
            | Command::Shortcuts
            | Command::Focus => self.ui_command(&cmd.to_string(), Value::Null),
            Command::Bookmarks => {
                self.push_bookmarks();
                self.ui_command("bookmarks", Value::Null);
            }
            Command::Bookmark => self.toggle_bookmark(),
            Command::ImportDismiss => self.dismiss_import_offer(),
            Command::Onboarding => {
                self.onboarding = true;
                self.push_state();
            }
            Command::OnboardingDone => {
                self.onboarding = false;
                // Перенос из Chrome и Edge предлагался в знакомстве — карточка на New Page не нужна.
                self.dismiss_import_offer();
                self.push_state();
            }
            Command::Downloads => {
                self.push_downloads();
                self.ui_command("downloads", Value::Null);
            }
            Command::EditUrl => self.ui_command("edit_url", Value::Null),
            Command::NewTab => {
                self.show_home();
                self.ui_command("new_tab", Value::Null);
            }
            Command::Close => {
                if let Some(id) = self.pages.active() {
                    self.close_tab(id, true);
                }
            }
            Command::Reopen => self.reopen_closed(),
            Command::Next | Command::Prev => self.mru_step(cmd == Command::Next),
            Command::NextSeq => self.activate_opt(self.pages.cycle_target(1)),
            Command::PrevSeq => self.activate_opt(self.pages.cycle_target(-1)),
            Command::Later => {
                if let Some(id) = self.pages.active() {
                    self.move_to_later(id);
                }
            }
            Command::Pin => {
                if let Some(id) = self.pages.active() {
                    let p = self.pages.get(id).is_some_and(|t| t.pinned);
                    self.set_pinned(id, !p);
                }
            }
            Command::Back => self.with_active_wv(|wv| {
                wv.eval("history.back()");
            }),
            Command::Forward => self.with_active_wv(|wv| {
                wv.eval("history.forward()");
            }),
            Command::Reload => self.with_active_wv(|wv| {
                wv.reload();
            }),
            Command::HardReload => self.with_active_wv(|wv| wv.hard_reload()),
            Command::Stop => self.with_active_wv(|wv| {
                wv.eval("window.stop()");
            }),
            Command::Devtools => self.with_active_wv(|wv| wv.open_devtools()),
            Command::Fullscreen => self.toggle_fullscreen(),
            Command::WinMin => self.window.set_minimized(true),
            Command::WinMax => self.window.set_maximized(!self.window.is_maximized()),
            Command::WinClose => self.exit_requested = true,
            Command::CopyUrl => {
                if let Some(url) = self.pages.active_page().map(|t| t.url.clone()) {
                    if hotkeys::copy_to_clipboard(&url) {
                        self.toast(i18n::tr("Link copied"));
                    } else {
                        self.toast(i18n::tr("Couldn't copy the link"));
                    }
                }
            }
            Command::Clear => self.clear_unpinned(),
            Command::SpaceNew => self.new_space(),
            Command::Split => {
                if self.pages.active().is_some() {
                    self.ui_command("split", Value::Null);
                }
            }
            Command::Unsplit => {
                if let Some(id) = self.pages.active() {
                    self.unsplit(id);
                }
            }
            Command::SplitSwap => {
                if let Some(id) = self.pages.active()
                    && self.pages.swap_split(id)
                {
                    self.layout();
                    self.push_state();
                    self.save();
                }
            }
            Command::Find => self.open_find(),
            Command::OpenDownloadsFolder => {
                if let Some(d) = self.download_dir() {
                    self.open_folder(&d);
                }
            }
            Command::OpenDataFolder => self.open_folder(self.paths.base()),
            Command::Licenses => {
                // Установщик кладёт лицензии рядом с jiyda.exe; без него (сборка для
                // разработки) — та же страница на сайте.
                let local = std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(|d| d.join("third-party-licenses.html")))
                    .filter(|p| p.is_file())
                    .and_then(|p| url::Url::from_file_path(p).ok());
                let url = local.map_or_else(|| LICENSES_URL.to_string(), |u| u.to_string());
                self.open_tab(url, false, None);
            }
            Command::ClearHistory => self.clear_history(),
            Command::ClearData => self.clear_site_data(),
            Command::ClearClosed => {
                self.pages.clear_closed();
                self.push_state();
                self.save();
            }
            Command::DefaultBrowser => self.make_default_browser(),
            Command::CheckUpdates => self.check_updates(true),
            Command::Theme => self.cycle_theme(),
            Command::Home => self.show_home(),
            Command::UpdateNow => self.install_update(),
            Command::UpdateDismiss => {
                self.updates.dismissed = self.updates.release.as_ref().map(|u| u.version.clone());
                self.push_state();
            }
            Command::Recover => self.recover(),
            Command::RecoverDismiss => {
                self.recovery = None;
                self.push_state();
            }
            Command::Space(n) => self.switch_space(n - 1),
            Command::Goto(n) => self.activate_opt(self.pages.goto_target(n)),
        }
    }

    /// Ctrl+Tab: по последним открытым страницам. Нажатия подряд идут дальше по
    /// списку, замороженному при первом нажатии (как Alt+Tab в Windows).
    pub(super) fn mru_step(&mut self, forward: bool) {
        let tabs = &self.pages;
        let target = self.mru_walk.step(Instant::now(), MRU_WALK, forward, || tabs.mru(), |id| tabs.get(id).is_some());
        if let Some(id) = target {
            self.activate(id);
        }
    }
}
