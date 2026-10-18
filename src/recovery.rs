//! Восстановление после сбоя.
//!
//! Пока Jiyda работает, в папке данных лежит метка `running`. Нормальный выход её
//! снимает — в том числе когда Windows выключается или установщик закрывает Jiyda
//! (main.rs, `Event::LoopDestroyed`). Метка осталась с прошлого раза — значит, тот
//! запуск оборвался: падение, снятие процесса, отключение питания. Тогда страницы
//! возвращаются в список, но сами не загружаются: если Jiyda уронила страница, она
//! не уронит его снова сразу при запуске (см. `App::restore`).

use std::path::{Path, PathBuf};

/// Метка «Jiyda работает».
pub struct RunMarker {
    path: PathBuf,
}

impl RunMarker {
    /// Поставить метку. Второе значение — прошлый запуск оборвался (метка осталась).
    pub fn begin(path: &Path) -> (Self, bool) {
        let crashed = path.exists();
        if let Err(e) = std::fs::write(path, std::process::id().to_string()) {
            log::warn!("не удалось поставить метку запуска {}: {e}", path.display());
        }
        (Self { path: path.to_path_buf() }, crashed)
    }

    /// Нормальный выход: снять метку.
    pub fn finish(&self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::warn!("не удалось снять метку запуска {}: {e}", self.path.display()),
        }
    }
}

/// Попросить Windows открыть Jiyda снова, если его закрыла перезагрузка или обновление
/// системы (параметр Windows «Автоматически сохранять мои перезапускаемые приложения»).
/// После падения или зависания Windows его не перезапускает: страница, из-за которой
/// он упал, уронила бы его снова.
#[cfg(windows)]
pub fn register_restart() {
    use windows::Win32::System::Recovery::{RESTART_NO_CRASH, RESTART_NO_HANG, RegisterApplicationRestart};
    // SAFETY: без командной строки (null) — Jiyda откроется как обычно, с сессией.
    if let Err(e) =
        unsafe { RegisterApplicationRestart(windows::core::PCWSTR::null(), RESTART_NO_CRASH | RESTART_NO_HANG) }
    {
        log::debug!("RegisterApplicationRestart: {e}");
    }
}

#[cfg(not(windows))]
pub fn register_restart() {}

#[cfg(test)]
#[path = "../tests/unit/recovery.rs"]
mod tests;
