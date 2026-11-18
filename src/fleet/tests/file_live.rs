//! Keeping open files in step (component 11, piece C): `use-file-live-updates.test.ts` (7 cases, the two stop events as one
//! each) under Fleet's names, and `use-diff-base.test.ts` (4) over `Bases` and over a fake server. The queue is checked as a
//! rule (`Live`); what a read does is checked over a fake server that answers `/files/content` from what the test sets.
use super::super::events::{Event, Kind};
use super::super::file_base::{Bases, Found, MAX_CACHED};
use super::super::file_buffers::Status;
use super::super::file_live::{FILE_EVENT_DEBOUNCE, FLASH_SECONDS, Live, Queued, is_same_file};
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PATH: &str = "src/app.py";

// ─── the rules ──────────────────────────────────────────────────────────────

#[test]
fn matches_the_agents_absolute_paths_and_a_saves_relative_ones() {
    assert!(is_same_file("/home/me/repo/src/app.ts", "src/app.ts"));
    assert!(is_same_file("src/app.ts", "src/app.ts"));
    assert!(is_same_file("C:\\repo\\src\\app.ts", "src/app.ts"));
    assert!(!is_same_file("/repo/src/app.tsx", "src/app.ts"));
    assert!(!is_same_file("/repo/xsrc/app.ts", "src/app.ts"));
}

#[test]
fn a_burst_of_events_waits_for_it_to_settle_and_reads_each_file_once() {
    let mut live = Live::default();
    let one = |l: &mut Live| l.queue("s1", vec!["src/app.ts".into()], FILE_EVENT_DEBOUNCE);
    let Queued::Later(first) = one(&mut live) else { panic!("a wait") };
    let Queued::Later(second) = one(&mut live) else { panic!("a wait") };
    let Queued::Later(third) = one(&mut live) else { panic!("a wait") };
    assert_eq!(FILE_EVENT_DEBOUNCE, Duration::from_millis(500));
    // Each queue restarts the wait: the timers of the first two find it replaced.
    assert!(live.fire("s1", first).is_empty());
    assert!(live.fire("s1", second).is_empty());
    assert_eq!(live.fire("s1", third), ["src/app.ts"]);
    assert!(live.fire("s1", third).is_empty(), "read once");
}

#[test]
fn paths_of_one_burst_are_read_together_and_a_tab_or_focus_reads_at_once() {
    let mut live = Live::default();
    live.queue("s1", vec!["b.ts".into()], FILE_EVENT_DEBOUNCE);
    live.queue("s1", vec!["a.ts".into(), "b.ts".into()], FILE_EVENT_DEBOUNCE);
    // Straight away reads what was waiting too, and the wait that was running finds nothing left.
    assert_eq!(live.queue("s1", vec!["c.ts".into()], Duration::ZERO), Queued::Now(vec!["a.ts".into(), "b.ts".into(), "c.ts".into()]));
    assert_eq!(live.queue("s1", Vec::new(), Duration::ZERO), Queued::Nothing);
}

#[test]
fn another_session_starts_the_queue_afresh() {
    let mut live = Live::default();
    let Queued::Later(old) = live.queue("s1", vec!["a.ts".into()], FILE_EVENT_DEBOUNCE) else { panic!("a wait") };
    assert_eq!(live.queue("s2", vec!["b.ts".into()], Duration::ZERO), Queued::Now(vec!["b.ts".into()]));
    assert!(live.fire("s1", old).is_empty());
}

#[test]
fn the_lines_the_agent_changed_flash_for_one_point_six_seconds() {
    let mut live = Live::default();
    live.flash("home", "s1", "a.ts", (2, 3), 10.0);
    assert_eq!(live.flashing("home", "s1", "a.ts", 10.0).map(|f| (f.lines, f.progress)), Some(((2, 3), 0.0)));
    let half = live.flashing("home", "s1", "a.ts", 10.0 + FLASH_SECONDS / 2.0).unwrap();
    assert!((half.progress - 0.5).abs() < 1e-9);
    assert_eq!(live.flashing("home", "s1", "a.ts", 10.0 + FLASH_SECONDS + 0.01), None);
    assert_eq!(live.flashing("home", "s2", "a.ts", 10.0), None);
}

#[test]
fn a_tab_coming_forward_is_seen_once() {
    let mut live = Live::default();
    assert!(live.came_forward("s1", Some("a.ts")));
    assert!(!live.came_forward("s1", Some("a.ts")));
    assert!(!live.came_forward("s1", None), "Changes in front: nothing to read");
    assert!(live.came_forward("s1", Some("a.ts")), "and a.ts coming back is forward again");
    assert!(live.came_forward("s1", Some("b.ts")));
}

// ─── over a fake server ─────────────────────────────────────────────────────

#[derive(Default)]
struct Seen {
    /// What the file on disk is now.
    content: String,
    hash: String,
    /// Every `/files/content` read, by path.
    reads: Vec<String>,
    /// Every `/diffs/file` read, by path.
    bases: Vec<String>,
    base_status: u16,
    before: String,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if request.is_empty() {
            request = line.clone();
        }
        if line == "\r\n" {
            break;
        }
    }
    let path = request.split(' ').nth(1).unwrap_or("").to_string();
    let mut status = "200 OK";
    let body = if path.starts_with("/hubs/session-events/negotiate") {
        "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string()
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        fixture("api-sessions.json")
    } else if path.ends_with("/diffs") {
        json!({"diffs": [{"file": PATH, "status": "modified", "additions": 1, "deletions": 1}], "available": true,
            "base": {"kind": "branch", "branch": "main", "commit": "aaa1111"}})
        .to_string()
    } else if let Some((_, asked)) = path.split_once("/diffs/file?path=") {
        let mut seen = seen.lock().unwrap();
        seen.bases.push(asked.replace("%2F", "/"));
        if seen.base_status != 0 && seen.base_status != 200 {
            status = "404 Not Found";
        }
        json!({"file": PATH, "status": "modified", "before": seen.before, "after": "x"}).to_string()
    } else if let Some((_, asked)) = path.split_once("/files/content?path=") {
        let mut seen = seen.lock().unwrap();
        seen.reads.push(asked.replace("%2F", "/"));
        json!({"content": seen.content, "hash": seen.hash, "isBinary": false, "isTruncated": false}).to_string()
    } else {
        "{}".to_string()
    };
    let _ = stream.write_all(
        format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
            .as_bytes(),
    );
}

/// What a wait's condition looks at besides the state.
struct Probe {
    machine: String,
    id: String,
    reads: Vec<String>,
}

impl Probe {
    fn lines(&self, s: &State) -> Vec<String> {
        s.file_buffers.buffer(&self.machine, &self.id, PATH).unwrap().lines().to_vec()
    }
}

struct Fake {
    rt: rune::Runtime<State>,
    id: String,
    machine: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

impl Fake {
    /// A Fleet with `src/app.py` open and read ("one\ntwo\n", hash h1); the read that opened it isn't counted.
    fn start() -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen { content: "one\ntwo\n".into(), hash: "h1".into(), ..Default::default() }));
        let shared = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = shared.clone();
                std::thread::spawn(move || serve(stream, seen));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
        });
        let id = rt.state.open_id().unwrap();
        let machine = rt.state.machine_key_of(&id).to_string();
        let mut fake = Fake { rt, id, machine, jobs, seen };
        let session = fake.id.clone();
        fake.rt.update(move |s, _| s.open_file_buffer(&session, PATH));
        fake.wait("the file to be read", |s, p| s.file_buffers.info(&p.machine, &p.id, PATH).is_some_and(|i| i.status == Status::Ready));
        fake.seen.lock().unwrap().reads.clear();
        fake
    }

    fn lines(&self) -> Vec<String> {
        self.rt.state.file_buffers.buffer(&self.machine, &self.id, PATH).unwrap().lines().to_vec()
    }

    fn reads(&self) -> Vec<String> {
        self.seen.lock().unwrap().reads.clone()
    }

    fn set_disk(&self, content: &str, hash: &str) {
        let mut seen = self.seen.lock().unwrap();
        seen.content = content.into();
        seen.hash = hash.into();
    }

    /// Runs the app until `done` holds, looking at the state and at what the server has been asked.
    fn wait(&mut self, what: &str, done: impl Fn(&State, &Probe) -> bool) {
        let (seen, described) = (self.seen.clone(), self.seen.clone());
        let (machine, id) = (self.machine.clone(), self.id.clone());
        crate::test_wait::drive(
            &mut self.rt,
            &self.jobs,
            what,
            |s| {
                let reads = seen.lock().unwrap().reads.clone();
                done(s, &Probe { machine: machine.clone(), id: id.clone(), reads })
            },
            |s| format!("{}; reads {:?}", crate::test_wait::app_state(s), described.lock().unwrap().reads),
        );
    }

    fn emit(&mut self, event: Event) {
        let id = self.id.clone();
        self.rt.update(move |s, _| s.dispatch(&Topic::Session(id), &[event]));
    }

    fn files_changed(&mut self, paths: &[&str]) {
        let files: Vec<Value> = paths.iter().map(|p| json!({"path": p, "changeType": "change"})).collect();
        let id = self.id.clone();
        self.emit(Event::Other(Kind::FilesChanged, json!({"sessionId": id, "files": files})));
    }

    fn pulsed(&self) -> bool {
        self.rt.state.canvases.updated_at(&format!("file:{PATH}")).is_some()
    }
}

#[test]
fn a_clean_buffer_takes_the_agents_change_in_place_and_the_tab_pulses() {
    let mut f = Fake::start();
    f.set_disk("one\nTWO\n", "h2");
    f.files_changed(&["/repo/src/app.py"]);
    assert!(f.reads().is_empty(), "the read waits for the burst to settle");
    f.wait("the changed file to be read again", |s, p| p.lines(s) == ["one", "TWO", ""]);
    assert_eq!(f.reads(), [PATH]);
    assert!(f.pulsed());
    // The lines the agent changed flash, from the clock of the update.
    let flash = f.rt.state.file_live.flashing(&f.machine, &f.id, PATH, f.rt.state.clock).expect("a flash");
    assert_eq!(flash.lines, (2, 2));
}

#[test]
fn a_dirty_buffer_isnt_touched_it_gets_the_conflict_bar() {
    let mut f = Fake::start();
    let (m, id) = (f.machine.clone(), f.id.clone());
    f.rt.update(move |s, _| {
        s.file_buffers.buffers_for_test(&m, &id, PATH).lines[0] = "mine one".into();
        s.file_buffers.patch(&m, &id, PATH, super::super::file_buffers::Patch { dirty: Some(true), ..Default::default() });
    });
    f.set_disk("agent's\n", "h2");
    f.files_changed(&["/repo/src/app.py"]);
    f.wait("the conflict", |s, p| s.file_buffers.info(&p.machine, &p.id, PATH).unwrap().conflict.is_some());
    assert_eq!(f.lines(), ["mine one", "two", ""]);
    let conflict = f.rt.state.file_buffers.info(&f.machine, &f.id, PATH).unwrap().conflict.clone().unwrap();
    assert_eq!((conflict.disk_text.as_deref(), conflict.hash.as_str()), (Some("agent's\n"), "h2"));
    assert!(f.pulsed(), "a conflict pulses the tab too");
}

#[test]
fn changes_nothing_when_the_file_on_disk_still_has_the_same_hash() {
    let mut f = Fake::start();
    f.files_changed(&["src/app.py"]);
    f.wait("the read to be applied", |s, p| !p.reads.is_empty() && !s.file_buffers.is_reading(&p.machine, &p.id, PATH));
    // A sentinel read after it proves nothing else came.
    f.rt.update(|s, _| s.file_window_focus());
    f.wait("the second read", |s, p| p.reads.len() == 2 && !s.file_buffers.is_reading(&p.machine, &p.id, PATH));
    assert_eq!(f.lines(), ["one", "two", ""]);
    assert!(!f.pulsed());
    assert!(f.rt.state.file_live.flashing(&f.machine, &f.id, PATH, f.rt.state.clock).is_none());
}

#[test]
fn doesnt_read_files_that_arent_open_and_reads_a_burst_of_events_once() {
    let mut f = Fake::start();
    f.files_changed(&["/repo/src/other.py"]);
    f.files_changed(&["/repo/src/app.py"]);
    f.files_changed(&["/repo/src/app.py"]);
    f.files_changed(&["/repo/src/app.py"]);
    f.wait("the burst to be read", |s, p| !p.reads.is_empty() && !s.file_buffers.is_reading(&p.machine, &p.id, PATH));
    // A sentinel after the burst: the one more read it asks for is the only other one there is.
    f.rt.update(|s, _| s.file_window_focus());
    f.wait("the sentinel read", |_, p| p.reads.len() >= 2);
    assert_eq!(f.reads(), [PATH, PATH], "other.py never, and the burst once");
}

#[test]
fn re_reads_open_files_when_the_agent_stops_turn_ended_which_catches_shell_edits() {
    let mut f = Fake::start();
    f.set_disk("sed did this\n", "h3");
    f.emit(Event::TurnEnded);
    f.wait("the file to be read again", |s, p| p.lines(s) == ["sed did this", ""]);
    // One turn ending is one read of the one open file.
    assert_eq!(f.reads(), [PATH]);
    println!("requests after turn.ended: {:?}", f.reads());
}

#[test]
fn re_reads_open_files_when_the_agent_stops_session_idled_which_catches_shell_edits() {
    let mut f = Fake::start();
    f.set_disk("sed did this\n", "h3");
    f.emit(Event::SessionIdled);
    f.wait("the file to be read again", |s, p| p.lines(s) == ["sed did this", ""]);
}

#[test]
fn re_reads_when_the_window_gets_focus_back_without_waiting() {
    let mut f = Fake::start();
    let started = std::time::Instant::now();
    f.rt.update(|s, _| s.file_window_focus());
    f.wait("the file to be read", |_, p| p.reads.len() == 1);
    assert!(started.elapsed() < FILE_EVENT_DEBOUNCE, "no 500 ms wait");
}

#[test]
fn a_file_tab_coming_to_the_front_is_read_at_once() {
    let mut f = Fake::start();
    let id = f.id.clone();
    f.rt.update(move |s, _| {
        s.canvases.open_changes(&id, None);
        s.keep_file_front();
    });
    assert!(f.reads().is_empty(), "Changes is in front: no file");
    let (id, tab) = (f.id.clone(), format!("file:{PATH}"));
    f.rt.update(move |s, _| {
        let mut tabs = s.canvases.session(&id);
        tabs.canvases.push(super::super::canvases::CanvasInstance { id: tab.clone(), ..tabs.canvases[0].clone() });
        s.canvases.replace(&id, tabs);
        s.canvases.activate(&id, &tab);
        s.keep_file_front();
    });
    f.wait("the file to be read", |_, p| p.reads.len() == 1);
}

#[test]
fn listens_on_the_sessions_own_topic() {
    let mut f = Fake::start();
    let id = f.id.clone();
    // Another session's topic and the sessions topic say nothing about this one's files.
    f.rt.update(|s, _| {
        s.dispatch(&Topic::Session("other".into()), &[Event::TurnEnded]);
        s.dispatch(&Topic::Sessions, &[Event::TurnEnded]);
    });
    // The relayed event of this session's topic names the harness's own session id and still counts.
    f.rt.update(move |s, _| {
        s.dispatch(
            &Topic::Session(id),
            &[Event::Other(Kind::FilesChanged, json!({"sessionID": "ses_opencode", "files": [{"path": PATH}]}))],
        )
    });
    f.wait("the read", |_, p| !p.reads.is_empty());
    assert_eq!(f.reads(), [PATH]);
}

// ─── use-diff-base.test.ts ──────────────────────────────────────────────────

fn answer(before: &str) -> Result<(u16, Value), String> {
    Ok((200, json!({"file": "src/a.ts", "status": "modified", "additions": 1, "deletions": 1, "before": before, "after": "x"})))
}

fn ask(b: &mut Bases, base: &str, before: &str) -> Option<String> {
    if let Found::Ask(request) = b.fetch("home", "s1", base, "src/a.ts") {
        b.finish(&request, answer(before));
    }
    b.get("home", "s1", base, "src/a.ts").map(str::to_string)
}

#[test]
fn fetches_a_changed_files_base_once_and_reuses_it() {
    let mut b = Bases::default();
    let Found::Ask(request) = b.fetch("home", "s1", "", "src/a.ts") else { panic!("a read") };
    assert_eq!(b.fetch("home", "s1", "", "src/a.ts"), Found::Known, "on its way: a second ask waits");
    b.finish(&request, answer("old\n"));
    assert_eq!(b.get("home", "s1", "", "src/a.ts"), Some("old\n"));
    assert_eq!(b.fetch("home", "s1", "", "src/a.ts"), Found::Known, "kept: no second read");
}

#[test]
fn reads_a_files_base_again_once_what_the_changes_are_compared_with_moves() {
    let mut b = Bases::default();
    assert_eq!(ask(&mut b, "branch:aaa1111", "old main\n").as_deref(), Some("old main\n"));
    assert_eq!(ask(&mut b, "branch:bbb2222", "newer main\n").as_deref(), Some("newer main\n"));
    // The first is still kept under its own key.
    assert_eq!(b.get("home", "s1", "branch:aaa1111", "src/a.ts"), Some("old main\n"));
}

#[test]
fn doesnt_cache_a_failed_read() {
    let mut b = Bases::default();
    let Found::Ask(request) = b.fetch("home", "s1", "", "src/a.ts") else { panic!("a read") };
    assert!(!b.finish(&request, Ok((404, Value::Null))));
    assert_eq!(b.get("home", "s1", "", "src/a.ts"), None);
    assert_eq!(ask(&mut b, "", "old\n").as_deref(), Some("old\n"), "the next ask reads again");
    // A failure of the network is the same.
    let mut b = Bases::default();
    let Found::Ask(request) = b.fetch("home", "s1", "", "src/a.ts") else { panic!("a read") };
    b.finish(&request, Err("down".into()));
    assert!(matches!(b.fetch("home", "s1", "", "src/a.ts"), Found::Ask(_)));
}

#[test]
fn keeps_the_latest_fifty_and_an_added_file_has_an_empty_base() {
    let mut b = Bases::default();
    for n in 0..MAX_CACHED + 1 {
        let Found::Ask(request) = b.fetch("home", "s1", "", &format!("f{n}.ts")) else { panic!("a read") };
        b.finish(&request, Ok((200, json!({"file": format!("f{n}.ts"), "status": "added"}))));
    }
    assert_eq!(b.get("home", "s1", "", "f0.ts"), None, "the oldest went");
    assert_eq!(b.get("home", "s1", "", "f1.ts"), Some(""), "no `before` is an empty base");
    assert_eq!(b.get("home", "s1", "", &format!("f{MAX_CACHED}.ts")), Some(""));
    // Another machine or session is another file.
    assert_eq!(b.get("work", "s1", "", "f1.ts"), None);
    assert_eq!(b.get("home", "s2", "", "f1.ts"), None);
}

#[test]
fn only_loads_while_the_file_is_among_the_changes() {
    let mut f = Fake::start();
    f.seen.lock().unwrap().before = "old\n".into();
    let (id, m) = (f.id.clone(), f.machine.clone());
    // A file that isn't among the changes: nothing is read and there is no base.
    f.rt.update({
        let id = id.clone();
        move |s, _| s.load_file_base(&id, "src/untouched.py")
    });
    assert_eq!(f.rt.state.file_base(&id, "src/untouched.py"), None);
    // The session's changes arrive (the open session's list was asked for by opening it).
    f.wait("the changes", |s, _| s.diffs.by_file(PATH).is_some());
    f.rt.update({
        let id = id.clone();
        move |s, _| {
            s.load_file_base(&id, PATH);
            s.load_file_base(&id, PATH);
        }
    });
    f.wait("the base", |s, p| s.file_base(&p.id, PATH).is_some());
    assert_eq!(f.rt.state.file_base(&id, PATH), Some("old\n"));
    assert_eq!(f.seen.lock().unwrap().bases, [PATH], "asked once for two loads, and never for the file that isn't changed");
    assert!(f.rt.state.file_has_changes(&id, PATH));
    // An image is never in the changes' base.
    assert_eq!(f.rt.state.file_base(&id, "assets/logo.svg"), None);
    let _ = m;
    println!("requests for the base: {:?}", f.seen.lock().unwrap().bases);
}
