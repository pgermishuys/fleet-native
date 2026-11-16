//! The fork dialog and the analytics popover (component 8, piece E). Fleet has no test for either, so these are written from
//! reading `ForkSessionDialog.vue`, `useForkSession` and `SessionAnalyticsPopover.vue`. The requests are read from the fake
//! server `tests_header_edit` runs.
use crate::fleet::fork::{fork_body, fork_path, prefill, source_title};
use crate::fleet::header::usage_strings;
use crate::mock::header::Case;
use crate::tests::{FRAME, click};
use crate::tests_header_edit::{Live, has, scene};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::json;

const ANSWER: &str = r#"{"instanceId":"inst-7","workspaceId":"ws-7","harnessType":"opencode","forkedFromSessionId":"a-open","session":{"id":"a-forked","title":"Fork of Try a smaller cache limit","tags":[]}}"#;

fn typed(rt: &mut Runtime<crate::State>, words: &str) {
    let words = words.to_string();
    rt.update(move |s, _| s.head.bar.fork.as_mut().expect("the dialog is open").title.replace_text(words));
    rt.frame(FRAME);
}

fn field(rt: &Runtime<crate::State>) -> String {
    rt.state.head.bar.fork.as_ref().map(|d| d.title.text.clone()).unwrap_or_default()
}

fn open(rt: &mut Runtime<crate::State>) {
    click(rt, "fork");
    assert!(has(rt, "fork-session-dialog"), "the dialog opened");
}

#[test]
fn the_dialog_names_the_source_and_prefills_the_title_the_server_would_give() {
    let mut rt = scene(Case::Fork);
    assert!(!has(&rt, "fork-session-dialog"));
    open(&mut rt);
    assert_eq!(field(&rt), "Fork of Try a smaller cache limit");
    assert_eq!(rt.state.head.bar.fork.as_ref().unwrap().source_title, "Try a smaller cache limit");
    assert!(has(&rt, "fork-session-source-title") && has(&rt, "fork-session-submit") && has(&rt, TITLE));
    assert_eq!(prefill("  "), "Fork of Untitled session");
    assert_eq!(source_title(""), "Untitled session");
}

const TITLE: &str = crate::fleet::fork::TITLE_FIELD;

#[test]
fn a_blank_title_is_sent_as_null_and_a_typed_one_trimmed() {
    assert_eq!(fork_path("a b"), "/api/sessions/a%20b/fork");
    assert_eq!(fork_body("   "), json!({ "title": null }));
    assert_eq!(fork_body("  Try it  "), json!({ "title": "Try it" }));
}

#[test]
fn enter_submits_and_the_title_goes_out_trimmed() {
    let mut live = Live::start(Case::Fork, Some((200, ANSWER)));
    open(&mut live.rt);
    typed(&mut live.rt, "  Smaller limit, take two ");
    live.rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    live.rt.frame(FRAME);
    live.wait("the fork to be sent", |l| !l.calls().is_empty());
    assert_eq!(
        live.calls(),
        [("POST".to_string(), "/api/sessions/a-open/fork".to_string(), json!({ "title": "Smaller limit, take two" }))]
    );
}

#[test]
fn forking_disables_the_dialog_and_a_second_press_sends_nothing() {
    let mut live = Live::start(Case::Fork, Some((200, ANSWER)));
    open(&mut live.rt);
    click(&mut live.rt, "fork-session-submit");
    assert!(live.rt.state.head.bar.fork.as_ref().unwrap().forking, "the button says Forking… and the field is off");
    live.rt.update(|s, tx| s.fork_submit(tx));
    live.wait("the answer", |l| l.rt.state.head.bar.fork.is_none());
    assert_eq!(live.calls().len(), 1, "one request");
}

#[test]
fn a_refused_fork_keeps_the_dialog_open_with_the_servers_words_and_the_next_open_clears_them() {
    let mut live = Live::start(Case::Fork, Some((409, r#"{"error":"Nothing to copy yet."}"#)));
    open(&mut live.rt);
    click(&mut live.rt, "fork-session-submit");
    live.wait("the refusal", |l| l.rt.state.head.bar.fork.as_ref().is_some_and(|d| d.error.is_some()));
    let d = live.rt.state.head.bar.fork.as_ref().unwrap();
    assert_eq!(d.error.as_deref(), Some("Nothing to copy yet."));
    assert!(!d.forking, "pressing again is possible");
    assert!(has(&live.rt, "fork-session-error") && has(&live.rt, "fork-session-dialog"));
    live.rt.update(|s, _| s.close_fork_dialog());
    live.rt.frame(FRAME);
    open(&mut live.rt);
    assert!(live.rt.state.head.bar.fork.as_ref().unwrap().error.is_none());
    assert!(!has(&live.rt, "fork-session-error"));
}

#[test]
fn a_fork_that_names_no_session_is_a_failure() {
    let mut live = Live::start(Case::Fork, Some((200, "{}")));
    open(&mut live.rt);
    click(&mut live.rt, "fork-session-submit");
    live.wait("the failure", |l| l.rt.state.head.bar.fork.as_ref().is_some_and(|d| d.error.is_some()));
    assert_eq!(live.rt.state.head.bar.fork.as_ref().unwrap().error.as_deref(), Some("Failed to fork session"));
}

#[test]
fn a_fork_closes_the_dialog_opens_the_new_session_and_lists_it_under_its_parent() {
    let mut live = Live::start(Case::Fork, Some((200, ANSWER)));
    open(&mut live.rt);
    click(&mut live.rt, "fork-session-submit");
    live.wait("the new session", |l| l.rt.state.open_id().as_deref() == Some("a-forked"));
    assert!(live.rt.state.head.bar.fork.is_none(), "the dialog closed");
    let made = live.rt.state.projects.iter().flat_map(|p| &p.sessions).find(|s| s.id == "a-forked").expect("listed");
    assert_eq!(made.title, "Fork of Try a smaller cache limit");
    assert_eq!(made.listing.forked_from.as_deref(), Some("a-open"));
    assert_eq!(made.listing.spawn_kind.as_deref(), Some("fork"));
    assert_eq!(made.row.harness_type.as_deref(), Some("opencode"));
    assert_eq!(made.row.workspace_id, "ws-7");
}

#[test]
fn the_fork_without_a_server_is_made_from_the_invented_row() {
    let mut rt = scene(Case::Fork);
    open(&mut rt);
    click(&mut rt, "fork-session-submit");
    rt.frame(FRAME);
    assert!(rt.state.head.bar.fork.is_none());
    assert!(rt.state.session().title.starts_with("Fork of "));
    assert_eq!(rt.state.session().listing.forked_from.as_deref(), Some("a-open"));
}

// ---- the analytics popover -------------------------------------------------------------------------------------------------

fn analytics(tokens: Option<u64>, cost: Option<f64>) -> Runtime<crate::State> {
    let mut rt = scene(Case::Fork);
    rt.update(move |s, _| {
        let row = &mut s.projects[0].sessions[0].row;
        (row.total_tokens, row.total_cost) = (tokens, cost);
    });
    rt.frame(FRAME);
    click(&mut rt, "analytics");
    assert!(has(&rt, "analytics-popover"), "the card opened");
    rt
}

#[test]
fn the_popover_shows_the_two_numbers() {
    let rt = analytics(Some(128_400), Some(0.4231));
    assert_eq!(rt.state.header_model().tokens_text, "128,400");
    assert_eq!(rt.state.header_model().cost_text, "$0.42");
    assert!(has(&rt, "analytics-tokens-value") && has(&rt, "analytics-cost-value"));
}

#[test]
fn the_popover_says_a_dash_for_a_number_the_server_does_not_have() {
    let rt = analytics(None, None);
    assert_eq!(rt.state.header_model().tokens_text, "\u{2014}");
    assert_eq!(rt.state.header_model().cost_text, "\u{2014}");
    let rt = analytics(Some(0), None);
    assert_eq!(rt.state.header_model().tokens_text, "0", "a known zero is a number");
}

#[test]
fn money_rounds_to_cents_and_a_trace_says_less_than_a_cent() {
    assert_eq!(usage_strings(Some(1_234_567), Some(1234.5)), ("1,234,567".to_string(), "$1,234.50".to_string()));
    assert_eq!(usage_strings(None, Some(0.004)).1, "<$0.01");
    assert_eq!(usage_strings(None, Some(0.0)).1, "$0.00");
}

#[test]
fn the_card_closes_on_a_press_outside_it() {
    let mut rt = analytics(None, None);
    click(&mut rt, "analytics-scrim");
    assert!(!has(&rt, "analytics-popover"));
}
