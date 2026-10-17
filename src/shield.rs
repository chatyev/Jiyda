//! Защита от трекеров и рекламы.
//!
//! Встроенный список известных сервисов слежки и рекламных сетей. Каждый адрес
//! из списка становится фильтром WebView2 (`AddWebResourceRequestedFilter`), так что
//! браузер видит только подозрительные запросы, а остальные идут без задержки.
//! Совпавший запрос получает ответ 403 и учитывается: по странице (Security panel)
//! и за день (New Page). На сайтах из списка исключений защита выключена.
//! Переход на сам сайт трекера (документ, который пользователь открыл) не блокируется.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Tracker,
    Ad,
}

/// Сервисы слежки: аналитика, запись сессий, пиксели соцсетей.
const TRACKERS: &[&str] = &[
    "google-analytics.com",
    "analytics.google.com",
    "googletagmanager.com",
    "region1.google-analytics.com",
    "mc.yandex.ru",
    "mc.yandex.com",
    "mc.webvisor.org",
    "top-fwz1.mail.ru",
    "counter.yadro.ru",
    "hotjar.com",
    "hotjar.io",
    "clarity.ms",
    "scorecardresearch.com",
    "quantserve.com",
    "quantcount.com",
    "segment.io",
    "segment.com/analytics.js",
    "cdn.segment.com",
    "api-js.mixpanel.com",
    "cdn.mxpnl.com",
    "api.amplitude.com",
    "api2.amplitude.com",
    "cdn.amplitude.com",
    "fullstory.com",
    "heapanalytics.com",
    "mouseflow.com",
    "crazyegg.com",
    "luckyorange.com",
    "luckyorange.net",
    "smartlook.com",
    "statcounter.com",
    "chartbeat.com",
    "chartbeat.net",
    "bam.nr-data.net",
    "js-agent.newrelic.com",
    "connect.facebook.net/*/fbevents.js",
    "facebook.com/tr",
    "bat.bing.com",
    "px.ads.linkedin.com",
    "snap.licdn.com",
    "analytics.tiktok.com",
    "analytics.twitter.com",
    "static.ads-twitter.com",
    "ct.pinterest.com",
    "sc-static.net/scevent.min.js",
    "tr.snapchat.com",
    "logx.optimizely.com",
    "cdn.heapanalytics.com",
    "browser-intake-datadoghq.com",
    "stats.wp.com",
    "pixel.wp.com",
    "omtrdc.net",
    "demdex.net",
    "everesttech.net",
    "krxd.net",
    "bluekai.com",
    "exelator.com",
    "mathtag.com",
    "tapad.com",
    "rlcdn.com",
    "agkn.com",
];

/// Рекламные сети и их счётчики.
const ADS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adservice.google.com",
    "pagead2.googlesyndication.com",
    "amazon-adsystem.com",
    "adnxs.com",
    "criteo.com",
    "criteo.net",
    "taboola.com",
    "outbrain.com",
    "rubiconproject.com",
    "pubmatic.com",
    "openx.net",
    "adsrvr.org",
    "moatads.com",
    "an.yandex.ru",
    "yandexadexchange.net",
    "adfox.ru",
    "adriver.ru",
    "ads.yahoo.com",
    "advertising.com",
    "casalemedia.com",
    "smartadserver.com",
    "sharethrough.com",
    "teads.tv",
    "media.net",
    "zemanta.com",
    "adform.net",
    "bidswitch.net",
    "contextweb.com",
    "3lift.com",
    "yieldmo.com",
    "adcolony.com",
    "applovin.com",
    "popads.net",
    "propellerads.com",
    "mgid.com",
    "revcontent.com",
    "33across.com",
    "indexww.com",
    "lijit.com",
    "gumgum.com",
    "serving-sys.com",
    "flashtalking.com",
];

/// Фильтры WebView2 для записи списка: сайт и все его поддомены, путь — как есть.
fn filters(entry: &str) -> Vec<String> {
    let (host, path) = match entry.split_once('/') {
        Some((h, p)) => (h, format!("/{p}*")),
        None => (entry, "/*".to_string()),
    };
    vec![format!("*://{host}{path}"), format!("*://*.{host}{path}")]
}

/// Все фильтры для WebView2.
pub fn all_filters() -> Vec<String> {
    TRACKERS.iter().chain(ADS).flat_map(|e| filters(e)).collect()
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Простое сопоставление пути с `*` (как в фильтрах WebView2).
fn path_matches(path: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = path;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if let Some(pos) = rest.find(part) {
            rest = &rest[pos + part.len()..];
        } else {
            return false;
        }
    }
    true
}

/// Что это за запрос: трекер, реклама или ничего из списка.
pub fn classify(url: &str) -> Option<Kind> {
    let u = url::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let path = u.path();
    let hit = |list: &[&str]| {
        list.iter().any(|e| match e.split_once('/') {
            Some((h, p)) => host_matches(&host, h) && path_matches(path, &format!("/{p}")),
            None => host_matches(&host, e),
        })
    };
    if hit(TRACKERS) {
        Some(Kind::Tracker)
    } else if hit(ADS) {
        Some(Kind::Ad)
    } else {
        None
    }
}

/// Домен для списка исключений: без «www.», нижний регистр.
pub fn site_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let h = u.host_str()?.to_ascii_lowercase();
    Some(h.strip_prefix("www.").unwrap_or(&h).to_string())
}

/// Счётчик заблокированного за день (New Page: «12 trackers blocked today»).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Daily {
    /// Дата в виде ГГГГ-ММ-ДД (местное время).
    pub day: String,
    pub trackers: u64,
    pub ads: u64,
}

impl Daily {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Синхронная запись — для тестов; браузер пишет через `config::Writer`.
    #[cfg(test)]
    pub fn save(&self, path: &Path) {
        if let Some(s) = self.to_json() {
            crate::config::write_atomic(path, &s);
        }
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    /// Учесть заблокированный запрос; в новый день счёт начинается заново.
    pub fn add(&mut self, today: &str, kind: Kind) {
        if self.day != today {
            *self = Daily { day: today.to_string(), ..Default::default() };
        }
        match kind {
            Kind::Tracker => self.trackers += 1,
            Kind::Ad => self.ads += 1,
        }
    }

    /// Счёт за `today` (вчерашний не показываем).
    pub fn for_day(&self, today: &str) -> (u64, u64) {
        if self.day == today { (self.trackers, self.ads) } else { (0, 0) }
    }
}

/// Сегодняшняя дата по местному времени: ГГГГ-ММ-ДД.
#[cfg(windows)]
pub fn today() -> String {
    // SAFETY: функция только заполняет структуру с местным временем.
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

#[cfg(not(windows))]
pub fn today() -> String {
    let days = crate::store::now() / 86_400;
    format!("day-{days}")
}

#[cfg(windows)]
pub use win::install;

#[cfg(windows)]
mod win {
    use super::{Kind, all_filters, classify};
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

#[cfg(not(windows))]
pub fn install(_wv: &wry::WebView, _should_block: impl Fn(&str) -> bool + 'static, _on_block: impl Fn(Kind) + 'static) {
}

#[cfg(test)]
#[path = "../tests/unit/shield.rs"]
mod tests;
