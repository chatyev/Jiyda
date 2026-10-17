//! Orbit: смысловые группы открытых страниц (Development, Research, Shopping…).
//!
//! Когда страниц много, Orbit показывает сначала группы, а не отдельные страницы.
//! Группа определяется по сайту и пути без сети и без ИИ — правилами ниже. Сайты,
//! которые ни под одно правило не попали, но открыты три раза и больше, образуют
//! группу по имени сайта; остальное — «Other».

/// Группа страниц. `key` — ключ категории (интерфейс переводит его в название),
/// либо "site" — тогда название группы — `label` (домен).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub key: &'static str,
    pub label: String,
    pub ids: Vec<u64>,
}

/// Правила: категория и сайты. Сайт — суффикс домена («github.com» подходит и для
/// gist.github.com), либо «имя.*» — сайт с таким именем в любой доменной зоне.
const RULES: &[(&str, &[&str])] = &[
    (
        "ai",
        &[
            "chatgpt.com",
            "chat.openai.com",
            "claude.ai",
            "gemini.google.com",
            "perplexity.ai",
            "copilot.microsoft.com",
            "chat.mistral.ai",
            "deepseek.com",
            "alice.yandex.ru",
            "giga.chat",
            "poe.com",
        ],
    ),
    (
        "communication",
        &[
            "mail.google.com",
            "outlook.live.com",
            "outlook.office.com",
            "outlook.office365.com",
            "slack.com",
            "discord.com",
            "web.telegram.org",
            "teams.microsoft.com",
            "teams.live.com",
            "meet.google.com",
            "zoom.us",
            "web.whatsapp.com",
            "messenger.com",
            "mail.yandex.ru",
            "e.mail.ru",
            "proton.me",
            "mail.proton.me",
            "fastmail.com",
            "zoom.com",
        ],
    ),
    (
        "design",
        &[
            "figma.com",
            "dribbble.com",
            "behance.net",
            "canva.com",
            "pinterest.*",
            "fonts.google.com",
            "coolors.co",
            "unsplash.com",
            "framer.com",
            "sketch.com",
            "adobe.com",
            "spline.design",
            "mobbin.com",
            "fontshare.com",
            "lucide.dev",
        ],
    ),
    (
        "documentation",
        &[
            "docs.rs",
            "readthedocs.io",
            "learn.microsoft.com",
            "developer.mozilla.org",
            "devdocs.io",
            "doc.rust-lang.org",
            "developer.apple.com",
            "developer.android.com",
            "docs.python.org",
            "cppreference.com",
            "w3.org",
            "whatwg.org",
        ],
    ),
    (
        "development",
        &[
            "github.com",
            "gitlab.com",
            "bitbucket.org",
            "stackoverflow.com",
            "stackexchange.com",
            "superuser.com",
            "serverfault.com",
            "crates.io",
            "npmjs.com",
            "pypi.org",
            "rust-lang.org",
            "codepen.io",
            "codesandbox.io",
            "replit.com",
            "huggingface.co",
            "kaggle.com",
            "jsfiddle.net",
            "dev.to",
            "habr.com",
            "vercel.com",
            "vercel.app",
            "netlify.app",
            "netlify.com",
            "githubusercontent.com",
            "sourcegraph.com",
            "godbolt.org",
            "play.rust-lang.org",
            "regex101.com",
            "nuget.org",
            "localhost",
            "127.0.0.1",
        ],
    ),
    (
        "productivity",
        &[
            "notion.so",
            "notion.site",
            "docs.google.com",
            "drive.google.com",
            "sheets.google.com",
            "slides.google.com",
            "calendar.google.com",
            "keep.google.com",
            "trello.com",
            "asana.com",
            "atlassian.net",
            "linear.app",
            "miro.com",
            "airtable.com",
            "clickup.com",
            "office.com",
            "onedrive.live.com",
            "sharepoint.com",
            "dropbox.com",
            "todoist.com",
            "obsidian.md",
            "evernote.com",
            "monday.com",
            "basecamp.com",
            "loom.com",
            "disk.yandex.ru",
            "tracker.yandex.ru",
        ],
    ),
    (
        "research",
        &[
            "wikipedia.org",
            "wikimedia.org",
            "arxiv.org",
            "scholar.google.com",
            "researchgate.net",
            "jstor.org",
            "medium.com",
            "substack.com",
            "nature.com",
            "sciencedirect.com",
            "britannica.com",
            "wolframalpha.com",
            "khanacademy.org",
            "coursera.org",
            "edx.org",
            "pubmed.ncbi.nlm.nih.gov",
            "ncbi.nlm.nih.gov",
            "semanticscholar.org",
            "springer.com",
            "acm.org",
            "ieee.org",
            "openreview.net",
            "paperswithcode.com",
            "cyberleninka.ru",
            "elibrary.ru",
        ],
    ),
    (
        "shopping",
        &[
            "amazon.*",
            "ebay.*",
            "aliexpress.*",
            "ozon.ru",
            "wildberries.ru",
            "market.yandex.ru",
            "etsy.com",
            "avito.ru",
            "walmart.com",
            "bestbuy.com",
            "ikea.com",
            "uzum.uz",
            "temu.com",
            "shein.com",
            "target.com",
            "zalando.*",
            "lamoda.ru",
            "dns-shop.ru",
            "mvideo.ru",
            "citilink.ru",
            "olx.*",
            "allegro.pl",
            "rakuten.*",
            "newegg.com",
        ],
    ),
    (
        "travel",
        &[
            "booking.com",
            "airbnb.*",
            "tripadvisor.*",
            "skyscanner.*",
            "expedia.*",
            "aviasales.*",
            "kayak.*",
            "hotels.com",
            "agoda.com",
            "ostrovok.ru",
            "tutu.ru",
            "rzd.ru",
            "trip.com",
            "maps.google.com",
            "google.com/maps",
            "yandex.ru/maps",
            "2gis.*",
            "flightradar24.com",
            "rome2rio.com",
        ],
    ),
    (
        "media",
        &[
            "youtube.com",
            "youtu.be",
            "netflix.com",
            "twitch.tv",
            "spotify.com",
            "music.yandex.ru",
            "kinopoisk.ru",
            "vimeo.com",
            "soundcloud.com",
            "music.apple.com",
            "tv.apple.com",
            "primevideo.com",
            "disneyplus.com",
            "hulu.com",
            "rutube.ru",
            "ivi.ru",
            "okko.tv",
            "music.youtube.com",
            "bandcamp.com",
            "dzen.ru",
        ],
    ),
    (
        "social",
        &[
            "twitter.com",
            "x.com",
            "facebook.com",
            "instagram.com",
            "linkedin.com",
            "reddit.com",
            "vk.com",
            "tiktok.com",
            "threads.net",
            "bsky.app",
            "mastodon.social",
            "ok.ru",
            "tumblr.com",
            "pikabu.ru",
            "quora.com",
        ],
    ),
    (
        "news",
        &[
            "news.google.com",
            "bbc.com",
            "bbc.co.uk",
            "cnn.com",
            "nytimes.com",
            "theguardian.com",
            "reuters.com",
            "apnews.com",
            "ria.ru",
            "meduza.io",
            "rbc.ru",
            "lenta.ru",
            "news.ycombinator.com",
            "theverge.com",
            "techcrunch.com",
            "bloomberg.com",
            "ft.com",
            "wsj.com",
            "washingtonpost.com",
            "kommersant.ru",
            "vedomosti.ru",
            "tass.ru",
            "interfax.ru",
        ],
    ),
    (
        "finance",
        &[
            "paypal.com",
            "coinbase.com",
            "binance.com",
            "tradingview.com",
            "investing.com",
            "finance.yahoo.com",
            "revolut.com",
            "wise.com",
            "tinkoff.ru",
            "tbank.ru",
            "sberbank.ru",
            "online.sberbank.ru",
            "alfabank.ru",
            "stripe.com",
        ],
    ),
];

/// Порядок категорий по умолчанию (когда групп одинакового размера).
const ORDER: &[&str] = &[
    "development",
    "documentation",
    "design",
    "research",
    "productivity",
    "communication",
    "ai",
    "shopping",
    "travel",
    "media",
    "social",
    "news",
    "finance",
    "search",
];

/// Сколько страниц одного сайта нужно, чтобы сделать из него свою группу.
const SITE_GROUP_MIN: usize = 3;

/// Домен без «www.» и путь (нижний регистр). None — не веб-адрес.
fn host_path(url: &str) -> Option<(String, String)> {
    let u = url::Url::parse(url).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    let host = u.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    Some((host, u.path().to_ascii_lowercase()))
}

fn matches_site(host: &str, path: &str, site: &str) -> bool {
    if let Some(name) = site.strip_suffix(".*") {
        // «amazon.*»: amazon.com, amazon.co.uk, smile.amazon.de — имя стоит не последним.
        let labels: Vec<&str> = host.split('.').collect();
        return labels.len() >= 2 && labels[..labels.len() - 1].contains(&name);
    }
    let (domain, prefix) = match site.split_once('/') {
        Some((d, p)) => (d, Some(format!("/{p}"))),
        None => (site, None),
    };
    let host_ok = host == domain || host.ends_with(&format!(".{domain}"));
    host_ok && prefix.is_none_or(|p| path == p || path.starts_with(&format!("{p}/")))
}

/// Категория страницы по правилам. None — правила не знают этот сайт.
pub fn category(url: &str) -> Option<&'static str> {
    let (host, path) = host_path(url)?;
    // Поисковая выдача — своя группа, что бы ни было в домене.
    let search_host = [
        "google.",
        "bing.com",
        "duckduckgo.com",
        "yandex.",
        "ya.ru",
        "search.brave.com",
        "ecosia.org",
        "startpage.com",
    ]
    .iter()
    .any(|s| host.starts_with(s) || host.contains(&format!(".{s}")) || host == s.trim_end_matches('.'));
    if search_host && (path.starts_with("/search") || (host == "duckduckgo.com" && url.contains("q="))) {
        return Some("search");
    }
    // Сначала более точные правила с путём (google.com/maps), потом по домену.
    for exact_path in [true, false] {
        for (cat, sites) in RULES {
            if sites.iter().filter(|s| s.contains('/') == exact_path).any(|s| matches_site(&host, &path, s)) {
                return Some(cat);
            }
        }
    }
    // Документация на любом сайте: docs.* и разделы /docs, /reference, /api.
    if host.starts_with("docs.")
        || host.starts_with("developer.")
        || host.starts_with("devdocs.")
        || host.contains(".readthedocs.")
    {
        return Some("documentation");
    }
    if ["/docs", "/documentation", "/reference", "/api-reference", "/manual"]
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
    {
        return Some("documentation");
    }
    None
}

/// Домен без «www.» для подписи страницы.
pub fn domain(url: &str) -> String {
    host_path(url).map(|(h, _)| h).unwrap_or_default()
}

/// Разложить страницы (id, адрес) по группам. Группы — от больших к маленьким,
/// при равенстве — в порядке ORDER; «Other» всегда последняя. Порядок страниц
/// внутри группы сохраняется.
pub fn group(pages: &[(u64, &str)]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut unknown: Vec<(u64, String)> = Vec::new();
    for &(id, url) in pages {
        match category(url) {
            Some(cat) => match groups.iter_mut().find(|g| g.key == cat) {
                Some(g) => g.ids.push(id),
                None => groups.push(Group { key: cat, label: String::new(), ids: vec![id] }),
            },
            None => unknown.push((id, domain(url))),
        }
    }
    // Неизвестные сайты, которых много, — своей группой по имени сайта.
    let mut other = Vec::new();
    let mut site_groups: Vec<Group> = Vec::new();
    for (id, host) in &unknown {
        let count = unknown.iter().filter(|(_, h)| h == host).count();
        if !host.is_empty() && count >= SITE_GROUP_MIN {
            match site_groups.iter_mut().find(|g| &g.label == host) {
                Some(g) => g.ids.push(*id),
                None => site_groups.push(Group { key: "site", label: host.clone(), ids: vec![*id] }),
            }
        } else {
            other.push(*id);
        }
    }
    let rank = |g: &Group| ORDER.iter().position(|k| *k == g.key).unwrap_or(ORDER.len());
    groups.extend(site_groups);
    groups.sort_by(|a, b| b.ids.len().cmp(&a.ids.len()).then(rank(a).cmp(&rank(b))).then(a.label.cmp(&b.label)));
    if !other.is_empty() {
        groups.push(Group { key: "other", label: String::new(), ids: other });
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_by_site() {
        let cases = [
            ("https://github.com/rust-lang/rust", Some("development")),
            ("https://gist.github.com/x", Some("development")),
            ("http://localhost:3000/", Some("development")),
            ("https://www.figma.com/file/1", Some("design")),
            ("https://docs.rs/serde", Some("documentation")),
            ("https://docs.tauri.app/start/", Some("documentation")),
            ("https://example.com/docs/intro", Some("documentation")),
            ("https://en.wikipedia.org/wiki/Rust", Some("research")),
            ("https://www.amazon.co.uk/dp/1", Some("shopping")),
            ("https://smile.amazon.de/", Some("shopping")),
            ("https://mail.google.com/mail/u/0", Some("communication")),
            ("https://docs.google.com/document/d/1", Some("productivity")),
            ("https://www.google.com/maps/place/x", Some("travel")),
            ("https://www.google.com/search?q=rust", Some("search")),
            ("https://duckduckgo.com/?q=rust", Some("search")),
            ("https://yandex.ru/search/?text=rust", Some("search")),
            ("https://www.youtube.com/watch?v=1", Some("media")),
            ("https://claude.ai/new", Some("ai")),
            ("https://news.ycombinator.com/", Some("news")),
            ("https://example.com/", None),
            ("about:blank", None),
            ("not a url", None),
        ];
        for (url, cat) in cases {
            assert_eq!(category(url), cat, "{url}");
        }
    }

    #[test]
    fn site_suffix_does_not_match_lookalikes() {
        assert_eq!(category("https://notgithub.com/"), None);
        assert_eq!(category("https://github.com.evil.test/"), None);
        assert_eq!(category("https://www.google.com/mapsearch"), None);
        // «amazon.*» — не доменная зона.
        assert_eq!(category("https://shop.amazon/"), None);
    }

    #[test]
    fn groups_sorted_by_size_with_other_last() {
        let pages = [
            (1, "https://github.com/a"),
            (2, "https://example.com/"),
            (3, "https://en.wikipedia.org/wiki/A"),
            (4, "https://github.com/b"),
            (5, "https://stackoverflow.com/q/1"),
            (6, "https://arxiv.org/abs/1"),
        ];
        let g = group(&pages);
        let summary: Vec<_> = g.iter().map(|g| (g.key, g.ids.clone())).collect();
        assert_eq!(summary, [("development", vec![1, 4, 5]), ("research", vec![3, 6]), ("other", vec![2])]);
    }

    #[test]
    fn many_pages_of_unknown_site_make_their_own_group() {
        let pages = [
            (1, "https://intranet.corp/a"),
            (2, "https://intranet.corp/b"),
            (3, "https://www.intranet.corp/c"),
            (4, "https://blog.test/"),
        ];
        let g = group(&pages);
        assert_eq!(g[0], Group { key: "site", label: "intranet.corp".into(), ids: vec![1, 2, 3] });
        assert_eq!(g[1].key, "other");
    }

    #[test]
    fn ties_follow_default_order() {
        let g = group(&[(1, "https://amazon.com/"), (2, "https://github.com/")]);
        assert_eq!(g.iter().map(|g| g.key).collect::<Vec<_>>(), ["development", "shopping"]);
        assert!(group(&[]).is_empty());
    }

    #[test]
    fn domain_strips_www() {
        assert_eq!(domain("https://www.github.com/x"), "github.com");
        assert_eq!(domain("about:blank"), "");
    }
}
