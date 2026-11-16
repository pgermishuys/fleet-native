//! What the header does on its own (component 8, piece C): rename in place, tags, the folder path's copy button and the
//! removed checkout's Restore now. `SessionDetailHeader.test.ts` under its names; tags and the requests from the source, where
//! Fleet has no test. The invented list takes a change at once; the tests that read a request use a fake server.
use crate::fleet::header_actions::{restore_failure, restore_path, tags_body, tags_path, tags_with, tags_without};
use crate::fleet::link::{Job, Link, Post};
use crate::mock::header::Case;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const DIRECTORY: &str = "/srv/work/orchard-worktrees/fleet-session-4c2d1e90";

pub(crate) fn scene(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_header(case, true, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

pub(crate) fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn double_click(rt: &mut Runtime<crate::State>, key: &str) {
    click(rt, key);
    click(rt, key);
    rt.frame(FRAME);
}

fn title(rt: &Runtime<crate::State>) -> String {
    rt.state.projects[0].sessions[0].title.clone()
}

fn tags(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.projects[0].sessions[0].row.tags.clone()
}

/// Replaces what the open field holds, keeping its focus (`TextEdit::new` over a focused input drops it).
fn type_over(rt: &mut Runtime<crate::State>, field: &'static str, words: &str) {
    let words = words.to_string();
    rt.update(move |s, _| {
        let edit = if field == "title" { &mut s.head.title_edit } else { &mut s.head.tag_edit };
        edit.replace_text(words);
    });
    rt.frame(FRAME);
}

fn start_title(rt: &mut Runtime<crate::State>) {
    double_click(rt, "session-title");
    assert!(rt.state.head.editing_title("a-open"), "the rename started");
    rt.frame(FRAME);
}

// ---- the requests ---------------------------------------------------------------------------------------------------

#[test]
fn the_requests_are_the_ones_fleet_sends() {
    assert_eq!(tags_path("s 1"), "/api/sessions/s%201/tags");
    assert_eq!(tags_body(&["a".into(), "b".into()]), json!({ "tags": ["a", "b"] }));
    assert_eq!(restore_path("ws-1"), "/api/storage/worktrees/ws-1/restore");
}

#[test]
fn a_restore_failure_says_what_the_server_said_else_the_fixed_sentence() {
    assert_eq!(restore_failure(r#"{"error":"The branch fleet/x isn't there any more."}"#), "The branch fleet/x isn't there any more.");
    assert_eq!(restore_failure(r#"{"detail":"first","error":"second"}"#), "first");
    assert_eq!(restore_failure(r#"{"title":"Conflict"}"#), "Conflict");
    assert_eq!(restore_failure("{}"), "Couldn't put the checkout back.");
    assert_eq!(restore_failure("not json"), "Couldn't put the checkout back.");
}

// ---- rename ----------------------------------------------------------------------------------------------------------

// SessionDetailHeader.test.ts: "asks to edit the title on double-click"
#[test]
fn asks_to_edit_the_title_on_double_click() {
    let mut rt = scene(Case::Fork);
    assert!(!has(&rt, "session-title-input"));
    start_title(&mut rt);
    assert!(has(&rt, "session-title-input"), "the field took the title's place");
    assert!(!has(&rt, "session-title"));
    assert!(!has(&rt, "session-lineage-link"), "the pill is hidden while editing");
    // All of the raw title is selected, so typing replaces it.
    let edit = &rt.state.head.title_edit;
    assert_eq!((edit.text.as_str(), edit.selected()), ("Try a smaller cache limit", "Try a smaller cache limit"));
}

#[test]
fn f2_on_the_focused_title_asks_to_edit_it() {
    let mut rt = scene(Case::Fork);
    rt.update(|_, tx| tx.focus("session-title", true));
    rt.frame(FRAME);
    press(&mut rt, Key::Function(2));
    assert!(rt.state.head.editing_title("a-open"));
}

// SessionDetailHeader.test.ts: "does not offer rename for an archived session"
#[test]
fn does_not_offer_rename_for_an_archived_session() {
    let mut rt = scene(Case::Archived);
    double_click(&mut rt, "session-title");
    assert!(rt.state.head.title.is_none());
    assert!(!has(&rt, "session-title-input"));
    rt.update(|s, tx| s.start_header_rename(tx));
    assert!(rt.state.head.title.is_none(), "not from the menu either");
}

// SessionDetailHeader.test.ts: "saves a changed title on Enter"
#[test]
fn saves_a_changed_title_on_enter() {
    let mut rt = scene(Case::Fork);
    start_title(&mut rt);
    type_over(&mut rt, "title", "Unify status");
    press(&mut rt, Key::Enter);
    assert_eq!(title(&rt), "Unify status");
    assert!(rt.state.head.title.is_none() && has(&rt, "session-title"), "the field closed");
}

// SessionDetailHeader.test.ts: "keeps the old title on Escape"
#[test]
fn keeps_the_old_title_on_escape() {
    let mut rt = scene(Case::Fork);
    start_title(&mut rt);
    type_over(&mut rt, "title", "Something else");
    press(&mut rt, Key::Escape);
    assert_eq!(title(&rt), "Try a smaller cache limit");
    assert!(rt.state.head.title.is_none());
}

#[test]
fn a_blank_or_unchanged_title_is_not_saved_and_blur_saves_once() {
    let mut rt = scene(Case::Fork);
    start_title(&mut rt);
    type_over(&mut rt, "title", "   ");
    press(&mut rt, Key::Enter);
    assert_eq!(title(&rt), "Try a smaller cache limit");
    start_title(&mut rt);
    type_over(&mut rt, "title", "  Try a smaller cache limit ");
    press(&mut rt, Key::Enter);
    assert_eq!(title(&rt), "Try a smaller cache limit");
    // Losing focus after having it saves; Enter then blur does not save twice.
    start_title(&mut rt);
    type_over(&mut rt, "title", "By blur");
    rt.update(|s, tx| {
        s.head.title_edit.focused = true;
        s.keep_header(0.0, tx);
        s.head.title_edit.focused = false;
        assert!(s.keep_header(0.0, tx), "blur is a commit");
        assert!(!s.keep_header(0.0, tx), "and only one");
    });
    assert_eq!(title(&rt), "By blur");
}

// SessionDetailHeader.test.ts: "offers Restore on the archived banner"
#[test]
fn offers_restore_on_the_archived_banner() {
    // Restore itself (`PATCH …/retention`) is piece D's; here the banner offers it for an archived session only.
    assert!(has(&scene(Case::Archived), "session-restore-button"));
    assert!(!has(&scene(Case::Fork), "session-restore-button"));
}

// ---- tags ------------------------------------------------------------------------------------------------------------

#[test]
fn tags_add_remove_and_a_duplicate_or_blank_one_only_closes_the_field() {
    let mut rt = scene(Case::Tags);
    assert_eq!(tags(&rt), ["bug", "cache"]);
    assert!(has(&rt, "session-tag-bug") && has(&rt, "session-tag-cache") && has(&rt, "add-tag"));
    // Add.
    click(&mut rt, "add-tag");
    assert!(has(&rt, "tag-input") && !has(&rt, "add-tag"), "the field replaced the button");
    type_over(&mut rt, "tag", "  perf ");
    press(&mut rt, Key::Enter);
    assert_eq!(tags(&rt), ["bug", "cache", "perf"], "trimmed and appended");
    assert!(has(&rt, "add-tag") && !has(&rt, "tag-input"));
    // A duplicate and a blank one close it and change nothing.
    for typed in ["bug", "   "] {
        click(&mut rt, "add-tag");
        type_over(&mut rt, "tag", typed);
        press(&mut rt, Key::Enter);
        assert_eq!(tags(&rt), ["bug", "cache", "perf"]);
        assert!(!has(&rt, "tag-input"));
    }
    // Escape cancels.
    click(&mut rt, "add-tag");
    type_over(&mut rt, "tag", "never");
    press(&mut rt, Key::Escape);
    assert_eq!(tags(&rt), ["bug", "cache", "perf"]);
    // Remove.
    click(&mut rt, "session-tag-remove-cache");
    assert_eq!(tags(&rt), ["bug", "perf"]);
    assert!(!has(&rt, "session-tag-cache"));
}

#[test]
fn blur_adds_the_tag() {
    let mut rt = scene(Case::Tags);
    click(&mut rt, "add-tag");
    type_over(&mut rt, "tag", "late");
    rt.update(|s, tx| {
        s.head.tag_edit.focused = true;
        s.keep_header(0.0, tx);
        s.head.tag_edit.focused = false;
        s.keep_header(0.0, tx);
    });
    assert_eq!(tags(&rt), ["bug", "cache", "late"]);
}

#[test]
fn the_tag_lists_are_worked_out_as_fleet_does() {
    let have: Vec<String> = vec!["a".into(), "b".into()];
    assert_eq!(tags_with(&have, " c "), Some(vec!["a".into(), "b".into(), "c".into()]));
    assert_eq!(tags_with(&have, "a"), None);
    assert_eq!(tags_with(&have, " "), None);
    assert_eq!(tags_without(&have, "a"), ["b"]);
    assert_eq!(tags_without(&have, "zzz"), ["a", "b"]);
}

// ---- the folder path -------------------------------------------------------------------------------------------------

// SessionDetailHeader.test.ts: "copies the full path, says Copied, and clears it after 1.5 s"
#[test]
fn copies_the_full_path_says_copied_and_clears_it_after_1_5_s() {
    let mut rt = scene(Case::Fork);
    assert_eq!(rt.state.session().row.directory, DIRECTORY);
    let help = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-copy-directory")).and_then(|e| e.label);
    assert_eq!(help.as_deref(), Some("Copy folder path"));
    assert!(!has(&rt, "session-copied"));
    click(&mut rt, "session-copy-directory");
    assert!(has(&rt, "session-copied"), "Copied shows");
    // The icon turns into the check while it does (the button stays in the same place).
    rt.frame(1.4);
    assert!(has(&rt, "session-copied"), "still there before 1.5 s");
    rt.frame(0.2);
    rt.frame(FRAME);
    assert!(!has(&rt, "session-copied"), "gone after 1.5 s");
    assert!(rt.state.head.copied_until.is_none());
}

// SessionDetailHeader.test.ts: "stays quiet when the clipboard refuses"
#[test]
fn stays_quiet_when_the_clipboard_refuses() {
    let mut rt = scene(Case::Fork);
    rt.update(|s, tx| s.directory_copied(false, tx));
    rt.frame(FRAME);
    assert!(!has(&rt, "session-copied") && rt.state.head.copied_until.is_none());
}

// ---- the removed checkout ----------------------------------------------------------------------------------------------

/// A fake server and an app talking to it, the invented header open.
pub(crate) struct Live {
    pub rt: Runtime<crate::State>,
    pub jobs: Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

#[derive(Default)]
struct Seen {
    /// Method, path and body of every call about the session or its checkout.
    calls: Vec<(String, String, Value)>,
    /// What those calls answer: a status and a body.
    answer: Option<(u16, String)>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let (mut head, mut length) = (String::new(), 0usize);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
        head.push_str(&line);
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let mut parts = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let (status, text) = if path.starts_with("/api/sessions/a-open") || path.starts_with("/api/storage/worktrees/") {
        let mut seen = seen.lock().unwrap();
        seen.calls.push((method, path, body));
        seen.answer.clone().unwrap_or((200, "{}".into()))
    } else {
        // The hub and the list are not what is being tested.
        (404, "{}".into())
    };
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
            text.len()
        )
        .as_bytes(),
    );
}

impl Live {
    pub(crate) fn start(case: Case, answer: Option<(u16, &str)>) -> Live {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen { answer: answer.map(|(c, b)| (c, b.to_string())), ..Default::default() }));
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
        let mut rt = scene(case);
        let link = Link::start(base, post);
        rt.update(move |s, _| s.link = Some(link));
        rt.frame(FRAME);
        Live { rt, jobs, seen }
    }

    /// Runs the app until `done` holds. Only what the fake server was asked and the answers of the calls under test are
    /// applied: the list and the hub never answer, so the invented session stays.
    pub(crate) fn wait(&mut self, what: &str, done: impl Fn(&Live) -> bool) {
        crate::test_wait::until(
            self,
            what,
            |l| {
                if done(l) {
                    return true;
                }
                while let Ok(job) = l.jobs.try_recv() {
                    l.rt.update(job);
                }
                l.rt.frame(FRAME);
                done(l)
            },
            |l| format!("{}; calls {:?}", crate::test_wait::app_state(&l.rt.state), l.calls()),
        );
    }

    pub(crate) fn calls(&self) -> Vec<(String, String, Value)> {
        self.seen.lock().unwrap().calls.clone()
    }
}

// SessionDetailHeader.test.ts: "offers Restore now while the checkout is removed, and puts it back"
#[test]
fn offers_restore_now_while_the_checkout_is_removed_and_puts_it_back() {
    let mut live = Live::start(Case::Checkout, None);
    assert!(has(&live.rt, "session-checkout-chip") && has(&live.rt, "session-checkout-restore"));
    assert_eq!(live.rt.state.header_model().checkout.map(|c| c.text()), Some("Checkout removed"));
    click(&mut live.rt, "session-checkout-restore");
    live.wait("the checkout to be put back", |l| l.rt.state.projects[0].sessions[0].row.checkout_removed_at.is_none());
    assert_eq!(live.calls(), [("POST".to_string(), "/api/storage/worktrees/ws-4c2d1e90/restore".to_string(), json!({}))]);
    // The note after a restore stays 6 s, then the chip goes.
    live.rt.frame(FRAME);
    assert!(has(&live.rt, "session-checkout-chip") && !has(&live.rt, "session-checkout-restore"));
    assert_eq!(live.rt.state.header_model().checkout.map(|c| c.text()), Some("Checkout restored. Dependencies and .env aren't back."));
    live.rt.frame(5.9);
    assert!(has(&live.rt, "session-checkout-chip"));
    live.rt.frame(0.3);
    live.rt.frame(FRAME);
    assert!(!has(&live.rt, "session-checkout-chip"));
}

// SessionDetailHeader.test.ts: "shows the server's sentence when the restore fails, and no chip when nothing was removed"
#[test]
fn shows_the_servers_sentence_when_the_restore_fails_and_no_chip_when_nothing_was_removed() {
    let sentence = "The branch fleet/x isn't there any more.";
    let mut live = Live::start(Case::Checkout, Some((409, r#"{"error":"The branch fleet/x isn't there any more."}"#)));
    click(&mut live.rt, "session-checkout-restore");
    live.wait("the failure to show", |l| l.rt.state.head.restore_error_of("a-open").is_some());
    live.rt.frame(FRAME);
    assert_eq!(live.rt.state.head.restore_error_of("a-open"), Some(sentence));
    assert!(has(&live.rt, "session-checkout-error"), "the sentence is drawn inside the chip");
    assert!(has(&live.rt, "session-checkout-restore"), "and Restore now stays, to try again");
    // The row's removal is gone (another machine put it back): no chip at all.
    live.rt.update(|s, _| s.projects[0].sessions[0].row.checkout_removed_at = None);
    live.rt.frame(FRAME);
    assert!(!has(&live.rt, "session-checkout-chip"));
    assert!(!has(&scene(Case::Fork), "session-checkout-chip"), "nothing was removed");
}

#[test]
fn a_session_with_no_workspace_does_nothing_on_restore_now() {
    let mut rt = scene(Case::Checkout);
    rt.update(|s, _| s.projects[0].sessions[0].row.workspace_id.clear());
    rt.update(|s, tx| s.restore_checkout(tx));
    assert!(rt.state.head.restoring.is_none() && rt.state.head.restore_error.is_none());
    assert!(rt.state.projects[0].sessions[0].row.checkout_removed_at.is_some());
}

#[test]
fn a_restore_under_way_says_so_and_has_no_button() {
    let mut rt = scene(Case::Checkout);
    rt.update(|s, _| s.show_header_restoring());
    rt.frame(FRAME);
    assert_eq!(rt.state.header_model().checkout.map(|c| c.text()), Some("Putting the checkout back\u{2026}"));
    assert!(!has(&rt, "session-checkout-restore"));
}

// ---- the requests of rename and tags, read from the fake server ---------------------------------------------------------

#[test]
fn a_rename_sends_the_title_and_a_refusal_puts_the_old_one_back() {
    let mut live = Live::start(Case::Fork, Some((409, r#"{"error":"no"}"#)));
    start_title(&mut live.rt);
    type_over(&mut live.rt, "title", "Unify status");
    press(&mut live.rt, Key::Enter);
    assert_eq!(title(&live.rt), "Unify status", "shown at once");
    live.wait("the refusal", |l| l.rt.state.projects[0].sessions[0].title == "Try a smaller cache limit");
    assert_eq!(live.calls(), [("PATCH".to_string(), "/api/sessions/a-open".to_string(), json!({ "title": "Unify status" }))]);
}

#[test]
fn tags_are_sent_whole_and_shown_only_once_the_server_took_them() {
    let mut live = Live::start(Case::Tags, None);
    click(&mut live.rt, "add-tag");
    type_over(&mut live.rt, "tag", "perf");
    press(&mut live.rt, Key::Enter);
    live.wait("the tags to be taken", |l| tags(&l.rt).len() == 3);
    assert_eq!(live.calls(), [("PATCH".to_string(), "/api/sessions/a-open/tags".to_string(), json!({ "tags": ["bug", "cache", "perf"] }))]);
    click(&mut live.rt, "session-tag-remove-bug");
    live.wait("the removal", |l| tags(&l.rt).len() == 2);
    assert_eq!(live.calls()[1].2, json!({ "tags": ["cache", "perf"] }));
}

#[test]
fn a_refused_tag_change_leaves_the_tags_as_they_were() {
    let mut live = Live::start(Case::Tags, Some((500, "{}")));
    click(&mut live.rt, "session-tag-remove-bug");
    live.wait("the request", |l| !l.calls().is_empty());
    for _ in 0..30 {
        while let Ok(job) = live.jobs.try_recv() {
            live.rt.update(job);
        }
        live.rt.frame(FRAME);
    }
    assert_eq!(tags(&live.rt), ["bug", "cache"], "Fleet only logs a console error");
}
