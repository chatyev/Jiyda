// В релизной сборке под Windows не показываем консольное окно.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod ai;
mod app;
mod bookmarks;
mod bookmarks_html;
mod command;
mod config;
mod engine;
mod frame;
mod hotkeys;
mod i18n;
mod import;
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
mod recovery;
mod settings;
mod shield;
mod spaces;
mod store;
mod telemetry;
mod update;

use app::{App, UserEvent};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

fn main() {
    // Адреса из командной строки (так Windows передаёт ссылки браузеру по умолчанию).
    let cwd = std::env::current_dir().unwrap_or_default();
    let urls = instance::urls_from_args(std::env::args().skip(1), &cwd);
    // Jiyda уже работает — отдаём адреса ему и выходим, второе окно не нужно.
    if instance::send_to_running(&urls) {
        return;
    }

    // Раньше браузер назывался Seyro (а до того Drift): переносим его данные до того, как что-то их откроет.
    let migration = config::migrate_old_data();
    logging::init(&config::Paths::new().log);
    match migration {
        Ok(true) => log::info!("данные Seyro / Drift перенесены в папку Jiyda"),
        Ok(false) => {}
        Err(e) => log::warn!("не удалось перенести старые данные, работаю со старой папкой: {e}"),
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
    let waker = Waker::start(event_loop.create_proxy());
    if !urls.is_empty() {
        app.handle(UserEvent::External { urls });
    }

    // Перезагрузка Windows для обновления закроет Jiyda — пусть потом откроет снова.
    recovery::register_restart();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
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
            // Цикл заканчивается: окно закрыли, или Windows выключается, или установщик
            // закрывает Jiyda (WM_ENDSESSION — после него tao сразу завершает процесс).
            // Сохраняем здесь — это единственное место, через которое проходят все выходы.
            Event::LoopDestroyed => {
                app.shutdown();
                return;
            }
            _ => {}
        }
        // Закрыть просили не окном (команда, обновление) — выходим после любой обработки.
        if app.exit_requested() {
            *control_flow = ControlFlow::Exit;
            return;
        }
        if *control_flow != ControlFlow::Exit {
            // Не WaitUntil: см. Waker.
            *control_flow = ControlFlow::Wait;
            waker.wake_at(app.next_wakeup());
        }
    });
}

/// Будильник цикла событий: в нужный момент присылает UserEvent::Wake, а цикл ждёт без срока
/// (ControlFlow::Wait).
///
/// Почему не ControlFlow::WaitUntil: создание WebView2 крутит свой цикл сообщений внутри нашего
/// обработчика, и если срок WaitUntil истекал в это время, tao начинал новый проход цикла прямо
/// внутри незавершённого и падал («either event handler is re-entrant»). Пользовательские
/// события tao в такой момент откладывает в очередь — поэтому будим ими.
struct Waker {
    tx: std::sync::mpsc::Sender<std::time::Instant>,
    last: std::cell::Cell<Option<std::time::Instant>>,
}

impl Waker {
    fn start(proxy: tao::event_loop::EventLoopProxy<UserEvent>) -> Self {
        use std::sync::mpsc::RecvTimeoutError;
        let (tx, rx) = std::sync::mpsc::channel::<std::time::Instant>();
        std::thread::Builder::new()
            .name("jiyda-waker".into())
            .spawn(move || {
                let mut at: Option<std::time::Instant> = None;
                loop {
                    let next = match at {
                        Some(t) => rx.recv_timeout(t.saturating_duration_since(std::time::Instant::now())),
                        None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    match next {
                        Ok(t) => at = Some(t),
                        Err(RecvTimeoutError::Timeout) => {
                            at = None;
                            if proxy.send_event(UserEvent::Wake).is_err() {
                                break; // цикл событий закрыт
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("поток будильника");
        Self { tx, last: std::cell::Cell::new(None) }
    }

    /// Разбудить цикл в момент `at` (последний заданный срок заменяет прежний).
    fn wake_at(&self, at: std::time::Instant) {
        if self.last.get() != Some(at) {
            self.last.set(Some(at));
            let _ = self.tx.send(at);
        }
    }
}
