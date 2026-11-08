//! Windows only: embed the app icon and version info (VERSIONINFO) into `scorecraft.exe`.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `SCORECRAFT_REQUIRE_WINRES=1` (for
//! release builds) turns it into an error.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/scorecraft.ico");
    println!("cargo:rerun-if-env-changed=SCORECRAFT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/scorecraft.ico")
        .set("ProductName", "ScoreCraft")
        .set("FileDescription", "ScoreCraft sheet music reader")
        .set(
            "LegalCopyright",
            "Copyright (c) the ScoreCraft authors. MIT OR Apache-2.0.",
        )
        .set("OriginalFilename", "scorecraft.exe")
        .set("InternalName", "scorecraft");
    if let Err(e) = res.compile() {
        // A build script reports failure by panicking; there is no error to return.
        #[allow(clippy::panic)]
        if std::env::var_os("SCORECRAFT_REQUIRE_WINRES").is_some() {
            panic!("embedding Windows resources failed: {e}");
        }
        println!("cargo:warning=scorecraft.exe built without icon/version resources: {e}");
    }
}
