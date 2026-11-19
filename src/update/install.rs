//! Fetching `latest.json`, downloading the asset for this platform, checking it, and putting it in the running app's place.
//! All of it blocks: it runs on a thread (`update::mod`), never in a view, a handler or `poll`.
//!
//! The rule for every platform: the old app is only touched by the last step, one rename (Windows: `self-replace`'s
//! swap), and anything that fails before it leaves the old app exactly as it was.
use super::manifest::{Asset, Manifest, parse, sha256_of};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Where the release's files are: the `latest` pre-release. `FLEET_NATIVE_UPDATE_URL` points the updater at another folder
/// that holds the same files (a test serves a fake release from one).
pub const RELEASE_URL: &str = "https://github.com/pgermishuys/fleet-native/releases/download/latest";

pub fn release_url(get: impl Fn(&str) -> Option<String>) -> String {
    get("FLEET_NATIVE_UPDATE_URL").filter(|u| !u.is_empty()).unwrap_or_else(|| RELEASE_URL.to_string()).trim_end_matches('/').to_string()
}

fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(seconds))).build().into()
}

/// `latest.json` of the release at `base`.
pub fn fetch_manifest(base: &str) -> Result<Manifest, String> {
    let mut response = agent(20).get(format!("{base}/latest.json")).call().map_err(|e| e.to_string())?;
    parse(&response.body_mut().read_to_string().map_err(|e| e.to_string())?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Bytes so far and in all.
    Downloading(u64, u64),
    Installing,
}

/// Downloads `asset` into `dir`, hashing as it goes, and refuses a file whose size or SHA-256 isn't the manifest's. The
/// file is only kept when it matches, so a half or a wrong download never reaches an installer.
pub fn download(base: &str, asset: &Asset, dir: &Path, progress: &mut dyn FnMut(Progress)) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Couldn't make a folder for the download: {e}"))?;
    let path = dir.join(&asset.name);
    let response = agent(30 * 60).get(format!("{base}/{}", asset.name)).call().map_err(|e| format!("The download failed: {e}"))?;
    let mut body = response.into_body().into_reader();
    let mut out = File::create(&path).map_err(|e| format!("Couldn't write the download: {e}"))?;
    let (mut hash, mut buf, mut done) = (Sha256::new(), vec![0u8; 64 * 1024], 0u64);
    let result = (|| {
        loop {
            let n = body.read(&mut buf).map_err(|e| format!("The download was cut off: {e}"))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| format!("Couldn't write the download: {e}"))?;
            done += n as u64;
            progress(Progress::Downloading(done, asset.size));
        }
        out.flush().map_err(|e| e.to_string())?;
        let sum: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if done != asset.size || sum != asset.sha256 {
            return Err(format!(
                "The download doesn't match its checksum, so it was thrown away (expected {}, got {}).",
                &asset.sha256[..12],
                &sum[..12]
            ));
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&path);
        return Err(e);
    }
    Ok(path)
}

/// Whether `file` is the checksum `want`: used by tests and by `--update-now` to say what it did.
pub fn checksum_matches(file: &Path, want: &str) -> bool {
    File::open(file).and_then(sha256_of).is_ok_and(|(sum, _)| sum == want)
}

/// The file called `member` (at any depth) from a `.tar.gz`, written to `to`.
pub fn extract_tar_gz(archive: &Path, member: &str, to: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries().map_err(|e| format!("The archive can't be read: {e}"))? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        if entry.header().entry_type().is_file() && entry.path().ok().and_then(|p| p.file_name().map(|n| n == member)).unwrap_or(false) {
            let mut out = File::create(to).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("Couldn't unpack {member}: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("The download has no {member} in it."))
}

/// The file called `member` from a `.zip`, written to `to`.
pub fn extract_zip(archive: &Path, member: &str, to: &Path) -> Result<(), String> {
    let mut zip =
        zip::ZipArchive::new(File::open(archive).map_err(|e| e.to_string())?).map_err(|e| format!("The archive can't be read: {e}"))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_file() && Path::new(entry.name()).file_name().is_some_and(|n| n == member) {
            let mut out = File::create(to).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("Couldn't unpack {member}: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("The download has no {member} in it."))
}

/// What a program of this system starts with, so an archive with something else in it is refused before it replaces the app.
fn looks_like_program(file: &Path) -> bool {
    let mut head = [0u8; 4];
    if File::open(file).and_then(|mut f| f.read_exact(&mut head)).is_err() {
        return false;
    }
    if cfg!(windows) {
        head.starts_with(b"MZ")
    } else if cfg!(target_os = "macos") {
        head.len() == 4
    } else {
        head == *b"\x7fELF"
    }
}

/// Linux: the new program replaces the running one with a rename, which the system allows for a program that is running (the
/// old file lives on until the process ends). It is written next to it first, so the rename never crosses a disk, and
/// given the old one's permissions. Any failure removes the half-made file and leaves the old program alone.
pub fn replace_unix(archive: &Path, exe: &Path) -> Result<(), String> {
    let dir = exe.parent().ok_or("The app's folder is unknown.")?;
    let staged = dir.join(format!(".fleet-native.update-{}", std::process::id()));
    let result = (|| {
        extract_tar_gz(archive, "fleet-native", &staged)
            .map_err(|e| if e.contains("denied") { format!("The folder {} isn't writable ({e}).", dir.display()) } else { e })?;
        if !looks_like_program(&staged) {
            return Err("The download isn't a program for this system.".to_string());
        }
        let permissions = fs::metadata(exe).map_err(|e| e.to_string())?.permissions();
        fs::set_permissions(&staged, permissions).map_err(|e| e.to_string())?;
        fs::rename(&staged, exe).map_err(|e| format!("Couldn't put the new app in place: {e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

/// Windows: unpack the exe and let `self-replace` swap it for the running one (it renames the running exe aside, puts the new
/// one in its place, and a helper deletes the old file after this process ends).
pub fn replace_windows(archive: &Path, work: &Path) -> Result<(), String> {
    let staged = work.join("fleet-native-new.exe");
    extract_zip(archive, "fleet-native.exe", &staged)?;
    if !looks_like_program(&staged) {
        let _ = fs::remove_file(&staged);
        return Err("The download isn't a program for this system.".to_string());
    }
    #[cfg(windows)]
    let result =
        while_locked(|| self_replace::self_replace(&staged), SWAP_TRIES, SWAP_PAUSE).map_err(|e| format!("Couldn't swap the app: {e}"));
    #[cfg(not(windows))]
    let result: Result<(), String> = Err("This is not Windows.".to_string());
    let _ = fs::remove_file(&staged);
    result
}

/// How long a swap Windows refuses because a file is in use is tried again: about five seconds in all.
#[cfg_attr(not(windows), allow(dead_code))]
const SWAP_TRIES: u32 = 20;
#[cfg_attr(not(windows), allow(dead_code))]
const SWAP_PAUSE: Duration = Duration::from_millis(250);

/// Runs `attempt` until it succeeds, fails for another reason, or `tries` run out. On Windows a file just written or just run
/// is often held for a moment (a virus scanner reading the new exe, the process that just exited), and the swap is then
/// refused as in use (os error 32) or denied (5), though a second later it goes through; CI's updater check failed that way.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn while_locked<T>(mut attempt: impl FnMut() -> std::io::Result<T>, tries: u32, pause: Duration) -> std::io::Result<T> {
    let mut left = tries.max(1);
    loop {
        match attempt() {
            Err(e) if left > 1 && held(&e) => {
                left -= 1;
                std::thread::sleep(pause);
            }
            result => return result,
        }
    }
}

/// Windows' "access denied", "in use by another process" and "locked": what a file held for a moment says.
#[cfg_attr(not(windows), allow(dead_code))]
fn held(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// The `.app` that holds the program at `exe` (`X.app/Contents/MacOS/name`), if it is in one.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let app = exe.parent()?.parent()?.parent()?;
    let layout = exe.parent()?.file_name()? == "MacOS" && exe.parent()?.parent()?.file_name()? == "Contents";
    (layout && app.extension().is_some_and(|e| e == "app")).then(|| app.to_path_buf())
}

/// What the Mac update came to: the app replaced, or the reason it couldn't be (the `.dmg` is then opened for a drag).
pub enum MacResult {
    Replaced,
    Manual(String),
}

/// macOS: mount the `.dmg`, copy its `Fleet Native.app` next to the running one, clear the quarantine mark, then swap the two
/// by rename. If the folder can't be written the app says so and opens the `.dmg` for a drag instead.
pub fn replace_bundle(dmg: &Path, bundle: &Path, work: &Path) -> Result<MacResult, String> {
    let parent = bundle.parent().ok_or("The app's folder is unknown.")?;
    let name = bundle.file_name().ok_or("The app has no name.")?.to_string_lossy().to_string();
    let staged = parent.join(format!(".{name}.new"));
    let old = parent.join(format!(".{name}.old"));
    let _ = fs::remove_dir_all(&staged);
    if fs::create_dir(&staged).is_err() {
        return Ok(MacResult::Manual(format!("{} can't be written to by this app.", parent.display())));
    }
    let _ = fs::remove_dir(&staged);
    let mount = work.join("mount");
    fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    run(
        Command::new("hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"]).arg(&mount).arg(dmg),
        "mount the disk image",
    )?;
    let copied = (|| {
        let source = mount.join("Fleet Native.app");
        if !source.join("Contents/MacOS/fleet-native").is_file() {
            return Err("The disk image has no Fleet Native.app in it.".to_string());
        }
        run(Command::new("ditto").arg(&source).arg(&staged), "copy the new app")
    })();
    let _ = Command::new("hdiutil").args(["detach", "-force"]).arg(&mount).stdout(Stdio::null()).stderr(Stdio::null()).status();
    if let Err(e) = copied {
        let _ = fs::remove_dir_all(&staged);
        return Err(e);
    }
    // A download the app made itself carries no quarantine mark, but a disk image opened by hand might; the new bundle must not.
    let _ = Command::new("xattr").args(["-dr", "com.apple.quarantine"]).arg(&staged).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let _ = fs::remove_dir_all(&old);
    fs::rename(bundle, &old).map_err(|e| {
        let _ = fs::remove_dir_all(&staged);
        format!("Couldn't move the old app aside: {e}")
    })?;
    if let Err(e) = fs::rename(&staged, bundle) {
        let _ = fs::rename(&old, bundle);
        let _ = fs::remove_dir_all(&staged);
        return Err(format!("Couldn't put the new app in place, the old one was kept: {e}"));
    }
    let _ = fs::remove_dir_all(&old);
    Ok(MacResult::Replaced)
}

fn run(cmd: &mut Command, what: &str) -> Result<(), String> {
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("Couldn't {what}: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(format!("Couldn't {what}: {}", String::from_utf8_lossy(&out.stderr).trim())) }
}

/// How to start the new app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Relaunch {
    Exe(PathBuf),
    /// A macOS bundle, started with `open`.
    App(PathBuf),
}

/// Starts the new app with the arguments the old one had (`--server …`). The caller exits afterwards.
pub fn relaunch(how: &Relaunch, args: &[String]) -> std::io::Result<()> {
    let mut cmd = match how {
        Relaunch::Exe(path) => {
            let mut c = Command::new(path);
            c.args(args);
            c
        }
        Relaunch::App(bundle) => {
            let mut c = Command::new("open");
            c.arg("-n").arg(bundle).arg("--args").args(args);
            c
        }
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| ())
}

pub enum Done {
    Relaunch(Relaunch),
    /// The `.dmg` is downloaded and opened for the user to drag; the reason it isn't automatic.
    Manual {
        dmg: PathBuf,
        why: String,
    },
}

/// Downloads, checks and installs `asset`; the whole update for this platform. `exe` is the running program.
pub fn apply(base: &str, asset: &Asset, exe: &Path, work: &Path, progress: &mut dyn FnMut(Progress)) -> Result<Done, String> {
    let file = download(base, asset, work, progress)?;
    progress(Progress::Installing);
    let done = if cfg!(windows) {
        replace_windows(&file, work)?;
        Done::Relaunch(Relaunch::Exe(exe.to_path_buf()))
    } else if cfg!(target_os = "macos") {
        let Some(bundle) = bundle_of(exe) else {
            let why = "This copy isn't running from a Fleet Native.app, so it can't replace itself.".to_string();
            return keep_for_drag(file, why);
        };
        match replace_bundle(&file, &bundle, work)? {
            MacResult::Replaced => Done::Relaunch(Relaunch::App(bundle)),
            MacResult::Manual(why) => return keep_for_drag(file, why),
        }
    } else {
        replace_unix(&file, exe)?;
        Done::Relaunch(Relaunch::Exe(exe.to_path_buf()))
    };
    let _ = fs::remove_dir_all(work);
    Ok(done)
}

/// Opens the downloaded `.dmg` in Finder, for the user to drag the app over the old one.
fn keep_for_drag(dmg: PathBuf, why: String) -> Result<Done, String> {
    let _ = Command::new("open").arg(&dmg).status();
    let why = format!("{why} The disk image is open: drag Fleet Native over the old one in Applications.");
    Ok(Done::Manual { dmg, why })
}
