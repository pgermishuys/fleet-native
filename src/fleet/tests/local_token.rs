//! This machine's own token (`fleet::local_token`): read from Fleet's file when Fleet asks for one, in the layout of each
//! system, after `--token`, for a loopback Fleet only, and with a message that says where it looked.
use super::auth::HOME_TOKEN;
use crate::fleet::discover::{self, Found, Os, Place};
use crate::fleet::local_token::{self, Outcome, machine_file_for, parse_machine_file, read_access_token};
use crate::fleet::{auth, http};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const KEY: &str = "MVrTPB67r4BYDD-vhHCn56pZKngtOOO1BXngIJ3sbNA";
const TOKEN: &str = "machine-token-0123456789abcdef-0123456789";

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
    assert_eq!(parse_machine_file(&format!(r#"{{"id":"a","accessToken":" {KEY} "}}"#)).as_deref(), Some(KEY));
    // Fleet writes nothing under 32 characters, so a shorter value is a half-written file, not a key.
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
    write_machine(&root.join(".weave"), "fleet.machine.json", "another-fleets-token-0123456789-abcdefghij");
    // The key was read, and Fleet turned it away: a different outcome, with different words, from a key that can't be read.
    assert_eq!(local_token::adopt(&found, &places), Outcome::Refused);
    assert!(!auth::is_set());
    let note = auth::refused_note(&base);
    assert!(!note.contains("another-fleets-token") && !note.contains(TOKEN));
    assert!(http::get(&base, "/api/projects").is_err());
    auth::note_local_lookup(Vec::new());
    let _ = std::fs::remove_dir_all(&root);
}

// Piece D of component 24: the cases of `desktop.test.ts`, "the key of the Fleet running here".

#[test]
fn the_key_is_in_the_machine_file_beside_the_database() {
    assert_eq!(machine_file_for(Path::new("/home/me/.weave/fleet.db")), Path::new("/home/me/.weave/fleet.machine.json"));
    assert_eq!(machine_file_for(Path::new("/data/profiles/work/atlas.db")), Path::new("/data/profiles/work/atlas.machine.json"));
}

#[test]
fn the_key_is_read_from_what_the_server_saves() {
    let saved = serde_json::json!({"id": "bf36", "name": null, "accessToken": KEY, "publicUrl": null});
    assert_eq!(parse_machine_file(&saved.to_string()).as_deref(), Some(KEY));
}

#[test]
fn the_key_is_not_taken_from_what_is_not_a_key() {
    let half = &KEY[..31];
    assert_eq!(half.len(), 31);
    for (what, json) in [
        ("not JSON", "{".to_string()),
        ("no key", r#"{"id":"bf36"}"#.to_string()),
        ("a key that isn't text", r#"{"accessToken":12}"#.to_string()),
        ("a half-written key", format!(r#"{{"accessToken":"{half}"}}"#)),
        ("a list", "[]".to_string()),
    ] {
        assert_eq!(parse_machine_file(&json), None, "{what}");
    }
}

#[test]
fn the_key_is_read_from_disk_and_nothing_when_the_file_is_missing() {
    let dir = temp("disk");
    assert_eq!(read_access_token(&dir.join("fleet.db")), None);
    write_machine(&dir, "fleet.machine.json", KEY);
    assert_eq!(read_access_token(&dir.join("fleet.db")).as_deref(), Some(KEY));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_key_fleet_turned_away_is_reported_with_its_own_words() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    auth::set(None);
    let root = temp("turned-away");
    let data = root.join(".weave");
    let places = discover::all_places(Os::Linux, &env_of(&root));
    // A Fleet that answers 401 to the key it was given.
    let (base, _) = fleet(true);
    write_machine(&data, "fleet.machine.json", KEY);
    let found = Found { url: base.clone(), place: Place::Port(6262) };
    let outcome = local_token::adopt_required(&found, &places);
    assert_eq!(outcome, Outcome::Refused);
    assert_eq!(outcome.words(), Some(local_token::TURNED_AWAY));
    assert!(!auth::is_set() && auth::token_for(&base).is_none(), "a key Fleet refused is not kept");
    // A Fleet that doesn't answer has not accepted it either.
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let gone = format!("http://{}", dead.local_addr().unwrap());
    drop(dead);
    let found = Found { url: gone, place: Place::Port(6262) };
    assert_eq!(local_token::adopt_required(&found, &places), Outcome::Refused);
    // No readable key at all is the other message.
    std::fs::remove_dir_all(&data).unwrap();
    let outcome = local_token::adopt_required(&found, &places);
    assert!(matches!(outcome, Outcome::Missing(_)));
    assert_eq!(outcome.words(), Some(local_token::CANT_READ));
    assert!(local_token::CANT_READ.contains("fleet.machine.json") && !local_token::CANT_READ.contains(KEY));
    auth::note_local_lookup(Vec::new());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_key_fleet_accepts_is_kept_for_that_address_alone() {
    let _held = HOME_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    auth::set(None);
    let root = temp("accepted");
    write_machine(&root.join(".weave"), "fleet.machine.json", TOKEN);
    let places = discover::all_places(Os::Linux, &env_of(&root));
    let (base, seen) = fleet(true);
    let found = Found { url: base.clone(), place: Place::Port(6262) };
    assert_eq!(local_token::adopt_required(&found, &places), Outcome::Adopted);
    assert_eq!(auth::token_for(&base).unwrap().expose(), TOKEN);
    assert_eq!(seen.lock().unwrap().len(), 1, "the key goes with the one call that checks it, no call to learn it is needed first");
    auth::set(None);
    let _ = std::fs::remove_dir_all(&root);
}
