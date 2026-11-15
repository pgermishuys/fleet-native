//! The updater against a fake release served from this machine: no test reaches GitHub.
use super::install::{self, Progress, Relaunch};
use super::manifest::{self, Asset};
use super::{Phase, Update};
use crate::State;
use crate::tests::{FRAME, app, click};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

/// A folder that is gone when the test ends.
struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("fleet-native-update-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Serves `files` (path → bytes) on a loopback port until the test ends; unknown paths are 404.
fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 2048];
            let n = stream.read(&mut request).unwrap_or(0);
            let line = String::from_utf8_lossy(&request[..n]).lines().next().unwrap_or("").to_string();
            let path = line.split(' ').nth(1).unwrap_or("").to_string();
            match files.iter().find(|(p, _)| *p == path) {
                Some((_, body)) => {
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = stream.write_all(body);
                }
                None => {
                    let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
    });
    base
}

fn asset_for(name: &str, bytes: &[u8]) -> Asset {
    Asset { name: name.into(), size: bytes.len() as u64, sha256: manifest::sha256_of(bytes).unwrap().0 }
}

fn tar_gz(path: &Path, member: &str, content: &[u8]) {
    let gz = flate2::write::GzEncoder::new(std::fs::File::create(path).unwrap(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gz);
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, member, content).unwrap();
    tar.into_inner().unwrap().finish().unwrap();
}

fn zip_of(path: &Path, member: &str, content: &[u8]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    zip.start_file(member, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(content).unwrap();
    zip.finish().unwrap();
}

#[test]
fn fetches_and_parses_latest_json() {
    let json = r#"{"commit":"abc","built_at":"2026-11-15T14:19:00Z","build":7,"assets":{"linux-x64":{"name":"a.tar.gz","size":3,"sha256":"aa00000000000000000000000000000000000000000000000000000000000000"}}}"#;
    let base = serve(vec![("/latest.json", json.as_bytes().to_vec())]);
    let m = install::fetch_manifest(&base).unwrap();
    assert_eq!(m.build, 7);
    assert_eq!(m.assets["linux-x64"].name, "a.tar.gz");
    assert!(install::fetch_manifest(&format!("{base}/nowhere")).is_err(), "a 404 is an error, not an offer");
}

#[test]
fn the_release_url_can_be_pointed_elsewhere() {
    assert_eq!(install::release_url(|_| None), install::RELEASE_URL);
    assert_eq!(install::release_url(|_| Some("http://127.0.0.1:9/x/".into())), "http://127.0.0.1:9/x");
    assert_eq!(install::release_url(|_| Some(String::new())), install::RELEASE_URL);
}

#[test]
fn downloads_with_progress_and_keeps_a_file_that_matches() {
    let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let base = serve(vec![("/app.bin", body.clone())]);
    let dir = Scratch::new("good");
    let mut seen = Vec::new();
    let file = install::download(&base, &asset_for("app.bin", &body), &dir.0, &mut |p| seen.push(p)).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), body);
    assert!(matches!(seen.last(), Some(Progress::Downloading(d, t)) if *d == body.len() as u64 && *t == body.len() as u64));
    assert!(seen.len() > 1, "progress arrives while it downloads");
}

#[test]
fn refuses_a_download_whose_checksum_is_wrong_and_throws_it_away() {
    let base = serve(vec![("/app.bin", b"tampered bytes".to_vec())]);
    let dir = Scratch::new("bad");
    let mut asset = asset_for("app.bin", b"tampered bytes");
    asset.sha256 = manifest::sha256_of(&b"the real bytes"[..]).unwrap().0;
    let err = install::download(&base, &asset, &dir.0, &mut |_| {}).unwrap_err();
    assert!(err.contains("checksum"), "{err}");
    assert!(!dir.0.join("app.bin").exists(), "a wrong file is not kept");
}

#[test]
fn refuses_a_download_of_the_wrong_size() {
    let base = serve(vec![("/app.bin", b"short".to_vec())]);
    let dir = Scratch::new("size");
    let mut asset = asset_for("app.bin", b"short");
    asset.size = 99;
    assert!(install::download(&base, &asset, &dir.0, &mut |_| {}).is_err());
    assert!(!dir.0.join("app.bin").exists());
}

#[test]
fn a_missing_file_is_a_failure_with_a_reason() {
    let base = serve(vec![]);
    let dir = Scratch::new("gone");
    let err = install::download(&base, &asset_for("app.bin", b"x"), &dir.0, &mut |_| {}).unwrap_err();
    assert!(err.starts_with("The download failed"), "{err}");
}

#[test]
fn unpacks_the_program_from_either_archive() {
    let dir = Scratch::new("archives");
    tar_gz(&dir.0.join("a.tar.gz"), "fleet-native", b"program");
    install::extract_tar_gz(&dir.0.join("a.tar.gz"), "fleet-native", &dir.0.join("out1")).unwrap();
    assert_eq!(std::fs::read(dir.0.join("out1")).unwrap(), b"program");
    assert!(install::extract_tar_gz(&dir.0.join("a.tar.gz"), "other", &dir.0.join("out2")).is_err());

    zip_of(&dir.0.join("a.zip"), "fleet-native.exe", b"program");
    install::extract_zip(&dir.0.join("a.zip"), "fleet-native.exe", &dir.0.join("out3")).unwrap();
    assert_eq!(std::fs::read(dir.0.join("out3")).unwrap(), b"program");
    assert!(install::extract_zip(&dir.0.join("a.zip"), "nope.exe", &dir.0.join("out4")).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn linux_replaces_the_program_in_place_and_keeps_its_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Scratch::new("linux");
    let exe = dir.0.join("fleet-native");
    std::fs::write(&exe, b"\x7fELF old").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o750)).unwrap();
    tar_gz(&dir.0.join("new.tar.gz"), "fleet-native", b"\x7fELF new");
    install::replace_unix(&dir.0.join("new.tar.gz"), &exe).unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF new");
    assert_eq!(std::fs::metadata(&exe).unwrap().permissions().mode() & 0o777, 0o750);
    let left: Vec<_> = std::fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(!left.iter().any(|n| n.starts_with(".fleet-native.update")), "no half-made file stays: {left:?}");
}

#[cfg(target_os = "linux")]
#[test]
fn linux_leaves_the_old_program_alone_when_the_archive_is_wrong() {
    let dir = Scratch::new("linux-bad");
    let exe = dir.0.join("fleet-native");
    std::fs::write(&exe, b"\x7fELF old").unwrap();
    tar_gz(&dir.0.join("empty.tar.gz"), "README.md", b"hello");
    assert!(install::replace_unix(&dir.0.join("empty.tar.gz"), &exe).is_err());
    tar_gz(&dir.0.join("text.tar.gz"), "fleet-native", b"#!/bin/sh");
    let err = install::replace_unix(&dir.0.join("text.tar.gz"), &exe).unwrap_err();
    assert!(err.contains("isn't a program"), "{err}");
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF old");
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 3, "only the old program and the two archives");
}

#[cfg(target_os = "linux")]
#[test]
fn the_whole_apply_over_http_replaces_the_program() {
    let dir = Scratch::new("apply");
    tar_gz(&dir.0.join("release.tar.gz"), "fleet-native", b"\x7fELF new build");
    let bytes = std::fs::read(dir.0.join("release.tar.gz")).unwrap();
    let asset = asset_for("fleet-native-linux-x64.tar.gz", &bytes);
    let base = serve(vec![("/fleet-native-linux-x64.tar.gz", bytes)]);
    let exe = dir.0.join("fleet-native");
    std::fs::write(&exe, b"\x7fELF old build").unwrap();
    let work = dir.0.join("work");
    let done = install::apply(&base, &asset, &exe, &work, &mut |_| {}).unwrap();
    assert!(matches!(done, install::Done::Relaunch(Relaunch::Exe(p)) if p == exe));
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF new build");
    assert!(!work.exists(), "the download is cleaned up");
}

#[cfg(target_os = "linux")]
#[test]
fn a_bad_checksum_over_http_leaves_the_program_untouched() {
    let dir = Scratch::new("apply-bad");
    tar_gz(&dir.0.join("release.tar.gz"), "fleet-native", b"\x7fELF new build");
    let bytes = std::fs::read(dir.0.join("release.tar.gz")).unwrap();
    let mut asset = asset_for("fleet-native-linux-x64.tar.gz", &bytes);
    asset.sha256 = "0".repeat(64);
    let base = serve(vec![("/fleet-native-linux-x64.tar.gz", bytes)]);
    let exe = dir.0.join("fleet-native");
    std::fs::write(&exe, b"\x7fELF old build").unwrap();
    assert!(install::apply(&base, &asset, &exe, &dir.0.join("work"), &mut |_| {}).is_err());
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF old build");
}

#[test]
fn finds_the_app_a_program_is_inside() {
    let bundle = Path::new("/Applications/Fleet Native.app");
    assert_eq!(install::bundle_of(&bundle.join("Contents/MacOS/fleet-native")), Some(bundle.to_path_buf()));
    assert_eq!(install::bundle_of(Path::new("/home/me/target/release/fleet-native")), None);
    assert_eq!(install::bundle_of(Path::new("/x/Thing.app/bin/fleet-native")), None, "only the Contents/MacOS layout");
}

/// A real disk image with a fake app in it, on the macOS runner: the replace swaps the bundle and leaves nothing behind.
#[cfg(target_os = "macos")]
#[test]
fn macos_swaps_the_app_bundle_from_a_disk_image() {
    use std::process::Command;
    let dir = Scratch::new("mac");
    let make_app = |root: &Path, marker: &str| {
        let macos = root.join("Fleet Native.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::write(macos.join("fleet-native"), marker).unwrap();
    };
    let installed = dir.0.join("Applications");
    make_app(&installed, "old");
    let source = dir.0.join("image");
    make_app(&source, "new");
    let dmg = dir.0.join("new.dmg");
    let made = Command::new("hdiutil")
        .args(["create", "-quiet", "-volname", "Fleet Native", "-fs", "HFS+", "-srcfolder"])
        .arg(&source)
        .arg(&dmg)
        .status()
        .unwrap();
    assert!(made.success());
    let bundle = installed.join("Fleet Native.app");
    let result = install::replace_bundle(&dmg, &bundle, &dir.0.join("work")).unwrap();
    assert!(matches!(result, install::MacResult::Replaced));
    assert_eq!(std::fs::read(bundle.join("Contents/MacOS/fleet-native")).unwrap(), b"new");
    let left: Vec<_> = std::fs::read_dir(&installed).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(left, ["Fleet Native.app"], "no staged or old copy stays");
}

#[test]
fn only_a_newer_build_for_this_platform_makes_an_offer() {
    let json = r#"{"commit":"abc","built_at":"2026-11-15T14:19:00Z","build":200,"assets":{"linux-x64":{"name":"a.tar.gz","size":3,"sha256":"aa00000000000000000000000000000000000000000000000000000000000000"}}}"#;
    let found = manifest::parse(json).unwrap();
    let mut s = State::default();
    s.update_found_for(Some(200), Some("linux-x64"), &found);
    assert!(s.update.available.is_none(), "the same build");
    s.update_found_for(None, Some("linux-x64"), &found);
    assert!(s.update.available.is_none(), "a dev build");
    s.update_found_for(Some(100), Some("macos-arm64"), &found);
    assert!(s.update.available.is_none(), "no asset for this platform");
    s.update_found_for(Some(100), Some("linux-x64"), &found);
    let offer = s.update.available.clone().unwrap();
    assert_eq!((offer.build, offer.date.as_str()), (200, "15 November 2026"));
}

#[test]
fn the_status_bar_item_opens_the_dialog_and_not_now_closes_it() {
    let mut rt = app();
    rt.frame(FRAME);
    assert!(rt.find("update-item").is_none(), "nothing shows until a newer build is known");
    rt.update(|s, _| crate::mock::update::show(s, crate::mock::update::Case::Item));
    rt.frame(FRAME);
    assert!(rt.find("update-item").is_some() && rt.find("update-dialog").is_none());
    click(&mut rt, "update-item");
    rt.frame(FRAME);
    assert!(rt.state.update.dialog && rt.find("update-dialog").is_some());
    click(&mut rt, "update-later");
    rt.frame(FRAME);
    assert!(!rt.state.update.dialog && rt.find("update-dialog").is_none());
    assert!(rt.find("update-item").is_some(), "the item stays until the app is updated");
}

#[test]
fn a_running_download_cannot_be_closed_and_a_failure_can_be_retried() {
    let mut rt = app();
    rt.update(|s, _| crate::mock::update::show(s, crate::mock::update::Case::Downloading));
    rt.frame(FRAME);
    assert!(rt.find("update-progress").is_some());
    rt.update(|s, _| s.update_close());
    assert!(rt.state.update.dialog, "closing is refused mid-download");
    rt.update(|s, _| crate::mock::update::show(s, crate::mock::update::Case::Failed));
    rt.frame(FRAME);
    assert!(rt.find("update-failed").is_some());
    rt.update(|s, _| s.update_close());
    assert!(!rt.state.update.dialog && rt.state.update.phase == Phase::Idle);
}

#[test]
fn update_and_restart_needs_a_way_to_reach_the_state() {
    let mut u = Update::default();
    assert!(!u.busy());
    u.phase = Phase::Installing;
    assert!(u.busy());
    // No poster (a picture, a test): the button does nothing instead of starting a thread nobody hears.
    let mut s = State::default();
    crate::mock::update::show(&mut s, crate::mock::update::Case::Dialog);
    s.update_start();
    assert_eq!(s.update.phase, Phase::Idle);
}

#[test]
fn the_drafts_are_on_disk_before_the_app_restarts() {
    let dir = Scratch::new("flush");
    let mut rt = app();
    let folder = dir.0.clone();
    rt.update(move |s, _| {
        s.adopt_drafts(folder, Default::default());
        s.draft = rune::TextEdit::new("half a thought");
        // A session the file keeps (the invented ones are never written down).
        s.drafts.write("session.s-real", "typed a moment ago", s.clock);
    });
    rt.frame(FRAME);
    // The save is normally a moment after typing, on a thread. Restarting can't wait for it.
    rt.update(|s, tx| s.update_flush(tx));
    let file = std::fs::read_to_string(dir.0.join(crate::composer::drafts::FILE)).unwrap();
    assert!(file.contains("typed a moment ago"), "{file}");
    assert_eq!(rt.state.drafts.text_of("mock.0.0"), "half a thought", "the text area's own text is put away first");
}
