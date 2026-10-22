//! Build script: gives the Windows executable its icon. Nothing happens on other systems.

fn main() {
    println!("cargo:rerun-if-changed=packaging/windows/tubeglass.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon();
    }
}

#[cfg(windows)]
fn embed_windows_icon() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("packaging/windows/tubeglass.ico");
    res.set("ProductName", "TubeGlass");
    res.set("FileDescription", "TubeGlass");
    if let Err(e) = res.compile() {
        // a missing resource compiler only costs the icon, never the build
        println!("cargo:warning=could not embed the icon: {e}");
    }
}

#[cfg(not(windows))]
fn embed_windows_icon() {
    // cross-compiling to Windows from another system: the icon is skipped
}
