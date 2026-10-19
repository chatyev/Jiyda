//! Автообновление: проверка версии на GitHub, скачивание и запуск установщика.

use super::*;

impl App {
    // ---------- Обновления ----------

    /// Спросить GitHub о новой версии (в фоне). `manual` — по команде пользователя:
    /// тогда сообщаем и «у вас последняя версия», и ошибки.
    pub(super) fn check_updates(&mut self, manual: bool) {
        if self.updates.busy {
            return;
        }
        self.updates.busy = true;
        let proxy = self.proxy.clone();
        let beta = self.cfg.beta_updates;
        std::thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::UpdateChecked { result: update::check(beta), manual });
        });
    }

    pub(super) fn on_update_checked(
        &mut self,
        result: Result<Option<update::Release>, update::CheckError>,
        manual: bool,
    ) {
        self.updates.busy = false;
        match result {
            Ok(Some(release)) => {
                log::info!("доступна версия {}", release.version);
                if manual {
                    self.updates.dismissed = None;
                }
                self.updates.release = Some(release);
                self.push_state();
            }
            Ok(None) => {
                if manual {
                    self.toast(&i18n::trf("You have the latest version of Jiyda ({})", &[&update::CURRENT]));
                }
            }
            Err(e) => {
                log::warn!("не удалось проверить обновления: {e}");
                if manual {
                    self.toast(i18n::tr(match e {
                        update::CheckError::Limited => "GitHub is limiting update checks right now. Try again later.",
                        update::CheckError::Other(_) => "Couldn't check for updates. Is GitHub reachable?",
                    }));
                }
            }
        }
    }

    /// Скачать установщик (в фоне), а когда скачается — закрыться и запустить его.
    pub(super) fn install_update(&mut self) {
        let Some(release) = self.updates.release.clone() else { return };
        if self.updates.busy {
            return;
        }
        self.updates.busy = true;
        self.updates.downloading = true;
        self.push_state();
        let dir = std::env::temp_dir().join("Jiyda");
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::UpdateDownloaded { result: update::download(&release, &dir) });
        });
    }

    pub(super) fn on_update_downloaded(&mut self, result: Result<std::path::PathBuf, String>) {
        self.updates.busy = false;
        self.updates.downloading = false;
        let path = match result {
            Ok(p) => p,
            Err(e) => {
                log::error!("не удалось скачать обновление: {e}");
                self.toast(i18n::tr("Couldn't download the update"));
                self.push_state();
                return;
            }
        };
        // Всё сохраняем до запуска установщика: он заменит jiyda.exe и откроет Jiyda заново.
        self.save_now();
        match std::process::Command::new(&path).args(update::INSTALLER_ARGS).spawn() {
            Ok(_) => {
                log::info!("запущен установщик обновления {}", path.display());
                self.exit_requested = true;
            }
            Err(e) => {
                log::error!("не удалось запустить установщик {}: {e}", path.display());
                self.toast(i18n::tr("Couldn't start the update installer"));
                self.push_state();
            }
        }
    }
}
