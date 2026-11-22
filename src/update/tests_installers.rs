//! Piece F of component 25: the updater for installers. The modes (Fleet's `updateMode`), the notify mode of a `.deb`, the
//! whole app folder replaced with the server in it, an AppImage replaced as one file, the size said before the download, and
//! the off switch. Fleet's `updates.test.ts` (13 cases) is accounted for here, case by case, under native names.
use super::install;
use super::manifest::{self, Kind};
// Most of the folder tests run on Linux only (they make Unix programs), so the helpers they use are unused elsewhere.
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
use super::tests::{Scratch, asset_for, serve, tar_gz_of, zip_files};
use super::{Mode, Phase, mode};
use crate::State;
use crate::tests::{FRAME as FRAME_RUN, app, click};
use std::path::Path;

const KEY: Option<&str> = Some("linux-x64");

fn manifest_with(key: &str, name: &str, build: u64) -> manifest::Manifest {
    let json = format!(
        r#"{{"commit":"abc","built_at":"2026-11-15T14:19:00Z","build":{build},"assets":{{"{key}":{{"name":"{name}","size":131400000,"sha256":"{}"}}}}}}"#,
        "a".repeat(64)
    );
    manifest::parse(&json).unwrap()
}

// ─── updates.test.ts "update mode" ──────────────────────────────────────────────

#[test]
fn installs_on_windows_and_from_an_appimage() {
    let image = Kind::AppImage("/home/u/Fleet-Native.AppImage".into());
    assert_eq!(mode(Some(5), None, Some("windows-x64"), &Kind::Folder), Mode::Install);
    assert_eq!(mode(Some(5), None, Some("linux-x64-appimage"), &image), Mode::Install);
    assert_eq!(mode(Some(5), None, KEY, &Kind::Folder), Mode::Install, "an unpacked tar replaces its folder");
}

#[test]
fn only_tells_you_about_a_new_version_from_the_deb_and_macos_installs() {
    assert_eq!(mode(Some(5), None, Some("linux-x64-deb"), &Kind::Deb), Mode::Notify);
    // Fleet only links on macOS because its build is unsigned and electron-updater can't install into it. This app swaps its
    // own bundle by rename (and opens the disk image for a drag when it can't write the folder), so it installs.
    assert_eq!(mode(Some(5), None, Some("macos-arm64"), &Kind::Folder), Mode::Install);
}

#[test]
fn is_off_in_development_and_when_turned_off() {
    assert_eq!(mode(None, None, KEY, &Kind::Folder), Mode::Off, "a dev build has no build number");
    assert_eq!(mode(Some(5), Some("off"), KEY, &Kind::Folder), Mode::Off);
    assert_eq!(mode(Some(5), Some(" OFF "), Some("linux-x64-deb"), &Kind::Deb), Mode::Off, "off beats notify");
    assert_eq!(mode(Some(5), Some("on"), KEY, &Kind::Folder), Mode::Install, "only `off` turns it off");
    assert_eq!(mode(Some(5), None, None, &Kind::Folder), Mode::Off, "no download is published for this system");
}

// ─── updates.test.ts "update state" ─────────────────────────────────────────────

#[test]
fn downloads_a_new_version_where_it_can_install_it() {
    let mut s = State::default();
    s.update_found_for(Some(100), Some(("linux-x64", Mode::Install)), "http://127.0.0.1:1", &manifest_with("linux-x64", "a.tar.gz", 200));
    let offer = s.update.available.clone().unwrap();
    assert_eq!((offer.build, offer.link), (200, None), "nothing to open by hand: the app downloads it itself");
    assert!(!s.update.notify_only());
}

#[test]
fn links_to_the_release_where_it_cant_install() {
    let mut s = State::default();
    let found = manifest_with("linux-x64-deb", "fleet-native-linux-x64.deb", 200);
    s.update_found_for(Some(100), Some(("linux-x64-deb", Mode::Notify)), "http://127.0.0.1:1", &found);
    let offer = s.update.available.clone().unwrap();
    assert_eq!(offer.link.as_deref(), Some("http://127.0.0.1:1/fleet-native-linux-x64.deb"));
    assert!(s.update.notify_only());
    s.update_start();
    assert_eq!(s.update.phase, Phase::Idle, "notify mode never downloads or replaces");
    let off = Some(("linux-x64-deb", Mode::Off));
    let mut quiet = State::default();
    quiet.update_found_for(Some(100), off, "http://127.0.0.1:1", &found);
    assert!(quiet.update.available.is_none(), "an off app offers nothing");
}

#[test]
fn keeps_a_found_update_through_later_checks_and_errors() {
    let mut s = State::default();
    let install = Some(("linux-x64", Mode::Install));
    s.update_found_for(Some(100), install, "http://x", &manifest_with("linux-x64", "a.tar.gz", 200));
    s.update_checked(5, Err("offline".into()));
    assert_eq!(s.update.available.as_ref().map(|a| a.build), Some(200), "a failed look keeps the offer");
    assert_eq!(s.update.check_error.as_deref(), Some("offline"));
    s.update.checking = true;
    assert_eq!(s.update.available.as_ref().map(|a| a.build), Some(200), "and so does a look under way");
}

#[test]
fn stays_the_same_when_a_check_finds_the_build_it_is_already_downloading() {
    // Native has no "ready" resting state (it installs and restarts); the matching rule is that a look during a download
    // doesn't restart or replace it.
    let mut s = State::default();
    let install = Some(("linux-x64", Mode::Install));
    let found = manifest_with("linux-x64", "a.tar.gz", 200);
    s.update_found_for(Some(100), install, "http://x", &found);
    s.update.phase = Phase::Downloading(40, 100);
    s.update_found_for(Some(100), install, "http://x", &manifest_with("linux-x64", "b.tar.gz", 201));
    assert_eq!(s.update.available.as_ref().map(|a| a.asset.name.as_str()), Some("a.tar.gz"));
    assert_eq!(s.update.phase, Phase::Downloading(40, 100));
}

#[test]
fn reports_a_failed_check() {
    let mut s = State::default();
    s.update.running = Some(100);
    s.update.checking = true;
    s.update_checked(5, Err("The release isn't reachable".into()));
    assert!(!s.update.checking);
    let row = crate::fleet::settings::updates_page::app_row(&s.update, 10, 0);
    assert_eq!((row.label.as_str(), row.error.as_deref()), ("Update check failed", Some("The release isn't reachable")));
}

#[test]
fn goes_back_to_idle_when_theres_nothing_new() {
    let mut s = State::default();
    s.update.running = Some(200);
    s.update.check_error = Some("old failure".into());
    s.update_checked(5, Ok(manifest_with("linux-x64", "a.tar.gz", 200)));
    assert!(s.update.available.is_none() && s.update.check_error.is_none());
    let row = crate::fleet::settings::updates_page::app_row(&s.update, 10, 0);
    assert_eq!(row.label, "Up to date");
}

#[test]
fn ignores_progress_when_nothing_is_downloading() {
    let mut s = State::default();
    s.update_progress(install::Progress::Downloading(10, 100));
    assert_eq!(s.update.phase, Phase::Idle);
    s.update.phase = Phase::Failed("x".into());
    s.update_progress(install::Progress::Downloading(10, 100));
    assert_eq!(s.update.phase, Phase::Failed("x".into()), "a late step after a failure changes nothing");
    s.update.phase = Phase::Downloading(0, 100);
    s.update_progress(install::Progress::Downloading(50, 100));
    assert_eq!(s.update.phase, Phase::Downloading(50, 100));
    s.update_progress(install::Progress::Installing);
    assert_eq!(s.update.phase, Phase::Installing);
}

fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

// ─── The off switch, the .deb's words, the size ─────────────────────────────────

#[test]
fn an_update_turned_off_says_so_on_the_updates_page_and_keeps_its_build() {
    let mut s = State::default();
    s.update.running = Some(210);
    s.update.off = true;
    let row = crate::fleet::settings::updates_page::app_row(&s.update, 10, 0);
    assert_eq!((row.build.as_str(), row.label.as_str()), ("build 210", "This build doesn't update itself"));
    assert!(!row.can_check && !row.can_update);
}

#[test]
fn the_size_is_said_in_whole_megabytes() {
    use crate::fleet::settings::updates_page::about_size;
    assert_eq!(about_size(131_400_000), "about 131 MB");
    assert_eq!(about_size(130_500_000), "about 131 MB");
    assert_eq!(about_size(499_999), "under 1 MB");
    assert_eq!(about_size(0), "under 1 MB");
}

#[test]
fn the_dialog_says_the_size_before_it_downloads_and_the_deb_one_only_links() {
    let mut rt = app();
    rt.update(|s, _| crate::mock::update::show(s, crate::mock::update::Case::Dialog));
    rt.frame(FRAME_RUN);
    assert!(words(&rt).contains("Download: about 131 MB."), "{}", words(&rt));
    assert!(words(&rt).contains("Update and restart"));

    rt.update(|s, _| crate::mock::update::show(s, crate::mock::update::Case::Deb));
    rt.frame(FRAME_RUN);
    assert!(words(&rt).contains("Download the new .deb") && !words(&rt).contains("Update and restart"), "{}", words(&rt));
    click(&mut rt, "update-go");
    rt.frame(FRAME_RUN);
    assert_eq!(rt.state.update.phase, Phase::Idle, "the button opens the link, nothing is downloaded");
}

#[test]
fn a_deb_install_cant_be_applied_even_when_asked() {
    let dir = Scratch::new("deb");
    let err = install::apply(
        "http://127.0.0.1:1",
        &asset_for("x.deb", b"x"),
        &Kind::Deb,
        &dir.0.join("fleet-native"),
        &dir.0,
        &mut || {},
        &mut |_| {},
    )
    .err()
    .unwrap();
    assert!(err.contains(".deb"), "{err}");
}

// ─── The whole app folder, server included ─────────────────────────────────────

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn old_install(dir: &Path) -> std::path::PathBuf {
    let exe = dir.join("fleet-native");
    std::fs::write(&exe, b"\x7fELF old").unwrap();
    std::fs::create_dir_all(dir.join("fleet/lib")).unwrap();
    std::fs::write(dir.join("fleet/Fleet"), b"server 1").unwrap();
    std::fs::write(dir.join("fleet/lib/gone-in-the-new-one.dll"), b"x").unwrap();
    std::fs::write(dir.join("README.md"), b"old readme").unwrap();
    std::fs::write(dir.join("mine.txt"), b"the user's own file").unwrap();
    exe
}

#[cfg(target_os = "linux")]
#[test]
fn an_update_replaces_the_app_and_its_server_together() {
    let dir = Scratch::new("folder");
    let exe = old_install(&dir.0);
    tar_gz_of(
        &dir.0.join("new.tar.gz"),
        &[("./fleet-native", b"\x7fELF new"), ("./fleet/Fleet", b"server 2"), ("./README.md", b"new readme")],
    );
    let mut stopped = 0;
    install::replace_folder(&dir.0.join("new.tar.gz"), &exe, &mut || stopped += 1).unwrap();
    assert_eq!(stopped, 1, "the Fleet this app started is stopped once, before anything is moved");
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF new");
    assert_eq!(std::fs::read(dir.0.join("fleet/Fleet")).unwrap(), b"server 2", "the server is the new one");
    assert!(!dir.0.join("fleet/lib").exists(), "a file the new server doesn't have is gone, not mixed in");
    assert_eq!(std::fs::read(dir.0.join("README.md")).unwrap(), b"new readme");
    assert_eq!(std::fs::read(dir.0.join("mine.txt")).unwrap(), b"the user's own file", "a user's file stays");
    let mut left: Vec<_> = std::fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    left.sort();
    assert_eq!(left, ["README.md", "fleet", "fleet-native", "mine.txt", "new.tar.gz"], "no staged or old copy stays");
}

#[cfg(target_os = "linux")]
#[test]
fn an_update_that_fails_midway_puts_the_old_app_and_server_back() {
    let dir = Scratch::new("folder-undo");
    let exe = old_install(&dir.0);
    tar_gz_of(&dir.0.join("new.tar.gz"), &[("./fleet-native", b"\x7fELF new"), ("./fleet/Fleet", b"server 2")]);
    // The running program can't be put in place (it is gone): the entries moved before it must come back.
    std::fs::remove_file(&exe).unwrap();
    assert!(install::replace_folder(&dir.0.join("new.tar.gz"), &exe, &mut || {}).is_err());
    assert_eq!(std::fs::read(dir.0.join("fleet/Fleet")).unwrap(), b"server 1");
    assert!(dir.0.join("fleet/lib/gone-in-the-new-one.dll").exists());
    assert_eq!(std::fs::read(dir.0.join("README.md")).unwrap(), b"old readme");
    assert!(!std::fs::read_dir(&dir.0).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with('.')), "nothing hidden stays");
}

#[cfg(target_os = "linux")]
#[test]
fn a_bad_package_stops_nothing_and_changes_nothing() {
    let dir = Scratch::new("folder-bad");
    let exe = old_install(&dir.0);
    tar_gz_of(&dir.0.join("text.tar.gz"), &[("fleet-native", b"#!/bin/sh"), ("fleet/Fleet", b"server 2")]);
    let mut stopped = 0;
    assert!(install::replace_folder(&dir.0.join("text.tar.gz"), &exe, &mut || stopped += 1).is_err());
    assert_eq!(stopped, 0, "the server keeps running when the download is refused");
    assert_eq!(std::fs::read(dir.0.join("fleet/Fleet")).unwrap(), b"server 1");
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF old");
}

/// The Windows package is a zip with the same layout; the swap is the same code (the exe step is `self-replace` there).
#[cfg(target_os = "linux")]
#[test]
fn a_zip_package_replaces_the_folder_the_same_way() {
    let dir = Scratch::new("folder-zip");
    let exe = old_install(&dir.0);
    zip_files(&dir.0.join("new.zip"), &[("fleet-native", b"\x7fELF zipped"), ("fleet/Fleet", b"server 3")]);
    install::replace_folder(&dir.0.join("new.zip"), &exe, &mut || {}).unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"\x7fELF zipped");
    assert_eq!(std::fs::read(dir.0.join("fleet/Fleet")).unwrap(), b"server 3");
}

#[cfg(target_os = "linux")]
#[test]
fn an_appimage_is_replaced_as_one_file_and_started_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Scratch::new("appimage");
    let image = dir.0.join("Fleet-Native.AppImage");
    std::fs::write(&image, b"\x7fELF old image").unwrap();
    std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o755)).unwrap();
    let new = b"\x7fELF new image".to_vec();
    let base = serve(vec![("/fleet-native-linux-x64.AppImage", new.clone())]);
    let asset = asset_for("fleet-native-linux-x64.AppImage", &new);
    let mut stopped = 0;
    let done = install::apply(
        &base,
        &asset,
        &Kind::AppImage(image.clone()),
        Path::new("/tmp/.mount_x/usr/bin/fleet-native"),
        &dir.0.join("work"),
        &mut || stopped += 1,
        &mut |_| {},
    )
    .unwrap();
    assert!(
        matches!(done, install::Done::Relaunch(install::Relaunch::Exe(p)) if p == image),
        "the image starts again, not the mounted copy"
    );
    assert_eq!(std::fs::read(&image).unwrap(), new);
    assert_eq!(std::fs::metadata(&image).unwrap().permissions().mode() & 0o777, 0o755);
    assert_eq!(stopped, 1);
    assert!(!std::fs::read_dir(&dir.0).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".fleet-native.update")));
}

#[cfg(target_os = "linux")]
#[test]
fn an_appimage_that_isnt_a_program_leaves_the_old_one() {
    let dir = Scratch::new("appimage-bad");
    let image = dir.0.join("Fleet-Native.AppImage");
    std::fs::write(&image, b"\x7fELF old image").unwrap();
    std::fs::write(dir.0.join("download"), b"<html>not found</html>").unwrap();
    let err = install::replace_appimage(&dir.0.join("download"), &image, &mut || {}).unwrap_err();
    assert!(err.contains("isn't a program"), "{err}");
    assert_eq!(std::fs::read(&image).unwrap(), b"\x7fELF old image");
}

#[test]
fn the_fleet_app_row_of_a_deb_install_offers_the_download_not_the_restart() {
    let mut s = State::default();
    s.show_updates(true, crate::mock::updates_page::Case::Deb);
    let row = crate::fleet::settings::updates_page::app_row(&s.update, 10, 0);
    assert_eq!((row.label.as_str(), row.update_label), ("Build 214 is available", "Download the new .deb"));
    assert!(row.can_update);
    let note = row.note.unwrap();
    assert!(note.contains(".deb") && note.contains("about 131 MB"), "{note}");
}
