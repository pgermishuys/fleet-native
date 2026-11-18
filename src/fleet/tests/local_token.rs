//! This machine's own token (`fleet::local_token`): read from Fleet's file when Fleet asks for one, in the layout of each
//! system, after `--token`, for a loopback Fleet only, and with a message that says where it looked.
use super::auth::HOME_TOKEN;
use crate::fleet::discover::{self, Found, Os, Place};
use crate::fleet::local_token::{self, Outcome, parse_machine_file};
use crate::fleet::{auth, http};
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const TOKEN: &str = "machine-token-0123456789abcdef";

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-localtoken-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A Fleet that wants `TOKEN` on `/api/projects` (401 without it) when `needs` is true, and records every head.
fn fleet(needs: bool) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let text = crate::test_http::read(&mut stream).raw().to_lowercase();
            let ok = !needs || text.contains(&format!("authorization: bearer {TOKEN}"));
            log.lock().unwrap().push(text);
            let reply: &[u8] = if ok {
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n[]"
            } else {
                b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            };
            let _ = stream.write_all(reply);
        }
    });
    (base, seen)
}

fn env_of(root: &std::path::Path) -> impl Fn(&str) -> Option<String> {
    let root = root.display().to_string();
    move |k| matches!(k, "HOME" | "USERPROFILE" | "LOCALAPPDATA" | "APPDATA").then(|| root.clone())
}

fn write_machine(dir: &std::path::Path, name: &str, token: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), format!(r#"{{"id":"0a1b","name":null,"accessToken":"{token}","publicUrl":null}}"#)).unwrap();
}

#[test]
fn a_machine_file_gives_its_access_token_and_nothing_else_does() {
    assert_eq!(parse_machine_file(r#"{"id":"a","accessToken":" abcdefghijklmnop1 "}"#).as_deref(), Some("abcdefghijklmnop1"));
    // Fleet accepts nothing under 16 characters, so a shorter value isn't a token.
    assert_eq!(parse_machine_file(r#"{"accessToken":"short"}"#), None);
    assert_eq!(parse_machine_file(r#"{"accessToken":null}"#), None);
    assert_eq!(parse_machine_file(r#"{"id":"a"}"#), None);
    assert_eq!(parse_machine_file("not json"), None);
}

#[test]
fn the_token_is_read_from_fleets_data_folder_on_each_system() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    for (os, name) in [(Os::Linux, "linux"), (Os::Mac, "mac"), (Os::Windows, "windows")] {
        auth::set(None);
        let root = temp(name);
        let get = env_of(&root);
        // The launcher's and the desktop app's folder, and the server program's own default: each system's layout.
        let folders = [root.join(".weave"), discover::server_data_dir(os, &get).unwrap()];
        for folder in folders {
            let (base, seen) = fleet(true);
            write_machine(&folder, "fleet.machine.json", TOKEN);
            let places = discover::all_places(os, &get);
            let found = Found { url: base.clone(), place: Place::Port(6262) };
            assert_eq!(local_token::adopt(&found, &places), Outcome::Adopted, "{name}: {}", folder.display());
            assert_eq!(auth::token_for(&base).unwrap().expose(), TOKEN);
            // The first call went without a token (to learn it is needed), the one that adopted carried it.
            let seen = seen.lock().unwrap();
            assert!(!seen[0].contains("authorization") && seen.last().unwrap().contains("authorization: bearer"), "{seen:?}");
            std::fs::remove_dir_all(&folder).unwrap();
            auth::set(None);
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn the_instance_files_database_names_the_machine_file() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    auth::set(None);
    let root = temp("instance");
    let data = root.join("elsewhere");
    let (base, _) = fleet(true);
    write_machine(&data, "atlas.machine.json", TOKEN);
    let instance = data.join("fleet.instance.json");
    let db = data.join("atlas.db");
    let json =
        serde_json::json!({"pid": std::process::id(), "url": base, "version": "1", "databasePath": db, "desktop": false, "startedAt": "x"});
    std::fs::write(&instance, json.to_string()).unwrap();
    let found = Found { url: base.clone(), place: Place::InstanceFile(instance.clone()) };
    // A database that isn't called fleet.db keeps its token under its own name.
    assert_eq!(local_token::candidates(&found, &[]), vec![data.join("atlas.machine.json")]);
    assert_eq!(local_token::adopt(&found, &[Place::InstanceFile(instance)]), Outcome::Adopted);
    auth::set(None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_given_token_wins_and_a_fleet_that_needs_none_is_left_alone() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    let root = temp("given");
    write_machine(&root.join(".weave"), "fleet.machine.json", TOKEN);
    let places = discover::all_places(Os::Linux, &env_of(&root));
    let (base, seen) = fleet(true);
    let found = Found { url: base.clone(), place: Place::Port(6262) };
    auth::set(Some("typed-by-hand-0123456789".into()));
    assert_eq!(local_token::adopt(&found, &places), Outcome::NotNeeded);
    assert_eq!(auth::token_for(&base), None, "not even bound to home, nothing was touched");
    auth::bind_home(&base);
    assert_eq!(auth::token_for(&base).unwrap().expose(), "typed-by-hand-0123456789");
    assert!(seen.lock().unwrap().is_empty(), "no call was made");

    auth::set(None);
    let (open, _) = fleet(false);
    let found = Found { url: open.clone(), place: Place::Port(6262) };
    assert_eq!(local_token::adopt(&found, &places), Outcome::NotNeeded);
    assert!(auth::token_for(&open).is_none(), "a Fleet that asks for nothing is sent nothing");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_fleet_at_another_address_never_gets_this_machines_token() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    auth::set(None);
    let root = temp("remote");
    write_machine(&root.join(".weave"), "fleet.machine.json", TOKEN);
    let places = discover::all_places(Os::Linux, &env_of(&root));
    for url in ["http://192.0.2.7:6262", "https://fleet.example.net", "http://100.64.0.9:6262", "http://127.0.0.1.example.net:6262"] {
        let found = Found { url: url.into(), place: Place::Port(6262) };
        assert_eq!(local_token::adopt(&found, &places), Outcome::NotNeeded, "{url}");
        assert!(auth::token_for(url).is_none() && !auth::is_set(), "{url}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn without_a_readable_file_the_message_says_where_it_looked_and_never_shows_a_token() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    auth::set(None);
    let root = temp("missing");
    let places = discover::all_places(Os::Linux, &env_of(&root));
    let (base, _) = fleet(true);
    let found = Found { url: base.clone(), place: Place::Port(6262) };
    // No file at all.
    let Outcome::Missing(looked) = local_token::adopt(&found, &places) else { panic!("expected Missing") };
    assert!(looked.contains(&root.join(".weave")), "{looked:?}");
    let note = auth::refused_note(&base);
    assert!(note.contains("needs an access token") && note.contains("--token") && note.contains("fleet.machine.json"), "{note}");
    assert!(note.contains(&root.join(".weave").display().to_string()), "{note}");
    // A file holding another Fleet's token: Fleet refuses it, so it isn't kept.
    write_machine(&root.join(".weave"), "fleet.machine.json", "another-fleets-token-0123456789");
    assert!(matches!(local_token::adopt(&found, &places), Outcome::Missing(_)));
    assert!(!auth::is_set());
    let note = auth::refused_note(&base);
    assert!(!note.contains("another-fleets-token") && !note.contains(TOKEN));
    assert!(http::get(&base, "/api/projects").is_err());
    auth::note_local_lookup(Vec::new());
    let _ = std::fs::remove_dir_all(&root);
}
