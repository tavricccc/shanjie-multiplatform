fn main() {
    // TaskDialogIndirect only exists in comctl32 v6, which needs a manifest dependency.
    println!("cargo:rustc-link-arg-bin=shanjie-manager=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bin=shanjie-manager=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
    println!("cargo:rerun-if-env-changed=SHANJIE_SETUP_RESOURCE");
    if let Ok(resource) = std::env::var("SHANJIE_SETUP_RESOURCE") {
        println!("cargo:rerun-if-changed={resource}");
        println!("cargo:rustc-link-arg-bin=shanjie-setup={resource}");
    }
    println!("cargo:rerun-if-env-changed=SHANJIE_ICON_RESOURCE");
    if let Ok(resource) = std::env::var("SHANJIE_ICON_RESOURCE") {
        println!("cargo:rerun-if-changed={resource}");
        println!("cargo:rustc-link-arg-cdylib={resource}");
    }
}
