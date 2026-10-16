//! Журнал в %LOCALAPPDATA%\Seyro\seyro.log (+ консоль в отладочной сборке).

use std::fs::OpenOptions;
use std::path::Path;

use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, LevelFilter, SharedLogger, TermLogger,
    TerminalMode, WriteLogger,
};

/// Больше этого размера журнал при запуске переносится в seyro.old.log.
const MAX_LOG_SIZE: u64 = 1024 * 1024;

pub fn init(path: &Path) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_SIZE) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }

    let mut builder = ConfigBuilder::new();
    builder.set_time_format_rfc3339();
    let _ = builder.set_time_offset_to_local();
    let cfg = builder.build();

    let level = if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info };
    let mut loggers: Vec<Box<dyn SharedLogger>> = Vec::new();
    if cfg!(debug_assertions) {
        loggers.push(TermLogger::new(level, cfg.clone(), TerminalMode::Stderr, ColorChoice::Auto));
    }
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => loggers.push(WriteLogger::new(level, cfg, file)),
        Err(e) => eprintln!("не удалось открыть журнал {}: {e}", path.display()),
    }
    let _ = CombinedLogger::init(loggers);

    // В релизе panic = "abort", поэтому без хука причина падения потеряется.
    std::panic::set_hook(Box::new(|info| {
        log::error!("паника: {info}");
    }));

    log::info!("Seyro Browser {} запущен", env!("CARGO_PKG_VERSION"));
}
