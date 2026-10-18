//! Создание браузера: окно, интерфейс, профили пространств, восстановление сессии.

use super::*;

impl App {
    pub fn new(event_loop: &EventLoop<UserEvent>) -> Result<Self, String> {
        let paths = Paths::new();
        let cfg = Config::load(&paths);
        // Язык — до всего, что пишет пользователю (в том числе имён пространств по умолчанию).
        let lang = i18n::init(&cfg.language);
        log::info!("язык интерфейса: {lang:?}");
        let (run_marker, crashed) = crate::recovery::RunMarker::begin(&paths.running);
        if crashed {
            log::warn!(
                "прошлый запуск Jiyda оборвался (сбой или снятие процесса): страницы восстановлены без загрузки"
            );
        }
        let mut session = Session::load(&paths.session);
        let theme = session
            .theme
            .clone()
            .filter(|t| matches!(t.as_str(), "system" | "light" | "dark"))
            .unwrap_or_else(|| "system".into());
        let deleted_spaces = remove_deleted_spaces(&paths, std::mem::take(&mut session.deleted_spaces));
        let saved_spaces: Vec<SavedSpace> =
            spaces::normalize(session.take_spaces()).into_iter().filter(|s| !deleted_spaces.contains(&s.id)).collect();
        let history = History::load(&paths.history);
        let downloads = Downloads::load(&paths.downloads);
        let favicons = Favicons::load(&paths.favicons);
        let proxy = event_loop.create_proxy();

        // Решения ядра, которые движку нужны прямо во время работы страниц.
        let keys = Rc::new(Keys::default());
        *keys.map.borrow_mut() = Keymap::new(&cfg.shortcuts);
        let shield_state = Rc::new(RefCell::new(ShieldState { enabled: cfg.shield, allow: cfg.shield_allow.clone() }));
        let perms = Rc::new(RefCell::new(permissions::Store::load(&paths.permissions)));
        let policy = engine::Policy {
            keys: keys.clone(),
            shield: {
                let s = shield_state.clone();
                Rc::new(move |page: &str| s.borrow().blocks_on(page))
            },
            permission: {
                let p = perms.clone();
                Rc::new(move |origin: &str, kind: &str| p.borrow().get(origin, kind))
            },
        };
        // События движка — в цикл событий (обработчики WebView2 могут звать и из других потоков).
        let sink: engine::Sink = {
            let p = std::sync::Mutex::new(proxy.clone());
            std::sync::Arc::new(move |id, ev| {
                if let (Some(ev), Ok(p)) = (page_event(id, ev), p.lock()) {
                    let _ = p.send_event(ev);
                }
            })
        };
        let options = engine::Options {
            devtools: cfg.devtools,
            extensions: cfg.extensions,
            extensions_dir: paths.extensions.clone(),
        };
        let engine = engine::default_engine(options, policy, sink);

        let mut space_slots: Vec<Space> = saved_spaces
            .iter()
            .map(|s| Space {
                id: s.id,
                name: s.name.clone(),
                color: s.color.clone(),
                ctx: engine.profile(spaces::profile_dir(&paths, s.id)),
                pages: PageList::with_id_base(s.closed.clone(), spaces::page_id_base(s.id)),
            })
            .collect();

        let builder = WindowBuilder::new()
            .with_title("Jiyda")
            .with_inner_size(tao::dpi::LogicalSize::new(1360.0, 860.0))
            .with_min_inner_size(tao::dpi::LogicalSize::new(720.0, 480.0))
            .with_theme(window_theme(&theme))
            .with_decorations(!cfg.custom_titlebar);
        // Без рамки Windows всё равно рисует тень, скругляет углы и даёт тянуть края.
        // Значок окна и панели задач — тот же, что встроен в jiyda.exe (ресурс 1, см. build.rs).
        #[cfg(windows)]
        let builder = {
            use tao::platform::windows::{IconExtWindows, WindowBuilderExtWindows};
            use tao::window::Icon;
            let icon = |size: Option<u32>| Icon::from_resource(1, size.map(|s| tao::dpi::PhysicalSize::new(s, s))).ok();
            builder.with_undecorated_shadow(true).with_window_icon(icon(None)).with_taskbar_icon(icon(Some(256)))
        };
        let window = builder.build(event_loop).expect("не удалось создать окно");
        let frame = if cfg.custom_titlebar { frame::Frame::install(&window) } else { None };

        // Интерфейс — прозрачная страница во всё окно поверх страниц, обрезанная регионом.
        // Он живёт в профиле «Основного» (normalize ставит его первым).
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        let nav_proxy = proxy.clone();
        let ui_spec = engine::UiSpec {
            // Файлы интерфейса — изнутри jiyda.exe по адресу jiyda://ui/ (ui_assets.rs).
            url: ui_assets::INDEX_URL.into(),
            scheme: ui_assets::SCHEME,
            asset: ui_assets::asset,
            is_own_url: ui_assets::is_ui_url,
            on_leave: Box::new(move |url| {
                let _ = nav_proxy.send_event(UserEvent::OpenTab { url, background: false });
            }),
            init_script: format!(
                "window.JIYDA_CONFIG = {};",
                json!({
                    "frameless": cfg.custom_titlebar,
                    "lang": i18n::code(),
                    "logo": format!("data:image/png;base64,{}", base64(LOGO_PNG)),
                    "chromeH": CHROME_H,
                    "captionW": frame::CAPTION_W,
                    "splitGap": SPLIT_GAP,
                    "paneH": PANE_H,
                    "version": update::CURRENT,
                })
            ),
            bounds: rect(0.0, 0.0, size.width, size.height),
            theme: engine::Theme::from_setting(&theme),
            dark: theme_is_dark(&theme, &window),
        };
        let ui = engine.create_ui(&window, &mut space_slots[0].ctx, ui_spec)?;
        let daily = shield::Daily::load(&paths.shield);
        let ai_key = ai::load_key();

        let active_space = session.active_space.min(space_slots.len() - 1);
        let mut app = Self {
            writer: crate::config::Writer::new(),
            run_marker,
            recovery: None,
            frame,
            window,
            ui,
            proxy,
            cfg,
            paths,
            pages: PageList::new(vec![]),
            spaces: space_slots,
            active_space,
            deleted_spaces,
            theme,
            find_open: false,
            history,
            downloads,
            favicons,
            save_due: None,
            next_sleep_check: Instant::now() + SLEEP_CHECK,
            page_tabs: RateLimit::new(PAGE_TABS_MAX, PAGE_TABS_WINDOW),
            exit_requested: false,
            maximized: false,
            fullscreen: false,
            page_fullscreen: None,
            updates: Updates::new(),
            keys,
            shield: shield_state,
            daily,
            page_blocks: HashMap::new(),
            perms,
            engine,
            region: UiRegion { full: true, ..Default::default() },
            ui_modal: false,
            mru_walk: MruWalk::default(),
            state_dirty: Cell::new(false),
            home_return: None,
            removed_history: Vec::new(),
            capture_due: Vec::new(),
            ai_key,
            ai_jobs: HashMap::new(),
            peek: None,
        };
        app.maximized = app.window.is_maximized();
        app.apply_region();
        app.restore(saved_spaces, crashed);
        Ok(app)
    }
}
