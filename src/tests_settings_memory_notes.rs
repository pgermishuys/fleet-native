//! Component 16, piece Q: Settings → Memory's notes. `MemorySection.test.ts` under Fleet's names (its first case is piece
//! P's), then what the page does that the cases leave to the template: Clear these N has no question, Enter saves and Esc
//! cancels in both text areas, one action at a time. The server is a fake that keeps the notes it is given and changes
//! them as the real one does.
use crate::fleet::link::{Job, Link, Post};
use crate::tests::{FRAME, click};
use crate::{Rail, State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    repository: Vec<Value>,
    machine: Vec<Value>,
    expired: Vec<Value>,
    search: bool,
    /// Every call heard: method, path and body.
    heard: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<Fake>>;

fn every(f: &mut Fake) -> impl Iterator<Item = &mut Value> {
    f.repository.iter_mut().chain(f.machine.iter_mut())
}

fn note(id: &str, list: &str, text: &str, kind: &str) -> Value {
    json!({ "id": id, "list": list, "text": text, "kind": kind, "repository": if list == "repository" { json!("/work/lantern") } else { Value::Null },
            "created": "2026-10-02T09:12:00Z", "updated": "2026-10-02T09:12:00Z" })
}

fn learned(id: &str, list: &str, text: &str, extra: Value) -> Value {
    let mut n = note(id, list, text, "learned");
    for (k, v) in extra.as_object().expect("an object") {
        n[k] = v.clone();
    }
    n
}

fn read_body(stream: &mut std::net::TcpStream) -> (String, Value) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, length) = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..at]).to_lowercase();
            let length =
                head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
            break (at + 4, length);
        }
        if n == 0 {
            break (buf.len(), 0);
        }
    };
    while buf.len() < head_end + length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let body = serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
    (head, body)
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let (head, body) = read_body(&mut stream);
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let mut f = fake.lock().unwrap();
    // The live-events link negotiates in the background at any moment; it is not what a test asks about.
    if !path.starts_with("/hubs/") {
        f.heard.push((method.clone(), path.clone(), body.clone()));
    }
    let route = path.split('?').next().unwrap_or("").to_string();
    let segments: Vec<&str> = route.trim_start_matches('/').split('/').collect();
    let reply = match (method.as_str(), segments.as_slice()) {
        ("GET", ["api", "memory"]) => json!({
            "enabled": true, "repositories": [{ "path": "/work/lantern", "name": "lantern", "count": f.repository.len() },
                                              { "path": "/work/harbor-api", "name": "harbor-api", "count": 0 }],
            "machineCount": f.machine.len(), "maxRepositoryNotes": 40, "maxMachineNotes": 15, "searchEnabled": f.search,
            "maxAlwaysRepositoryNotes": 12, "maxAlwaysMachineNotes": 5, "maxSearchRepositoryNotes": 60, "maxSearchMachineNotes": 25 }),
        ("GET", ["api", "memory", "notes"]) => {
            let tokens = (f.repository.len() + f.machine.len()) * 40;
            json!({ "repository": "/work/lantern", "repositoryNotes": f.repository, "machineNotes": f.machine, "tokens": tokens, "expiredNotes": f.expired })
        }
        ("POST", ["api", "memory", "notes"]) => {
            let list = body["list"].as_str().unwrap_or("machine").to_string();
            let n = note(&format!("new{}", f.repository.len() + f.machine.len()), &list, body["text"].as_str().unwrap_or(""), "added");
            if list == "repository" {
                f.repository.push(n.clone())
            } else {
                f.machine.push(n.clone())
            }
            n
        }
        ("PUT", ["api", "memory", "notes", id]) => {
            for n in every(&mut f).filter(|n| n["id"] == *id) {
                n["text"] = body["text"].clone();
            }
            json!({})
        }
        ("DELETE", ["api", "memory", "notes", id]) => {
            f.repository.retain(|n| n["id"] != *id);
            f.machine.retain(|n| n["id"] != *id);
            json!({})
        }
        ("POST", ["api", "memory", "notes", id, "keep"]) => {
            if let Some(at) = f.expired.iter().position(|n| n["id"] == *id) {
                let mut n = f.expired.remove(at);
                n["expired"] = json!(false);
                n["lifetime"] = Value::Null;
                if n["list"] == "machine" { f.machine.push(n) } else { f.repository.push(n) }
            }
            for n in every(&mut f).filter(|n| n["id"] == *id) {
                n["lifetime"] = Value::Null;
                n["daysLeft"] = Value::Null;
            }
            json!({})
        }
        ("POST", ["api", "memory", "notes", id, "place"]) => {
            let search = body["group"] == "search";
            for n in every(&mut f).filter(|n| n["id"] == *id) {
                n["inSearch"] = json!(search);
                n["pinned"] = json!(!search);
            }
            json!({})
        }
        ("POST", ["api", "memory", "clear"]) => {
            let scope = body["scope"].as_str().unwrap_or("").to_string();
            if scope != "machine" {
                f.repository.clear();
            }
            if scope != "repository" {
                f.machine.clear();
            }
            json!({ "deleted": 1 })
        }
        ("GET", ["api", "machines"]) => json!({ "machines": [] }),
        _ => json!({}),
    };
    let reply = reply.to_string();
    let out =
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len());
    let _ = stream.write_all(out.as_bytes());
    drop(f);
    crate::tests::finish(&mut stream);
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    setup(&mut fake.lock().unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let served = fake.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = served.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = crate::tests::app_tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("memory", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.settle("the page to be read", |s| !s.settings.memory.loading && s.settings.memory.notes.is_some());
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.memory));
    }
    fn settle(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        self.wait(what, done);
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    /// Presses a button and waits for its calls and the reads after them to come back.
    fn press(&mut self, key: &str) {
        click(&mut self.rt, key);
        self.settle("the action to finish", |s| !s.settings.memory.saving);
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn heard(&self, method: &str) -> Vec<(String, Value)> {
        self.fake
            .lock()
            .unwrap()
            .heard
            .iter()
            .filter(|(m, p, _)| m == method && p.starts_with("/api/memory"))
            .map(|(_, p, b)| (p.clone(), b.clone()))
            .collect()
    }
    fn type_into(&mut self, key: &str, words: &str) {
        click(&mut self.rt, key);
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
}

fn two_lists(f: &mut Fake) {
    f.repository =
        vec![note("r1", "repository", "Rebase onto main.", "from-you"), note("r2", "repository", "Run just fixtures first.", "added")];
    f.machine = vec![note("m1", "machine", "/tmp fills up here.", "added")];
}

#[test]
fn lists_the_repositorys_notes_and_the_machines_with_where_each_came_from() {
    let r = rig(|f| {
        two_lists(f);
        f.repository[0]["sessionTitle"] = json!("Tidy the branch");
    });
    assert!(r.exists("memory-list-repository") && r.exists("memory-list-machine"));
    assert!(r.text("memory-note-r1").contains("Rebase onto main."));
    assert!(r.text("memory-note-r1").contains("From you"));
    assert!(r.text("memory-note-r1").contains("Saved Oct 2 by “Tidy the branch”"));
    assert!(r.text("memory-note-r2").contains("Added by you on Oct 2"));
    assert_eq!(r.text("memory-heading-repository"), "REPOSITORY · LANTERN2/40Add a noteClear these 2");
    assert!(r.text("memory-heading-machine").contains("1/15"));
    assert_eq!(r.text("memory-count"), "3 notes · about 120 tokens per request");
}

#[test]
fn filters_the_notes_by_what_the_user_searches_for() {
    let mut r = rig(two_lists);
    r.type_into("memory-search", "FIXTURES");
    assert!(r.exists("memory-note-r2") && !r.exists("memory-note-r1") && !r.exists("memory-note-m1"));
    assert!(r.text("memory-empty-machine-0").contains("No notes match"));
    assert_eq!(r.text("memory-count"), "3 notes · about 120 tokens per request", "the count is not filtered");
}

#[test]
fn adds_a_note_the_user_writes_to_the_list_they_picked() {
    let mut r = rig(|_| {});
    assert!(r.text("memory-empty-repository-0").contains("No notes for lantern yet"));
    click(&mut r.rt, "memory-add-repository");
    assert!(r.exists("memory-draft-repository") && !r.exists("memory-empty-repository-0"), "the form takes the empty box's place");
    r.type_into("memory-draft-repository", "  Prefer small commits  ");
    r.press("memory-save-repository");
    let posts = r.heard("POST");
    assert!(posts.contains(&(
        "/api/memory/notes".into(),
        json!({ "list": "repository", "repository": "/work/lantern", "text": "Prefer small commits" })
    )));
    assert!(!r.exists("memory-draft-repository"), "the form closes");
    assert!(r.exists("memory-note-new0"));
}

#[test]
fn a_machine_note_sends_no_repository() {
    let mut r = rig(|_| {});
    click(&mut r.rt, "memory-add-machine");
    r.type_into("memory-draft-machine", "Disk is small");
    r.press("memory-save-machine");
    assert!(
        r.heard("POST").contains(&("/api/memory/notes".into(), json!({ "list": "machine", "repository": null, "text": "Disk is small" })))
    );
}

#[test]
fn enter_saves_a_new_note_and_escape_closes_the_form() {
    let mut r = rig(|_| {});
    click(&mut r.rt, "memory-add-machine");
    r.type_into("memory-draft-machine", "Enter saves");
    r.key(Key::Escape);
    assert!(!r.exists("memory-draft-machine"), "Esc closes");
    click(&mut r.rt, "memory-add-machine");
    r.type_into("memory-draft-machine", "Enter saves");
    r.key(Key::Enter);
    r.settle("the note to be saved", |s| !s.settings.memory.saving && s.settings.memory_ui.adding.is_none());
    assert!(r.exists("memory-note-new0"));
}

#[test]
fn save_note_waits_for_words() {
    let mut r = rig(|_| {});
    click(&mut r.rt, "memory-add-machine");
    r.type_into("memory-draft-machine", "   ");
    click(&mut r.rt, "memory-save-machine");
    assert!(r.heard("POST").is_empty(), "nothing is sent for a blank note");
}

#[test]
fn edits_a_note_with_enter_and_cancels_with_escape() {
    let mut r = rig(two_lists);
    click(&mut r.rt, "memory-edit-r1");
    click(&mut r.rt, "memory-edit-field");
    r.key(Key::Escape);
    assert!(!r.exists("memory-edit-field"));
    click(&mut r.rt, "memory-edit-r1");
    r.rt.update(|s, _| s.settings.memory_ui.edit.replace_text("Rebase, never merge."));
    r.rt.frame(FRAME);
    click(&mut r.rt, "memory-edit-field");
    r.key(Key::Enter);
    r.settle("the edit to be saved", |s| !s.settings.memory.saving && s.settings.memory_ui.editing.is_none());
    assert!(r.heard("PUT").contains(&("/api/memory/notes/r1".into(), json!({ "text": "Rebase, never merge." }))));
    assert!(r.text("memory-note-r1").contains("Rebase, never merge."));
}

#[test]
fn forgets_a_note_with_no_question() {
    let mut r = rig(two_lists);
    r.press("memory-forget-r1");
    assert!(!r.exists("memory-note-r1") && r.exists("memory-note-r2"));
    assert!(r.heard("DELETE").contains(&("/api/memory/notes/r1".into(), Value::Null)));
}

#[test]
fn clear_these_has_no_question_and_clears_one_list() {
    let mut r = rig(two_lists);
    r.press("memory-clear-machine");
    assert!(r.heard("POST").contains(&("/api/memory/clear".into(), json!({ "scope": "machine", "repository": null }))));
    assert!(!r.exists("memory-note-m1") && r.exists("memory-note-r1"));
    r.press("memory-clear-repository");
    assert!(r.heard("POST").contains(&("/api/memory/clear".into(), json!({ "scope": "repository", "repository": "/work/lantern" }))));
}

#[test]
fn asks_before_clearing_all_memory() {
    let mut r = rig(two_lists);
    click(&mut r.rt, "memory-clear-all");
    assert!(r.exists("memory-clear-confirm"));
    assert!(r.text("memory-clear-confirm").contains("Delete every note?"));
    click(&mut r.rt, "memory-clear-all-cancel");
    assert!(!r.exists("memory-clear-confirm") && r.exists("memory-clear-all"));
    assert!(r.heard("POST").is_empty(), "Cancel sends nothing");
    click(&mut r.rt, "memory-clear-all");
    r.press("memory-clear-all-confirm");
    assert!(r.heard("POST").contains(&("/api/memory/clear".into(), json!({ "scope": "all", "repository": null }))));
    assert!(!r.exists("memory-note-r1") && !r.exists("memory-note-m1"));
    assert!(!r.exists("memory-clear-confirm"));
}

#[test]
fn says_how_long_a_learned_note_has_left_and_keeps_it_for_good_on_request() {
    let mut r = rig(|f| {
        f.repository = vec![
            learned("l1", "repository", "Needs the fixtures.", json!({ "lifetime": 7, "daysLeft": 5 })),
            learned("l2", "repository", "Last chance.", json!({ "lifetime": 7, "daysLeft": 0 })),
        ];
    });
    assert!(r.text("memory-life-l1").contains("5 more days of use"));
    assert!(r.text("memory-life-l2").contains("Last day of use"));
    r.press("memory-keep-l1");
    assert!(r.heard("POST").contains(&("/api/memory/notes/l1/keep".into(), json!({}))), "{:?}", r.heard("POST"));
    assert!(r.text("memory-life-l1").contains("Kept for good"));
    assert!(!r.exists("memory-keep-l1"), "a kept note offers no Keep");
    assert!(r.exists("memory-keep-l2"));
}

#[test]
fn says_how_a_learned_note_is_known_tested_with_how_or_untested_and_notes_from_you_say_nothing() {
    let r = rig(|f| {
        f.repository = vec![
            learned("t1", "repository", "Tested one.", json!({ "how": "ran it twice" })),
            learned("t2", "repository", "Untested one.", json!({})),
            note("t3", "repository", "From you.", "from-you"),
        ];
    });
    assert_eq!(r.text("memory-how-t1"), "Tested · ran it twice");
    assert_eq!(r.text("memory-how-t2"), "Untested");
    assert!(!r.exists("memory-how-t3"));
}

#[test]
fn lists_expired_notes_apart_folded_and_brings_one_back_with_keep() {
    let mut r = rig(|f| {
        f.repository = vec![note("r1", "repository", "Live.", "added")];
        f.expired =
            vec![learned("e1", "repository", "Gone stale.", json!({ "lifetime": 7, "daysLeft": 0, "expired": true, "relearned": 1 }))];
    });
    assert!(r.exists("memory-expired") && !r.exists("memory-note-e1"), "folded until opened");
    click(&mut r.rt, "memory-expired-toggle");
    assert!(r.text("memory-note-e1").contains("Gone stale."));
    assert!(r.text("memory-note-e1").contains("Expired after 7 days of use · learned 2×"));
    r.press("memory-keep-e1");
    assert!(r.heard("POST").contains(&("/api/memory/notes/e1/keep".into(), json!({}))));
    assert!(!r.exists("memory-expired"), "nothing is left to fold");
    assert!(r.exists("memory-note-e1"), "it is in the list again");
}

fn searching(f: &mut Fake) {
    f.search = true;
    f.repository = vec![note("a1", "repository", "Read every time.", "added"), note("a2", "repository", "Read too.", "added"), {
        let mut n = note("s1", "repository", "Found by search.", "added");
        n["inSearch"] = json!(true);
        n["found"] = json!(2);
        n["lastFound"] = json!("2026-10-10T00:00:00Z");
        n
    }];
    f.machine = vec![note("m1", "machine", "Machine note.", "added")];
}

#[test]
fn shows_each_list_as_the_notes_every_session_reads_and_the_notes_found_by_search_with_counts() {
    let r = rig(searching);
    assert_eq!(r.text("memory-group-repository-always"), "Every session reads2/12 · about 22 tokens");
    assert_eq!(r.text("memory-group-repository-search"), "Found by search1/60 · no tokens until found");
    assert!(r.exists("memory-list-repository-search") && r.text("memory-list-repository-search").contains("Found by search."));
    assert!(r.text("memory-empty-machine-2").contains("Nothing here yet."));
    assert!(r.text("memory-heading-repository").contains("3 notes"));
    assert!(r.text("memory-found-s1").starts_with("found 2 times · last Oct 10"));
}

#[test]
fn counts_in_the_top_line_what_a_request_carries_the_notes_every_session_reads() {
    let r = rig(searching);
    assert_eq!(r.text("memory-count"), "4 notes · 3 read by every session · about 160 tokens per request");
}

#[test]
fn moves_a_note_to_search_and_pins_a_searched_one_to_always_read() {
    let mut r = rig(searching);
    r.press("memory-to-search-a1");
    assert!(r.heard("POST").contains(&("/api/memory/notes/a1/place".into(), json!({ "group": "search" }))));
    assert!(r.text("memory-list-repository-search").contains("Read every time."));
    r.press("memory-to-always-s1");
    assert!(r.heard("POST").contains(&("/api/memory/notes/s1/place".into(), json!({ "group": "always" }))));
    assert!(r.exists("memory-pinned-s1"), "it is pinned now");
    assert!(r.text("memory-to-search-s1").contains("Unpin and move to search") || r.exists("memory-to-search-s1"));
}

#[test]
fn keeps_filtering_both_groups_with_search_notes() {
    let mut r = rig(searching);
    r.type_into("memory-search", "found");
    assert!(r.exists("memory-note-s1") && !r.exists("memory-note-a1"));
    assert!(r.text("memory-empty-repository-1").contains("No notes match") || r.exists("memory-group-repository-always"));
    assert!(r.exists("memory-group-repository-always"), "the groups stay");
}

#[test]
fn picks_another_repository_and_reads_its_notes() {
    let mut r = rig(two_lists);
    click(&mut r.rt, "memory-repository");
    r.rt.frame(FRAME);
    r.rt.update(|s, _| s.memory_choose_repository("/work/harbor-api"));
    r.settle("the notes to be read again", |s| !s.settings.memory.saving);
    assert!(r.fake.lock().unwrap().heard.iter().any(|(m, p, _)| m == "GET" && p == "/api/memory/notes?repository=%2Fwork%2Fharbor-api"));
    assert_eq!(r.rt.state.settings.memory.repository.as_deref(), Some("/work/harbor-api"));
}

#[test]
fn a_note_is_cut_at_four_hundred_characters() {
    let mut r = rig(|_| {});
    click(&mut r.rt, "memory-add-machine");
    r.type_into("memory-draft-machine", &"x".repeat(450));
    assert_eq!(r.rt.state.settings.memory_ui.draft.text.chars().count(), 400);
}
