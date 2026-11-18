//! The page tab's bar, warnings and choice bar (component 14, piece C): the five `PageCanvas.test.ts` cases of its piece
//! under their names, and ours for what a press sends. A headless run has no web view, so the page area is the fallback panel;
//! the bar and the bar under it are the same. Requests go to a fake Fleet that records them.
use crate::mock::pages::TabCase;
use crate::tests::{FRAME, app, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::web::wanted::{Wanted, wanted};
use rune::Runtime;
use serde_json::{Value, json};

type Rt = Runtime<crate::State>;
const TAB: &str = "canvas:page_1";

fn shown(case: TabCase) -> Rt {
    let mut rt = app();
    rt.update(move |s, _| s.show_page_tab(case));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn all_text(rt: &Rt) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn text_of(rt: &Rt, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.or(e.label)).unwrap_or_default()
}

fn hear(rt: &mut Rt, data: Value) {
    rt.update(move |s, tx| {
        s.hear_page(TAB, &data, tx);
    });
    rt.frame(FRAME);
}

fn with_rig(case: TabCase, fake: &Fake) -> Rig {
    let mut r = Rig::home(fake);
    r.rt.update(move |s, _| s.show_page_tab(case));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    r
}

// PageCanvas.test.ts: "frames Fleet's copy of the page…" (as: the address the host is told to load)
#[test]
fn frames_fleets_copy_of_the_page_on_the_machine_it_was_shown_on() {
    let mut rt = shown(TabCase::Waiting);
    rt.update(|s, _| s.web.rect = Some([600.0, 60.0, 360.0, 500.0]));
    let Wanted::Shown(p) = wanted(&rt.state) else { panic!("the page should be wanted") };
    assert_eq!(p.url, "http://127.0.0.1:6411/pages/p1/index.html");
    assert_eq!(rt.state.page_outside_url(TAB).as_deref(), Some(p.url.as_str()), "Open in a new tab goes to the same address");
}

// Row 14.16: another machine's session has its page on that machine's base.
#[test]
fn a_session_on_another_machine_has_its_page_on_that_machines_base() {
    let home = Fake::start(|_, _| (200, "{}".into()));
    let mut r = with_rig(TabCase::Waiting, &home);
    let body =
        json!({"machines": [{"id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "name": "mini", "baseUrl": "http://10.0.0.7:6262", "token": "t"}]});
    r.rt.update(move |s, _| {
        s.machines.apply_list(crate::fleet::machines::parse_machine_list(200, &body.to_string()));
        let session = s.canvas_session();
        s.machines.session_machines.insert(session, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    });
    assert_eq!(r.state().page_outside_url(TAB).as_deref(), Some("http://10.0.0.7:6262/pages/p1/index.html"));
}

// "says what a page Fleet wrote itself is, in place of a file"
#[test]
fn says_what_a_page_fleet_wrote_itself_is_in_place_of_a_file() {
    let rt = shown(TabCase::Label);
    assert_eq!(text_of(&rt, "page-tab-file"), "Walkthrough \u{b7} compared with main at 55650b2");
    assert!(!has(&rt, "page-tab-path"), "no path beside a label");
}

#[test]
fn shows_the_file_name_and_the_whole_source_when_there_is_no_label() {
    let rt = shown(TabCase::Waiting);
    assert_eq!(text_of(&rt, "page-tab-file"), "index.html");
    assert_eq!(text_of(&rt, "page-tab-path"), "/work/pocket-notes/layouts/index.html");
    let long = shown(TabCase::LongPath);
    assert!(
        text_of(&long, "page-tab-path").ends_with("with-a-very-long-folder-name/index.html"),
        "the whole source is the text; the row clips it"
    );
}

// "lists the links that won't load"
#[test]
fn lists_the_links_that_wont_load() {
    let rt = shown(TabCase::Warnings);
    assert!(has(&rt, "page-tab-warnings"));
    for i in 0..3 {
        assert!(has(&rt, &format!("page-tab-warning-{i}")), "warning {i}");
    }
    assert!(all_text(&rt).contains("fonts/inter.woff2 is outside the page's folder"));
    assert!(!has(&shown(TabCase::Waiting), "page-tab-warnings"), "no list without warnings");
}

// "loads the page again, and on Reload": a new shownAt is A's; Reload is a new load key for the same page.
#[test]
fn loads_the_page_again_on_reload_and_not_on_any_other_redraw() {
    let mut rt = shown(TabCase::Waiting);
    rt.update(|s, _| s.web.rect = Some([600.0, 60.0, 360.0, 500.0]));
    let key = |rt: &Rt| match wanted(&rt.state) {
        Wanted::Shown(p) => p.load_key,
        other => panic!("{other:?}"),
    };
    let first = key(&rt);
    rt.frame(FRAME);
    assert_eq!(key(&rt), first, "a redraw leaves the page alone");
    click(&mut rt, "page-tab-reload");
    let second = key(&rt);
    assert_ne!(second, first, "Reload loads it again");
    assert!(second.starts_with("p1:"), "still the same page: {second}");
}

// "opens the page in a tab of its own": the bar has the button, and it has an address to open.
#[test]
fn offers_to_open_the_page_in_a_tab_of_its_own() {
    let rt = shown(TabCase::Waiting);
    assert!(has(&rt, "page-tab-open-new"));
    assert!(has(&rt, "page-tab-reload"));
    assert_eq!(rt.state.page_outside_url("canvas:another"), None);
}

// "says when the page's pick is too long to send, and keeps Send off with that reason"
#[test]
fn says_when_the_pages_pick_is_too_long_to_send_and_keeps_send_off_with_that_reason() {
    let mut rt = shown(TabCase::Waiting);
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "x".repeat(20_000), "summary": "Every answer"}));
    let reason = "The page's pick was too long to send: 20,000 characters, and the most is 16,000.";
    assert!(text_of(&rt, "page-choice-text").contains(reason));
    assert!(has(&rt, "page-choice-skip"), "Skip is still there");
    assert!(has(&rt, "page-choice-send"));
    click(&mut rt, "page-choice-send");
    assert_eq!(rt.state.web.choices.get(TAB).and_then(|ui| ui.sending), None, "Send does nothing with no pick");
}

// "takes a pick the page sends after a refused one"
#[test]
fn takes_a_pick_the_page_sends_after_a_refused_one() {
    let mut rt = shown(TabCase::Waiting);
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "  "}));
    assert!(text_of(&rt, "page-choice-text").contains("The page's pick was empty"));
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "a", "summary": "Layout A"}));
    assert!(text_of(&rt, "page-choice-text").contains("You picked Layout A."));
    assert!(has(&rt, "page-choice-change"), "Change takes Skip's place");
    assert!(!has(&rt, "page-choice-skip"));
}

#[test]
fn keeps_the_first_pick_until_change_and_then_takes_another() {
    let mut rt = shown(TabCase::Waiting);
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "a", "summary": "Layout A"}));
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "b", "summary": "Layout B"}));
    assert!(text_of(&rt, "page-choice-text").contains("Layout A"), "a page can't swap it under the user");
    click(&mut rt, "page-choice-change");
    assert!(text_of(&rt, "page-choice-text").contains("waiting for your pick"));
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "b", "summary": "Layout B"}));
    assert!(text_of(&rt, "page-choice-text").contains("Layout B"));
}

#[test]
fn takes_no_pick_once_the_wait_is_over() {
    let mut rt = shown(TabCase::Stale);
    hear(&mut rt, json!({"type": "fleet:page-choice", "value": "a", "summary": "Layout A"}));
    assert_eq!(text_of(&rt, "page-choice-text"), "Not answered.");
    assert!(!has(&rt, "page-choice-send"), "no buttons on a bar that waits for nothing");
    let answered = shown(TabCase::Answered);
    assert!(text_of(&answered, "page-choice-text").starts_with("Sent: Paged table."));
    assert!(text_of(&answered, "page-choice-text").contains("Answered "));
}

#[test]
fn a_new_wait_starts_clean() {
    let mut rt = shown(TabCase::Proposed);
    rt.update(|s, _| {
        let tab = TAB.to_string();
        let mut canvas = crate::fleet::canvases::read_server_canvas(&json!({
            "canvasId": "page_1", "kind": "page", "title": "Layouts", "version": 9,
            "state": {"pageId": "p1", "entry": "index.html", "choice": {"callId": "call_2", "status": "waiting", "at": "2026-10-07T01:52:00Z"}},
        }))
        .unwrap();
        canvas.version = 9;
        s.canvases.set_server_canvases("mock.0.0", vec![canvas]);
        let _ = tab;
    });
    rt.frame(FRAME);
    assert_eq!(text_of(&rt, "page-choice-text"), "The agent is waiting for your pick.", "call_1's proposal is not call_2's");
}

// Ours: what Send and Skip post, and what a failure says.
#[test]
fn send_posts_the_proposal_and_sending_lasts_until_the_canvas_changes() {
    let fake = Fake::start(|_, _| (200, "{}".into()));
    let mut r = with_rig(TabCase::Proposed, &fake);
    click(&mut r.rt, "page-choice-send");
    r.wait("the send request", |_| !fake.calls("/page-choices/").is_empty());
    let calls = fake.calls("/page-choices/");
    assert_eq!(calls.len(), 1);
    assert_eq!((calls[0].0.as_str(), calls[0].1.as_str()), ("POST", "/api/sessions/mock.0.0/page-choices/call_1/send"));
    assert_eq!(calls[0].2, json!({"value": "b", "summary": "Paged table"}));
    assert_eq!(text_of(&r.rt, "page-choice-send"), "Sending\u{2026}");
    click(&mut r.rt, "page-choice-send");
    assert_eq!(fake.calls("/page-choices/").len(), 1, "a second press while sending sends nothing");
}

#[test]
fn skip_posts_to_skip() {
    let fake = Fake::start(|_, _| (200, "{}".into()));
    let mut r = with_rig(TabCase::Waiting, &fake);
    click(&mut r.rt, "page-choice-skip");
    r.wait("the skip request", |_| !fake.calls("/page-choices/").is_empty());
    let calls = fake.calls("/page-choices/");
    assert_eq!((calls[0].0.as_str(), calls[0].1.as_str()), ("POST", "/api/sessions/mock.0.0/page-choices/call_1/skip"));
}

#[test]
fn a_failure_says_fleets_words_and_lets_the_user_try_again() {
    let fake = Fake::start(|_, path| {
        if path.ends_with("/send") {
            (409, r#"{"error":"The agent is no longer waiting for this pick."}"#.into())
        } else {
            (500, "nope".into())
        }
    });
    let mut r = with_rig(TabCase::Proposed, &fake);
    click(&mut r.rt, "page-choice-send");
    r.wait("Fleet's reason", |s| s.web.choices.get(TAB).is_some_and(|ui| !ui.error.is_empty()));
    r.rt.frame(FRAME);
    assert!(text_of(&r.rt, "page-choice-text").contains("The agent is no longer waiting for this pick."));
    assert_eq!(text_of(&r.rt, "page-choice-send"), "Send to agent", "off sending: the user can press it again");
    click(&mut r.rt, "page-choice-send");
    r.wait("the second send", |_| fake.calls("/page-choices/").len() == 2);
    // A refusal with no reason in it gets the bar's own sentence.
    let plain = Fake::start(|_, _| (500, "nope".into()));
    let mut r = with_rig(TabCase::Waiting, &plain);
    click(&mut r.rt, "page-choice-skip");
    r.wait("the sentence", |s| s.web.choices.get(TAB).is_some_and(|ui| !ui.error.is_empty()));
    assert_eq!(r.state().web.choices[TAB].error, "Couldn't send that. Try again.");
}
