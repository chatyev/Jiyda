//! Встраивает в seyro.exe значок и сведения о программе (название, версия),
//! которые Windows показывает в Проводнике, на панели задач и в свойствах файла.

fn main() {
    println!("cargo:rerun-if-changed=assets/seyro.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // Номер ресурса 1 — по нему же окно берёт значок (app.rs, Icon::from_resource).
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
