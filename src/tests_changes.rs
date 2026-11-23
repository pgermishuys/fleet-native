//! The Changes body's states (component 12, piece D): `ChangesCanvas.test.ts` under its names. The scenes hold invented
//! states (`mock::changes`); Try again talks to a fake Fleet that answers HTTP 500 and counts what it was asked.
use crate::fleet::link::{Job, Link, Post};
use crate::mock::changes::{Case, FIX};
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn shown(case: Case) -> Rt {
    let mut rt = app();
    rt.update(move |s, _| s.show_changes_scene(case, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn all_text(rt: &Rt) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn text_of(rt: &Rt, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn settle(rt: &mut Rt) {
    for _ in 0..3 {
        rt.frame(FRAME);
    }
}

// ChangesCanvas.test.ts: "says the changes could not be loaded, with Try again, instead of No changes when /diffs fails"
#[test]
fn says_the_changes_could_not_be_loaded_with_try_again_instead_of_no_changes_when_diffs_fails() {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = crate::test_http::read(&mut stream).raw();
            if request.starts_with("GET") && request.contains("/diffs") {
                counter.fetch_add(1, Ordering::SeqCst);
            }
            let _ = stream.write_all(b"HTTP/1.1 500 X\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}");
            crate::tests::finish(&mut stream);
        }
    });
    let mut rt = shown(Case::Failed);
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    rt.update(move |s, _| s.link = Some(link));
    settle(&mut rt);

    let text = all_text(&rt);
    assert!(!text.contains("No changes"), "{text}");
    assert_eq!(text_of(&rt, "changes-load-failed"), None, "the note's words are its children's");
    assert!(text.contains("Couldn't load the changes.") && text.contains("HTTP 500"), "{text}");
    assert!(has(&rt, "changes-retry"), "Try again");
    assert_eq!(asked.load(Ordering::SeqCst), 0);
    click(&mut rt, "changes-retry");
    test_wait::drive(&mut rt, &queue, "the request Try again makes", |_| asked.load(Ordering::SeqCst) >= 1, |_| "no request".into());
    assert_eq!(asked.load(Ordering::SeqCst), 1, "one request");
}

// ChangesCanvas.test.ts: "says_why_when_git_refuses_the_folder_and_shows_the_fix_command"
#[test]
fn says_why_when_git_refuses_the_folder_and_shows_the_fix_command() {
    let mut rt = shown(Case::Refused);
    assert!(has(&rt, "changes-git-refused"));
    let text = all_text(&rt);
    assert!(text.contains("another user owns it"), "{text}");
    assert!(text.contains("To let git read it, run this once:"), "{text}");
    assert_eq!(text_of(&rt, "git-fix-command").as_deref(), Some(FIX));
    assert!(!text.contains("No changes"), "{text}");
    assert!(text.contains("Copy command") && !text.contains("Copied"), "{text}");
    // Copy command puts the command on the clipboard and reads "Copied".
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "changes-copy-command");
    settle(&mut rt);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some(FIX));
    let text = all_text(&rt);
    assert!(text.contains("Copied") && !text.contains("Copy command"), "{text}");
}

// ChangesCanvas.test.ts: "says so when the session has no changes"
#[test]
fn says_so_when_the_session_has_no_changes() {
    let rt = shown(Case::EmptySession);
    assert!(all_text(&rt).contains("No changes in this session yet"));
}

// ChangesCanvas.test.ts: "says it is loading, not empty, until the first answer arrives"
#[test]
fn says_it_is_loading_not_empty_until_the_first_answer_arrives() {
    let mut rt = shown(Case::Loading);
    let text = all_text(&rt);
    assert!(text.contains("Loading changes…") && !text.contains("No changes"), "{text}");
    rt.update(|s, _| {
        s.diffs.has_loaded = true;
    });
    settle(&mut rt);
    let text = all_text(&rt);
    assert!(text.contains("No changes in this session yet") && !text.contains("Loading changes"), "{text}");
}

// ChangesCanvas.test.ts: "says a branch is compared with where it left main"
#[test]
fn says_a_branch_is_compared_with_where_it_left_main() {
    let rt = shown(Case::EmptyBranch);
    assert_eq!(text_of(&rt, "changes-base-commit").as_deref(), Some("8a61fdd"));
    let line = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("changes-base")).expect("the base line");
    assert_eq!(line.label.as_deref(), Some("Compared with main8a61fdd"), "the words, then the commit");
    assert!(
        rt.state
            .diffs
            .base
            .as_ref()
            .and_then(|b| crate::fleet::diffs::diff_base_label(Some(b)))
            .is_some_and(|l| l.detail.contains("since it left main at 8a61fdd")),
        "the tooltip"
    );
    assert!(all_text(&rt).contains("No changes on this branch yet"));
}

// ChangesCanvas.test.ts: "says a folder off any branch is compared with the session start"
#[test]
fn says_a_folder_off_any_branch_is_compared_with_the_session_start() {
    let rt = shown(Case::EmptySession);
    let line = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("changes-base")).expect("the base line");
    assert_eq!(line.label.as_deref(), Some("Since this session started"));
    assert!(!has(&rt, "changes-base-commit"));
    assert!(all_text(&rt).contains("No changes in this session yet"));
}

// ChangesCanvas.test.ts: "leaves the base line out when changes show: the scope select carries it"
#[test]
fn leaves_the_base_line_out_when_changes_show() {
    let mut rt = shown(Case::EmptySession);
    assert!(has(&rt, "changes-base"));
    rt.update(|s, _| {
        s.diffs
            .apply(&json!({"diffs": [{"file": "a.ts", "status": "modified", "additions": 1, "deletions": 0}], "base": {"kind": "session"}}))
    });
    settle(&mut rt);
    assert!(!has(&rt, "changes-base"), "the review's scope names it");
    assert!(has(&rt, "changes-review"), "the review's place");
    assert!(!all_text(&rt).contains("No changes"));
}
