//! Piece C's tests: Fleet's `desktop/test/desktop.test.ts` "login shell environment" under the same names, then the shell
//! itself against a fake one (a script, so Unix only; Windows asks no shell).
use super::shell_env::{NAMES, apply, capture_command, merge_path, parse_captured};
use std::collections::HashMap;

#[test]
fn reads_each_variable_between_its_markers_ignoring_shell_noise() {
    let names = ["PATH", "SSH_AUTH_SOCK", "LANG"];
    let output = [
        "Welcome to your shell!",
        "__FLEET_ENV_PATH_START__",
        "/opt/homebrew/bin:/usr/bin",
        "__FLEET_ENV_PATH_END__",
        "__FLEET_ENV_SSH_AUTH_SOCK_START__",
        "__FLEET_ENV_SSH_AUTH_SOCK_END__",
        "__FLEET_ENV_LANG_START__",
        "en_GB.UTF-8",
        "__FLEET_ENV_LANG_END__",
    ]
    .join("\n");
    let expected: HashMap<String, String> =
        [("PATH", "/opt/homebrew/bin:/usr/bin"), ("LANG", "en_GB.UTF-8")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    assert_eq!(parse_captured(&output, &names), expected);
}

#[test]
fn asks_the_shell_with_printenv_for_each_name() {
    assert_eq!(
        capture_command(&["PATH"]),
        "printf '%s\\n' '__FLEET_ENV_PATH_START__'; printenv PATH || true; printf '%s\\n' '__FLEET_ENV_PATH_END__'"
    );
    let all = capture_command(NAMES);
    assert_eq!(all.matches("printenv ").count(), 8);
    assert!(all.contains("__FLEET_ENV_HOMEBREW_REPOSITORY_END__"));
}

#[test]
fn puts_the_shells_path_first_and_keeps_the_apps_extra_entries() {
    assert_eq!(
        merge_path(Some("/opt/homebrew/bin:/usr/bin"), Some("/usr/bin:/snap/bin"), ':').as_deref(),
        Some("/opt/homebrew/bin:/usr/bin:/snap/bin")
    );
    assert_eq!(merge_path(None, Some("/usr/bin"), ':').as_deref(), Some("/usr/bin"));
    assert_eq!(merge_path(None, None, ':'), None);
    assert_eq!(merge_path(Some("/a::/b"), Some("/b:/c:"), ':').as_deref(), Some("/a:/b:/c"));
}

#[test]
fn a_marker_that_never_closes_gives_nothing() {
    let output = "__FLEET_ENV_PATH_START__\n/usr/bin\n";
    assert!(parse_captured(output, &["PATH"]).is_empty());
    let backwards = "__FLEET_ENV_PATH_END__\n__FLEET_ENV_PATH_START__\n/usr/bin\n";
    assert!(parse_captured(backwards, &["PATH"]).is_empty());
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn value<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

#[test]
fn the_other_variables_are_taken_only_when_the_app_has_none() {
    let captured: HashMap<String, String> =
        [("PATH", "/shell/bin"), ("LANG", "en_GB.UTF-8"), ("LC_ALL", "C.UTF-8"), ("SSH_AUTH_SOCK", "/run/agent")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    let mut env = pairs(&[("PATH", "/usr/bin"), ("LANG", "de_DE.UTF-8"), ("LC_ALL", "")]);
    apply(&mut env, &captured);
    assert_eq!(value(&env, "PATH"), Some("/shell/bin:/usr/bin"));
    assert_eq!(value(&env, "LANG"), Some("de_DE.UTF-8"), "the app's own wins");
    assert_eq!(value(&env, "LC_ALL"), Some("C.UTF-8"), "an empty one counts as none");
    assert_eq!(value(&env, "SSH_AUTH_SOCK"), Some("/run/agent"));
    assert_eq!(value(&env, "HOMEBREW_PREFIX"), None);
}

#[test]
fn an_app_with_no_path_gets_the_shells() {
    let captured: HashMap<String, String> = [("PATH".to_string(), "/shell/bin".to_string())].into();
    let mut env = Vec::new();
    apply(&mut env, &captured);
    assert_eq!(value(&env, "PATH"), Some("/shell/bin"));
}

#[test]
fn the_server_gets_the_merged_path() {
    let captured: HashMap<String, String> = [("PATH".to_string(), "/shell/bin".to_string())].into();
    let mut env = pairs(&[("PATH", "/usr/bin"), ("ELECTRON_RUN_AS_NODE", "1")]);
    apply(&mut env, &captured);
    let server = super::server::server_env(env, std::path::Path::new("/data"), 6262);
    assert_eq!(value(&server, "PATH"), Some("/shell/bin:/usr/bin"));
    assert_eq!(value(&server, "ELECTRON_RUN_AS_NODE"), None);
}

#[test]
fn the_shell_defaults_to_zsh_on_macos_and_bash_elsewhere() {
    use super::shell_env::shell_from;
    assert_eq!(shell_from(Some("/usr/bin/fish"), true), "/usr/bin/fish");
    assert_eq!(shell_from(None, true), "/bin/zsh");
    assert_eq!(shell_from(Some(""), false), "/bin/bash");
}

#[cfg(unix)]
mod shell {
    use super::*;
    use crate::desktop::shell_env::{merged_with, read_login_shell_env};
    use crate::test_wait;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// A folder of its own that goes when the test does.
    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Dir {
            let dir = std::env::temp_dir().join(format!("fleet-native-shellenv-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Dir(dir)
        }
        /// A shell that prints `before`, then runs the `-c` command it was given with `exports` set, as a login shell does.
        fn shell(&self, before: &str, exports: &str) -> String {
            let path = self.0.join("fake-shell");
            let body = format!("#!/bin/sh\n{before}\n[ \"$1\" = -ilc ] || exit 64\n{exports}\neval \"$2\"\n");
            std::fs::write(&path, body).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.display().to_string()
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Writing a script and running it at once can meet "text file busy" when another test forks in between: ask again.
    fn read(shell: &str) -> HashMap<String, String> {
        let mut got = HashMap::new();
        test_wait::until_true("the fake shell to answer", || {
            got = read_login_shell_env(shell, Duration::from_secs(30));
            !got.is_empty()
        });
        got
    }

    #[test]
    fn reads_the_variables_a_login_shell_sets_and_skips_its_noise() {
        let dir = Dir::new("reads");
        let shell = dir.shell(
            "echo 'Last login: today'; echo 'nvm: loaded'",
            "PATH=/shell/bin:/usr/bin; LANG=en_GB.UTF-8; export PATH LANG; unset LC_ALL",
        );
        let got = read(&shell);
        assert_eq!(got.get("PATH").map(String::as_str), Some("/shell/bin:/usr/bin"));
        assert_eq!(got.get("LANG").map(String::as_str), Some("en_GB.UTF-8"));
        assert_eq!(got.get("LC_ALL"), None);
        assert_eq!(got.get("HOMEBREW_PREFIX"), None);
    }

    #[test]
    fn the_shell_is_asked_with_a_dumb_terminal() {
        let dir = Dir::new("term");
        let shell = dir.shell("", "LANG=$TERM; export LANG");
        assert_eq!(read(&shell).get("LANG").map(String::as_str), Some("dumb"));
    }

    #[test]
    fn merges_into_the_apps_environment_with_the_shells_path_first() {
        let dir = Dir::new("merge");
        let shell = dir.shell("", "PATH=/shell/bin:/usr/bin; export PATH");
        let base = pairs(&[("PATH", "/usr/bin:/snap/bin"), ("HOME", "/home/someone")]);
        let mut got = base.clone();
        test_wait::until_true("the merge to include the shell's PATH", || {
            got = merged_with(base.clone(), &shell, Duration::from_secs(30));
            value(&got, "PATH").is_some_and(|p| p.starts_with("/shell/bin"))
        });
        assert_eq!(value(&got, "PATH"), Some("/shell/bin:/usr/bin:/snap/bin"));
        assert_eq!(value(&got, "HOME"), Some("/home/someone"));
    }

    #[test]
    fn a_shell_that_cannot_start_leaves_the_environment_alone() {
        let base = pairs(&[("PATH", "/usr/bin")]);
        assert_eq!(merged_with(base.clone(), "/nonexistent/shell", Duration::from_secs(5)), base);
    }

    #[test]
    fn a_shell_that_hangs_is_stopped_at_the_timeout_and_what_it_said_counts() {
        let dir = Dir::new("hang");
        // Says its PATH, then never finishes the rest.
        let shell = dir.shell("", "printf '%s\\n' __FLEET_ENV_PATH_START__ /shell/bin __FLEET_ENV_PATH_END__; exec sleep 600");
        let started = Instant::now();
        let mut got = HashMap::new();
        test_wait::until_true("the hung shell's PATH", || {
            got = read_login_shell_env(&shell, Duration::from_millis(1500));
            !got.is_empty()
        });
        assert_eq!(got.get("PATH").map(String::as_str), Some("/shell/bin"));
        assert!(started.elapsed() < Duration::from_secs(30), "it did not wait for the sleep");
    }

    #[test]
    fn output_past_the_cap_is_cut() {
        let dir = Dir::new("cap");
        let shell = dir.shell("yes 'noise noise noise' | head -c 3000000", "PATH=/late; export PATH");
        let got = read_login_shell_env(&shell, Duration::from_secs(20));
        assert!(got.is_empty(), "the answer came after 1 MB of noise, so it is not read: {got:?}");
    }
}

/// By hand, like `live_server_starts_answers_and_stops_with_the_input`: a fake login shell that sets an invented PATH,
/// merged in, then the released server started with it. The server's own `/proc/<pid>/environ` must show that PATH first.
/// `FLEET_NATIVE_LIVE_SERVER_DIR=<unpacked server package> FLEET_NATIVE_LIVE_SCRATCH=<an empty folder> HOME=<a scratch
/// folder> cargo test live_server_gets -- --ignored --nocapture`. Linux only (`/proc`).
#[cfg(target_os = "linux")]
#[test]
#[ignore = "needs a released server package"]
fn live_server_gets_the_login_shells_path() {
    use super::server::{Event, FleetServer, Options};
    use crate::desktop::paths::Os;
    use std::path::PathBuf;
    use std::time::Duration;
    let (Ok(package), Ok(work)) = (std::env::var("FLEET_NATIVE_LIVE_SERVER_DIR"), std::env::var("FLEET_NATIVE_LIVE_SCRATCH")) else {
        panic!("set FLEET_NATIVE_LIVE_SERVER_DIR and FLEET_NATIVE_LIVE_SCRATCH");
    };
    let (package, work) = (PathBuf::from(package), PathBuf::from(work));
    let data = work.join("data");
    let shell = work.join("login-shell");
    std::fs::write(
        &shell,
        "#!/bin/sh\necho 'Welcome back!'\nPATH=/opt/demo-tools/bin:$PATH\nLANG=en_GB.UTF-8\nexport PATH LANG\neval \"$2\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut env: Vec<(String, String)> = std::env::vars().filter(|(k, _)| k != "LANG").collect();
    env.push(("FLEET_DESKTOP_DATA_DIR".into(), data.display().to_string()));
    let env = super::shell_env::merged_with(env, &shell.display().to_string(), Duration::from_secs(10));
    let mut o = Options::new(package.join(Os::current().server_name()), package.clone(), data, work.join("logs").join("fleet-server.log"));
    o.preferred_ports = vec![];
    o.env = env;
    let server = FleetServer::start(o).unwrap();
    let ready = crate::test_wait::recv("the released server to answer /readyz", server.events());
    assert!(matches!(ready, Event::Ready { .. }), "not ready: {ready:?}");
    let pid = server.pid().expect("a running server");
    let raw = std::fs::read(format!("/proc/{pid}/environ")).unwrap();
    let vars: Vec<(String, String)> =
        String::from_utf8_lossy(&raw).split('\0').filter_map(|e| e.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let path = vars.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.clone()).unwrap_or_default();
    let first: Vec<&str> = path.split(':').take(3).collect();
    println!("server pid {pid}: PATH has {} entries, first three: {first:?}", path.split(':').count());
    println!("LANG = {:?}", vars.iter().find(|(k, _)| k == "LANG").map(|(_, v)| v));
    println!("names set: {}", vars.iter().map(|(k, _)| k.as_str()).filter(|k| NAMES.contains(k)).collect::<Vec<_>>().join(", "));
    assert_eq!(path.split(':').next(), Some("/opt/demo-tools/bin"));
    assert_eq!(vars.iter().find(|(k, _)| k == "LANG").map(|(_, v)| v.as_str()), Some("en_GB.UTF-8"));
    server.stop(Duration::from_secs(20));
}
