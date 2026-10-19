//! Модульные тесты src/bookmarks_html.rs (подключены там через `#[path]`, видят приватное).

use super::*;

fn titles(nodes: &[Node]) -> Vec<&str> {
    nodes.iter().map(|n| n.title.as_str()).collect()
}

#[test]
fn reads_a_chrome_export() {
    let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1700000000" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://github.com/" ADD_DATE="1700000001" ICON="data:image/png;base64,AAA">GitHub</A>
        <DT><H3>Work &amp; study</H3>
        <DL><p>
            <DT><A HREF="https://docs.rs/">Docs.rs &mdash; &#8220;crates&#8221;</A>
            <DT><A HREF="javascript:alert(1)">Bookmarklet</A>
        </DL><p>
    </DL><p>
    <DT><A HREF='http://example.com/?a=1&amp;b=2'>Example</A>
    <DT><a href="https://nameless.test/"></a>
</DL><p>"#;
    let nodes = parse(html);
    assert_eq!(titles(&nodes), ["Bookmarks bar", "Example", "https://nameless.test/"]);
    let bar = nodes[0].children.as_ref().unwrap();
    assert_eq!(titles(bar), ["GitHub", "Work & study"]);
    assert_eq!((bar[0].url.as_str(), bar[0].added), ("https://github.com/", 1_700_000_001));
    let work = bar[1].children.as_ref().unwrap();
    assert_eq!(work.len(), 1, "javascript: не переносится");
    assert_eq!(work[0].title, "Docs.rs &mdash; “crates”", "известные сущности раскрыты, незнакомые оставлены");
    assert_eq!(nodes[1].url, "http://example.com/?a=1&b=2");
}

#[test]
fn export_then_import_gives_the_same_tree() {
    let tree = vec![
        Node::folder(
            "Work <1>",
            vec![Node::link("A & B", "https://a.test/?x=\"1\"", 5), Node::folder("Empty", vec![])],
        ),
        Node::link("C", "https://c.test/", 0),
    ];
    let back = parse(&export(&tree));
    assert_eq!(back, tree);
}

#[test]
fn junk_and_truncated_files_do_not_panic() {
    for html in [
        "",
        "<",
        "<a",
        "<A HREF=",
        "<DL><p><DT><H3>Open",
        "</DL></DL>",
        "&#99999999;",
        "<a href=\"https://x.test/\">&",
        "<DL><DL><DL>",
        "<H3>&ʻʻʻʻʻʻʻ</H3>",
        "<a href=\"https://ʻʻʻ.test/\">&amʻ;</a>",
    ] {
        let _ = parse(html);
    }
    // Обрезанный файл: папка без </DL> всё равно приходит со ссылкой.
    let nodes = parse("<DL><p><DT><H3>F</H3><DL><p><DT><A HREF=\"https://f.test/\">F1</A>");
    assert_eq!(nodes[0].children.as_ref().unwrap()[0].url, "https://f.test/");
}
