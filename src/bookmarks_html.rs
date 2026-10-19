//! Закладки в формате HTML «Netscape Bookmark File» — его понимают Chrome, Edge, Firefox
//! и Safari: экспорт (Закладки → «Сохранить в файл») и импорт («Открыть файл…»).
//!
//! Разбор терпимый: смотрим только на <H3> (папка), <A HREF> (ссылка) и <DL>…</DL>
//! (содержимое папки); остальная разметка пропускается. Берутся только ссылки на сайты
//! (http, https) — javascript: и прочее не переносится.

use crate::bookmarks::Node;

/// Записать дерево закладок файлом HTML.
pub fn export(items: &[Node]) -> String {
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
         <!-- This is an automatically generated file. It will be read and overwritten. DO NOT EDIT! -->\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
         <TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    write_nodes(&mut out, items, 1);
    out.push_str("</DL><p>\n");
    out
}

fn write_nodes(out: &mut String, nodes: &[Node], depth: usize) {
    let pad = "    ".repeat(depth);
    for n in nodes {
        match &n.children {
            Some(children) => {
                out.push_str(&format!("{pad}<DT><H3>{}</H3>\n{pad}<DL><p>\n", escape(&n.title)));
                write_nodes(out, children, depth + 1);
                out.push_str(&format!("{pad}</DL><p>\n"));
            }
            None => {
                let added = if n.added > 0 { format!(" ADD_DATE=\"{}\"", n.added) } else { String::new() };
                out.push_str(&format!("{pad}<DT><A HREF=\"{}\"{added}>{}</A>\n", escape(&n.url), escape(&n.title)));
            }
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Разобрать файл закладок: папки и ссылки верхнего уровня (корневой <DL>).
pub fn parse(html: &str) -> Vec<Node> {
    // Стек папок: нижняя — корень. Заголовок <H3> ждёт своего <DL>.
    let mut stack: Vec<Vec<Node>> = vec![Vec::new()];
    let mut titles: Vec<String> = Vec::new();
    let mut pending_folder: Option<String> = None;
    let mut opened = 0usize; // сколько <DL> открыто (первый — корень файла)
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    while let Some(off) = lower[i..].find('<') {
        let at = i + off;
        let rest = &lower[at..];
        if rest.starts_with("<h3") {
            let (text, end) = tag_text(html, &lower, at, "</h3>");
            pending_folder = Some(text);
            i = end;
        } else if rest.starts_with("<a ") || rest.starts_with("<a\t") || rest.starts_with("<a\n") {
            let tag_end = lower[at..].find('>').map_or(lower.len(), |e| at + e + 1);
            let href = attr(&html[at..tag_end], &lower[at..tag_end], "href");
            let added = attr(&html[at..tag_end], &lower[at..tag_end], "add_date").parse().unwrap_or(0);
            let (text, end) = tag_text(html, &lower, at, "</a>");
            let href = href.trim();
            let web = href.get(..8).unwrap_or(href).to_ascii_lowercase();
            if (web.starts_with("http://") || web.starts_with("https://")) && href.len() <= 4096 {
                let title = if text.trim().is_empty() { href.to_string() } else { text.trim().to_string() };
                if let Some(top) = stack.last_mut() {
                    top.push(Node::link(&title, href, added));
                }
            }
            i = end;
        } else if rest.starts_with("<dl") {
            opened += 1;
            // Первый <DL> — корень файла; следующие — содержимое папок.
            if opened > 1 {
                titles.push(pending_folder.take().unwrap_or_default());
                stack.push(Vec::new());
            }
            i = at + 3;
        } else if rest.starts_with("</dl") {
            if stack.len() > 1 {
                let children = stack.pop().unwrap_or_default();
                let title = titles.pop().unwrap_or_default();
                if let Some(top) = stack.last_mut() {
                    top.push(Node::folder(&title, children));
                }
            }
            i = at + 4;
        } else {
            i = at + 1;
        }
    }
    // Незакрытые папки (обрезанный файл) — всё равно забираем.
    while stack.len() > 1 {
        let children = stack.pop().unwrap_or_default();
        let title = titles.pop().unwrap_or_default();
        if let Some(top) = stack.last_mut() {
            top.push(Node::folder(&title, children));
        }
    }
    stack.pop().unwrap_or_default()
}

/// Текст элемента от открывающего тега в `at` до `close` (без тегов внутри) и где он кончился.
fn tag_text(html: &str, lower: &str, at: usize, close: &str) -> (String, usize) {
    let start = lower[at..].find('>').map_or(lower.len(), |e| at + e + 1);
    let end = lower[start..].find(close).map_or(lower.len(), |e| start + e);
    let inner = strip_tags(&html[start..end]);
    (unescape(inner.trim()), (end + close.len()).min(lower.len()))
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Значение атрибута `name` внутри тега (кавычки двойные, одинарные или без них).
fn attr(tag: &str, lower: &str, name: &str) -> String {
    let mut from = 0;
    while let Some(p) = lower[from..].find(name) {
        let at = from + p;
        let before_ok = at > 0 && lower.as_bytes()[at - 1].is_ascii_whitespace();
        let after = lower[at + name.len()..].trim_start();
        if before_ok && after.starts_with('=') {
            let value_at = tag.len() - after.len() + 1;
            let v = tag[value_at..].trim_start();
            let raw = match v.chars().next() {
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
                _ => v.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or(""),
            };
            return unescape(raw);
        }
        from = at + name.len();
    }
    String::new()
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let Some(semi) = rest.bytes().take(12).position(|b| b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") || e.starts_with("#X") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "../tests/unit/bookmarks_html.rs"]
mod tests;
