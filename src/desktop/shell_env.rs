//! The login shell's environment (`desktop/src/shell-env.ts`). An app started from a dock, a launcher or a desktop menu
//! doesn't get the PATH the user's shell sets up, so Fleet wouldn't find `opencode`, `node` or `git` installed through
//! Homebrew, nvm or `~/.local/bin`. Ask a login shell once and merge what it says into the environment the server (and
//! through it every harness) starts with. Blocking: run [`merged`] or [`spawn`] off the UI thread.
use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What is asked of the shell, in Fleet's order.
pub const NAMES: &[&str] =
    &["PATH", "SSH_AUTH_SOCK", "LANG", "LC_ALL", "LC_CTYPE", "HOMEBREW_PREFIX", "HOMEBREW_CELLAR", "HOMEBREW_REPOSITORY"];

/// How long the shell may take (Fleet: 5 s) and how much of its output is read (Fleet: 1 MB).
pub const TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_OUTPUT: usize = 1024 * 1024;

fn marker(name: &str, edge: &str) -> String {
    format!("__FLEET_ENV_{name}_{edge}__")
}

/// A command that prints each variable between markers, so what the shell prints while it starts doesn't matter.
pub fn capture_command(names: &[&str]) -> String {
    let parts: Vec<String> = names
        .iter()
        .map(|n| format!("printf '%s\\n' '{}'; printenv {n} || true; printf '%s\\n' '{}'", marker(n, "START"), marker(n, "END")))
        .collect();
    parts.join("; ")
}

/// Each variable between its markers; one that is missing, unfinished or empty is left out.
pub fn parse_captured(output: &str, names: &[&str]) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for name in names {
        let (open, close) = (marker(name, "START"), marker(name, "END"));
        let (Some(start), Some(end)) = (output.find(&open), output.find(&close)) else { continue };
        if end < start {
            continue;
        }
        let value = output[start + open.len()..end].trim();
        if !value.is_empty() {
            result.insert(name.to_string(), value.to_string());
        }
    }
    result
}

/// The shell's PATH first, then entries only the app's own PATH had, without repeats. `None` when both are empty.
pub fn merge_path(shell_path: Option<&str>, app_path: Option<&str>, delimiter: char) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let entries: Vec<&str> =
        shell_path.into_iter().chain(app_path).flat_map(|p| p.split(delimiter)).filter(|e| !e.is_empty() && seen.insert(*e)).collect();
    (!entries.is_empty()).then(|| entries.join(&delimiter.to_string()))
}

/// The shell to ask: `$SHELL`, else zsh on macOS and bash elsewhere.
pub fn shell_from(shell_var: Option<&str>, macos: bool) -> String {
    match shell_var {
        Some(s) if !s.is_empty() => s.to_string(),
        _ if macos => "/bin/zsh".into(),
        _ => "/bin/bash".into(),
    }
}

/// Runs `shell -ilc <capture>` with `TERM=dumb` and returns what it printed. Whatever was read when the time runs out or
/// the output passes the cap is parsed as it is; a shell that can't start gives nothing.
pub fn read_login_shell_env(shell: &str, timeout: Duration) -> HashMap<String, String> {
    let mut command = Command::new(shell);
    command.args(["-ilc", &capture_command(NAMES)]).env("TERM", "dumb").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else { return HashMap::new() };
    let Some(mut out) = child.stdout.take() else { return HashMap::new() };
    let got = Arc::new(Mutex::new(Vec::<u8>::new()));
    let (done_tx, done) = channel::<()>();
    let reader = got.clone();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = out.read(&mut chunk) {
            let mut buf = reader.lock().unwrap();
            let room = MAX_OUTPUT.saturating_sub(buf.len());
            buf.extend_from_slice(&chunk[..n.min(room)]);
            if n == 0 || n > room {
                break;
            }
        }
        let _ = done_tx.send(());
    });
    let deadline = Instant::now() + timeout;
    // Finished once the output ended (or hit the cap) and the shell exited; otherwise stopped at the deadline.
    let _ = done.recv_timeout(timeout);
    while Instant::now() < deadline && matches!(child.try_wait(), Ok(None)) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = child.kill();
    let _ = child.wait();
    let text = String::from_utf8_lossy(&got.lock().unwrap()).into_owned();
    parse_captured(&text, NAMES)
}

/// Merges what the shell said into `env`: PATH is merged ([`merge_path`]), the others are taken only when `env` has none.
pub fn apply(env: &mut Vec<(String, String)>, captured: &HashMap<String, String>) {
    let get = |env: &[(String, String)], name: &str| env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    let set = |env: &mut Vec<(String, String)>, name: &str, value: String| match env.iter_mut().find(|(k, _)| k == name) {
        Some(slot) => slot.1 = value,
        None => env.push((name.to_string(), value)),
    };
    if let Some(path) = merge_path(captured.get("PATH").map(String::as_str), get(env, "PATH").as_deref(), PATH_SEPARATOR) {
        set(env, "PATH", path);
    }
    for name in NAMES.iter().filter(|n| **n != "PATH") {
        if get(env, name).is_none_or(|v| v.is_empty())
            && let Some(value) = captured.get(*name)
        {
            set(env, name, value.clone());
        }
    }
}

const PATH_SEPARATOR: char = ':';

/// `base` with the login shell's environment merged in. Windows apps already get the user's PATH, so nothing is asked.
pub fn merged(base: Vec<(String, String)>) -> Vec<(String, String)> {
    if cfg!(windows) {
        return base;
    }
    let shell = shell_from(base.iter().find(|(k, _)| k == "SHELL").map(|(_, v)| v.as_str()), cfg!(target_os = "macos"));
    merged_with(base, &shell, TIMEOUT)
}

/// [`merged`] with the shell and the time given.
pub fn merged_with(mut base: Vec<(String, String)>, shell: &str, timeout: Duration) -> Vec<(String, String)> {
    apply(&mut base, &read_login_shell_env(shell, timeout));
    base
}

/// [`merged`] on a thread of its own, for the app's start: the answer arrives once, after at most a few seconds.
pub fn spawn(base: Vec<(String, String)>) -> Receiver<Vec<(String, String)>> {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new().name("fleet-shell-env".into()).spawn({
        let tx = tx.clone();
        let base = base.clone();
        move || {
            let _ = tx.send(merged(base));
        }
    });
    if spawned.is_err() {
        let _ = tx.send(base);
    }
    rx
}
