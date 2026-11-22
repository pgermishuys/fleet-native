//! The updater. A thread asks the `latest` release's `latest.json` for the newest build, on start and every few hours; a
//! newer one than the running build puts "Update available · build N" in the status bar (`view::update`). Its dialog
//! downloads the asset for this platform, checks its SHA-256, replaces the running app and starts the new one (`install`).
//! Nothing here runs on the UI thread except small state changes the threads post.
pub mod install;
pub mod manifest;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_installers;

use crate::State;
use crate::fleet::link::Post;
use install::{Done, Progress, Relaunch};
use manifest::{Asset, Kind, Manifest};
use rune::{AppHandle, Tx};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long after a good answer the next check is.
const EVERY: Duration = Duration::from_secs(4 * 60 * 60);
/// How long after a failed one. A network failure is silent; this is only when it tries again.
const RETRY: Duration = Duration::from_secs(20 * 60);

/// What this install does with a newer build (`updateMode` in Fleet's `updates.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Downloads it, checks it and replaces the app folder, server included.
    Install,
    /// Says it is out and links to the download: a `.deb` can't replace itself (`/opt` is the package manager's).
    Notify,
    /// Never looks: a `dev` build, a platform with no download, or `FLEET_NATIVE_UPDATES=off`.
    Off,
}

/// `build` is the running build (`None`: a `dev` one), `off` the value of `FLEET_NATIVE_UPDATES`, `key` this install's asset
/// key. Unlike Fleet's, macOS installs: the app replaces its own signed-ad-hoc bundle when it can write the folder.
pub fn mode(build: Option<u64>, off: Option<&str>, key: Option<&str>, kind: &Kind) -> Mode {
    if build.is_none() || key.is_none() || off.is_some_and(|v| v.trim().eq_ignore_ascii_case("off")) {
        Mode::Off
    } else if *kind == Kind::Deb {
        Mode::Notify
    } else {
        Mode::Install
    }
}

/// The mode of the running app.
pub fn current_mode() -> Mode {
    mode(
        manifest::running_build(),
        std::env::var("FLEET_NATIVE_UPDATES").ok().as_deref(),
        manifest::platform_key(),
        &manifest::install_kind(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Available {
    pub build: u64,
    /// "15 November 2026".
    pub date: String,
    pub asset: Asset,
    /// Notify mode: where to download the package by hand.
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// Bytes so far and in all.
    Downloading(u64, u64),
    Installing,
    /// The update failed; the old app is running untouched. The reason is shown in the dialog.
    Failed(String),
    /// Something for the user to finish by hand (a Mac whose app folder can't be written gets the disk image opened for a
    /// drag; a new app that didn't start). Why.
    Manual {
        why: String,
    },
}

#[derive(Default)]
pub struct Update {
    pub available: Option<Available>,
    /// The dialog is open.
    pub dialog: bool,
    pub phase: Phase,
    /// How the threads reach the state; set when the app starts the checker. Without it (tests, pictures) nothing is fetched.
    post: Option<Post>,
    /// The build this app is, once the checker runs (`None`: a `dev` build or a platform with no download, which never checks).
    pub running: Option<u64>,
    /// `FLEET_NATIVE_UPDATES=off`: the build is known but nothing is checked.
    pub off: bool,
    /// A look at `latest.json` is under way.
    pub checking: bool,
    /// Why the last look failed, until one works.
    pub check_error: Option<String>,
    /// When the last look worked (ms since the epoch).
    pub checked_ms: Option<i64>,
    /// Wakes the checker now: Settings' "Check for updates".
    pub wake: Option<std::sync::mpsc::Sender<()>>,
}

impl Update {
    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Downloading(..) | Phase::Installing)
    }

    /// The newer build can only be downloaded by hand (a `.deb` install).
    pub fn notify_only(&self) -> bool {
        self.available.as_ref().is_some_and(|a| a.link.is_some())
    }
}

impl State {
    /// A fetched `latest.json`: this install's asset of a newer build becomes the offer.
    pub fn update_found(&mut self, found: &Manifest) {
        let key = manifest::platform_key();
        let base = install::release_url(|k| std::env::var(k).ok());
        self.update_found_for(manifest::running_build(), key.map(|k| (k, current_mode())), &base, found);
    }

    fn update_found_for(&mut self, running: Option<u64>, platform: Option<(&str, Mode)>, base: &str, found: &Manifest) {
        let Some((key, mode)) = platform.filter(|(_, m)| *m != Mode::Off) else { return };
        let Some(asset) = found.assets.get(key) else { return };
        if !manifest::is_newer(running, found.build) || self.update.busy() {
            return;
        }
        let link = (mode == Mode::Notify).then(|| format!("{base}/{}", asset.name));
        self.update.available =
            Some(Available { build: found.build, date: manifest::date_words(&found.built_at), asset: asset.clone(), link });
    }

    /// A look at `latest.json` ended: a newer build becomes the offer, a failure is remembered for the Updates page.
    pub fn update_checked(&mut self, now_ms: i64, result: Result<Manifest, String>) {
        self.update.checking = false;
        match result {
            Ok(found) => {
                self.update.check_error = None;
                self.update.checked_ms = Some(now_ms);
                self.update_found(&found);
            }
            Err(why) => self.update.check_error = Some(why),
        }
    }

    /// "Check for updates" (Settings → Updates & about): the checker looks now, not in four hours. Ignored while one runs.
    pub fn update_check_now(&mut self) {
        if self.update.checking || self.update.busy() {
            return;
        }
        let Some(wake) = &self.update.wake else { return };
        if wake.send(()).is_ok() {
            self.update.checking = true;
        }
    }

    pub fn update_open(&mut self) {
        if self.update.available.is_some() {
            self.update.dialog = true;
        }
    }

    /// Closing is refused while a download or an install runs: there is nothing to cancel safely.
    pub fn update_close(&mut self) {
        if !self.update.busy() {
            self.update.dialog = false;
            if matches!(self.update.phase, Phase::Failed(_)) {
                self.update.phase = Phase::Idle;
            }
        }
    }

    /// A step of the download, from the thread. Ignored when nothing is downloading (a late one after a failure).
    pub fn update_progress(&mut self, p: Progress) {
        self.update.phase = match (p, &self.update.phase) {
            (Progress::Downloading(d, t), Phase::Downloading(..)) => Phase::Downloading(d, t),
            (Progress::Installing, Phase::Downloading(..)) => Phase::Installing,
            _ => return,
        };
    }

    /// Notify mode's button: opens the package's download in the browser.
    pub fn update_download_link(&mut self, tx: &mut Tx) {
        if let Some(link) = self.update.available.as_ref().and_then(|a| a.link.as_deref()) {
            let _ = tx.open_url(link);
        }
    }

    /// "Update and restart": downloads, checks and installs on a thread.
    pub fn update_start(&mut self) {
        let (Some(offer), Some(post)) = (self.update.available.clone(), self.update.post.clone()) else { return };
        if self.update.busy() || offer.link.is_some() {
            return;
        }
        self.update.phase = Phase::Downloading(0, offer.asset.size);
        std::thread::spawn(move || run_update(post, offer.asset));
    }

    /// Everything the next start needs is on disk: the unsent drafts and the settings, written now and in order with the
    /// writes still in flight. The one place a write waits on the UI thread, because the process ends right after.
    pub(crate) fn update_flush(&mut self, tx: &mut Tx) {
        self.keep_drafts(self.clock, tx);
        if let Some((dir, json)) = self.drafts.take_save() {
            crate::composer::drafts::save_now(&dir, &json);
        }
        if let Some((dir, json)) = self.sidebar.take_save() {
            crate::fleet::settings::save_now(&dir, &json);
        }
    }

    /// The update is in place: save, start the new app and end this one.
    fn update_restart(&mut self, tx: &mut Tx, how: Relaunch) {
        self.update_flush(tx);
        let args: Vec<String> = std::env::args().skip(1).collect();
        match install::relaunch(&how, &args) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                self.update.phase =
                    Phase::Manual { why: format!("The update is installed, but the new app didn't start ({e}). Start it by hand.") };
            }
        }
    }
}

/// Where a download goes: the app's own folder, else the system's temporary one.
fn work_dir() -> PathBuf {
    crate::composer::drafts::app_dir(|k| std::env::var(k).ok()).unwrap_or_else(std::env::temp_dir).join("update")
}

/// How long the Fleet this app started has to stop before it is asked, then killed (`Stopper::stop`).
const STOP_GRACE: Duration = Duration::from_secs(20);

/// Takes the Fleet this app started out of the state and stops it, waiting until it is gone. On the calling thread, which
/// is never the UI thread: the state hands the server over, the thread waits. Nothing happens when the app started none.
pub(crate) fn stop_owned_fleet(post: &Post, grace: Duration) {
    let (send, hear) = std::sync::mpsc::channel();
    post.update(move |s, _| {
        let _ = send.send(s.start.owned.take());
    });
    if let Ok(Some(owned)) = hear.recv() {
        owned.stop(grace);
    }
}

fn run_update(post: Post, asset: Asset) {
    let fail = |post: &Post, e: String| post.update(move |s, _| s.update.phase = Phase::Failed(e));
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return fail(&post, format!("Can't tell where this app is: {e}")),
    };
    let base = install::release_url(|k| std::env::var(k).ok());
    let mut last = Instant::now() - Duration::from_secs(1);
    let mut progress = |p: Progress| {
        // About ten redraws a second is plenty for a bar.
        let done = matches!(p, Progress::Downloading(d, t) if d >= t);
        if matches!(p, Progress::Downloading(..)) && !done && last.elapsed() < Duration::from_millis(100) {
            return;
        }
        last = Instant::now();
        post.update(move |s, _| s.update_progress(p));
    };
    // The new process must not find a Fleet that is still stopping (`installUpdate`: `await server?.stop()`), and the folder
    // swap moves the server's files, so the Fleet this app started stops right before the old app is touched.
    let mut stop = || stop_owned_fleet(&post, STOP_GRACE);
    match install::apply(&base, &asset, &manifest::install_kind(), &exe, &work_dir(), &mut stop, &mut progress) {
        Ok(Done::Relaunch(how)) => post.update(move |s, tx| s.update_restart(tx, how)),
        Ok(Done::Manual { why, .. }) => post.update(move |s, _| s.update.phase = Phase::Manual { why }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(work_dir());
            fail(&post, e)
        }
    }
}

/// Starts the checker thread. Only a CI build has a build number; a `dev` build never checks.
pub fn start_checker(handle: &AppHandle<State>) {
    match current_mode() {
        Mode::Off => {
            // The build is still shown on the Updates page, with "This build doesn't update itself".
            let off = std::env::var("FLEET_NATIVE_UPDATES").is_ok_and(|v| v.trim().eq_ignore_ascii_case("off"));
            handle.update(move |s, _| {
                s.update.running = manifest::running_build();
                s.update.off = off;
            });
            return;
        }
        Mode::Install | Mode::Notify => {}
    }
    let post = Post::from_handle(handle.clone());
    let (wake, woken) = std::sync::mpsc::channel::<()>();
    {
        let post = post.clone();
        handle.update(move |s, _| {
            s.update.post = Some(post);
            s.update.running = manifest::running_build();
            s.update.wake = Some(wake);
        });
    }
    let base = install::release_url(|k| std::env::var(k).ok());
    std::thread::spawn(move || {
        loop {
            let found = install::fetch_manifest(&base);
            let wait = if found.is_ok() { EVERY } else { RETRY };
            post.update(move |s, _| s.update_checked(crate::fleet::link::now_ms(), found));
            // Waits out the interval, or is woken by a press of "Check for updates".
            if let Err(std::sync::mpsc::RecvTimeoutError::Disconnected) = woken.recv_timeout(wait) {
                std::thread::sleep(wait);
            }
        }
    });
}

/// `fleet-native --update-now [--no-restart]`: the same download, check and replace with no window, for a script or a CI run.
/// Prints what it did. Returns the exit code.
pub fn update_now(restart: bool) -> i32 {
    let base = install::release_url(|k| std::env::var(k).ok());
    let found = match install::fetch_manifest(&base) {
        Ok(m) => m,
        Err(e) => return say_err(&e),
    };
    let mode = current_mode();
    if mode == Mode::Off {
        println!("updates are off for this build");
        return 0;
    }
    let Some(asset) = manifest::platform_key().and_then(|k| found.assets.get(k)) else { return say_err("no build for this platform") };
    if !manifest::is_newer(manifest::running_build(), found.build) {
        println!("up to date (build {})", manifest::running_build().map_or("dev".to_string(), |b| b.to_string()));
        return 0;
    }
    if mode == Mode::Notify {
        println!("build {} is out; this install can't replace itself. Download {base}/{}", found.build, asset.name);
        return 0;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return say_err(&e.to_string()),
    };
    let work = std::env::temp_dir().join(format!("fleet-native-update-{}", std::process::id()));
    match install::apply(&base, asset, &manifest::install_kind(), &exe, &work, &mut || {}, &mut |_| {}) {
        Ok(Done::Relaunch(how)) => {
            println!("updated to build {}", found.build);
            if restart {
                let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--update-now").collect();
                let _ = install::relaunch(&how, &args);
            }
            0
        }
        Ok(Done::Manual { dmg, why }) => {
            println!("not replaced: {why} The disk image is {}", dmg.display());
            0
        }
        Err(e) => say_err(&e),
    }
}

fn say_err(e: &str) -> i32 {
    eprintln!("update failed: {e}");
    1
}
