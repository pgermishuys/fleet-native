//! The updater. A thread asks the `latest` release's `latest.json` for the newest build, on start and every few hours; a
//! newer one than the running build puts "Update available · build N" in the status bar (`view::update`). Its dialog
//! downloads the asset for this platform, checks its SHA-256, replaces the running app and starts the new one (`install`).
//! Nothing here runs on the UI thread except small state changes the threads post.
pub mod install;
pub mod manifest;
#[cfg(test)]
mod tests;

use crate::State;
use crate::fleet::link::Post;
use install::{Done, Progress, Relaunch};
use manifest::{Asset, Manifest};
use rune::{AppHandle, Tx};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long after a good answer the next check is.
const EVERY: Duration = Duration::from_secs(4 * 60 * 60);
/// How long after a failed one. A network failure is silent; this is only when it tries again.
const RETRY: Duration = Duration::from_secs(20 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Available {
    pub build: u64,
    /// "15 November 2026".
    pub date: String,
    pub asset: Asset,
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
}

impl Update {
    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Downloading(..) | Phase::Installing)
    }
}

impl State {
    /// A fetched `latest.json`: this platform's asset of a newer build becomes the offer.
    pub fn update_found(&mut self, found: &Manifest) {
        self.update_found_for(manifest::running_build(), manifest::platform_key(), found);
    }

    fn update_found_for(&mut self, running: Option<u64>, platform: Option<&str>, found: &Manifest) {
        let Some(asset) = platform.and_then(|k| found.assets.get(k)) else { return };
        if !manifest::is_newer(running, found.build) || self.update.busy() {
            return;
        }
        self.update.available = Some(Available { build: found.build, date: manifest::date_words(&found.built_at), asset: asset.clone() });
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

    /// "Update and restart": downloads, checks and installs on a thread.
    pub fn update_start(&mut self) {
        let (Some(offer), Some(post)) = (self.update.available.clone(), self.update.post.clone()) else { return };
        if self.update.busy() {
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
        post.update(move |s, _| {
            s.update.phase = match p {
                Progress::Downloading(d, t) => Phase::Downloading(d, t),
                Progress::Installing => Phase::Installing,
            }
        });
    };
    match install::apply(&base, &asset, &exe, &work_dir(), &mut progress) {
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
    if manifest::running_build().is_none() || manifest::platform_key().is_none() {
        return;
    }
    let post = Post::from_handle(handle.clone());
    {
        let post = post.clone();
        handle.update(move |s, _| s.update.post = Some(post));
    }
    let base = install::release_url(|k| std::env::var(k).ok());
    std::thread::spawn(move || {
        loop {
            let wait = match install::fetch_manifest(&base) {
                Ok(found) => {
                    post.update(move |s, _| s.update_found(&found));
                    EVERY
                }
                Err(_) => RETRY,
            };
            std::thread::sleep(wait);
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
    let Some(asset) = manifest::platform_key().and_then(|k| found.assets.get(k)) else { return say_err("no build for this platform") };
    if !manifest::is_newer(manifest::running_build(), found.build) {
        println!("up to date (build {})", manifest::running_build().map_or("dev".to_string(), |b| b.to_string()));
        return 0;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return say_err(&e.to_string()),
    };
    let work = std::env::temp_dir().join(format!("fleet-native-update-{}", std::process::id()));
    match install::apply(&base, asset, &exe, &work, &mut |_| {}) {
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
