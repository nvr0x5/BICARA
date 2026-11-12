fn main() {
    println!("cargo:rerun-if-changed=native/macos/tap.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/macos/tap.m")
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .flag("-Wno-unguarded-availability-new")
            .flag("-Wno-unused-parameter")
            .compile("bicara_macos_tap");
        for fw in ["CoreAudio", "AudioToolbox", "Foundation", "AppKit"] {
            println!("cargo:rustc-link-lib=framework={fw}");
        }
    }
}
