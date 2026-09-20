fn main() {
    slint_build::compile("ui/lite.slint").expect("compile Slint UI");
    println!("cargo:rerun-if-env-changed=DEEPCOOL_SKIP_MANIFEST");
    // Only relevant when the build target is Windows (this project is
    // Windows-only anyway, but this check avoids errors when `cargo check`
    // is run cross-platform).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && std::env::var_os("DEEPCOOL_SKIP_MANIFEST").is_none() {
        println!("cargo:rerun-if-changed=assets/app.rc");
        println!("cargo:rerun-if-changed=assets/app.manifest");
        println!("cargo:rerun-if-changed=assets/icon.ico");
        embed_resource::compile("assets/app.rc", embed_resource::NONE);
    }
}
