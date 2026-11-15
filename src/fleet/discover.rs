//! Finding the Fleet that is already running on this machine, in every place one writes itself down.
//!
//! weave-fleet's desktop app only looks in its own data folder (`~/.weave`, `desktop/src/paths.ts`), but a
//! Fleet can be running somewhere else: the server program's own default is the platform's local app data folder
//! (`FleetPaths.cs`; the `fleet` launcher overrides it with `~/.weave`), and the desktop app takes
//! any free port when 6262 is busy and remembers it in its own `desktop-settings.json` (`settings.ts`). Each
//! place is tried in turn and believed only when the Fleet it names answers `/readyz`.
//!
//! Blocking (file reads and HTTP calls): run it on a thread.
use super::instance::{self, FILE, READY_TIMEOUT};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The port Fleet's desktop app prefers (`DEFAULT_PORT` in `desktop/src/main.ts`).
pub const DEFAULT_PORT: u16 = 6262;

/// The system the app runs on; a value so a test can ask for each system's folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// Where to look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A `fleet.instance.json` a running Fleet wrote.
    InstanceFile(PathBuf),
    /// The desktop app's `desktop-settings.json`, which remembers the port its own Fleet used.
    SavedPort(PathBuf),
    /// Fleet's preferred port, with nothing written down.
    Port(u16),
}

impl Place {
    /// For the notice that says where it looked: the folder, or the port.
    pub fn describe(&self) -> String {
        match self {
            Place::InstanceFile(f) | Place::SavedPort(f) => f.parent().unwrap_or(f).display().to_string(),
            Place::Port(p) => format!("port {p}"),
        }
    }
}

/// The Fleet that was found and how it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub url: String,
    pub place: Place,
}

fn home(os: Os, get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    get(if os == Os::Windows { "USERPROFILE" } else { "HOME" }).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// `get(name)` when it is set (the Windows folder variables).
fn set(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<PathBuf> {
    get(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `get(name)` when it names an absolute folder: the XDG rule, which .NET follows for `XDG_DATA_HOME`.
fn xdg(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<PathBuf> {
    get(name).filter(|v| v.starts_with('/')).map(PathBuf::from)
}

/// The server's own default data folder (`FleetPaths.DefaultAppDataDirectory`): .NET's `LocalApplicationData` + `WeaveFleet`.
pub fn server_data_dir(os: Os, get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let local = match os {
        Os::Mac => home(os, get)?.join("Library").join("Application Support"),
        Os::Windows => set(get, "LOCALAPPDATA").or_else(|| Some(home(os, get)?.join("AppData").join("Local")))?,
        Os::Linux => xdg(get, "XDG_DATA_HOME").or_else(|| Some(home(os, get)?.join(".local").join("share")))?,
    };
    Some(local.join("WeaveFleet"))
}

/// The desktop app's own folder: Electron's `userData`, named after the product ("Fleet").
pub fn desktop_user_data_dir(os: Os, get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let config = match os {
        Os::Mac => home(os, get)?.join("Library").join("Application Support"),
        Os::Windows => set(get, "APPDATA").or_else(|| Some(home(os, get)?.join("AppData").join("Roaming")))?,
        Os::Linux => xdg(get, "XDG_CONFIG_HOME").or_else(|| Some(home(os, get)?.join(".config")))?,
    };
    Some(config.join("Fleet"))
}

/// Where a running Fleet may have written itself down, most likely first.
pub fn places(os: Os, get: &dyn Fn(&str) -> Option<String>) -> Vec<Place> {
    let mut out = Vec::new();
    // A server started with `Fleet__DatabasePath` in this environment writes its file next to that database
    // (`FleetInstanceLock.GetInstanceFilePath`: the extension becomes `.instance.json`).
    if let Some(db) = get("Fleet__DatabasePath").filter(|d| !d.is_empty()) {
        out.push(Place::InstanceFile(Path::new(&db).with_extension("instance.json")));
    }
    // The `fleet` command's launcher keeps the database in `WEAVE_FLEET_DATA_DIR` (or `--data-dir`, which can't be seen
    // from here), else in `~/.weave`: the same default as the desktop app's, which is why one folder is usually enough.
    if let Some(dir) = set(get, "WEAVE_FLEET_DATA_DIR") {
        out.push(Place::InstanceFile(dir.join(FILE)));
    }
    // The desktop app's data folder, which it hands its Fleet as the database folder.
    if let Some(dir) = instance::data_dir_for(os == Os::Windows, get) {
        out.push(Place::InstanceFile(dir.join(FILE)));
    }
    // The server program's own default when it is run without the launcher (`FleetPaths.cs`).
    if let Some(dir) = server_data_dir(os, get) {
        out.push(Place::InstanceFile(dir.join(FILE)));
    }
    if let Some(dir) = desktop_user_data_dir(os, get) {
        out.push(Place::SavedPort(dir.join("desktop-settings.json")));
    }
    let mut seen = Vec::new();
    out.retain(|p| {
        !seen.contains(p) && {
            seen.push(p.clone());
            true
        }
    });
    out
}

/// The port in a `desktop-settings.json`, if it holds a usable one.
pub fn saved_port(json: &str) -> Option<u16> {
    let port = serde_json::from_str::<Value>(json).ok()?.get("port")?.as_u64()?;
    u16::try_from(port).ok().filter(|p| *p > 0)
}

fn answers(url: &str) -> bool {
    instance::is_ready(url, READY_TIMEOUT)
}

/// The Fleet at the first place that names one that answers.
pub fn find_in(places: &[Place]) -> Option<Found> {
    places.iter().find_map(|place| {
        let url = match place {
            Place::InstanceFile(file) => instance::find_running_fleet(file)?.url,
            Place::SavedPort(file) => {
                let url = format!("http://127.0.0.1:{}", saved_port(&std::fs::read_to_string(file).ok()?)?);
                answers(&url).then_some(url)?
            }
            Place::Port(port) => {
                let url = format!("http://127.0.0.1:{port}");
                answers(&url).then_some(url)?
            }
        };
        Some(Found { url, place: place.clone() })
    })
}

/// Every place, then Fleet's preferred port, which a Fleet that wrote nothing down may still hold.
pub fn all_places(os: Os, get: &dyn Fn(&str) -> Option<String>) -> Vec<Place> {
    let mut all = places(os, get);
    all.push(Place::Port(DEFAULT_PORT));
    all
}

/// Looks for the Fleet on this machine: the places in order, each checked with `/readyz`.
pub fn discover(get: &dyn Fn(&str) -> Option<String>) -> Result<Found, Vec<Place>> {
    let all = all_places(Os::current(), get);
    find_in(&all).ok_or(all)
}

/// What the status bar says when nothing was found: where it looked and how to point the app at a Fleet.
pub fn not_found(looked: &[Place]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for place in looked {
        let d = place.describe();
        if !seen.contains(&d) {
            seen.push(d);
        }
    }
    format!(
        "No Fleet found (looked in {}). Start Fleet, or use --server <url> and --token <token> (or FLEET_NATIVE_SERVER and FLEET_NATIVE_TOKEN).",
        seen.join(", ")
    )
}

/// The address named by `--server` or, failing that, `FLEET_NATIVE_SERVER`.
pub fn explicit_server(arg: Option<String>, get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    arg.or_else(|| get("FLEET_NATIVE_SERVER")).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The notice for a scene: a Mac with an invented user and nothing running.
pub fn sample_not_found() -> String {
    let home = |k: &str| (k == "HOME").then(|| "/Users/demo".to_string());
    not_found(&all_places(Os::Mac, &home))
}
