//! Защита от трекеров в WebView2: фильтры WebResourceRequested на адреса из списков
//! shield.rs; заблокированный запрос получает ответ 403. Что блокировать — решает
//! shield.rs (без движка) и настройка защиты для сайта (Policy::shield).

#[cfg(windows)]
mod win {
    use crate::shield::{Kind, all_filters, classify};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
    };
    use webview2_com::{WebResourceRequestedEventHandler, take_pwstr};
    use windows::core::{HSTRING, PWSTR};
    use wry::WebViewExtWindows;

    /// Подключить защиту к WebView. `should_block(адрес страницы)` — включена ли она
    /// для сайта, на котором сейчас вкладка; `on_block` — учесть заблокированное.
    pub fn install(
        wv: &wry::WebView,
        should_block: impl Fn(&str) -> bool + 'static,
        on_block: impl Fn(Kind) + 'static,
    ) {
        let core = wv.webview();
        let env = wv.environment();
        // SAFETY: вызовы COM на живом WebView в главном потоке.
        unsafe {
            for f in all_filters() {
                if let Err(e) = core
                    .AddWebResourceRequestedFilter(&HSTRING::from(f.as_str()), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)
                {
                    log::warn!("фильтр {f} не добавлен: {e}");
                }
            }
            let handler = WebResourceRequestedEventHandler::create(Box::new(move |sender, args| {
                let (Some(sender), Some(args)) = (sender, args) else { return Ok(()) };
                let mut ctx = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                args.ResourceContext(&mut ctx)?;
                // Сайт, который пользователь открыл сам, не трогаем.
                if ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT {
                    return Ok(());
                }
                let mut uri = PWSTR::null();
                args.Request()?.Uri(&mut uri)?;
                let Some(kind) = classify(&take_pwstr(uri)) else { return Ok(()) };
                let mut page = PWSTR::null();
                sender.Source(&mut page)?;
                if !should_block(&take_pwstr(page)) {
                    return Ok(());
                }
                let response =
                    env.CreateWebResourceResponse(None, 403, &HSTRING::from("Blocked by Jiyda"), &HSTRING::new())?;
                args.SetResponse(&response)?;
                on_block(kind);
                Ok(())
            }));
            let mut token = 0i64;
            if let Err(e) = core.add_WebResourceRequested(&handler, &mut token) {
                log::warn!("защита от трекеров не подключена: {e}");
            }
        }
    }
}

#[cfg(windows)]
pub use win::install;

#[cfg(not(windows))]
pub fn install(
    _wv: &wry::WebView,
    _should_block: impl Fn(&str) -> bool + 'static,
    _on_block: impl Fn(crate::shield::Kind) + 'static,
) {
}
