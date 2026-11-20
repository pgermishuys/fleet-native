// Windows only: puts the Weave icon and version information into fleet-native.exe, so Explorer and the
// taskbar show them. Elsewhere this does nothing. The version is 0.0.<build>.0, from the same
// FLEET_NATIVE_BUILD that `--version` prints; a local build says 0.
fn main() {
    println!("cargo:rerun-if-env-changed=FLEET_NATIVE_BUILD");
    println!("cargo:rerun-if-changed=assets/icon/fleet-native.ico");
    // Windows gives the main thread 1 MB of stack, and the debug build's view functions sit close to it: the web-view walk's start
    // overflowed it on a pull request that only added a field to `State` (and on `main` once, 20 November 2026). The app draws on
    // the main thread, so the stack is made 8 MB, as macOS and Linux have by default.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
    #[cfg(windows)]
    {
        let build: u64 = std::env::var("FLEET_NATIVE_BUILD").ok().and_then(|b| b.parse().ok()).unwrap_or(0);
        let version = format!("0.0.{build}.0");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/fleet-native.ico");
        res.set("ProductName", "Fleet Native");
        res.set("FileDescription", "Fleet Native");
        res.set("CompanyName", "Weave");
        res.set("OriginalFilename", "fleet-native.exe");
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        res.set_version_info(winresource::VersionInfo::FILEVERSION, build_u64(build));
        res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, build_u64(build));
        res.compile().expect("compile the Windows resources");
    }
}

// The four 16-bit parts 0, 0, build, 0 packed as Windows wants them.
#[cfg(windows)]
fn build_u64(build: u64) -> u64 {
    (build.min(0xFFFF)) << 16
}
