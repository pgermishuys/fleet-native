//! Editing a buffer (component 11, piece Q): the cases of `file-buffer.test.ts` and `text-format.test.ts` that type into a buffer,
//! under Fleet's names, over `FileBuffers` itself; then a save over a fake server, with the hash it read and a 409.
use super::super::code_edit::Pos;
use super::super::file_buffers::{Conflict, DiskUpdate, FileBuffers, SaveOutcome};
use super::super::link::{Job, Link, Post};
use super::reconnect::app;
use super::*;
use std::io::Write;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const M: &str = "home";
const S: &str = "s1";
const P: &str = "src/app.ts";

fn opened(content: &str, hash: &str) -> FileBuffers {
    let mut b = FileBuffers::default();
    let request = b.open(M, S, P).expect("a read was asked");
    let body = json!({"content": content, "hash": hash, "isBinary": false, "isTruncated": false}).to_string();
    b.finish(&request, Ok((200, body)));
    b
}

/// Types `text` at the start of the file, one edit (Fleet's `type(buffer, insert, at)` at offset 0).
fn type_at_start(b: &mut FileBuffers, text: &str, now: f64) {
    b.edit(M, S, P, |e, l| {
        e.set_caret(Pos::default(), false);
        e.insert(l, text, now);
        ((), true)
    })
    .expect("the buffer was read");
}

fn buf(b: &FileBuffers) -> &super::super::file_buffers::Buffer {
    b.buffer(M, S, P).unwrap()
}

fn undo(b: &mut FileBuffers) -> bool {
    b.edit(M, S, P, |e, l| {
        let u = e.undo(l);
        (u, u)
    })
    .unwrap()
}

fn conflict_with(b: &mut FileBuffers, disk: &str, hash: &str) -> Conflict {
    let request = b.refresh(M, S, P).expect("a read was asked");
    let body = json!({"content": disk, "hash": hash, "isBinary": false, "isTruncated": false}).to_string();
    match b.finish(&request, Ok((200, body))) {
        Some(DiskUpdate::Conflict(c)) => c,
        other => panic!("a conflict was expected, got {other:?}"),
    }
}

// ─── file-buffer.test.ts ────────────────────────────────────────────────────

#[test]
fn is_dirty_after_typing_and_clean_again_when_the_text_goes_back() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "x", 0.0);
    assert!(buf(&b).is_dirty());
    assert!(b.info(M, S, P).unwrap().dirty, "the info the tab reads says so too");
    // Backspace takes the "x" back out: the text is the saved text again, so it is clean.
    b.edit(M, S, P, |e, l| {
        e.delete(l, false, false, 5.0);
        ((), true)
    });
    assert_eq!(buf(&b).lines(), ["abc"]);
    assert!(!buf(&b).is_dirty());
    assert!(!b.info(M, S, P).unwrap().dirty);
}

#[test]
fn never_touches_a_dirty_buffer_a_disk_change_is_a_conflict() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "mine ", 0.0);
    let conflict = conflict_with(&mut b, "agent's", "h2");
    assert_eq!(conflict, Conflict { disk_text: Some("agent's".into()), hash: "h2".into() });
    assert_eq!(buf(&b).disk_text(), "mine abc");
    assert_eq!(buf(&b).base_hash(), Some("h1"));
}

#[test]
fn use_the_agents_replaces_the_buffer_as_an_edit_you_can_undo() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "mine ", 0.0);
    conflict_with(&mut b, "agent's", "h2");
    assert!(b.use_disk_version(M, S, P, 5.0));
    assert_eq!(buf(&b).disk_text(), "agent's");
    assert_eq!(buf(&b).base_hash(), Some("h2"));
    assert!(!buf(&b).is_dirty());
    let info = b.info(M, S, P).unwrap();
    assert_eq!((info.dirty, info.conflict.clone()), (false, None));
    // Ctrl+Z brings the user's version back, and it is unsaved against the agent's file.
    assert!(undo(&mut b));
    assert_eq!(buf(&b).disk_text(), "mine abc");
    assert!(buf(&b).is_dirty());
    assert!(b.info(M, S, P).unwrap().dirty);
}

#[test]
fn use_the_agents_is_a_step_of_its_own_not_merged_with_the_typing_before_it() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "m", 0.0);
    conflict_with(&mut b, "agent's", "h2");
    // Typing at 0.1 s would have joined the step before; the agent's version, taken at 0.2 s, must not.
    b.use_disk_version(M, S, P, 0.2);
    assert!(undo(&mut b));
    assert_eq!(buf(&b).disk_text(), "mabc", "one undo takes back only the agent's version, not the typing too");
}

#[test]
fn after_compare_the_buffer_is_based_on_the_agents_version_and_still_unsaved() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "mine ", 0.0);
    conflict_with(&mut b, "agent's", "h2");
    assert!(b.finish_compare(M, S, P));
    assert_eq!(buf(&b).base_hash(), Some("h2"));
    assert!(buf(&b).is_dirty());
    assert!(b.info(M, S, P).unwrap().dirty);
    assert_eq!(b.info(M, S, P).unwrap().conflict, None);
    // A save now checks against the agent's hash.
    assert_eq!(b.begin_save(M, S, P, false).unwrap().base_hash, "h2");
}

#[test]
fn is_clean_after_a_save_and_dirty_again_if_typing_went_on_during_it() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "1", 0.0);
    let request = b.begin_save(M, S, P, false).expect("a save");
    assert_eq!((request.content.as_str(), request.base_hash.as_str()), ("1abc", "h1"));
    assert!(b.info(M, S, P).unwrap().saving);
    type_at_start(&mut b, "2", 5.0);
    assert_eq!(b.finish_save(&request, Ok((200, json!({"hash": "h2"}).to_string()))), Some(SaveOutcome::Saved));
    assert_eq!(buf(&b).base_hash(), Some("h2"));
    assert!(buf(&b).is_dirty());
    assert!(!b.info(M, S, P).unwrap().saving);
    assert!(b.info(M, S, P).unwrap().dirty);
    let again = b.begin_save(M, S, P, false).expect("a second save");
    assert_eq!(again.base_hash, "h2");
    b.finish_save(&again, Ok((200, json!({"hash": "h3"}).to_string())));
    assert!(!buf(&b).is_dirty());
    assert!(!b.info(M, S, P).unwrap().dirty);
}

#[test]
fn a_save_in_flight_blocks_another_and_a_read() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "1", 0.0);
    let request = b.begin_save(M, S, P, false).unwrap();
    assert!(b.begin_save(M, S, P, false).is_none(), "one save at a time");
    assert!(b.refresh(M, S, P).is_none(), "the save's own files.changed is not the agent's change (10.3)");
    b.cancel_save(&request);
    assert!(b.refresh(M, S, P).is_some());
}

#[test]
fn a_save_that_loses_the_race_makes_the_conflict_and_keep_mine_carries_its_hash() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "mine ", 0.0);
    let request = b.begin_save(M, S, P, false).unwrap();
    let lost = json!({"content": "agent's", "hash": "h9"}).to_string();
    assert_eq!(b.finish_save(&request, Ok((409, lost))), Some(SaveOutcome::Conflict));
    let info = b.info(M, S, P).unwrap();
    assert_eq!((info.saving, info.conflict.clone()), (false, Some(Conflict { disk_text: Some("agent's".into()), hash: "h9".into() })));
    assert_eq!(buf(&b).disk_text(), "mine abc", "nothing was overwritten");
    // Keep mine replaces exactly the version that was shown; a plain save would still send the old hash.
    assert_eq!(b.begin_save(M, S, P, true).unwrap().base_hash, "h9");
}

#[test]
fn a_refused_save_says_why_and_leaves_the_buffer_unsaved() {
    let mut b = opened("abc", "h1");
    type_at_start(&mut b, "1", 0.0);
    let request = b.begin_save(M, S, P, false).unwrap();
    let outcome = b.finish_save(&request, Ok((400, json!({"error": "File is too large."}).to_string())));
    assert_eq!(outcome, Some(SaveOutcome::Error("File is too large.".into())));
    assert!(buf(&b).is_dirty() && !b.info(M, S, P).unwrap().saving);
    let request = b.begin_save(M, S, P, false).unwrap();
    assert_eq!(
        b.finish_save(&request, Ok((500, String::new()))),
        Some(SaveOutcome::Error("Failed to save file: 500 Internal Server Error".into()))
    );
}

#[test]
fn the_agents_change_to_a_clean_buffer_is_out_of_undo_history() {
    let mut b = opened("line 1\nline 2\nline 3\n", "h1");
    type_at_start(&mut b, "x", 0.0);
    b.begin_save(M, S, P, false).map(|r| b.finish_save(&r, Ok((200, json!({"hash": "h2"}).to_string()))));
    let request = b.refresh(M, S, P).unwrap();
    let body = json!({"content": "xline 1\nline TWO\nline 3\n", "hash": "h3", "isBinary": false, "isTruncated": false});
    b.finish(&request, Ok((200, body.to_string())));
    assert_eq!(buf(&b).disk_text(), "xline 1\nline TWO\nline 3\n");
    // Ctrl+Z has nothing to take back: the agent's edit is not a step, and the user's saved typing came before it.
    assert!(!undo(&mut b));
    assert_eq!(buf(&b).disk_text(), "xline 1\nline TWO\nline 3\n");
    assert!(!buf(&b).is_dirty());
}

// ─── text-format.test.ts: "editing keeps the file's line breaks" ────────────

#[test]
fn a_new_line_in_a_crlf_file_uses_crlf() {
    let mut b = opened("a\r\nb\r\n", "h1");
    b.edit(M, S, P, |e, l| {
        e.set_caret(Pos::new(0, 1), false);
        e.insert(l, "\nx", 0.0);
        ((), true)
    });
    assert_eq!(buf(&b).disk_text(), "a\r\nx\r\nb\r\n");
}

#[test]
fn pasted_lf_text_becomes_crlf_lines_in_a_crlf_file() {
    let mut b = opened("a\r\nb\r\n", "h1");
    // What the editor is handed is what `paste_text` makes of the clipboard, whatever breaks it holds.
    let pasted = crate::conv::text_format::paste_text("x\ny\n");
    b.edit(M, S, P, |e, l| {
        e.set_caret(Pos::default(), false);
        e.paste(l, &pasted, 0.0);
        ((), true)
    });
    assert_eq!(buf(&b).lines().len(), 5);
    assert_eq!(buf(&b).disk_text(), "x\r\ny\r\na\r\nb\r\n");
    assert_eq!(crate::conv::text_format::paste_text("p\r\nq\rr\ns"), "p\nq\nr\ns");
}

#[test]
fn an_untouched_mixed_file_keeps_its_odd_breaks_after_an_edit_elsewhere() {
    let mut b = opened("a\r\nb\nc\r\n", "h1");
    type_at_start(&mut b, "z", 0.0);
    assert_eq!(buf(&b).disk_text(), "za\r\nb\nc\r\n");
}

// ─── the editor belongs to the buffer ───────────────────────────────────────

#[test]
fn the_caret_and_the_history_are_the_buffers_own_and_the_agents_edit_moves_the_caret() {
    let mut b = opened("one\ntwo\nthree\n", "h1");
    b.edit(M, S, P, |e, _| {
        e.set_caret(Pos::new(2, 2), false);
        ((), false)
    });
    let request = b.refresh(M, S, P).unwrap();
    let body = json!({"content": "zero\none\ntwo\nthree\n", "hash": "h2", "isBinary": false, "isTruncated": false});
    b.finish(&request, Ok((200, body.to_string())));
    assert_eq!(buf(&b).editor.caret, Pos::new(3, 2), "the caret stays on its text when a line is added above");
}

// ─── a save over a fake server ──────────────────────────────────────────────

#[derive(Default)]
struct Seen {
    puts: Vec<serde_json::Value>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let route = heard.path.clone();
    if heard.method == "PUT" && route.contains("/files/content") {
        let sent = heard.json();
        let status = if sent["baseHash"] == "h1" { 200 } else { 409 };
        seen.lock().unwrap().puts.push(sent);
        let body = if status == 200 { json!({"saved": true, "hash": "h2"}) } else { json!({"content": "agent's", "hash": "h9"}) };
        return crate::test_http::respond_json(&mut stream, status, &body.to_string());
    }
    let body = if route.starts_with("/hubs/session-events/negotiate") {
        "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string()
    } else if route.starts_with("/api/sessions?") || route == "/api/sessions" {
        fixture("api-sessions.json")
    } else if route.contains("/files/content?path=") {
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
fn saving_sends_the_text_in_the_files_own_breaks_with_the_hash_it_read_and_a_409_is_the_conflict() {
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
    rt.update(move |s, _| s.open_file_buffer(&session, "src/app.py"));
    let (m, session) = (machine.clone(), id.clone());
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the file's buffer to be ready",
        move |s| {
            s.file_buffers
                .info(&m, &session, "src/app.py")
                .is_some_and(|i| i.dirty || i.status == super::super::file_buffers::Status::Ready)
        },
        crate::test_wait::app_state,
    );
    let (m, session) = (machine.clone(), id.clone());
    rt.update(move |s, _| {
        s.file_buffers.edit(&m, &session, "src/app.py", |e, l| {
            e.set_caret(Pos::new(1, 1), false);
            e.insert(l, "!", 0.0);
            ((), true)
        });
        s.save_file_buffer(&session, "src/app.py", false);
    });
    let (m, session) = (machine.clone(), id.clone());
    let described = seen.clone();
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the save to be answered",
        move |s| s.file_buffers.info(&m, &session, "src/app.py").is_some_and(|i| !i.saving) && !seen.lock().unwrap().puts.is_empty(),
        move |s| format!("{}; puts {:?}", crate::test_wait::app_state(s), described.lock().unwrap().puts),
    );
    let buf = rt.state.file_buffers.buffer(&machine, &id, "src/app.py").unwrap();
    assert_eq!(buf.base_hash(), Some("h2"));
    assert!(!buf.is_dirty());
    assert_eq!(buf.disk_text(), "a\r\nb!\r\n");
}
