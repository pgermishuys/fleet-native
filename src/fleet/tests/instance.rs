//! Finding the Fleet (checklist rows 1.1-1.3 and 1.8): the cases of weave-fleet's `desktop/test/desktop.test.ts`
//! ("instance file", "resolvePaths") and of `instance.ts`'s `isReady` and `findRunningFleet`, with the instance
//! file in a temporary folder.
use crate::fleet::instance::{
    self, Instance, data_dir, find_running_fleet, is_loopback_http_url, is_process_alive, is_ready, parse_instance,
};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

fn valid() -> serde_json::Value {
    json!({"pid": 4242, "url": "http://127.0.0.1:2113", "version": "1.2.3", "databasePath": "/data/fleet.db", "desktop": true, "startedAt": "2026-01-01T00:00:00Z"})
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-instance-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A server that answers every request with `status`, on a port of its own; stops when dropped.
fn server(status: &'static str) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let _ = s.write_all(format!("HTTP/1.1 {status}\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").as_bytes());
        }
    });
    (url, thread)
}

#[test]
fn parses_a_valid_instance_file() {
    let got = parse_instance(&valid().to_string()).unwrap();
    assert_eq!(
        got,
        Instance {
            pid: 4242,
            url: "http://127.0.0.1:2113".into(),
            version: "1.2.3".into(),
            database_path: "/data/fleet.db".into(),
            desktop: true,
            started_at: "2026-01-01T00:00:00Z".into()
        }
    );
}

#[test]
fn fills_defaults_for_the_optional_fields() {
    let got = parse_instance(r#"{"pid": 7, "url": "http://localhost:6262"}"#).unwrap();
    assert_eq!((got.version.as_str(), got.database_path.as_str(), got.desktop, got.started_at.as_str()), ("unknown", "", false, ""));
}

#[test]
fn rejects_what_is_not_json_or_not_an_object() {
    for text in ["", "not json", "[]", "null", "42", "\"x\""] {
        assert_eq!(parse_instance(text), None, "{text}");
    }
}

#[test]
fn rejects_a_bad_pid() {
    for pid in [json!(0), json!(-5), json!(4242.5), json!("4242"), json!(null), json!(true)] {
        let mut v = valid();
        v["pid"] = pid.clone();
        assert_eq!(parse_instance(&v.to_string()), None, "{pid}");
    }
    let mut v = valid();
    v.as_object_mut().unwrap().remove("pid");
    assert_eq!(parse_instance(&v.to_string()), None);
}

#[test]
fn rejects_a_remote_url() {
    for url in [
        "https://127.0.0.1:2113",
        "http://example.com:2113",
        "http://10.0.0.5:2113",
        "ftp://127.0.0.1",
        "http://127.0.0.1@example.com",
        "http://localhost.example.com",
        "http://127.0.0.1:99999",
        "http://127.0.0.1:12ab",
        "127.0.0.1:2113",
        "",
    ] {
        let mut v = valid();
        v["url"] = json!(url);
        assert_eq!(parse_instance(&v.to_string()), None, "{url}");
    }
    let mut v = valid();
    v["url"] = json!(7);
    assert_eq!(parse_instance(&v.to_string()), None);
}

#[test]
fn accepts_localhost_and_ipv6_loopback() {
    assert!(is_loopback_http_url("http://localhost:2113"));
    assert!(is_loopback_http_url("http://LOCALHOST:2113"));
    assert!(is_loopback_http_url("http://[::1]:2113"));
    assert!(is_loopback_http_url("http://[::1]"));
    assert!(is_loopback_http_url("http://127.0.0.1"));
    assert!(!is_loopback_http_url("http://[::2]:2113"));
    assert!(!is_loopback_http_url("http://[::1"));
}

#[test]
fn strips_trailing_slashes_from_the_url() {
    let mut v = valid();
    v["url"] = json!("http://127.0.0.1:2113///");
    assert_eq!(parse_instance(&v.to_string()).unwrap().url, "http://127.0.0.1:2113");
}

// ─── resolvePaths: the data folder ──────────────────────────────────────────

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

#[test]
fn the_data_folder_is_the_override_or_weave_in_the_home_folder() {
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let got = data_dir(env(&[("FLEET_DESKTOP_DATA_DIR", "/custom/data"), (home, "/home/me")])).unwrap();
    assert_eq!(got, std::path::PathBuf::from("/custom/data"));
    let got = data_dir(env(&[(home, "/home/me")])).unwrap();
    assert_eq!(got, std::path::Path::new("/home/me").join(".weave"));
    // An empty override is no override (`||` in paths.ts).
    let got = data_dir(env(&[("FLEET_DESKTOP_DATA_DIR", ""), (home, "/home/me")])).unwrap();
    assert_eq!(got, std::path::Path::new("/home/me").join(".weave"));
}

#[test]
fn with_no_home_folder_there_is_no_data_folder_and_no_fixed_path_is_tried() {
    assert_eq!(data_dir(env(&[])), None);
    assert_eq!(data_dir(env(&[("HOME", ""), ("USERPROFILE", "")])), None);
    assert!(instance::discover(env(&[])).is_none());
}

#[cfg(unix)]
#[test]
fn the_data_folder_follows_the_home_variable_of_the_process() {
    // The real environment, as `start_desktop` reads it: a scratch HOME gives a scratch folder.
    let before = std::env::var("HOME").ok();
    let home = temp("home");
    // SAFETY: no other test reads HOME while this runs (this is the only one that does).
    unsafe { std::env::set_var("HOME", &home) };
    let got = data_dir(|k| std::env::var(k).ok());
    match before {
        Some(b) => unsafe { std::env::set_var("HOME", b) },
        None => unsafe { std::env::remove_var("HOME") },
    }
    assert_eq!(got, Some(home.join(".weave")));
}

// ─── isReady ────────────────────────────────────────────────────────────────

#[test]
fn is_ready_is_true_on_200() {
    let (url, t) = server("200 OK");
    assert!(is_ready(&url, Duration::from_millis(1500)));
    t.join().unwrap();
}

#[test]
fn is_ready_is_false_on_a_server_error() {
    let (url, t) = server("503 Service Unavailable");
    assert!(!is_ready(&url, Duration::from_millis(1500)));
    t.join().unwrap();
}

#[test]
fn is_ready_is_false_when_the_connection_is_refused() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    assert!(!is_ready(&format!("http://127.0.0.1:{port}"), Duration::from_millis(1500)));
}

#[test]
fn is_ready_is_false_on_timeout() {
    // Accepts the connection and never answers.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let started = std::time::Instant::now();
    assert!(!is_ready(&url, Duration::from_millis(200)));
    assert!(started.elapsed() < Duration::from_secs(3));
    drop(listener);
}

// ─── findRunningFleet ───────────────────────────────────────────────────────

fn write_file(dir: &std::path::Path, pid: u32, url: &str) -> std::path::PathBuf {
    let file = dir.join(instance::FILE);
    let mut v = valid();
    v["pid"] = json!(pid);
    v["url"] = json!(url);
    std::fs::write(&file, v.to_string()).unwrap();
    file
}

#[test]
fn find_running_fleet_is_none_without_a_file() {
    let dir = temp("nofile");
    assert_eq!(find_running_fleet(&dir.join(instance::FILE)), None);
}

#[cfg(unix)]
#[test]
fn find_running_fleet_is_none_for_a_dead_pid() {
    let dir = temp("dead");
    // A child that has exited and been reaped: its pid is free.
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    assert!(!is_process_alive(pid));
    // The server answers, so only the pid says no.
    let (url, t) = server("200 OK");
    let file = write_file(&dir, pid, &url);
    assert_eq!(find_running_fleet(&file), None);
    drop(t);
}

#[test]
fn find_running_fleet_finds_a_live_pid_that_answers() {
    let dir = temp("live");
    let (url, t) = server("200 OK");
    let file = write_file(&dir, std::process::id(), &url);
    let found = find_running_fleet(&file).unwrap();
    assert_eq!((found.pid, found.url.as_str()), (std::process::id(), url.as_str()));
    t.join().unwrap();
}

#[test]
fn find_running_fleet_is_none_for_a_live_pid_that_does_not_answer() {
    let dir = temp("silent");
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let file = write_file(&dir, std::process::id(), &format!("http://127.0.0.1:{port}"));
    assert_eq!(find_running_fleet(&file), None);
}

#[test]
fn discover_reads_the_instance_file_in_the_data_folder_and_nowhere_else() {
    let dir = temp("discover");
    let (url, t) = server("200 OK");
    write_file(&dir, std::process::id(), &url);
    let dir_s = dir.to_string_lossy().to_string();
    let found =
        instance::discover(env(&[("FLEET_DESKTOP_DATA_DIR", &dir_s), ("HOME", "/nonexistent-home"), ("USERPROFILE", "/nonexistent-home")]))
            .unwrap();
    assert_eq!(found.url, url);
    t.join().unwrap();
    // A home folder with no `.weave` finds nothing.
    let empty = temp("empty-home");
    let home_s = empty.to_string_lossy().to_string();
    assert!(instance::discover(env(&[("HOME", &home_s), ("USERPROFILE", &home_s)])).is_none());
}

#[test]
fn the_host_of_a_server_url() {
    use crate::fleet::link::host_of;
    assert_eq!(host_of("http://127.0.0.1:6411").unwrap(), "127.0.0.1:6411");
    assert_eq!(host_of("http://127.0.0.1:6411/").unwrap(), "127.0.0.1:6411");
    assert_eq!(host_of("http://[::1]:6411").unwrap(), "[::1]:6411");
    assert_eq!(host_of("http://[::1]").unwrap(), "[::1]:80");
    assert_eq!(host_of("http://localhost").unwrap(), "localhost:80");
    assert!(host_of("https://127.0.0.1:6411").is_err());
}
