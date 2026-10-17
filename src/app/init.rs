//! Создание браузера: окно, интерфейс, профили пространств, восстановление сессии.

use super::*;

impl App {
    pub fn new(event_loop: &EventLoop<UserEvent>) -> wry::Result<Self> {
        let paths = Paths::new();
        let cfg = Config::load(&paths);
        // Язык — до всего, что пишет пользователю (в том числе имён пространств по умолчанию).
        let lang = i18n::init(&cfg.language);
        log::info!("язык интерфейса: {lang:?}");
        let mut session = Session::load(&paths.session);
        let theme = session
            .theme
            .clone()
            .filter(|t| matches!(t.as_str(), "system" | "light" | "dark"))
            .unwrap_or_else(|| "system".into());
        let deleted_spaces = remove_deleted_spaces(&paths, std::mem::take(&mut session.deleted_spaces));
        let saved_spaces: Vec<SavedSpace> =
            spaces::normalize(session.take_spaces()).into_iter().filter(|s| !deleted_spaces.contains(&s.id)).collect();
        let mut space_slots: Vec<Space> = saved_spaces
            .iter()
            .map(|s| Space {
                id: s.id,
                name: s.name.clone(),
                color: s.color.clone(),
                ctx: WebContext::new(Some(spaces::profile_dir(&paths, s.id))),
                pages: PageList::with_id_base(s.closed.clone(), spaces::page_id_base(s.id)),
            })
            .collect();
        let history = History::load(&paths.history);
        let downloads = Downloads::load(&paths.downloads);
        let favicons = Favicons::load(&paths.favicons);
        let proxy = event_loop.create_proxy();

        let builder = WindowBuilder::new()
            .with_title("Seyro")
            .with_inner_size(tao::dpi::LogicalSize::new(1360.0, 860.0))
            .with_min_inner_size(tao::dpi::LogicalSize::new(720.0, 480.0))
            .with_theme(window_theme(&theme))
            .with_decorations(!cfg.custom_titlebar);
        // Без рамки Windows всё равно рисует тень, скругляет углы и даёт тянуть края.
        // Значок окна и панели задач — тот же, что встроен в seyro.exe (ресурс 1, см. build.rs).
        #[cfg(windows)]
        let builder = {
            use tao::platform::windows::{IconExtWindows, WindowBuilderExtWindows};
            use tao::window::Icon;
            let icon = |size: Option<u32>| Icon::from_resource(1, size.map(|s| tao::dpi::PhysicalSize::new(s, s))).ok();
            builder.with_undecorated_shadow(true).with_window_icon(icon(None)).with_taskbar_icon(icon(Some(256)))
        };
        let window = builder.build(event_loop).expect("не удалось создать окно");
        let frame = if cfg.custom_titlebar { frame::Frame::install(&window) } else { None };

        // Интерфейс — прозрачный WebView во всё окно поверх страниц, обрезанный регионом.
        // Он живёт в профиле «Основного» (normalize ставит его первым).
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        let ui_proxy = proxy.clone();
        let nav_proxy = proxy.clone();
        let ui_builder = common(
            WebViewBuilder::new_with_web_context(&mut space_slots[0].ctx),
            &cfg,
            &paths,
            &theme,
            theme_is_dark(&theme, &window),
        );
        // Интерфейс — с https-адреса (https://seyro.ui/): защищённый источник.
        #[cfg(windows)]
        let ui_builder = {
            use wry::WebViewBuilderExtWindows;
            ui_builder.with_https_scheme(true)
        };
        let ui = ui_builder
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            // Файлы интерфейса — изнутри seyro.exe по адресу seyro://ui/ (ui_assets.rs).
            .with_custom_protocol(ui_assets::SCHEME.into(), |_, req| ui_assets::respond(&req))
            .with_url(ui_assets::INDEX_URL)
            .with_bounds(rect(0.0, 0.0, size.width, size.height))
            .with_initialization_script(format!(
                "window.SEYRO_CONFIG = {};",
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
            ))
            .with_ipc_handler(move |req| {
                let _ = ui_proxy.send_event(UserEvent::FromUi(req.body().clone()));
            })
            .with_navigation_handler(move |url| {
                // Интерфейс никуда не уходит: внешние ссылки открываем страницей.
                if ui_assets::is_ui_url(&url) {
                    true
                } else if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = nav_proxy.send_event(UserEvent::OpenTab { url, background: false });
                    false
                } else {
                    true
                }
            })
            .build_as_child(&window)?;
        frame::configure_ui(&ui);

        let keys = Rc::new(Keys::default());
        *keys.map.borrow_mut() = Keymap::new(&cfg.shortcuts);
        {
            let p = proxy.clone();
            hotkeys::install(&ui, keys.clone(), move |ev| {
                let _ = p.send_event(match ev {
                    KeyEvent::Command(cmd) => UserEvent::Key { id: None, cmd },
                    KeyEvent::Recorded(c) => UserEvent::KeyRecorded(c),
                });
            });
        }
        let shield_state = ShieldState { enabled: cfg.shield, allow: cfg.shield_allow.clone() };
        let daily = shield::Daily::load(&paths.shield);
        let perms = permissions::Store::load(&paths.permissions);
        let ai_key = ai::load_key();

        let active_space = session.active_space.min(space_slots.len() - 1);
        let mut app = Self {
            writer: crate::config::Writer::new(),
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
            shield: Rc::new(RefCell::new(shield_state)),
            daily,
            page_blocks: HashMap::new(),
            perms: Rc::new(RefCell::new(perms)),
            permission_requests: Rc::default(),
            active_downloads: Rc::default(),
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
        app.restore(saved_spaces);
        Ok(app)
    }
}
