// В релизной сборке под Windows не показываем консольное окно.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod hotkeys;
mod input;
mod limit;
mod logging;
mod store;
mod tabs;

use app::{App, UserEvent};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

fn main() {
    logging::init(&config::Paths::new().log);
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let mut app = match App::new(&event_loop) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Не удалось запустить WebView2: {e}");
            log::error!("Проверьте, что установлен WebView2 Runtime: https://developer.microsoft.com/microsoft-edge/webview2/");
            std::process::exit(1);
        }
    };

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    app.shutdown();
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => app.layout(),
                _ => {}
            },
            Event::UserEvent(ev) => app.handle(ev),
            // Все события пачки обработаны — время для отложенных дел.
            Event::MainEventsCleared => app.tick(),
            _ => {}
        }
        if *control_flow != ControlFlow::Exit {
            *control_flow = ControlFlow::WaitUntil(app.next_wakeup());
        }
    });
}
