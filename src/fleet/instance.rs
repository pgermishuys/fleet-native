//! Finding the Fleet already running on this machine: weave-fleet's `desktop/src/instance.ts` and the
//! data folder from `desktop/src/paths.ts`. A running Fleet writes `fleet.instance.json` next to its
//! database; the file is only believed when its pid is alive and the server answers `/readyz`.
//!
//! Blocking (a file read and one HTTP call): run it on a thread, never in a view or a handler.
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What a running Fleet writes to `fleet.instance.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub pid: u32,
    pub url: String,
    pub version: String,
    pub database_path: String,
    pub desktop: bool,
    pub started_at: String,
}

/// The instance file in the data folder.
pub const FILE: &str = "fleet.instance.json";

/// Where Fleet keeps its data: `FLEET_DESKTOP_DATA_DIR`, else `.weave` in the home folder (`resolvePaths`).
/// `get` reads the environment, so a test can give it one. The home folder is `HOME` (`USERPROFILE` on
/// Windows), as Node's `os.homedir()` has it; with neither there is no data folder, never a fixed path.
pub fn data_dir(get: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    data_dir_for(cfg!(windows), &get)
}

/// `data_dir` for a system that isn't the one running (Windows has `USERPROFILE`, the others `HOME`).
pub fn data_dir_for(windows: bool, get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(dir) = get("FLEET_DESKTOP_DATA_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = get(if windows { "USERPROFILE" } else { "HOME" }).filter(|h| !h.is_empty())?;
    Some(Path::new(&home).join(".weave"))
}

/// Parses an instance file, or gives nothing for anything that isn't one.
pub fn parse_instance(json: &str) -> Option<Instance> {
    let value: Value = serde_json::from_str(json).ok()?;
    let record = value.as_object()?;
    // `Number.isInteger(pid) && pid > 0`: 4242.0 counts, 4242.5 and "4242" don't.
    let pid = record.get("pid")?;
    let pid = pid.as_u64().or_else(|| pid.as_f64().filter(|f| f.fract() == 0.0 && *f > 0.0 && *f < 4.3e9).map(|f| f as u64))?;
    let pid = u32::try_from(pid).ok().filter(|p| *p > 0)?;
    let url = record.get("url")?.as_str().filter(|u| is_loopback_http_url(u))?;
    let text = |key: &str, default: &str| record.get(key).and_then(Value::as_str).unwrap_or(default).to_string();
    Some(Instance {
        pid,
        url: url.trim_end_matches('/').to_string(),
        version: text("version", "unknown"),
        database_path: text("databasePath", ""),
        desktop: record.get("desktop") == Some(&Value::Bool(true)),
        started_at: text("startedAt", ""),
    })
}

/// Only loopback `http:` URLs: the file sits in a folder the user can write, and the app must never
/// connect to a remote host because of what it says.
pub fn is_loopback_http_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("http://").or_else(|| value.strip_prefix("HTTP://")) else {
        return false;
    };
    let authority = rest.split(['/', '\\', '?', '#']).next().unwrap_or("");
    // A user name before an `@` makes the real host the one after it; refuse the shape outright.
    if authority.contains('@') {
        return false;
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(v6) => match v6.split_once(']') {
            Some((inside, after)) => (format!("[{inside}]"), after.strip_prefix(':').or(if after.is_empty() { Some("") } else { None })),
            None => return false,
        },
        None => match authority.split_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (authority.to_string(), Some("")),
        },
    };
    let port_ok = match port {
        Some(p) => p.is_empty() || (p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().is_ok_and(|n| n <= 65535)),
        None => false,
    };
    port_ok && (host == "127.0.0.1" || host == "[::1]" || host.eq_ignore_ascii_case("localhost"))
}

/// The instance file's contents, if it can be read and is one.
pub fn read_instance(file: &Path) -> Option<Instance> {
    parse_instance(&std::fs::read_to_string(file).ok()?)
}

/// Whether a process with this pid exists (`kill(pid, 0)`; a process of another user counts as alive).
#[cfg(unix)]
pub fn is_process_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the process exists and can be signalled.
    unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// Not checked off Unix (no new crate for it): `/readyz` alone says whether a stale file's Fleet is there.
#[cfg(not(unix))]
pub fn is_process_alive(_pid: u32) -> bool {
    true
}

/// Whether the Fleet at `url` answers `/readyz` with a success within `timeout`.
pub fn is_ready(url: &str, timeout: std::time::Duration) -> bool {
    super::http::ready(url, timeout)
}

/// How long `/readyz` may take (`isReady`'s default).
pub const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

/// The Fleet the instance file describes, if it's still running and answering.
pub fn find_running_fleet(file: &Path) -> Option<Instance> {
    let instance = read_instance(file)?;
    (is_process_alive(instance.pid) && is_ready(&instance.url, READY_TIMEOUT)).then_some(instance)
}
