//! The pull request popover and Send to agent (component 8, piece H): `SessionContextChips.test.ts`'s two tests that belong
//! to the popover under their names, the keyed buttons of `use-send-to-agent.ts`, the sent state, the error lines and the
//! footer's four forms. Sending runs against a fake Fleet that records what `POST /api/sessions/{id}/prompt` was given.
//! Invented GitHub data only (`mock::context`).
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::pr_popover::{PrPopover, Primary, RunEnd};
use crate::fleet::send_to_agent::{FIX, check_key, thread_key};
use crate::fleet::smart_links::{format_check_failure_prompt, format_fix_prompt, format_review_thread_prompt};
use crate::mock::context::{Case, Pop, SESSION};
use crate::tests::{FRAME, app, click};
use crate::tests_header_edit::has;
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::Value;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Fake {
    /// The (path, body) of every `POST .../prompt`.
    pub(crate) posted: Vec<(String, Value)>,
    /// What it answers instead of a receipt.
    refuse: Option<u16>,
    /// While set, the answer waits for a message here.
    gate: Option<Arc<Mutex<Receiver<()>>>>,
}

pub(crate) type Shared = Arc<Mutex<Fake>>;

pub(crate) fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let req = crate::test_http::read(&mut stream);
    if req.method.is_empty() {
        return;
    }
    let (method, path, body) = (req.method, req.path, req.body);
    let (status, reply) = if method == "POST" && path.ends_with("/prompt") {
        let posted: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let (gate, refuse) = {
            let mut f = fake.lock().unwrap();
            f.posted.push((path, posted));
            (f.gate.clone(), f.refuse)
        };
        if let Some(gate) = gate {
            let _ = gate.lock().unwrap().recv_timeout(test_wait::BOUND);
        }
        match refuse {
            Some(code) => (code, "{}".to_string()),
            None => (200, r#"{"eventId":1,"correlationId":"x"}"#.to_string()),
        }
    } else {
        (404, "{}".to_string())
    };
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

/// The popover of `case` open on an app whose Fleet is the fake one (`base` given), or on one with no server.
fn rig_at(case: Case, base: impl FnOnce(Shared) -> Option<String>) -> Rig {
    let fake: Shared = Default::default();
    let base = base(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    if let Some(base) = base {
        let link = Link::start(base, post);
        rt.update(move |s, _| s.link = Some(link));
    }
    rt.update(move |s, _| s.show_popover(case, true, Pop::Plain));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    Rig { rt, queue, fake }
}

fn rig(case: Case) -> Rig {
    rig_at(case, |fake| Some(serve(fake)))
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            format!("open {:?}, error {:?}", s.ui.open.is_some(), s.agent_sends.error(SESSION))
        });
    }

    fn posted(&self) -> Vec<(String, Value)> {
        self.fake.lock().unwrap().posted.clone()
    }

    fn has(&self, key: &str) -> bool {
        has(&self.rt, key)
    }

    fn link(&self) -> crate::fleet::smart_links::SmartLink {
        self.rt.state.smart_links.session_pull_request(SESSION).expect("the pull request").clone()
    }

    fn label(&self, key: &str) -> String {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label).unwrap_or_default()
    }
}

fn model(case: Case) -> PrPopover {
    let r = rig_at(case, |_| None);
    PrPopover::of(&r.link())
}

// SessionContextChips.test.ts: "sends every failing check and open thread to the agent from the pill"
#[test]
fn sends_every_failing_check_and_open_thread_to_the_agent_from_the_pill() {
    let mut r = rig(Case::PopBlocked);
    assert!(r.has("session-pr-popover"), "the pill opened the popover");
    assert!(r.label("session-pr-fix").contains("Fix checks and address review"), "{}", r.label("session-pr-fix"));
    click(&mut r.rt, "session-pr-fix");
    r.wait("the prompt to arrive and the button to say so", |s| s.agent_sends.is_sent(SESSION, FIX));
    let posted = r.posted();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].0, format!("/api/sessions/{SESSION}/prompt"));
    let text = posted[0].1["text"].as_str().expect("text").to_string();
    assert!(text.contains("Workflow: e2e") && text.contains("Workflow: unit"), "every failing check: {text}");
    assert!(text.contains("File: src/cache.rs:12") && text.contains("File: src/cache.rs:48"), "every open thread: {text}");
    assert!(!text.contains("Workflow: lint") && !text.contains("Workflow: build"), "only the failing ones");
    assert!(posted[0].1["userMessageId"].as_str().is_some_and(|id| !id.is_empty()), "a fresh message id");
    let link = r.link();
    let m = PrPopover::of(&link);
    assert_eq!(text, format_fix_prompt(&link, &m.failing, &m.threads), "the same text Fleet's formatter writes");
    r.rt.frame(FRAME);
    assert!(r.label("session-pr-fix").contains("Sent to the agent"), "{}", r.label("session-pr-fix"));
}

// SessionContextChips.test.ts: "offers to archive the session once its pull request is merged"
#[test]
fn offers_to_archive_the_session_once_its_pull_request_is_merged() {
    let mut r = rig(Case::PopMerged);
    assert_eq!(r.rt.state.smart_links.session_pull_request(SESSION).map(|l| l.status.as_str()), Some("merged"));
    assert!(r.has("session-pr-merged"), "the merged section");
    assert!(r.label("session-pr-archive").contains("Archive session"));
    click(&mut r.rt, "session-pr-archive");
    assert!(r.rt.state.ui.open.is_none(), "the popover closed");
    assert!(r.rt.state.archive.hides(SESSION), "the session is on its way to the archive");
}

#[test]
fn the_pill_opens_the_popover_and_a_press_outside_closes_it() {
    let mut r = rig(Case::PopBlocked);
    r.rt.update(|s, _| s.ui.open = None);
    r.rt.frame(FRAME);
    assert!(!r.has("session-pr-popover"));
    click(&mut r.rt, "session-pr-pill");
    assert!(r.has("session-pr-popover"));
    click(&mut r.rt, "session-pr-scrim");
    assert!(!r.has("session-pr-popover"));
}

#[test]
fn each_send_button_works_alone() {
    let mut r = rig(Case::PopBlocked);
    let link = r.link();
    click(&mut r.rt, "session-pr-send-check-e2e");
    r.wait("the check to be sent", |s| s.agent_sends.is_sent(SESSION, &check_key("e2e")));
    assert!(!r.rt.state.agent_sends.is_off(SESSION, &check_key("unit")), "another check is still to send");
    assert!(!r.rt.state.agent_sends.is_off(SESSION, &thread_key("t1")), "so is a thread");
    assert!(!r.rt.state.agent_sends.is_off(SESSION, FIX), "and the footer");
    click(&mut r.rt, "session-pr-send-thread-t1");
    r.wait("the thread to be sent", |s| s.agent_sends.is_sent(SESSION, &thread_key("t1")));
    let posted = r.posted();
    assert_eq!(posted.len(), 2);
    let m = PrPopover::of(&link);
    let run = m.failing.iter().find(|f| f.name == "e2e").expect("e2e fails");
    assert_eq!(posted[0].1["text"], format_check_failure_prompt(&link, run));
    assert_eq!(posted[1].1["text"], format_review_thread_prompt(&link, &m.threads[0]));
    assert_ne!(posted[0].1["userMessageId"], posted[1].1["userMessageId"], "each send has its own id");
}

#[test]
fn a_button_is_off_while_it_sends_and_once_it_has_sent_so_nothing_goes_twice() {
    let mut r = rig(Case::PopBlocked);
    let (hold, gate) = channel::<()>();
    r.fake.lock().unwrap().gate = Some(Arc::new(Mutex::new(gate)));
    click(&mut r.rt, "session-pr-send-check-e2e");
    r.wait("the request to reach the server", |s| s.agent_sends.is_sending(SESSION, &check_key("e2e")));
    r.rt.frame(FRAME);
    click(&mut r.rt, "session-pr-send-check-e2e");
    hold.send(()).expect("open the gate");
    r.wait("the check to be sent", |s| s.agent_sends.is_sent(SESSION, &check_key("e2e")));
    click(&mut r.rt, "session-pr-send-check-e2e");
    r.rt.frame(FRAME);
    assert_eq!(r.posted().len(), 1, "pressed three times, sent once");
    assert!(r.rt.state.agent_sends.is_sent(SESSION, &check_key("e2e")));
}

#[test]
fn says_what_fleet_answered_when_it_refuses() {
    let mut r = rig(Case::PopBlocked);
    r.fake.lock().unwrap().refuse = Some(502);
    click(&mut r.rt, "session-pr-fix");
    r.wait("the error line", |s| s.agent_sends.error(SESSION).is_some());
    assert_eq!(r.rt.state.agent_sends.error(SESSION), Some("Couldn't send to the agent (HTTP 502)."));
    assert!(!r.rt.state.agent_sends.is_sent(SESSION, FIX), "a refused send can be tried again");
    r.rt.frame(FRAME);
    assert!(r.has("session-pr-error"));
    // The next try clears the line.
    r.fake.lock().unwrap().refuse = None;
    click(&mut r.rt, "session-pr-fix");
    r.wait("the retry to be sent", |s| s.agent_sends.is_sent(SESSION, FIX));
    assert_eq!(r.rt.state.agent_sends.error(SESSION), None);
}

#[test]
fn says_so_when_fleet_cannot_be_reached() {
    // A server that listened a moment ago and is gone.
    let dead = crate::test_http::dead_address();
    let mut r = rig_at(Case::PopBlocked, |_| Some(dead));
    click(&mut r.rt, "session-pr-fix");
    r.wait("the error line", |s| s.agent_sends.error(SESSION).is_some());
    assert_eq!(r.rt.state.agent_sends.error(SESSION), Some("Couldn't reach Fleet to send this to the agent."));
}

#[test]
fn with_no_server_at_all_it_says_the_same() {
    let mut r = rig_at(Case::PopChecks, |_| None);
    click(&mut r.rt, "session-pr-send-check-e2e");
    assert_eq!(r.rt.state.agent_sends.error(SESSION), Some("Couldn't reach Fleet to send this to the agent."));
}

#[test]
fn show_in_context_closes_the_popover_and_asks_the_tab_to_show_the_card() {
    let mut r = rig(Case::PopBlocked);
    r.rt.update(|s, _| s.content_open = false);
    click(&mut r.rt, "session-pr-show-context");
    assert!(r.rt.state.ui.open.is_none());
    assert!(r.rt.state.content_open);
    let focus = r.rt.state.smart_links.focus.clone().expect("a focus request");
    assert_eq!((focus.session_id.as_str(), focus.target.as_str()), (SESSION, "link-187"));
}

#[test]
fn the_footer_has_four_forms() {
    assert_eq!(model(Case::PopBlocked).primary, Primary::Fix("Fix checks and address review"));
    assert_eq!(model(Case::PopChecks).primary, Primary::Fix("Fix failing checks"));
    assert_eq!(model(Case::PopReview).primary, Primary::Fix("Address review"));
    assert_eq!(model(Case::PopMerged).primary, Primary::Archive);
    assert_eq!(model(Case::PopReady).primary, Primary::MergeOnGithub);
    // Nothing blocks but nobody has approved: no primary button, and the ghost ones stay.
    let mut r = rig_at(Case::PopReady, |_| None);
    r.rt.update(|s, _| {
        let id = s.session().id.clone();
        let mut wire = crate::mock::links::pull_request(&id, 187, "open", vec![], None);
        wire["title"] = "example/demo #187: Change 187".into();
        s.smart_links.set_links(&id, &[wire]);
    });
    r.rt.frame(FRAME);
    assert_eq!(PrPopover::of(&r.link()).primary, Primary::None);
    assert!(r.has("session-pr-show-context") && r.has("session-pr-open"));
    assert!(!r.has("session-pr-fix") && !r.has("session-pr-archive") && !r.has("session-pr-merge"));
}

#[test]
fn lists_the_checks_still_running_or_failing_and_gives_only_a_failing_one_a_button() {
    let m = model(Case::PopBlocked);
    let names: Vec<&str> = m.listed.iter().map(|l| l.run.name.as_str()).collect();
    assert_eq!(names, ["build", "e2e", "unit"], "lint passed, so it is only in the bar");
    assert_eq!(m.listed[0].end, RunEnd::Word("running".into()));
    assert_eq!(m.listed[1].end, RunEnd::Send);
    assert_eq!(m.counts.map(|c| (c.passed, c.failing, c.pending)), Some((1, 2, 1)));
    let r = rig(Case::PopBlocked);
    assert!(r.has("session-pr-send-check-e2e") && r.has("session-pr-send-check-unit"));
    assert!(!r.has("session-pr-send-check-build"));
}

#[test]
fn the_review_section_names_who_asked_for_changes_else_counts_the_threads() {
    assert_eq!(model(Case::PopBlocked).review_line().as_deref(), Some("mira requested changes"));
    assert_eq!(model(Case::PopReview).review_line().as_deref(), Some("2 unresolved threads"));
    assert_eq!(model(Case::PopChecks).review_line(), None);
    assert!(model(Case::PopBlocked).conflict && !model(Case::PopReview).conflict);
}

#[test]
fn the_header_names_the_branches_and_defaults_the_base_to_main() {
    let m = model(Case::PopBlocked);
    assert_eq!((m.head_ref.as_deref(), m.base_ref.as_str()), (Some("fix/thumbnail-cache"), "main"));
    assert_eq!((m.repo.as_str(), m.number.as_str(), m.diff), ("demo", "187", Some((412, 88))));
    assert_eq!(m.title, "Cache thumbnails between reloads");
    let bare = model(Case::Open);
    assert_eq!((bare.head_ref, bare.base_ref.as_str()), (None, "main"));
}

#[test]
fn a_merged_popover_has_no_checks_review_or_conflict() {
    let r = rig(Case::PopMerged);
    assert!(r.has("session-pr-merged"));
    assert!(!r.has("session-pr-checks") && !r.has("session-pr-review") && !r.has("session-pr-conflict"));
    let r = rig(Case::PopBlocked);
    assert!(r.has("session-pr-checks") && r.has("session-pr-review") && r.has("session-pr-conflict"));
}
