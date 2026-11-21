//! Windows: embed the app icon and version info in the .exe (rc.exe from the MSVC toolchain).
//! Everywhere else this is a no-op.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/logo/AppIcon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
    }
}

// winresource is only a build-dependency on Windows hosts (see Cargo.toml)
#[cfg(windows)]
fn windows_resources() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/logo/AppIcon.ico");
    res.set("ProductName", "Vigía");
    res.set("FileDescription", "Vigía");
    res.set("InternalName", "vigia-app");
    res.set("OriginalFilename", "vigia-app.exe");
    res.compile().expect("embed Windows resources (icon, version info)");
}

#[cfg(not(windows))]
fn windows_resources() {}
