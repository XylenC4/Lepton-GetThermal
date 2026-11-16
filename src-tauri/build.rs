fn main() {
    tauri_build::build();

    // Windows single-exe: the ONNX Runtime DLLs embedded by processing/upscale.rs.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        for f in WIN_EMBEDDED_DLLS {
            let p = format!("windows-libs/{f}");
            println!("cargo:rerun-if-changed={p}");
            if !std::path::Path::new(&p).exists() {
                panic!("{p} missing: run scripts/build-windows.sh (it downloads the DLLs)");
            }
        }
    }

    // macOS uses a small C helper on top of IOKit; all other targets use libusb.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/usb_helper.c")
            .compile("usb_helper");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}

const WIN_EMBEDDED_DLLS: &[&str] = &[
    "onnxruntime.dll",
    "MSVCP140.dll",
    "MSVCP140_1.dll",
    "VCRUNTIME140.dll",
    "VCRUNTIME140_1.dll",
];
