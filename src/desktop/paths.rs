//! Which server binary the app starts (`desktop/src/paths.ts`, decision 1 of the checklist). Pure: the environment, the
//! file check and the folders come in as arguments, so every system's places are tested on every system.
use std::path::{Path, PathBuf};

/// The system the places are for (the one running, except in a test).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

impl Os {
    pub const fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }

    /// `WeaveFleet.Api`, with `.exe` on Windows.
    pub fn server_name(self) -> &'static str {
        if self == Os::Windows { "WeaveFleet.Api.exe" } else { "WeaveFleet.Api" }
    }

    fn launcher_names(self) -> &'static [&'static str] {
        if self == Os::Windows { &["fleet.exe", "fleet.cmd", "fleet.bat"] } else { &["fleet"] }
    }
}

/// A server to start: the binary, and the folder it finds its UI and settings in (`--contentRoot`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerBin {
    pub binary: PathBuf,
    pub content_root: PathBuf,
}

/// Everything the search reads.
pub struct Inputs<'a> {
    pub get: &'a dyn Fn(&str) -> Option<String>,
    pub exists: &'a dyn Fn(&Path) -> bool,
    pub os: Os,
    /// The home folder (`HOME`, `USERPROFILE` on Windows); none means the places under it are skipped.
    pub home: Option<PathBuf>,
    /// The folder of the running app, where a bundled server will sit once component 25 ships one.
    pub exe_dir: Option<PathBuf>,
    /// The folders of `PATH`, after the login shell's PATH is merged in (piece C).
    pub path_dirs: Vec<PathBuf>,
}

impl<'a> Inputs<'a> {
    /// The running system: the process's environment, the real file system and `PATH`.
    pub fn from_system(get: &'a dyn Fn(&str) -> Option<String>) -> Inputs<'a> {
        let os = Os::current();
        let home = get(if os == Os::Windows { "USERPROFILE" } else { "HOME" }).filter(|h| !h.is_empty()).map(PathBuf::from);
        let path_dirs = get("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        Inputs {
            get,
            exists: &|p| p.is_file(),
            os,
            home,
            exe_dir: std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)),
            path_dirs,
        }
    }

    fn set(&self, key: &str) -> Option<String> {
        (self.get)(key).filter(|v| !v.is_empty())
    }
}

/// The places to look, in order (decision 1): an override, the server bundled beside the app, the CLI install, Fleet's
/// own desktop app's copy, then `fleet` on `PATH`. An override stands alone: someone who names a binary means that one.
pub fn candidates(inputs: &Inputs) -> Vec<ServerBin> {
    let exe = inputs.os.server_name();
    // A server folder holding the binary and its UI.
    let beside = |dir: PathBuf| ServerBin { binary: dir.join(exe), content_root: dir };
    if let Some(binary) = inputs.set("FLEET_NATIVE_SERVER_BIN").or_else(|| inputs.set("FLEET_DESKTOP_SERVER_BIN")) {
        let root = inputs.set("FLEET_NATIVE_SERVER_CONTENT_ROOT").or_else(|| inputs.set("FLEET_DESKTOP_SERVER_CONTENT_ROOT"));
        let binary = PathBuf::from(binary);
        let content_root = root.map(PathBuf::from).unwrap_or_else(|| binary.parent().map(Path::to_path_buf).unwrap_or_default());
        return vec![ServerBin { binary, content_root }];
    }
    let mut out = Vec::new();
    if let Some(dir) = &inputs.exe_dir {
        out.push(beside(dir.join("fleet")));
        if inputs.os == Os::Mac {
            // <bundle>/Contents/MacOS/<app>, with the server in <bundle>/Contents/Resources/fleet.
            out.push(beside(dir.join("../Resources/fleet")));
        }
    }
    // The CLI install: `install.sh` puts the server in `app/` and the launcher in `bin/`.
    let install =
        inputs.set("WEAVE_FLEET_INSTALL_DIR").map(PathBuf::from).or_else(|| inputs.home.as_ref().map(|h| h.join(".weave").join("fleet")));
    if let Some(dir) = install {
        out.push(beside(dir.join("app")));
    }
    match inputs.os {
        Os::Linux => out.push(beside(PathBuf::from("/opt/Fleet/resources/fleet"))),
        Os::Windows => {
            if let Some(local) = inputs.set("LOCALAPPDATA") {
                out.push(beside(Path::new(&local).join("Programs").join("Fleet").join("resources").join("fleet")));
            }
        }
        Os::Mac => {
            out.push(beside(PathBuf::from("/Applications/Fleet.app/Contents/Resources/fleet")));
            if let Some(home) = &inputs.home {
                out.push(beside(home.join("Applications/Fleet.app/Contents/Resources/fleet")));
            }
        }
    }
    // `fleet` on PATH is the launcher in `<install>/bin`; the server is in `<install>/app`.
    for dir in &inputs.path_dirs {
        if inputs.os.launcher_names().iter().any(|name| (inputs.exists)(&dir.join(name))) {
            out.push(beside(dir.join("..").join("app")));
        }
    }
    out
}

/// The first place that has a server, or every place looked at (for "No Fleet found" and the start error).
pub fn find_server(inputs: &Inputs) -> Result<ServerBin, Vec<PathBuf>> {
    let all = candidates(inputs);
    match all.iter().find(|c| (inputs.exists)(&c.binary)) {
        Some(found) => Ok(found.clone()),
        None => Err(all.into_iter().map(|c| c.binary).collect()),
    }
}

/// The words for a server that isn't there (`launch`): "Fleet isn't where the app expects it: <path>". With one place
/// that is Fleet's sentence; with several, the places follow.
pub fn not_found_message(looked: &[PathBuf]) -> String {
    match looked {
        [] => "Fleet isn't installed here: the app found no server to start.".to_string(),
        [one] => format!("Fleet isn't where the app expects it: {}", one.display()),
        many => {
            let list: Vec<String> = many.iter().map(|p| p.display().to_string()).collect();
            format!("Fleet isn't installed here. The app looked for its server in: {}", list.join(", "))
        }
    }
}

/// A CLI install's staged update (`<install>/update/`) is applied by the `fleet` launcher only, and the app doesn't run the
/// launcher (decision 2): the log says so when one is waiting.
pub fn staged_update_note(binary: &Path, exists_dir: &dyn Fn(&Path) -> bool) -> Option<String> {
    let install = binary.parent()?.parent()?;
    let update = install.join("update");
    exists_dir(&update).then(|| format!("a staged update waits in {}; it is applied the next time `fleet` is run", update.display()))
}
