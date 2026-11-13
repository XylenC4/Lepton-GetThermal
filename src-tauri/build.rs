fn main() {
    tauri_build::build();

    // macOS uses a small C helper on top of IOKit; all other targets use libusb.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/usb_helper.c")
            .compile("usb_helper");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}
