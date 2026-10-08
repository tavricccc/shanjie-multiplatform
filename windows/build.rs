fn main() {
    println!("cargo:rerun-if-env-changed=SHANJIE_ICON_RESOURCE");
    if let Ok(resource) = std::env::var("SHANJIE_ICON_RESOURCE") {
        println!("cargo:rerun-if-changed={resource}");
        println!("cargo:rustc-link-arg-cdylib={resource}");
    }
}
