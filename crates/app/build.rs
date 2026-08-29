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
    }

    slint_build::compile("ui/app.slint").expect("Slint build failed");
}
