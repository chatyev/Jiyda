// В релизной сборке под Windows не показываем консольное окно.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod ai;
mod app;
mod command;
mod config;
mod frame;
mod hotkeys;
mod i18n;
mod input;
mod instance;
mod ipc;
mod keymap;
mod limit;
mod logging;
mod net;
mod orbit;
mod page_list;
mod permissions;
mod settings;
mod shield;
mod spaces;
mod store;
mod update;
mod webhooks;

use app::{App, UserEvent};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

fn main() {
    // Адреса из командной строки (так Windows передаёт ссылки браузеру по умолчанию).
    let cwd = std::env::current_dir().unwrap_or_default();
    let urls = instance::urls_from_args(std::env::args().skip(1), &cwd);
    // Seyro уже работает — отдаём адреса ему и выходим, второе окно не нужно.
    if instance::send_to_running(&urls) {
        return;
    }

    // До ребрендинга браузер назывался Drift: переносим его данные до того, как что-то их откроет.
    let migration = config::migrate_old_data();
    logging::init(&config::Paths::new().log);
    match migration {
        Ok(true) => log::info!("данные Drift перенесены в папку Seyro"),
        Ok(false) => {}
        Err(e) => log::warn!("не удалось перенести данные Drift, работаю со старой папкой: {e}"),
    }
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let mut app = match App::new(&event_loop) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Не удалось запустить WebView2: {e}");
            log::error!(
                "Проверьте, что установлен WebView2 Runtime: https://developer.microsoft.com/microsoft-edge/webview2/"
            );
            std::process::exit(1);
        }
    };
    let proxy = event_loop.create_proxy();
    instance::listen(move |urls| {
        let _ = proxy.send_event(UserEvent::External { urls });
    });
    if !urls.is_empty() {
        app.handle(UserEvent::External { urls });
    }

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    app.shutdown();
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => app.on_resized(),
                WindowEvent::Focused(true) => app.on_focus(),
                _ => {}
            },
            Event::UserEvent(ev) => app.handle(ev),
            // Все события пачки обработаны — время для отложенных дел.
            Event::MainEventsCleared => app.tick(),
            _ => {}
        }
        // Закрыть просили не окном (команда, обновление) — выходим после любой обработки.
        if app.exit_requested() {
            app.shutdown();
            *control_flow = ControlFlow::Exit;
            return;
        }
        if *control_flow != ControlFlow::Exit {
            *control_flow = ControlFlow::WaitUntil(app.next_wakeup());
        }
    });
}
