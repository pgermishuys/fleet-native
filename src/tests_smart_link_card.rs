//! The smart-link card (component 8, piece J): `SessionContextCanvas.test.ts`'s "sends a failing check to the agent from a
//! visible button" under its name, the Checks and Review signals' expanding and collapsing, the pill and meta line's words,
//! and the pin, unpin and dismiss buttons. Sending runs against the popover tests' fake Fleet. Invented GitHub data only.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::send_to_agent::{check_key, thread_key};
use crate::fleet::smart_link_card::{Card, review_summary};
use crate::fleet::smart_links::{Relationship, SmartLink, format_check_failure_prompt, format_review_thread_prompt};
use crate::mock::context::SESSION;
use crate::mock::context_tab::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_header_edit::has;
use crate::tests_pr_popover::{Fake, Shared, serve};
use crate::{State, test_wait};
use rune::Runtime;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const CARD: &str = "link-187";

fn shown(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_context_tab_scene(case, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn link_of(rt: &Runtime<State>) -> SmartLink {
    rt.state.smart_links.by_session.get(SESSION).and_then(|v| v.iter().find(|l| l.id == CARD)).expect("the card's link").clone()
}

fn label(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label).unwrap_or_default()
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

/// The card of `case` on an app whose Fleet is the fake one.
fn rig(case: Case) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake::default()));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_context_tab_scene(case, true);
    });
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    Rig { rt, queue, fake }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("error {:?}", s.agent_sends.error(SESSION)));
    }
}

// SessionContextCanvas.test.ts: "sends a failing check to the agent from a visible button"
#[test]
fn sends_a_failing_check_to_the_agent_from_a_visible_button() {
    let mut r = rig(Case::CardFailing);
    let key = format!("context-send-{CARD}-check-e2e");
    assert!(label(&r.rt, &key).contains("Send the e2e failure to the agent"), "a visible button on the failing run");
    assert!(has(&r.rt, &format!("context-send-{CARD}-check-unit")), "and on every other failing run");
    assert!(!has(&r.rt, &format!("context-send-{CARD}-check-lint")), "none on a passing one");
    click(&mut r.rt, &key);
    r.wait("the check to be sent", |s| s.agent_sends.is_sent(SESSION, &check_key("e2e")));
    let posted = r.fake.lock().unwrap().posted.clone();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].0, format!("/api/sessions/{SESSION}/prompt"));
    let text = posted[0].1["text"].as_str().expect("text").to_string();
    assert!(text.contains("Workflow: e2e"), "{text}");
    let link = link_of(&r.rt);
    let run = crate::fleet::smart_links::ci_status(&link).unwrap().check_runs.into_iter().find(|r| r.name == "e2e").unwrap();
    assert_eq!(text, format_check_failure_prompt(&link, &run), "the text Fleet's formatter writes");
    r.rt.frame(FRAME);
    assert_eq!(label(&r.rt, &key), "Sent");
    assert!(!r.rt.state.agent_sends.is_off(SESSION, &check_key("unit")), "the other run's button still works");
}

#[test]
fn each_thread_has_a_send_button_that_sends_that_thread() {
    let mut r = rig(Case::CardReview);
    let key = format!("context-send-{CARD}-thread-t2");
    click(&mut r.rt, &key);
    r.wait("the thread to be sent", |s| s.agent_sends.is_sent(SESSION, &thread_key("t2")));
    let posted = r.fake.lock().unwrap().posted.clone();
    assert_eq!(posted.len(), 1, "only that thread");
    let link = link_of(&r.rt);
    let th = crate::fleet::smart_links::review_threads(&link).into_iter().find(|t| t.thread_node_id == "t2").unwrap();
    assert_eq!(posted[0].1["text"].as_str().unwrap(), format_review_thread_prompt(&link, &th));
    assert!(!r.rt.state.agent_sends.is_off(SESSION, &thread_key("t1")));
}

#[test]
fn checks_open_by_themselves_while_one_fails_and_a_press_closes_and_opens_them() {
    let mut rt = shown(Case::CardFailing);
    let list = format!("context-checks-list-{CARD}");
    let head = format!("context-checks-{CARD}");
    assert!(has(&rt, &list), "open by default while a check fails");
    click(&mut rt, &head);
    assert!(!has(&rt, &list), "closed by the press");
    click(&mut rt, &head);
    assert!(has(&rt, &list), "and open again");
}

#[test]
fn checks_that_all_passed_start_closed() {
    let mut rt = shown(Case::CardConflict);
    assert!(has(&rt, &format!("context-checks-{CARD}")) && has(&rt, &format!("context-checks-bar-{CARD}")), "the bar shows closed");
    assert!(!has(&rt, &format!("context-checks-list-{CARD}")));
    click(&mut rt, &format!("context-checks-{CARD}"));
    assert!(has(&rt, &format!("context-checks-list-{CARD}")));
}

#[test]
fn the_review_signal_lists_its_threads_when_opened() {
    let mut rt = shown(Case::CardReview);
    let list = format!("context-review-list-{CARD}");
    assert!(has(&rt, &list) && has(&rt, &format!("context-thread-{CARD}-t1")) && has(&rt, &format!("context-thread-{CARD}-t2")));
    click(&mut rt, &format!("context-review-{CARD}"));
    assert!(!has(&rt, &list), "closed by the press");
    click(&mut rt, &format!("context-review-{CARD}"));
    assert!(has(&rt, &list));
}

#[test]
fn a_merged_card_has_one_static_line_and_no_signals() {
    let rt = shown(Case::CardMerged);
    assert!(has(&rt, &format!("context-merged-{CARD}")));
    assert!(!has(&rt, &format!("context-checks-{CARD}")) && !has(&rt, &format!("context-reviews-{CARD}")));
    let c = Card::of(&link_of(&rt));
    assert!(c.merged && !c.open_pr && c.branch.is_none() && c.diff.is_none(), "no branch or diff once merged");
}

#[test]
fn a_conflict_names_the_branch_it_conflicts_with() {
    let rt = shown(Case::CardConflict);
    assert!(label(&rt, &format!("context-conflict-{CARD}")).contains("with main"));
    assert!(Card::of(&link_of(&rt)).attention, "a conflict wants the user");
}

#[test]
fn the_meta_line_says_what_github_said_or_what_it_has_not_yet() {
    let rt = shown(Case::CardPending);
    let c = Card::of(&link_of(&rt));
    assert_eq!(c.note, Some("Checking GitHub\u{2026}"));
    assert!(c.pill.is_none() && !c.open_pr);
    assert!(has(&rt, &format!("context-note-{CARD}")) && !has(&rt, &format!("context-state-{CARD}")));

    let rt = shown(Case::CardLabels);
    let c = Card::of(&link_of(&rt));
    assert_eq!(c.pill.as_ref().map(|p| p.label.as_str()), Some("Ready to merge"));
    assert_eq!(c.branch, Some(("fix/thumbnail-cache".to_string(), "main".to_string())));
    assert_eq!(c.diff, Some((412, 88)));
    assert_eq!(c.labels.len(), 3);
    assert!(has(&rt, &format!("context-branch-{CARD}")) && has(&rt, &format!("context-diff-{CARD}")));
    assert!(has(&rt, &format!("context-label-{CARD}-bug")), "a chip per label");
}

#[test]
fn the_pill_says_what_a_blocked_pull_request_waits_on() {
    let c = Card::of(&link_of(&shown(Case::CardFailing)));
    assert_eq!(c.pill.as_ref().map(|p| p.label.as_str()), Some("Blocked"), "a blocked pull request reads its state, not the words");
    assert_eq!(c.checks.text, "2 failing \u{b7} 1 running \u{b7} 1 passed");
    assert!(c.attention);
}

#[test]
fn review_summary_names_who_is_to_answer() {
    let c = Card::of(&link_of(&shown(Case::CardReview)));
    assert_eq!(review_summary(&c.reviewers), "Changes requested by mira");
    let c = Card::of(&link_of(&shown(Case::CardLabels)));
    assert_eq!(review_summary(&c.reviewers), "Approved by mira, jonas");
    assert_eq!(review_summary(&[]), "0 reviewers");
}

#[test]
fn pin_unpin_and_dismiss_are_on_the_card_by_how_it_relates() {
    let mut rt = shown(Case::CardLabels);
    let id = format!("context-dismiss-{CARD}");
    assert!(has(&rt, &id) && !has(&rt, &format!("context-unpin-{CARD}")) && !has(&rt, &format!("context-pin-{CARD}")), "own: dismiss only");
    rt.update(|s, _| {
        let mut wire = crate::mock::context::popover_pr("open", vec![], None, vec![], false, false);
        wire["relationship"] = "pinned".into();
        s.smart_links.set_links(SESSION, &[wire]);
    });
    rt.frame(FRAME);
    assert_eq!(link_of(&rt).relationship, Relationship::Pinned);
    assert!(has(&rt, &format!("context-unpin-{CARD}")) && has(&rt, &id), "pinned: unpin and dismiss");
    click(&mut rt, &id);
    assert!(!has(&rt, &format!("context-link-{CARD}")), "dismissed at once");
}

#[test]
fn the_card_with_something_to_fix_gets_the_attention_border() {
    assert!(Card::of(&link_of(&shown(Case::CardFailing))).attention);
    assert!(!Card::of(&link_of(&shown(Case::CardLabels))).attention);
}
