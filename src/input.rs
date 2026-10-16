//! Разбор того, что пользователь ввёл в адресную строку: адрес или поисковый запрос.

pub fn resolve(text: &str, search_url: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();

    if lower.starts_with("javascript:") {
        return Some(search(t, search_url));
    }
    for scheme in ["http://", "https://", "file://", "about:", "edge://", "view-source:"] {
        if lower.starts_with(scheme) {
            return Some(t.to_string());
        }
    }
    if t.contains(char::is_whitespace) {
        return Some(search(t, search_url));
    }

    let host = lower
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let host_no_port = host.split(':').next().unwrap_or("");

    if host_no_port == "localhost" || is_ipv4(host_no_port) {
        return Some(format!("http://{t}"));
    }
    if let Some((_, tld)) = host_no_port.rsplit_once('.') {
        let valid_tld = tld.len() >= 2 && tld.chars().all(|c| c.is_alphabetic() || c == '-');
        let valid_host = !host_no_port.starts_with('.') && !host_no_port.contains("..");
        if valid_tld && valid_host {
            return Some(format!("https://{t}"));
        }
    }
    Some(search(t, search_url))
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<_> = s.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok())
}

fn search(q: &str, search_url: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(q.as_bytes()).collect();
    if search_url.contains("%s") {
        search_url.replace("%s", &encoded)
    } else {
        format!("{search_url}{encoded}")
    }
}

#[cfg(test)]
mod tests {
    use super::resolve;
    const S: &str = "https://s.test/?q=%s";

    #[test]
    fn urls_and_searches() {
        assert_eq!(resolve("github.com", S).unwrap(), "https://github.com");
        assert_eq!(resolve("ya.ru/search?x=1", S).unwrap(), "https://ya.ru/search?x=1");
        assert_eq!(resolve("localhost:3000", S).unwrap(), "http://localhost:3000");
        assert_eq!(resolve("192.168.1.1", S).unwrap(), "http://192.168.1.1");
        assert_eq!(resolve("https://x.org", S).unwrap(), "https://x.org");
        assert_eq!(resolve("rust borrow checker", S).unwrap(), "https://s.test/?q=rust+borrow+checker");
        assert_eq!(resolve("привет", S).unwrap(), "https://s.test/?q=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82");
        assert_eq!(resolve("3.14", S).unwrap(), "https://s.test/?q=3.14");
        assert_eq!(resolve("javascript:alert(1)", S).unwrap(), "https://s.test/?q=javascript%3Aalert%281%29");
        assert!(resolve("   ", S).is_none());
    }
}
