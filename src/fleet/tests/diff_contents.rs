//! One file's contents (component 12, piece C): `use-file-diff-contents.test.ts` under Fleet's names, over `Contents` itself
//! (a rule needs no thread), then the same over a fake server for the threads and the at-most-4 limit.
use super::super::diff_contents::*;
use super::super::diffs::{FileDiff, FileStatus};
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::reconnect::app;
use super::*;
use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

fn item(path: &str) -> (u16, Value) {
    (200, json!({"file": path, "status": "modified", "additions": 2, "deletions": 1, "before": "x", "after": "y"}))
}

const V: &str = "modified:2:1";

fn ready(c: &Contents, path: &str) -> bool {
    matches!(c.entry(path), Some(Entry::Ready(_)))
}

// ─── use-file-diff-contents.test.ts ─────────────────────────────────────────

#[test]
fn draws_a_file_it_already_fetched_at_once_when_the_panel_is_reopened() {
    let mut c = Contents::default();
    let first = c.load("home", "s-reopen", "src/a.ts", V).unwrap();
    c.finish(&first, Ok(item("src/a.ts")));
    assert!(ready(&c, "src/a.ts"));
    // The panel closes and a session switched to and back to draws what it had at once, and still asks again quietly.
    c.load("home", "s-elsewhere", "src/b.ts", V);
    assert_eq!(c.entry("src/a.ts"), None);
    let again = c.load("home", "s-reopen", "src/a.ts", V).unwrap();
    assert!(ready(&c, "src/a.ts"));
    c.finish(&again, Ok(item("src/a.ts")));
    assert!(ready(&c, "src/a.ts"));
}

#[test]
fn asks_for_at_most_four_files_at_once_so_a_long_list_cant_hold_up_the_apps_other_requests() {
    let mut c = Contents::default();
    let mut waiting: VecDeque<Request> = VecDeque::new();
    for i in 0..10 {
        waiting.extend(c.load("limit-test", "s-many", &format!("src/f{i}.ts"), V));
    }
    assert_eq!((waiting.len(), c.in_flight("limit-test"), c.queued("limit-test")), (4, 4, 6));
    assert_eq!(c.entry("src/f9.ts"), Some(&Entry::Loading));
    let mut asked = 4;
    while let Some(request) = waiting.pop_front() {
        let next = c.finish(&request, Ok(item(&request.path)));
        asked += usize::from(next.is_some());
        waiting.extend(next);
        assert!(c.in_flight("limit-test") <= MAX_IN_FLIGHT);
    }
    assert_eq!((asked, c.in_flight("limit-test"), c.queued("limit-test")), (10, 0, 0));
    assert!((0..10).all(|i| ready(&c, &format!("src/f{i}.ts"))));
}

#[test]
fn doesnt_show_one_sessions_file_in_another() {
    let mut c = Contents::default();
    let a = c.load("home", "s-one", "src/a.ts", V).unwrap();
    c.load("home", "s-other", "src/a.ts", V);
    assert_eq!(c.entry("src/a.ts"), Some(&Entry::Loading));
    // The first session's answer arrives late: it is not shown in the other, though the next visit finds it cached.
    c.finish(&a, Ok(item("src/a.ts")));
    assert_eq!(c.entry("src/a.ts"), Some(&Entry::Loading));
}

// ─── the rules behind them ──────────────────────────────────────────────────

#[test]
fn load_does_nothing_for_a_version_already_there_and_reload_always_asks() {
    let mut c = Contents::default();
    let r = c.load("home", "s", "a", V).unwrap();
    assert!(c.load("home", "s", "a", V).is_none() || c.in_flight("home") == 1);
    c.finish(&r, Ok(item("a")));
    assert!(c.load("home", "s", "a", V).is_none());
    let again = c.reload("home", "s", "a", V).unwrap();
    // What is drawn stays drawn while it is asked again.
    assert!(ready(&c, "a"));
    c.finish(&again, Ok((200, json!({"file": "a", "before": "new", "after": "newer"}))));
    let Some(Entry::Ready(file)) = c.entry("a") else { panic!("not ready") };
    assert_eq!((file.before.as_str(), file.after.as_str()), ("new", "newer"));
}

#[test]
fn a_newer_version_replaces_a_request_still_waiting_and_an_older_answer_is_dropped() {
    let mut c = Contents::default();
    let mut running: Vec<Request> = (0..4).filter_map(|i| c.load("home", "s", &format!("f{i}"), V)).collect();
    // `late` waits for a place; the agent edits it again before one frees.
    assert!(c.load("home", "s", "late", "modified:1:0").is_none());
    assert!(c.load("home", "s", "late", "modified:2:0").is_none());
    assert_eq!(c.queued("home"), 2);
    let next = c.finish(&running.remove(0), Ok(item("f0"))).unwrap();
    // The superseded one was skipped: the freed place goes to the newer version.
    assert_eq!((next.path.as_str(), c.queued("home")), ("late", 0));
    // An older answer for a file that has moved on is dropped.
    let stale = running.remove(0);
    c.load("home", "s", "f1", "modified:9:9");
    c.finish(&stale, Ok(item("f1")));
    assert_eq!(c.entry("f1"), Some(&Entry::Loading));
}

#[test]
fn a_refusal_or_a_failure_is_an_error_entry_and_the_next_try_asks_again() {
    let mut c = Contents::default();
    let r = c.load("home", "s", "a", V).unwrap();
    c.finish(&r, Ok((500, json!({}))));
    assert_eq!(c.entry("a"), Some(&Entry::Error("HTTP 500".into())));
    let r = c.reload("home", "s", "a", V).unwrap();
    assert_eq!(c.entry("a"), Some(&Entry::Loading));
    c.finish(&r, Err("connection refused".into()));
    assert_eq!(c.entry("a"), Some(&Entry::Error("connection refused".into())));
    assert_eq!(c.in_flight("home"), 0);
}

#[test]
fn each_machine_has_its_own_four_places() {
    let mut c = Contents::default();
    let on = |c: &mut Contents, m: &str, i: usize| c.load(m, "s", &format!("{m}{i}"), V).is_some();
    assert_eq!((0..5).filter(|&i| on(&mut c, "one", i)).count(), 4);
    assert_eq!((0..5).filter(|&i| on(&mut c, "two", i)).count(), 4);
}

#[test]
fn keeps_the_last_200_answers_and_forgets_the_oldest() {
    let mut c = Contents::default();
    for i in 0..201 {
        let r = c.load("home", "s", &format!("f{i}"), V).unwrap();
        c.finish(&r, Ok(item(&format!("f{i}"))));
    }
    // Back from another session: the oldest is gone from the cache, the newest is not.
    c.load("home", "other", "x", V);
    c.load("home", "s", "f0", V);
    c.load("home", "s", "f200", V);
    assert_eq!(c.entry("f0"), Some(&Entry::Loading));
    assert!(ready(&c, "f200"));
}

#[test]
fn the_version_is_status_and_counts() {
    let file = FileDiff { file: "a".into(), status: FileStatus::Added, additions: 17, deletions: 0, ..Default::default() };
    assert_eq!(version_of(&file), "added:17:0");
}

// ─── over a fake server: threads, and never more than 4 at once ─────────────

#[derive(Default)]
struct Seen {
    asked: Vec<String>,
    now: usize,
    most: usize,
    /// How the requests went, as a log: "start <path> (n at once)".
    log: Vec<String>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let request = heard.head.lines().next().unwrap_or("").to_string();
    let path = request.split(' ').nth(1).unwrap_or("").to_string();
    let body = if path.starts_with("/hubs/session-events/negotiate") {
        "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string()
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        fixture("api-sessions.json")
    } else if let Some((_, asked)) = path.split_once("/diffs/file?path=") {
        let asked = asked.replace("%2F", "/");
        {
            let mut seen = seen.lock().unwrap();
            seen.now += 1;
            seen.most = seen.most.max(seen.now);
            let line = format!("start {asked} ({} at once)", seen.now);
            seen.log.push(line);
            seen.asked.push(asked.clone());
        }
        // Held until the limit's worth are in at once, so the reads overlap however slowly they arrive: a set 60 ms let a busy
        // machine send them further apart than that and the limit was never seen. Then held a little more, so a read over the
        // limit would overlap them too and be counted.
        let deadline = std::time::Instant::now() + crate::test_wait::BOUND;
        while seen.lock().unwrap().most < MAX_IN_FLIGHT && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
        seen.lock().unwrap().now -= 1;
        json!({"file": asked, "status": "modified", "additions": 1, "deletions": 1, "before": "old", "after": "new"}).to_string()
    } else {
        "{}".to_string()
    };
    let _ = stream.write_all(
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
            .as_bytes(),
    );
}

#[test]
fn a_12_file_change_asks_for_its_files_four_at_a_time() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
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
    let files: Vec<FileDiff> = (0..12)
        .map(|i| FileDiff {
            file: format!("src/part{i:02}.py"),
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            ..Default::default()
        })
        .collect();
    let (session, all) = (id.clone(), files.clone());
    rt.update(move |s, _| all.iter().for_each(|f| s.load_diff_file(&session, f)));
    let described = seen.clone();
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "all 12 files' contents",
        |s| files.iter().all(|f| matches!(s.diff_contents.entry(&f.file), Some(Entry::Ready(_)))),
        |s| format!("{}; asked {:?}", crate::test_wait::app_state(s), described.lock().unwrap().asked),
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.asked.len(), 12);
    assert!(seen.most <= MAX_IN_FLIGHT, "{} at once", seen.most);
    assert_eq!(seen.most, MAX_IN_FLIGHT, "the limit was never reached, so the queue wasn't tried");
    assert_eq!(rt.state.diff_contents.in_flight("home"), 0);
    assert_eq!(rt.state.diff_contents.queued("home"), 0);
    let Some(Entry::Ready(file)) = rt.state.diff_contents.entry("src/part07.py") else { panic!("part07 not ready") };
    assert_eq!((file.before.as_str(), file.after.as_str()), ("old", "new"));
    println!("{}", seen.log.join("\n"));
}
