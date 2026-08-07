fn main() {
    println!("cargo:rerun-if-changed=src/ios_photo_reader.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }

    cc::Build::new()
        .file("src/ios_photo_reader.m")
        .flag("-fobjc-arc")
        .compile("portalis_photo_reader");
    println!("cargo:rustc-link-lib=framework=Photos");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
}
