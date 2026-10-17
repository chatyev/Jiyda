//! Рамка окна Seyro в Windows.
//!
//! Окно без системной рамки, но с тенью, скруглёнными углами и невидимыми краями
//! изменения размера Windows 11 (tao, `with_undecorated_shadow`). Поверх страниц
//! лежит прозрачный WebView интерфейса; его обрезает регион окна — интерфейс
//! получает мышь только там, где он что-то рисует, остальное достаётся странице.
//!
//! Кнопки окна (свернуть, развернуть, закрыть) рисует само окно, а не интерфейс:
//! на `WM_NCHITTEST` оно отвечает HTMINBUTTON / HTMAXBUTTON / HTCLOSE — так Windows 11
//! показывает Snap Layouts при наведении на «Развернуть», как у обычных окон.
//! Пустое место верхней панели тянет окно: интерфейс сообщает о нажатии, ядро запускает
//! перетаскивание Windows (с прилипанием к краям); двойной щелчок разворачивает окно,
//! правый — открывает системное меню (`show_system_menu`). CSS `app-region` не
//! используется — почему, см. `configure_ui`.

/// Высота верхней панели в логических пикселях (`--chrome-h` в ui/styles.css).
pub const CHROME_H: f64 = 46.0;
/// Ширина одной кнопки окна (`--caption-w` в ui/styles.css).
pub const CAPTION_W: f64 = 46.0;

/// Прямоугольник в логических пикселях окна: x, y, ширина, высота.
pub type LRect = (f64, f64, f64, f64);

/// Что из интерфейса должно получать мышь (см. `set_ui_region`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiRegion {
    /// Интерфейс занимает всё окно (открыт модальный слой, New Page, настройки).
    pub full: bool,
    /// Плавающие элементы над страницей: панели, тосты, край Edge Reveal.
    pub rects: Vec<LRect>,
    /// Дыры внутри них (Peek: страница предпросмотра под интерфейсом).
    pub holes: Vec<LRect>,
}

/// Логический прямоугольник → физические пиксели, с округлением наружу.
pub fn to_physical((x, y, w, h): LRect, scale: f64) -> (i32, i32, i32, i32) {
    let x0 = (x * scale).floor() as i32;
    let y0 = (y * scale).floor() as i32;
    let x1 = ((x + w) * scale).ceil() as i32;
    let y1 = ((y + h) * scale).ceil() as i32;
    (x0, y0, x1.max(x0), y1.max(y0))
}

/// Кнопки окна в физических пикселях: (лево, верх, право, низ) каждой — свернуть,
/// развернуть, закрыть — у правого края клиентской области шириной `client_w`.
pub fn caption_buttons(client_w: i32, scale: f64) -> [(i32, i32, i32, i32); 3] {
    let bw = (CAPTION_W * scale).round() as i32;
    let bh = (CHROME_H * scale).round() as i32;
    std::array::from_fn(|i| {
        let right = client_w - bw * (2 - i as i32);
        (right - bw, 0, right, bh)
    })
}

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use super::*;
    use std::cell::RefCell;
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CombineRgn, CreateFontW, CreateRectRgn, CreateSolidBrush,
        DEFAULT_CHARSET, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FillRect,
        GetTextFaceW, HDC, HFONT, HGDIOBJ, HRGN, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, RGN_DIFF, RGN_OR,
        ScreenToClient, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
    };
    use windows::Win32::Graphics::Gdi::{GetSysColor, SYS_COLOR_INDEX};
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent};
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, HWND_TOP, IsZoomed, PostMessageW, SM_CYFRAME,
        SPI_GETHIGHCONTRAST, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetWindowPos, ShowWindow, SystemParametersInfoW, WM_ACTIVATE, WM_CLOSE,
        WM_ERASEBKGND, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
        WM_NCMOUSEMOVE, WM_NCRBUTTONDOWN, WM_NCRBUTTONUP, WM_PAINT, WM_SETTINGCHANGE, WM_SIZE, WM_THEMECHANGED,
    };
    use windows::core::PCWSTR;

    /// Цвета кнопок окна (из интерфейса — чтобы совпадали с верхней панелью).
    #[derive(Clone, Copy)]
    struct Colors {
        bg: COLORREF,
        fg: COLORREF,
        fg_inactive: COLORREF,
        hover: COLORREF,
        pressed: COLORREF,
    }

    struct State {
        colors: Colors,
        /// Кнопки видны (в полноэкранном режиме — нет).
        visible: bool,
        /// Кнопка под мышью и нажатая: 0 — нет, 1 — свернуть, 2 — развернуть, 3 — закрыть.
        hover: u8,
        pressed: u8,
        active: bool,
        /// Шрифт значков: Segoe Fluent Icons (Windows 11) или Segoe MDL2 Assets (Windows 10).
        icon_font: Option<Vec<u16>>,
    }

    /// Состояние кнопок окна. Им владеет `Frame`; процедура подкласса получает
    /// указатель на него через dwRefData.
    type Shared = RefCell<State>;

    impl State {
        fn new() -> Self {
            State {
                colors: colors_from((0x0B, 0x0F, 0x14), (0xF4, 0xF6, 0xF8)),
                visible: true,
                hover: 0,
                pressed: 0,
                active: true,
                icon_font: None,
            }
        }
    }

    fn rgb((r, g, b): (u8, u8, u8)) -> COLORREF {
        COLORREF(u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16))
    }

    fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
        let m = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
        (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
    }

    fn colors_from(bg: (u8, u8, u8), fg: (u8, u8, u8)) -> Colors {
        Colors {
            bg: rgb(bg),
            fg: rgb(fg),
            fg_inactive: rgb(mix(bg, fg, 0.45)),
            hover: rgb(mix(bg, fg, 0.07)),
            pressed: rgb(mix(bg, fg, 0.12)),
        }
    }

    const CLOSE_HOVER: COLORREF = COLORREF(0x1C2BC4); // #C42B1C — как у Windows 11
    const CLOSE_PRESSED: COLORREF = COLORREF(0x2C3BC8); // #C83B2C
    const WHITE: COLORREF = COLORREF(0xFFFFFF);

    fn hwnd_of(window: &tao::window::Window) -> HWND {
        use tao::platform::windows::WindowExtWindows;
        HWND(window.hwnd() as _)
    }

    fn scale(hwnd: HWND) -> f64 {
        // SAFETY: чтение DPI живого окна.
        f64::from(unsafe { GetDpiForWindow(hwnd) }.max(96)) / 96.0
    }

    fn client(hwnd: HWND) -> RECT {
        let mut rc = RECT::default();
        // SAFETY: rc — свой буфер.
        let _ = unsafe { GetClientRect(hwnd, &mut rc) };
        rc
    }

    fn buttons(hwnd: HWND) -> [RECT; 3] {
        caption_buttons(client(hwnd).right, scale(hwnd)).map(|(l, t, r, b)| RECT {
            left: l,
            top: t,
            right: r,
            bottom: b,
        })
    }

    fn high_contrast() -> bool {
        let mut hc = HIGHCONTRASTW { cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
        // SAFETY: структура нужного размера, Windows только заполняет её.
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                hc.cbSize,
                Some((&mut hc as *mut HIGHCONTRASTW).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        ok.is_ok() && hc.dwFlags.contains(HCF_HIGHCONTRASTON)
    }

    fn sys(i: i32) -> COLORREF {
        // SAFETY: чтение системного цвета.
        COLORREF(unsafe { GetSysColor(SYS_COLOR_INDEX(i)) })
    }

    /// Какая кнопка под точкой клиентской области (1–3), 0 — никакая.
    fn button_at(st: &Shared, hwnd: HWND, p: POINT) -> u8 {
        if !st.borrow().visible {
            return 0;
        }
        for (i, r) in buttons(hwnd).iter().enumerate() {
            if p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom {
                return i as u8 + 1;
            }
        }
        0
    }

    fn ht_button(ht: u32) -> u8 {
        match ht {
            HTMINBUTTON => 1,
            HTMAXBUTTON => 2,
            HTCLOSE => 3,
            _ => 0,
        }
    }

    fn redraw_buttons(hwnd: HWND) {
        for r in buttons(hwnd) {
            // SAFETY: перерисовка своего окна.
            let _ = unsafe { InvalidateRect(Some(hwnd), Some(&r), false) };
        }
    }

    unsafe fn select_icon_font(st: &Shared, dc: HDC, px: i32) -> (HFONT, HGDIOBJ) {
        let face = st.borrow().icon_font.clone();
        let make = |name: &[u16]| unsafe {
            CreateFontW(
                -px,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                PCWSTR(name.as_ptr()),
            )
        };
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        if let Some(face) = face {
            let f = make(&face);
            return (f, unsafe { SelectObject(dc, f.into()) });
        }
        // Первый раз: есть ли шрифт Windows 11; если GDI подставил другой — берём MDL2.
        let fluent = wide("Segoe Fluent Icons");
        let f = make(&fluent);
        let old = unsafe { SelectObject(dc, f.into()) };
        let mut buf = [0u16; 64];
        let n = unsafe { GetTextFaceW(dc, Some(&mut buf)) } as usize;
        let got = String::from_utf16_lossy(&buf[..n.saturating_sub(1).min(64)]);
        if got.eq_ignore_ascii_case("Segoe Fluent Icons") {
            st.borrow_mut().icon_font = Some(fluent);
            return (f, old);
        }
        unsafe {
            SelectObject(dc, old);
            let _ = DeleteObject(f.into());
        }
        let mdl2 = wide("Segoe MDL2 Assets");
        st.borrow_mut().icon_font = Some(mdl2.clone());
        let f = make(&mdl2);
        (f, unsafe { SelectObject(dc, f.into()) })
    }

    /// Нарисовать фон под верхней панелью и кнопки окна. Остальное окно закрыто WebView.
    unsafe fn paint(st: &Shared, hwnd: HWND) {
        let mut ps = PAINTSTRUCT::default();
        let dc = unsafe { BeginPaint(hwnd, &mut ps) };
        let (mut c, visible, hover, pressed, active) = {
            let s = st.borrow();
            (s.colors, s.visible, s.hover, s.pressed, s.active)
        };
        let hc = high_contrast();
        if hc {
            c = Colors { bg: sys(5), fg: sys(8), fg_inactive: sys(17), hover: sys(13), pressed: sys(13) };
        }
        let fill = |r: &RECT, color: COLORREF| unsafe {
            let br = CreateSolidBrush(color);
            FillRect(dc, r, br);
            let _ = DeleteObject(br.into());
        };
        fill(&client(hwnd), c.bg);
        if visible {
            let px = (10.0 * scale(hwnd)).round() as i32;
            let (font, old) = unsafe { select_icon_font(st, dc, px) };
            unsafe { SetBkMode(dc, TRANSPARENT) };
            let maximized = unsafe { IsZoomed(hwnd) }.as_bool();
            for (i, r) in buttons(hwnd).iter().enumerate() {
                let n = i as u8 + 1;
                let (bg, fg) = match (n, hover == n, pressed == n) {
                    (3, _, true) if !hc => (CLOSE_PRESSED, WHITE),
                    (3, true, _) if !hc => (CLOSE_HOVER, WHITE),
                    (_, _, true) => (c.pressed, if hc { sys(14) } else { c.fg }),
                    (_, true, _) => (c.hover, if hc { sys(14) } else { c.fg }),
                    _ => (c.bg, if active { c.fg } else { c.fg_inactive }),
                };
                fill(r, bg);
                unsafe { SetTextColor(dc, fg) };
                // Значки Segoe Fluent Icons / MDL2: свернуть, развернуть / восстановить, закрыть.
                let glyph = [0xE921u16, if maximized { 0xE923 } else { 0xE922 }, 0xE8BB][i];
                let mut text = [glyph];
                let mut rc = *r;
                unsafe { DrawTextW(dc, &mut text, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE) };
            }
            unsafe {
                SelectObject(dc, old);
                let _ = DeleteObject(font.into());
            }
        }
        let _ = unsafe { EndPaint(hwnd, &ps) };
    }

    unsafe extern "system" fn subclass(
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        // SAFETY: data — указатель на состояние в `Frame`, который живёт, пока стоит подкласс
        // (Drop снимает подкласс раньше, чем освобождает состояние).
        let st = unsafe { &*(data as *const Shared) };
        let point = || {
            let mut p = POINT { x: (lp.0 & 0xFFFF) as i16 as i32, y: ((lp.0 >> 16) & 0xFFFF) as i16 as i32 };
            // SAFETY: перевод координат своего окна.
            let _ = unsafe { ScreenToClient(hwnd, &mut p) };
            p
        };
        match msg {
            WM_NCHITTEST => match button_at(st, hwnd, point()) {
                1 => return LRESULT(HTMINBUTTON as isize),
                2 => return LRESULT(HTMAXBUTTON as isize),
                3 => return LRESULT(HTCLOSE as isize),
                _ => {}
            },
            WM_NCMOUSEMOVE => {
                let b = ht_button(wp.0 as u32);
                let changed = {
                    let mut s = st.borrow_mut();
                    let changed = s.hover != b;
                    s.hover = b;
                    if b == 0 {
                        s.pressed = 0;
                    }
                    changed
                };
                if changed {
                    redraw_buttons(hwnd);
                }
                if b != 0 {
                    let mut t = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    // SAFETY: структура нужного размера для своего окна.
                    let _ = unsafe { TrackMouseEvent(&mut t) };
                    // Не отдаём DefWindowProc: он нарисовал бы старые кнопки Windows.
                    return LRESULT(0);
                }
            }
            WM_NCMOUSELEAVE => {
                {
                    let mut s = st.borrow_mut();
                    s.hover = 0;
                    s.pressed = 0;
                }
                redraw_buttons(hwnd);
            }
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
                let b = ht_button(wp.0 as u32);
                if b != 0 {
                    st.borrow_mut().pressed = b;
                    redraw_buttons(hwnd);
                    return LRESULT(0);
                }
            }
            WM_NCLBUTTONUP => {
                let b = ht_button(wp.0 as u32);
                let pressed = std::mem::take(&mut st.borrow_mut().pressed);
                if b != 0 {
                    redraw_buttons(hwnd);
                    if b == pressed {
                        // SAFETY: управление своим окном.
                        unsafe {
                            match b {
                                1 => {
                                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                                }
                                2 => {
                                    let _ = ShowWindow(
                                        hwnd,
                                        if IsZoomed(hwnd).as_bool() { SW_RESTORE } else { SW_MAXIMIZE },
                                    );
                                }
                                _ => {
                                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                                }
                            }
                        }
                    }
                    return LRESULT(0);
                }
            }
            WM_NCRBUTTONDOWN | WM_NCRBUTTONUP if ht_button(wp.0 as u32) != 0 => return LRESULT(0),
            WM_ACTIVATE => {
                st.borrow_mut().active = (wp.0 & 0xFFFF) != 0;
                redraw_buttons(hwnd);
            }
            WM_PAINT => {
                // SAFETY: рисование своего окна между BeginPaint/EndPaint.
                unsafe { paint(st, hwnd) };
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_SIZE | WM_SETTINGCHANGE | WM_THEMECHANGED => {
                // SAFETY: перерисовка своего окна.
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
            _ => {}
        }
        // SAFETY: остальное — как у окна без подкласса.
        unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
    }

    /// Кнопки окна, которые рисует само окно (подкласс окна и его состояние).
    /// Пока объект жив, подкласс стоит; Drop его снимает.
    pub struct Frame {
        hwnd: HWND,
        state: Box<Shared>,
    }

    impl Frame {
        /// Взять кнопки окна на себя. None — Windows не дала поставить подкласс.
        pub fn install(window: &tao::window::Window) -> Option<Frame> {
            let hwnd = hwnd_of(window);
            let state = Box::new(RefCell::new(State::new()));
            let data = &*state as *const Shared as usize;
            // SAFETY: подкласс своего окна; состояние живёт в Box, пока жив Frame.
            if !unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, data) }.as_bool() {
                log::error!("не удалось подключить кнопки окна");
                return None;
            }
            Some(Frame { hwnd, state })
        }

        /// Цвета кнопок окна под верхнюю панель: фон и цвет значков.
        pub fn set_colors(&self, bg: (u8, u8, u8), fg: (u8, u8, u8)) {
            self.state.borrow_mut().colors = colors_from(bg, fg);
            self.redraw();
        }

        /// Показать или спрятать кнопки окна (полноэкранный режим).
        pub fn set_buttons_visible(&self, visible: bool) {
            self.state.borrow_mut().visible = visible;
            self.redraw();
        }

        fn redraw(&self) {
            // SAFETY: перерисовка своего окна.
            let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
        }
    }

    impl Drop for Frame {
        fn drop(&mut self) {
            // SAFETY: снимаем свой подкласс до того, как освободится состояние.
            let _ = unsafe { RemoveWindowSubclass(self.hwnd, Some(subclass), SUBCLASS_ID) };
        }
    }

    const SUBCLASS_ID: usize = 1;

    /// Фокус ушёл из-за действия пользователя: нажата кнопка мыши (щелчок по странице)
    /// или окно больше не на переднем плане (Alt+Tab, другая программа). Когда фокус
    /// переводит само ядро, ни того, ни другого нет.
    pub fn user_moved_focus(window: &tao::window::Window) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON};
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        // SAFETY: чтение состояния кнопок и переднего окна.
        unsafe {
            let pressed = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|k| GetAsyncKeyState(i32::from(k.0)) < 0);
            pressed || GetForegroundWindow() != hwnd_of(window)
        }
    }

    /// Системное меню окна у курсора (правый щелчок по пустому месту верхней панели),
    /// как у заголовка обычного окна: пункты включены по состоянию окна.
    pub fn show_system_menu(window: &tao::window::Window) {
        use windows::Win32::UI::WindowsAndMessaging::{
            EnableMenuItem, GetCursorPos, GetSystemMenu, MENU_ITEM_FLAGS, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED,
            SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE, SetMenuDefaultItem, TPM_RETURNCMD,
            TPM_RIGHTBUTTON, TrackPopupMenu, WM_SYSCOMMAND,
        };
        let hwnd = hwnd_of(window);
        // SAFETY: системное меню своего окна; меню принадлежит окну, освобождать его не нужно.
        unsafe {
            let menu = GetSystemMenu(hwnd, false);
            if menu.is_invalid() {
                return;
            }
            let maximized = IsZoomed(hwnd).as_bool();
            let state = |on: bool| MF_BYCOMMAND | if on { MF_ENABLED } else { MF_GRAYED };
            let set = |cmd: u32, on: bool| {
                let _ = EnableMenuItem(menu, cmd, state(on) as MENU_ITEM_FLAGS);
            };
            set(SC_RESTORE, maximized);
            set(SC_MOVE, !maximized);
            set(SC_SIZE, !maximized);
            set(SC_MINIMIZE, true);
            set(SC_MAXIMIZE, !maximized);
            set(SC_CLOSE, true);
            let _ = SetMenuDefaultItem(menu, SC_CLOSE, 0);
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, Some(0), hwnd, None);
            if cmd.0 != 0 {
                let _ = PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(cmd.0 as usize), LPARAM(0));
            }
        }
    }

    /// Поднять интерфейс над страницами (новые WebView создаются поверх).
    pub fn raise(ui: &wry::WebView) {
        use wry::WebViewExtWindows;
        // SAFETY: изменение порядка дочернего окна без перемещения и активации.
        let _ =
            unsafe { SetWindowPos(ui.hwnd(), Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }

    /// Обрезать WebView интерфейса регионом: верхняя панель (если она есть), плавающие
    /// элементы и всё окно при `full`, за вычетом дыр и кнопок окна. У не развёрнутого
    /// окна верхняя полоска остаётся окну — за неё Windows меняет высоту.
    /// `own_caption` — кнопки окна рисует Seyro (окно без системной рамки).
    pub fn set_ui_region(
        window: &tao::window::Window,
        ui: &wry::WebView,
        region: &UiRegion,
        chrome: bool,
        fullscreen: bool,
        own_caption: bool,
    ) {
        use wry::WebViewExtWindows;
        let hwnd = hwnd_of(window);
        let s = scale(hwnd);
        let cl = client(hwnd);
        let (w, h) = (cl.right, cl.bottom);
        // SAFETY: регионы создаются здесь; SetWindowRgn забирает итоговый, остальные удаляются.
        unsafe {
            let rgn = CreateRectRgn(0, 0, 0, 0);
            let add = |r: (i32, i32, i32, i32), op| {
                let part = CreateRectRgn(r.0, r.1, r.2, r.3);
                CombineRgn(Some(rgn), Some(rgn), Some(part), op);
                let _ = DeleteObject(part.into());
            };
            if region.full {
                add((0, 0, w, h), RGN_OR);
            } else {
                if chrome && !fullscreen {
                    add((0, 0, w, (CHROME_H * s).ceil() as i32), RGN_OR);
                }
                for r in &region.rects {
                    add(to_physical(*r, s), RGN_OR);
                }
            }
            for r in &region.holes {
                add(to_physical(*r, s), RGN_DIFF);
            }
            if !fullscreen && own_caption {
                for b in caption_buttons(w, s) {
                    add(b, RGN_DIFF);
                }
                if !IsZoomed(hwnd).as_bool() {
                    let border = GetSystemMetricsForDpi(SM_CYFRAME, GetDpiForWindow(hwnd)) + 1;
                    add((0, 0, w, border), RGN_DIFF);
                }
            }
            if SetWindowRgn(ui.hwnd(), Some(rgn), true) == 0 {
                let _ = DeleteObject(HRGN(rgn.0).into());
            }
        }
    }

    /// Настройки WebView интерфейса: без своих клавиш WebView (Ctrl+P, F5, Ctrl+F…) и
    /// масштаба — у интерфейса они не нужны, а клавиши Seyro по-прежнему приходят в
    /// AcceleratorKeyPressed.
    ///
    /// CSS `app-region: drag` (SetIsNonClientRegionSupportEnabled) не включаем: пересчитывая
    /// области перетаскивания, WebView2 синхронно двигает своё окно (UpdateDraggableRegions →
    /// SetWindowPos), и это может навсегда остановить главный поток — так зависал Seyro на CI.
    /// Окно тянет ядро по сообщению интерфейса (UiMsg::WindowDrag, titlebar.js).
    pub fn configure_ui(ui: &wry::WebView) {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows::core::Interface;
        use wry::WebViewExtWindows;
        // SAFETY: настройки живого WebView.
        unsafe {
            if let Ok(s) = ui.webview().Settings() {
                let _ = s.SetIsZoomControlEnabled(false);
                if let Ok(s3) = s.cast::<ICoreWebView2Settings3>() {
                    let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub struct Frame;
#[cfg(not(windows))]
impl Frame {
    pub fn install(_window: &tao::window::Window) -> Option<Frame> {
        None
    }
    pub fn set_colors(&self, _bg: (u8, u8, u8), _fg: (u8, u8, u8)) {}
    pub fn set_buttons_visible(&self, _visible: bool) {}
}
#[cfg(not(windows))]
pub fn user_moved_focus(_window: &tao::window::Window) -> bool {
    true
}
#[cfg(not(windows))]
pub fn raise(_ui: &wry::WebView) {}
#[cfg(not(windows))]
pub fn set_ui_region(
    _w: &tao::window::Window,
    _ui: &wry::WebView,
    _r: &UiRegion,
    _chrome: bool,
    _fullscreen: bool,
    _own: bool,
) {
}
#[cfg(not(windows))]
pub fn configure_ui(_ui: &wry::WebView) {}
#[cfg(not(windows))]
pub fn show_system_menu(_window: &tao::window::Window) {}

/// «#0b0f14» → (11, 15, 20).
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_rect_rounds_outward() {
        assert_eq!(to_physical((10.0, 10.0, 100.0, 20.0), 1.0), (10, 10, 110, 30));
        assert_eq!(to_physical((10.3, 0.5, 10.0, 10.0), 1.25), (12, 0, 26, 14));
        assert_eq!(to_physical((5.0, 5.0, -3.0, 0.0), 1.5), (7, 7, 7, 8));
    }

    #[test]
    fn caption_buttons_sit_at_right_edge_without_gaps() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let b = caption_buttons(1000, scale);
            assert_eq!(b[2].2, 1000, "{scale}");
            assert_eq!(b[0].2, b[1].0);
            assert_eq!(b[1].2, b[2].0);
            assert!(b.iter().all(|r| r.1 == 0 && r.3 == (CHROME_H * scale).round() as i32));
        }
    }

    #[test]
    fn hex_colors() {
        assert_eq!(parse_hex("#0b0F14"), Some((11, 15, 20)));
        for bad in ["", "0b0f14", "#0b0f1", "#0b0f1g", "#0b0f140"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }
}
