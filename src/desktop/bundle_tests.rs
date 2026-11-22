//! Fleet's `check-bundled-server.test.mjs`, eight cases, each under its own name (row 25.2).
use super::bundle::{check_bundled_server, compare_mach_o, is_mach_o};
use super::paths::{Inputs, Os, find_server};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const CODE: &[u8] = b"the server's code";

/// A minimal 64-bit Mach-O: header, `__TEXT` and `__LINKEDIT` segments, `LC_CODE_SIGNATURE`, the code, then the signature.
fn mach_o(code: &[u8], padding: usize, signature: &[u8]) -> Vec<u8> {
    let (header, segment, sig_cmd) = (32, 72, 16);
    let cmds = segment * 2 + sig_cmd;
    let code_at = header + cmds;
    let dataoff = code_at + code.len() + padding;
    let mut buf = vec![0u8; dataoff + signature.len()];
    let put32 = |buf: &mut Vec<u8>, at: usize, v: u32| buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |buf: &mut Vec<u8>, at: usize, v: u64| buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
    put32(&mut buf, 0, 0xfeed_facf);
    put32(&mut buf, 4, 0x0100_000c); // arm64
    put32(&mut buf, 12, 6); // dylib
    put32(&mut buf, 16, 3);
    put32(&mut buf, 20, cmds as u32);
    let mut at = header;
    for (name, size) in [("__TEXT", code_at + code.len()), ("__LINKEDIT", padding + signature.len())] {
        put32(&mut buf, at, 0x19);
        put32(&mut buf, at + 4, segment as u32);
        buf[at + 8..at + 8 + name.len()].copy_from_slice(name.as_bytes());
        put64(&mut buf, at + 32, size as u64); // vmsize
        put64(&mut buf, at + 48, size as u64); // filesize
        at += segment;
    }
    put32(&mut buf, at, 0x1d);
    put32(&mut buf, at + 4, sig_cmd as u32);
    put32(&mut buf, at + 8, dataoff as u32);
    put32(&mut buf, at + 12, signature.len() as u32);
    buf[code_at..code_at + code.len()].copy_from_slice(code);
    buf[dataoff..].copy_from_slice(signature);
    buf
}

fn built_mach_o() -> Vec<u8> {
    mach_o(CODE, 0, b"SIG-A")
}

fn fat(images: &[Vec<u8>]) -> Vec<u8> {
    let mut head = vec![0u8; 8 + images.len() * 20];
    head[..4].copy_from_slice(&0xcafe_babe_u32.to_be_bytes());
    head[4..8].copy_from_slice(&(images.len() as u32).to_be_bytes());
    let mut offset = 4096;
    let mut parts = Vec::new();
    for (i, image) in images.iter().enumerate() {
        head[8 + i * 20 + 8..8 + i * 20 + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        head[8 + i * 20 + 12..8 + i * 20 + 16].copy_from_slice(&(image.len() as u32).to_be_bytes());
        parts.push((offset, image));
        offset += image.len().div_ceil(4096) * 4096;
    }
    let mut out = vec![0u8; offset];
    out[..head.len()].copy_from_slice(&head);
    for (at, image) in parts {
        out[at..at + image.len()].copy_from_slice(image);
    }
    out
}

#[test]
fn compare_mach_o_sees_a_re_signed_file_as_the_same() {
    let resigned = mach_o(CODE, 0, b"a much longer ad-hoc signature from codesign");
    assert!(is_mach_o(&built_mach_o()));
    assert_eq!(compare_mach_o(&built_mach_o(), &resigned), None);
}

#[test]
fn compare_mach_o_allows_zero_padding_before_a_signature_codesign_placed_further_on() {
    assert_eq!(compare_mach_o(&built_mach_o(), &mach_o(CODE, 12, b"SIG-B")), None);
}

#[test]
fn compare_mach_o_catches_one_changed_byte_of_code() {
    let changed = mach_o(b"the server's codf", 0, b"SIG-A");
    assert!(compare_mach_o(&built_mach_o(), &changed).unwrap().contains("differs at byte"));
}

#[test]
fn compare_mach_o_catches_bytes_hidden_where_only_padding_may_be() {
    let mut padded = mach_o(CODE, 12, b"SIG-A");
    padded[32 + 160 + CODE.len() + 3] = 7;
    assert!(compare_mach_o(&built_mach_o(), &padded).unwrap().contains("not just padding"));
}

#[test]
fn compare_mach_o_compares_each_architecture_of_a_fat_file() {
    let resigned = mach_o(CODE, 0, b"SIG-LONGER");
    assert_eq!(compare_mach_o(&fat(&[built_mach_o(), built_mach_o()]), &fat(&[resigned.clone(), resigned.clone()])), None);
    let changed = mach_o(b"the server's cod3", 0, b"SIG-LONGER");
    let why = compare_mach_o(&fat(&[built_mach_o(), built_mach_o()]), &fat(&[resigned, changed])).unwrap();
    assert!(why.starts_with("slice 1: differs"), "{why}");
}

/// A scratch folder holding the given files, removed when it drops.
struct Layout(PathBuf);

impl Layout {
    fn new(files: &[(&str, Vec<u8>)]) -> Layout {
        static N: AtomicUsize = AtomicUsize::new(0);
        let root =
            std::env::temp_dir().join(format!("fleet-native-bundled-server-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        for (name, content) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        Layout(root)
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn built_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("WeaveFleet.Api", built_mach_o()),
        ("libe_sqlite3.dylib", mach_o(b"sqlite", 0, b"SIG-A")),
        ("wwwroot/index.html", b"<html>".to_vec()),
        ("WeaveFleet.Api.dbg", b"debug symbols".to_vec()),
    ]
}

const APP: &str = "mac-arm64/Fleet.app/Contents/Resources/fleet";

#[test]
fn check_bundled_server_passes_an_app_that_ships_the_server_as_built_re_signed_mach_o_files_included() {
    let server = Layout::new(&built_files());
    let release = Layout::new(&[
        (&format!("{APP}/WeaveFleet.Api"), mach_o(CODE, 0, b"electron-builder ad-hoc signature")),
        (&format!("{APP}/libe_sqlite3.dylib"), mach_o(b"sqlite", 0, b"SIG-2")),
        (&format!("{APP}/wwwroot/index.html"), b"<html>".to_vec()),
    ]);
    let check = check_bundled_server(&server.0, &release.0);
    assert!(check.ok, "{:?}", check.lines);
    assert!(check.lines[0].contains("3 of the 3 built files shipped as built (2 re-signed Mach-O files"), "{:?}", check.lines);
}

#[test]
fn check_bundled_server_fails_on_a_changed_missing_or_extra_file() {
    let server = Layout::new(&built_files());
    let release = Layout::new(&[
        ("linux-unpacked/resources/fleet/WeaveFleet.Api", mach_o(b"other server code", 0, b"SIG-A")),
        ("linux-unpacked/resources/fleet/wwwroot/index.html", b"<html>".to_vec()),
        ("linux-unpacked/resources/fleet/stray.txt", b"?".to_vec()),
    ]);
    let check = check_bundled_server(&server.0, &release.0);
    assert!(!check.ok);
    let text = check.lines.join("\n");
    assert!(text.contains("1 file(s) missing: libe_sqlite3.dylib"), "{text}");
    assert!(text.contains("1 file(s) not from the build: stray.txt"), "{text}");
    assert!(text.contains("1 file(s) different: WeaveFleet.Api (slice 0: "), "{text}");
}

#[test]
fn check_bundled_server_fails_when_nothing_was_packaged() {
    let check = check_bundled_server(&Layout::new(&built_files()).0, &Layout::new(&[("other/file", b"x".to_vec())]).0);
    assert!(!check.ok);
}

/// The native packages put the server beside the exe (`<app>/fleet`), not under a `resources` folder: that layout is
/// found too, by the binary inside it.
#[test]
fn check_bundled_server_finds_the_server_beside_the_exe() {
    let server = Layout::new(&built_files());
    let release = Layout::new(&[
        ("fleet-native/fleet-native", b"exe".to_vec()),
        ("fleet-native/fleet/WeaveFleet.Api", built_mach_o()),
        ("fleet-native/fleet/libe_sqlite3.dylib", mach_o(b"sqlite", 0, b"SIG-A")),
        ("fleet-native/fleet/wwwroot/index.html", b"<html>".to_vec()),
    ]);
    let check = check_bundled_server(&server.0, &release.0);
    assert!(check.ok, "{:?}", check.lines);
}

/// Row 25.3: each package's real folder layout (the files a package unpacks to) is where `find_server` looks first.
mod layouts {
    use super::*;

    fn found(os: Os, files: &[&str], exe_dir: &str) -> Option<PathBuf> {
        let layout = Layout::new(&files.iter().map(|f| (*f, b"x".to_vec())).collect::<Vec<_>>());
        let get = |_: &str| None;
        let exists = |p: &std::path::Path| p.is_file();
        let inputs = Inputs { get: &get, exists: &exists, os, home: None, exe_dir: Some(layout.0.join(exe_dir)), path_dirs: Vec::new() };
        find_server(&inputs).ok().map(|s| s.binary.strip_prefix(&layout.0).unwrap().components().collect::<PathBuf>())
    }

    #[test]
    fn the_linux_tar_and_appimage_hold_the_server_beside_the_exe() {
        let at = found(Os::Linux, &["app/fleet-native", "app/fleet/WeaveFleet.Api", "app/fleet/mods-host/host.js"], "app");
        assert_eq!(at, Some(PathBuf::from("app/fleet/WeaveFleet.Api")));
    }

    #[test]
    fn the_windows_folder_holds_the_server_beside_the_exe() {
        let at = found(Os::Windows, &["Fleet-Native/fleet-native.exe", "Fleet-Native/fleet/WeaveFleet.Api.exe"], "Fleet-Native");
        assert_eq!(at, Some(PathBuf::from("Fleet-Native/fleet/WeaveFleet.Api.exe")));
    }

    #[test]
    fn the_mac_bundle_holds_the_server_in_contents_resources_fleet() {
        let files = ["Fleet Native.app/Contents/MacOS/fleet-native", "Fleet Native.app/Contents/Resources/fleet/WeaveFleet.Api"];
        let at = found(Os::Mac, &files, "Fleet Native.app/Contents/MacOS");
        let found = at.expect("found").components().filter(|c| c.as_os_str() != "..").collect::<PathBuf>();
        assert!(found.ends_with("Resources/fleet/WeaveFleet.Api"), "{found:?}");
    }
}
