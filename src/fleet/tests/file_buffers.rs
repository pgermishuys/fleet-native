//! Reading a file (component 11, piece B): `file-buffers.test.ts` and the read-side cases of `file-buffer.test.ts` under
//! Fleet's names, over `FileBuffers` itself (a rule needs no thread), then opening one file twice over a fake server.
//! The cases that type into a buffer (undo, "is dirty after typing…", "Use the agent's", after Compare, a save) are slice 2's
//! piece Q; the conflict rule is checked here by writing into the buffer's lines directly.
use super::super::file_buffers::{
    Buffer, Conflict, DiskFile, DiskUpdate, FileBuffers, NOT_TEXT_MESSAGE, Patch, Status, TOO_LARGE_MESSAGE, unavailable_reason,
};
use super::super::link::{Job, Link, Post};
use super::reconnect::app;
use super::*;
use std::io::Write;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const M: &str = "home";
const S: &str = "s1";
const P: &str = "src/app.ts";

fn disk_body(content: Option<&str>, hash: &str, binary: bool, truncated: bool) -> (u16, String) {
    (200, json!({"content": content, "hash": hash, "isBinary": binary, "isTruncated": truncated}).to_string())
}

fn file(content: &str, hash: &str) -> (u16, String) {
    disk_body(Some(content), hash, false, false)
}

/// Opens `path` and answers its read.
fn read(b: &mut FileBuffers, path: &str, answer: (u16, String)) {
    let request = b.open(M, S, path).expect("a read was asked");
    b.finish(&request, Ok(answer));
}

fn opened(content: &str, hash: &str) -> FileBuffers {
    let mut b = FileBuffers::default();
    read(&mut b, P, file(content, hash));
    b
}

fn refresh(b: &mut FileBuffers, answer: (u16, String)) -> Option<DiskUpdate> {
    let request = b.refresh(M, S, P).expect("a read was asked");
    b.finish(&request, Ok(answer))
}

fn buffer(b: &FileBuffers) -> &Buffer {
    b.buffer(M, S, P).unwrap()
}

fn status(b: &FileBuffers) -> Status {
    b.info(M, S, P).unwrap().status
}

// ─── file-buffers.test.ts ───────────────────────────────────────────────────

#[test]
fn creates_a_buffer_once_loading() {
    let mut b = FileBuffers::default();
    b.ensure(M, S, P);
    b.ensure(M, S, P);
    assert_eq!(b.info(M, S, P).unwrap().status, Status::Loading);
    assert_eq!(b.open_paths(M, S), [P]);
}

#[test]
fn keeps_sessions_and_paths_apart() {
    let mut b = FileBuffers::default();
    b.ensure(M, "s1", "a.ts");
    b.ensure(M, "s2", "a.ts");
    b.patch(M, "s1", "a.ts", Patch { dirty: Some(true), ..Patch::default() });
    assert!(b.is_dirty(M, "s1", "a.ts"));
    assert!(!b.is_dirty(M, "s2", "a.ts"));
    assert_eq!(b.open_paths(M, "s1"), ["a.ts"]);
    // The same session id on another machine is another file.
    assert!(!b.is_dirty("work", "s1", "a.ts"));
    assert!(b.open_paths("work", "s1").is_empty());
}

#[test]
fn lists_unsaved_files_across_sessions_and_forgets_closed_ones() {
    let mut b = FileBuffers::default();
    b.ensure(M, "s1", "a.ts");
    b.ensure(M, "s2", "src/b.ts");
    for (s, p) in [("s1", "a.ts"), ("s2", "src/b.ts")] {
        b.patch(M, s, p, Patch { dirty: Some(true), ..Patch::default() });
    }
    let want = |s: &str, p: &str| (M.to_string(), s.to_string(), p.to_string());
    assert_eq!(b.unsaved(), [want("s1", "a.ts"), want("s2", "src/b.ts")]);
    b.remove(M, "s1", "a.ts");
    assert_eq!(b.unsaved(), [want("s2", "src/b.ts")]);
    assert!(b.buffer(M, "s1", "a.ts").is_none());
}

#[test]
fn doesnt_replace_the_info_object_when_nothing_changed() {
    let mut b = FileBuffers::default();
    b.ensure(M, S, P);
    let before = b.info(M, S, P).cloned();
    assert!(!b.patch(M, S, P, Patch { dirty: Some(false), ..Patch::default() }));
    assert_eq!(b.info(M, S, P).cloned(), before);
    assert!(b.patch(M, S, P, Patch { dirty: Some(true), ..Patch::default() }));
}

// ─── reading (buffers.ts openBuffer, 10.3) ──────────────────────────────────

#[test]
fn opening_reads_once_and_a_second_open_while_one_is_reading_waits_for_it() {
    let mut b = FileBuffers::default();
    let first = b.open(M, S, P).unwrap();
    assert_eq!(b.open(M, S, P), None, "a second open waits for the first read");
    assert_eq!(b.refresh(M, S, P), None, "so does a refresh, which needs a ready file anyway");
    b.finish(&first, Ok(file("x", "h1")));
    assert_eq!(b.open(M, S, P), None, "an open file isn't read again by opening it");
    assert_eq!(status(&b), Status::Ready);
}

#[test]
fn an_unavailable_file_isnt_read_again_by_opening_it() {
    let mut b = FileBuffers::default();
    read(&mut b, P, disk_body(None, "h", true, false));
    assert_eq!(status(&b), Status::Unavailable);
    assert_eq!(b.open(M, S, P), None);
}

#[test]
fn says_why_a_file_cant_be_edited() {
    assert_eq!(
        unavailable_reason(&DiskFile { content: None, hash: Some("h".into()), is_binary: false, is_truncated: true }),
        Some(TOO_LARGE_MESSAGE)
    );
    assert_eq!(
        unavailable_reason(&DiskFile { content: None, hash: Some("h".into()), is_binary: true, is_truncated: false }),
        Some(NOT_TEXT_MESSAGE)
    );
    // No content or no hash is the same as binary.
    assert_eq!(
        unavailable_reason(&DiskFile { content: None, hash: Some("h".into()), is_binary: false, is_truncated: false }),
        Some(NOT_TEXT_MESSAGE)
    );
    assert_eq!(
        unavailable_reason(&DiskFile { content: Some("x".into()), hash: None, is_binary: false, is_truncated: false }),
        Some(NOT_TEXT_MESSAGE)
    );
    assert_eq!(unavailable_reason(&DiskFile::new(Some("x"), "h")), None);
}

#[test]
fn says_why_a_file_too_large_to_edit_cant_be_opened() {
    let mut b = FileBuffers::default();
    read(&mut b, P, disk_body(None, "h", false, true));
    let info = b.info(M, S, P).unwrap();
    assert_eq!((info.status, info.message.as_deref()), (Status::Unavailable, Some("Too large to edit here (over 512 KB).")));
}

#[test]
fn says_why_a_binary_file_cant_be_opened() {
    let mut b = FileBuffers::default();
    read(&mut b, P, disk_body(None, "h", true, false));
    let info = b.info(M, S, P).unwrap();
    assert_eq!(
        (info.status, info.message.as_deref()),
        (Status::Unavailable, Some("This file is binary or not UTF-8 text, so it can't be edited here."))
    );
}

#[test]
fn a_failed_read_says_the_file_is_gone_for_a_404_and_otherwise_that_it_couldnt_be_read() {
    let mut b = FileBuffers::default();
    read(&mut b, P, (404, String::new()));
    let info = b.info(M, S, P).unwrap();
    assert_eq!((info.status, info.message.as_deref()), (Status::Error, Some("This file isn't in the session's folder any more.")));
    // An error is read again by opening it.
    let again = b.open(M, S, P).expect("an error is tried again");
    assert_eq!(status(&b), Status::Loading);
    b.finish(&again, Ok((500, String::new())));
    assert_eq!(b.info(M, S, P).unwrap().message.as_deref(), Some("The file couldn't be read."));
    let third = b.open(M, S, P).unwrap();
    b.finish(&third, Err("connection refused".into()));
    assert_eq!(b.info(M, S, P).unwrap().message.as_deref(), Some("The file couldn't be read."));
}

#[test]
fn an_answer_for_a_file_closed_meanwhile_is_dropped() {
    let mut b = FileBuffers::default();
    let request = b.open(M, S, P).unwrap();
    b.remove(M, S, P);
    b.finish(&request, Ok(file("x", "h1")));
    assert!(b.buffer(M, S, P).is_none());
    assert!(!b.is_reading(M, S, P));
}

// ─── file-buffer.test.ts ────────────────────────────────────────────────────

#[test]
fn opens_clean() {
    let b = opened("const one = 1;\n", "h1");
    let buf = buffer(&b);
    assert_eq!(buf.lines(), ["const one = 1;", ""]);
    assert_eq!(buf.disk_text(), "const one = 1;\n");
    assert!(!buf.is_dirty());
    assert_eq!(buf.base_hash(), Some("h1"));
    let info = b.info(M, S, P).unwrap();
    assert_eq!((info.status, info.message.as_deref(), info.dirty), (Status::Ready, None, false));
}

#[test]
fn does_nothing_when_the_hash_on_disk_is_the_same() {
    let mut b = opened("abc", "h1");
    assert_eq!(refresh(&mut b, file("abc", "h1")), Some(DiskUpdate::Unchanged));
    // Even if the text it came with differs: the hash says it is the same file.
    assert_eq!(refresh(&mut b, file("other", "h1")), Some(DiskUpdate::Unchanged));
    assert_eq!(buffer(&b).lines(), ["abc"]);
}

#[test]
fn takes_the_agents_change_into_a_clean_buffer_out_of_undo_history() {
    // There is no undo until slice 2: what is checked is the text, the base hash and the changed lines.
    let mut b = opened("line 1\nline 2\nline 3\n", "h1");
    let result = refresh(&mut b, file("line 1\nline TWO\nline 3\n", "h2"));
    assert_eq!(result, Some(DiskUpdate::Updated(Some((2, 2)))));
    assert_eq!(buffer(&b).disk_text(), "line 1\nline TWO\nline 3\n");
    assert_eq!(buffer(&b).base_hash(), Some("h2"));
    assert!(!buffer(&b).is_dirty());
    assert!(!b.info(M, S, P).unwrap().dirty);
}

#[test]
fn never_touches_a_dirty_buffer_a_disk_change_is_a_conflict() {
    let mut b = opened("abc", "h1");
    b.buffers_for_test(M, S, P).lines[0] = "mine abc".into();
    assert!(buffer(&b).is_dirty());
    let result = refresh(&mut b, file("agent's", "h2"));
    let conflict = Conflict { disk_text: Some("agent's".into()), hash: "h2".into() };
    assert_eq!(result, Some(DiskUpdate::Conflict(conflict.clone())));
    assert_eq!(buffer(&b).disk_text(), "mine abc");
    assert_eq!(buffer(&b).base_hash(), Some("h1"));
    assert_eq!(b.info(M, S, P).unwrap().conflict, Some(conflict));
}

#[test]
fn keeps_crlf_when_the_agent_changes_a_crlf_file() {
    let mut b = opened("a\r\nb\r\n", "h1");
    refresh(&mut b, file("a\r\nB\r\nc\r\n", "h2"));
    assert_eq!(buffer(&b).disk_text(), "a\r\nB\r\nc\r\n");
    assert_eq!(buffer(&b).lines().len(), 4);
}

#[test]
fn starts_a_fresh_state_when_the_agent_changes_the_line_breaks() {
    let mut b = opened("a\nb\n", "h1");
    let result = refresh(&mut b, file("a\r\nb\r\n", "h2"));
    assert_eq!(result, Some(DiskUpdate::Updated(None)));
    assert_eq!(buffer(&b).disk_text(), "a\r\nb\r\n");
    assert_eq!(buffer(&b).format().unwrap().line_separator.as_str(), "\r\n");
}

#[test]
fn a_file_that_became_binary_while_clean_cant_be_edited_any_more() {
    let mut b = opened("abc", "h1");
    let result = refresh(&mut b, disk_body(None, "h2", true, false));
    assert_eq!(result, Some(DiskUpdate::Unavailable(NOT_TEXT_MESSAGE.into())));
    let info = b.info(M, S, P).unwrap();
    assert_eq!((info.status, info.message.as_deref()), (Status::Unavailable, Some(NOT_TEXT_MESSAGE)));
}

#[test]
fn an_unavailable_file_that_is_readable_again_becomes_ready() {
    let mut b = FileBuffers::default();
    read(&mut b, P, disk_body(None, "h1", false, true));
    let result = refresh(&mut b, file("small now\n", "h2"));
    assert_eq!(result, Some(DiskUpdate::Updated(None)));
    assert_eq!(status(&b), Status::Ready);
    assert_eq!(buffer(&b).lines(), ["small now", ""]);
    // Still too large: the message stays and nothing else changes.
    let mut big = FileBuffers::default();
    read(&mut big, P, disk_body(None, "h1", false, true));
    assert_eq!(refresh(&mut big, disk_body(None, "h2", false, true)), Some(DiskUpdate::Unavailable(TOO_LARGE_MESSAGE.into())));
    assert_eq!(status(&big), Status::Unavailable);
}

#[test]
fn overlapping_refreshes_share_one_read_and_a_save_in_flight_skips_it() {
    let mut b = opened("abc", "h1");
    let first = b.refresh(M, S, P).unwrap();
    assert_eq!(b.refresh(M, S, P), None, "a second refresh waits for the first");
    b.finish(&first, Ok(file("abc", "h1")));
    b.patch(M, S, P, Patch { saving: Some(true), ..Patch::default() });
    assert_eq!(b.refresh(M, S, P), None, "no read while a save is in flight");
    b.patch(M, S, P, Patch { saving: Some(false), ..Patch::default() });
    assert!(b.refresh(M, S, P).is_some(), "asks again once the save is done");
    assert_eq!(b.refresh(M, S, P), None, "and that one is on its way");
}

#[test]
fn a_buffer_that_was_only_made_has_nothing_to_refresh() {
    let mut b = FileBuffers::default();
    b.ensure(M, S, P);
    assert_eq!(b.refresh(M, S, P), None, "loading isn't ready or unavailable");
    assert_eq!(b.refresh(M, S, "other.ts"), None, "a file that isn't open");
}

// ─── over a fake server: opening one file twice reads it once ───────────────

#[derive(Default)]
struct Seen {
    reads: Vec<String>,
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
    } else if let Some((_, asked)) = path.split_once("/files/content?path=") {
        seen.lock().unwrap().reads.push(asked.replace("%2F", "/"));
        // Long enough that a second open lands while this one is still being read.
        std::thread::sleep(std::time::Duration::from_millis(120));
        json!({"content": "a\r\nb\r\n", "hash": "h1", "isBinary": false, "isTruncated": false}).to_string()
    } else {
        "{}".to_string()
    };
    let _ = stream.write_all(
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
            .as_bytes(),
    );
}

#[test]
fn opening_one_file_twice_reads_it_once() {
    let listener = crate::test_http::bind().unwrap();
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
    let machine = rt.state.machine_key_of(&id).to_string();
    let session = id.clone();
    rt.update(move |s, _| {
        s.open_file_buffer(&session, "src/app.py");
        s.open_file_buffer(&session, "src/app.py");
    });
    let described = seen.clone();
    let (m, session) = (machine.clone(), id.clone());
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the file's buffer to be ready",
        move |s| s.file_buffers.info(&m, &session, "src/app.py").is_some_and(|i| i.status == Status::Ready),
        move |s| format!("{}; reads {:?}", crate::test_wait::app_state(s), described.lock().unwrap().reads),
    );
    // Opening it again, now it is there, asks nothing either.
    let session = id.clone();
    rt.update(move |s, _| s.open_file_buffer(&session, "src/app.py"));
    assert!(!rt.state.file_buffers.is_reading(&machine, &id, "src/app.py"));
    let reads = seen.lock().unwrap().reads.clone();
    assert_eq!(reads, ["src/app.py"], "one request for three opens");
    let buf = rt.state.file_buffers.buffer(&machine, &id, "src/app.py").unwrap();
    assert_eq!(buf.lines(), ["a", "b", ""]);
    assert_eq!(buf.disk_text(), "a\r\nb\r\n");
    println!("requests for the file: {reads:?}");
}

#[test]
fn a_failed_refresh_is_quiet_and_frees_the_place() {
    let mut b = opened("abc", "h1");
    let request = b.refresh(M, S, P).unwrap();
    assert_eq!(b.finish(&request, Ok((500, String::new()))), None);
    assert_eq!(status(&b), Status::Ready);
    assert_eq!(buffer(&b).lines(), ["abc"]);
    assert!(b.refresh(M, S, P).is_some());
}
