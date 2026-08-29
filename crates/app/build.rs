// =================================================================================================
// build — Windows icon and Slint compilation
// =================================================================================================

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let icon = std::path::Path::new("../../logo.ico");
        if icon.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(icon.to_str().unwrap());
            if let Err(e) = res.compile() {
                println!("cargo:warning=winresource compile failed: {e}");
            }
        }
        // Ensure release builds are GUI-only (no console), debug keeps console for logs.
        // This complements #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] in main.rs
        // and guarantees `cargo build --release` never spawns the black terminal, even from Explorer.
        if std::env::var("PROFILE").unwrap_or_default() == "release" {
            println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
            println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
        }
    }

    slint_build::compile("ui/app.slint").expect("Slint build failed");
}
