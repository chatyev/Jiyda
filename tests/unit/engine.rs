//! Модульные тесты src/engine/mod.rs (подключены там через `#[path]`, видят приватное).

use super::*;

/// Граница Engine Abstraction: `wry` и WebView2 COM — только внутри src/engine/webview2/.
/// Остальной код (ядро, защита, разрешения, клавиши, окно) знает лишь src/engine/mod.rs.
#[test]
fn webview2_is_only_used_inside_its_engine() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let engine_dir = root.join("src").join("engine").join("webview2");
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    let forbidden = ["wry::", "use wry", "webview2_com", "WebViewExtWindows", "WebViewBuilder", "ICoreWebView2"];
    let mut found = Vec::new();
    for f in files.iter().filter(|f| !f.starts_with(&engine_dir)) {
        let text = std::fs::read_to_string(f).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if let Some(word) = forbidden.iter().find(|w| code.contains(*w)) {
                found.push(format!("{}:{}: {word}", f.strip_prefix(root).unwrap().display(), n + 1));
            }
        }
    }
    assert!(found.is_empty(), "WebView2 вне src/engine/webview2/:\n{}", found.join("\n"));
    assert!(files.len() > 30, "файлы не найдены: {}", files.len());
}

#[test]
fn theme_from_setting() {
    assert_eq!(Theme::from_setting("light"), Theme::Light);
    assert_eq!(Theme::from_setting("dark"), Theme::Dark);
    assert_eq!(Theme::from_setting("system"), Theme::System);
    assert_eq!(Theme::from_setting("что-то другое"), Theme::System);
}

#[test]
fn profile_keeps_engine_specific_contents() {
    let mut p = Profile::new(42u32);
    assert_eq!(p.downcast_mut::<u32>().copied(), Some(42));
    assert!(p.downcast_mut::<String>().is_none(), "профиль другого движка не подходит");
}
