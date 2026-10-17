//! Сборка seyro.exe:
//! - встраивает значок и сведения о программе (название, версия), которые Windows
//!   показывает в Проводнике, на панели задач и в свойствах файла;
//! - собирает список файлов интерфейса из папки ui/ — ядро отдаёт их по адресу
//!   seyro://ui/… (src/app/ui_assets.rs), так что новый файл не нужно нигде регистрировать.

use std::path::{Path, PathBuf};

fn main() {
    embed_ui();
    println!("cargo:rerun-if-changed=assets/seyro.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // Номер ресурса 1 — по нему же окно берёт значок (app/init.rs, Icon::from_resource).
    res.set_icon_with_id("assets/seyro.ico", "1");
    res.set("ProductName", "Seyro");
    res.set("FileDescription", "Seyro");
    res.set("InternalName", "seyro");
    res.set("OriginalFilename", "seyro.exe");
    if let Err(e) = res.compile() {
        // Без значка программа всё равно соберётся и запустится.
        println!("cargo:warning=не удалось встроить значок: {e}");
    }
}

/// Все файлы ui/ (рекурсивно), в стабильном порядке.
fn ui_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            ui_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// $OUT_DIR/ui_files.rs: `UI_FILES: &[(путь, байты, тип)]`.
fn embed_ui() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("ui");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    ui_files(&root, &mut files);
    let mut code = String::from(
        "/// Файлы интерфейса (собирает build.rs из папки ui/).\npub const UI_FILES: &[(&str, &[u8], &str)] = &[\n",
    );
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let rel = f.strip_prefix(&root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        code.push_str(&format!("    ({rel:?}, include_bytes!({:?}), {:?}),\n", f.display().to_string(), mime(f)));
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("ui_files.rs");
    if let Err(e) = std::fs::write(&out, code) {
        panic!("не удалось записать {}: {e}", out.display());
    }
}
