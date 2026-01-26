use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    // Only run this on Windows
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    let logo_png = Path::new("logo.png");
    let icon_ico = out_path.join("icon.ico");

    // Check if we need to convert PNG to ICO
    if logo_png.exists() {
        println!("cargo:rerun-if-changed=logo.png");

        // Load the PNG
        match image::open(logo_png) {
            Ok(img) => {
                // Resize to standard icon sizes if needed, but for now let's just save as ICO.
                // The image crate's ICO encoder might be limited, but let's try a simple save first.
                // Actually, saving directly to ICO with `image` crate can be tricky if it expects multiple sizes.
                // A better approach for a simple script is to resize to 256x256 (standard large icon)
                // and save it.

                let img = img.resize(256, 256, image::imageops::FilterType::Lanczos3);

                match File::create(&icon_ico) {
                    Ok(mut file) => {
                        // Encode as ICO
                        if let Err(e) = img.write_to(&mut file, image::ImageOutputFormat::Ico) {
                            println!("cargo:warning=Failed to convert logo.png to ico: {}", e);
                        }
                    }
                    Err(e) => println!("cargo:warning=Failed to create icon.ico: {}", e),
                }
            }
            Err(e) => println!("cargo:warning=Failed to open logo.png: {}", e),
        }
    } else {
        println!("cargo:warning=logo.png not found, skipping icon generation.");
    }

    // Embed the icon if it exists (either we just made it, or it was there)
    let mut res = winres::WindowsResource::new();

    // valid paths to check for the icon
    if icon_ico.exists() {
        res.set_icon(icon_ico.to_str().unwrap());
    } else if std::path::Path::new("logo.ico").exists() {
        res.set_icon("logo.ico");
    }

    if let Err(e) = res.compile() {
        println!("cargo:warning=Failed to compile Windows resource: {}", e);
    }
}
