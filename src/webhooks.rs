//! События WebView2, которых нет в wry: снимок страницы для Orbit, ход загрузки,
//! полноэкранный элемент страницы (видео). Всё вызывается в главном потоке.

/// Ход загрузки: адрес, сколько получено и сколько всего (0 — неизвестно).
#[derive(Debug, Clone)]
pub struct Progress {
    pub url: String,
    pub received: u64,
    pub total: u64,
}

#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use super::Progress;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_JPEG, ICoreWebView2_4, ICoreWebView2DownloadOperation,
    };
    use webview2_com::{
        BytesReceivedChangedEventHandler, CapturePreviewCompletedHandler, ContainsFullScreenElementChangedEventHandler,
        DownloadStartingEventHandler, take_pwstr,
    };
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    use windows::Win32::System::Com::{STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};
    use windows::core::{Interface, PWSTR};
    use wry::WebViewExtWindows;

    /// Снимок больше этого не нужен Orbit (и не влезет в разумное сообщение).
    const CAPTURE_MAX: u64 = 8 * 1024 * 1024;
    /// Значок сайта больше этого — не значок (и раздует сессию).
    const FAVICON_MAX: usize = 64 * 1024;
    /// Как часто сообщать о ходе одной загрузки.
    const PROGRESS_EVERY: Duration = Duration::from_millis(250);

    /// Идущие загрузки по адресу — чтобы их можно было отменить. Принадлежат ядру,
    /// обработчики WebView держат на них `Rc`.
    #[derive(Default)]
    pub struct ActiveDownloads(RefCell<HashMap<String, ICoreWebView2DownloadOperation>>);

    impl ActiveDownloads {
        /// Отменить загрузку с этого адреса. false — такой нет.
        pub fn cancel(&self, url: &str) -> bool {
            let op = self.0.borrow_mut().remove(url);
            // SAFETY: отмена живой операции загрузки.
            op.is_some_and(|op| unsafe { op.Cancel() }.is_ok())
        }

        /// Загрузка закончилась — операция больше не нужна.
        pub fn forget(&self, url: &str) {
            self.0.borrow_mut().remove(url);
        }
    }

    /// Снять видимую часть страницы в JPEG. `done` получает байты или None.
    pub fn capture(wv: &wry::WebView, done: impl FnOnce(Option<Vec<u8>>) + 'static) {
        // SAFETY: COM-вызовы с живыми объектами; поток читается после завершения снимка.
        let res = unsafe {
            CreateStreamOnHGlobal(HGLOBAL::default(), true).and_then(|stream| {
                let s = stream.clone();
                let handler = CapturePreviewCompletedHandler::create(Box::new(move |hr| {
                    done(hr.ok().and_then(|()| read_stream(&s)));
                    Ok(())
                }));
                wv.webview().CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_JPEG, &stream, &handler)
            })
        };
        if let Err(e) = res {
            log::debug!("снимок страницы не удался: {e}");
        }
    }

    unsafe fn read_stream(s: &windows::Win32::System::Com::IStream) -> Option<Vec<u8>> {
        let mut stat = STATSTG::default();
        unsafe { s.Stat(&mut stat, STATFLAG_NONAME) }.ok()?;
        if stat.cbSize == 0 || stat.cbSize > CAPTURE_MAX {
            return None;
        }
        unsafe { s.Seek(0, STREAM_SEEK_SET, None) }.ok()?;
        let mut buf = vec![0u8; stat.cbSize as usize];
        let mut read = 0u32;
        unsafe { s.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut read)) }.ok().ok()?;
        buf.truncate(read as usize);
        Some(buf)
    }

    /// Следить за ходом загрузок, начатых в этом WebView.
    pub fn watch_downloads(
        wv: &wry::WebView,
        active: Rc<ActiveDownloads>,
        on_progress: impl Fn(Progress) + Clone + 'static,
    ) {
        let Ok(wv4) = wv.webview().cast::<ICoreWebView2_4>() else { return };
        let handler = DownloadStartingEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            // SAFETY: объекты, которые WebView2 передал в обработчик.
            unsafe {
                let op = args.DownloadOperation()?;
                let mut uri = PWSTR::null();
                op.Uri(&mut uri)?;
                let url = take_pwstr(uri);
                active.0.borrow_mut().insert(url.clone(), op.clone());
                let report = on_progress.clone();
                let mut last = Instant::now() - PROGRESS_EVERY;
                let mut token = 0i64;
                op.add_BytesReceivedChanged(
                    &BytesReceivedChangedEventHandler::create(Box::new(move |op, _| {
                        let Some(op) = op else { return Ok(()) };
                        if last.elapsed() < PROGRESS_EVERY {
                            return Ok(());
                        }
                        last = Instant::now();
                        let (mut received, mut total) = (0i64, 0i64);
                        op.BytesReceived(&mut received)?;
                        let _ = op.TotalBytesToReceive(&mut total);
                        report(Progress {
                            url: url.clone(),
                            received: received.max(0) as u64,
                            total: total.max(0) as u64,
                        });
                        Ok(())
                    })),
                    &mut token,
                )?;
            }
            Ok(())
        }));
        let mut token = 0i64;
        // SAFETY: подписка на событие живого WebView.
        if let Err(e) = unsafe { wv4.add_DownloadStarting(&handler, &mut token) } {
            log::warn!("не удалось следить за загрузками: {e}");
        }
    }

    /// Значок сайта меняется — получить его у самого WebView в PNG (из кэша страницы,
    /// без нового запроса в сеть). `on_icon` получает байты PNG.
    pub fn watch_favicon(wv: &wry::WebView, on_icon: impl Fn(Vec<u8>) + Clone + 'static) {
        use webview2_com::Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, ICoreWebView2_15};
        use webview2_com::{FaviconChangedEventHandler, GetFaviconCompletedHandler};
        let Ok(wv15) = wv.webview().cast::<ICoreWebView2_15>() else { return };
        let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(core) = sender.and_then(|s| s.cast::<ICoreWebView2_15>().ok()) else { return Ok(()) };
            let report = on_icon.clone();
            let done = GetFaviconCompletedHandler::create(Box::new(move |hr, stream| {
                // SAFETY: поток, который WebView2 передал в обработчик.
                if let (Some(()), Some(s)) = (hr.ok(), stream)
                    && let Some(png) = unsafe { read_stream(&s) }.filter(|b| b.len() <= FAVICON_MAX)
                {
                    report(png);
                }
                Ok(())
            }));
            // SAFETY: запрос значка у живого WebView.
            unsafe { core.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &done) }
        }));
        let mut token = 0i64;
        // SAFETY: подписка на событие живого WebView.
        if let Err(e) = unsafe { wv15.add_FaviconChanged(&handler, &mut token) } {
            log::warn!("не удалось следить за значком сайта: {e}");
        }
    }

    /// Перезагрузить страницу без кэша (Ctrl+Shift+R).
    pub fn hard_reload(wv: &wry::WebView) {
        use crate::logging::OrLog;
        use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
        use windows::core::w;
        let done = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(())));
        // SAFETY: вызов DevTools Protocol на живом WebView.
        if unsafe { wv.webview().CallDevToolsProtocolMethod(w!("Page.reload"), w!("{\"ignoreCache\":true}"), &done) }
            .is_err()
        {
            wv.reload().or_log("reload");
        }
    }

    /// Папка загрузок профиля WebView2. Пустой путь — не трогать (как в Windows).
    pub fn set_download_folder(wv: &wry::WebView, dir: &str) {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_13;
        if dir.is_empty() {
            return;
        }
        // SAFETY: настройка профиля живого WebView.
        let res = unsafe {
            wv.webview()
                .cast::<ICoreWebView2_13>()
                .and_then(|w| w.Profile())
                .and_then(|p| p.SetDefaultDownloadFolderPath(&windows::core::HSTRING::from(dir)))
        };
        if let Err(e) = res {
            log::warn!("папка загрузок не установлена: {e}");
        }
    }

    /// Страница развернула элемент на весь экран (видео) или вышла из этого режима.
    pub fn watch_fullscreen(wv: &wry::WebView, on_change: impl Fn(bool) + 'static) {
        let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(move |wv, _| {
            if let Some(wv) = wv {
                let mut on = windows::core::BOOL::default();
                // SAFETY: чтение свойства WebView, вызвавшего событие.
                unsafe { wv.ContainsFullScreenElement(&mut on)? };
                on_change(on.as_bool());
            }
            Ok(())
        }));
        let mut token = 0i64;
        // SAFETY: подписка на событие живого WebView.
        if let Err(e) = unsafe { wv.webview().add_ContainsFullScreenElementChanged(&handler, &mut token) } {
            log::warn!("не удалось следить за полноэкранным режимом страницы: {e}");
        }
    }
}

#[cfg(not(windows))]
pub fn capture(_wv: &wry::WebView, done: impl FnOnce(Option<Vec<u8>>) + 'static) {
    done(None)
}
#[cfg(not(windows))]
#[derive(Default)]
pub struct ActiveDownloads;
#[cfg(not(windows))]
impl ActiveDownloads {
    pub fn cancel(&self, _url: &str) -> bool {
        false
    }
    pub fn forget(&self, _url: &str) {}
}
#[cfg(not(windows))]
pub fn watch_downloads(
    _wv: &wry::WebView,
    _active: std::rc::Rc<ActiveDownloads>,
    _on_progress: impl Fn(Progress) + Clone + 'static,
) {
}
#[cfg(not(windows))]
pub fn watch_fullscreen(_wv: &wry::WebView, _on_change: impl Fn(bool) + 'static) {}
#[cfg(not(windows))]
pub fn hard_reload(wv: &wry::WebView) {
    use crate::logging::OrLog;
    wv.reload().or_log("reload");
}
#[cfg(not(windows))]
pub fn set_download_folder(_wv: &wry::WebView, _dir: &str) {}
#[cfg(not(windows))]
pub fn watch_favicon(_wv: &wry::WebView, _on_icon: impl Fn(Vec<u8>) + Clone + 'static) {}
