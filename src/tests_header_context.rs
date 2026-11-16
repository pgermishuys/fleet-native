//! The header's context chips and the pull request pill's trigger (component 8, piece G): `SessionContextChips.test.ts`'s
//! four tests that belong to the trigger, under their names (the two about the popover are piece H's), and the pill's words
//! and states beside it. Invented GitHub data only (`mock::context`).
use crate::fleet::context_chips::{ContextChips, describe};
use crate::fleet::pr_state::PrState;
use crate::fleet::rows::Origin;
use crate::mock::context::{Case, SESSION, issue};
use crate::tests::{FRAME, app, click};
use crate::tests_header_edit::has;
use rune::Runtime;

fn scene(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_chips(case, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn chips(rt: &Runtime<crate::State>) -> ContextChips {
    let row = &rt.state.session().row;
    ContextChips::of(&rt.state.smart_links, SESSION, row.origin.as_ref()).expect("the session has context")
}

// SessionContextChips.test.ts: "shows where the session came from, its pull request as a pill, and pinned links as chips"
#[test]
fn shows_where_the_session_came_from_its_pull_request_as_a_pill_and_pinned_links_as_chips() {
    let rt = scene(Case::Chips);
    let c = chips(&rt);
    assert_eq!(c.origin.as_ref().map(|o| o.number.as_str()), Some("42"));
    assert!(has(&rt, "context-chip-link-42"), "from #42");
    let pill = c.pill.as_ref().expect("the pull request is the pill");
    assert_eq!(pill.state, PrState::Blocked);
    assert_eq!(pill.number, "187");
    assert!(pill.words.contains("1 failing") && pill.words.contains("1 thread"), "{}", pill.words);
    assert!(has(&rt, "session-pr-pill"));
    assert!(has(&rt, "context-chip-link-9"), "a pinned link is a chip");
    // The pull request is not drawn twice, and nothing is left for "+N".
    assert!(!has(&rt, "context-chip-link-187"));
    assert!(!has(&rt, "context-more"));
    assert_eq!(c.links.len(), 2);
}

// SessionContextChips.test.ts: "opens the Context tab at the origin when clicked"
#[test]
fn opens_the_context_tab_at_the_origin_when_clicked() {
    let mut rt = scene(Case::Chips);
    rt.update(|s, _| s.content_open = false);
    rt.frame(FRAME);
    click(&mut rt, "context-chip-link-42");
    assert!(rt.state.content_open, "the right panel opened");
    let focus = rt.state.smart_links.focus.clone().expect("the tab was asked to show a card");
    assert_eq!((focus.session_id.as_str(), focus.target.as_str()), (SESSION, "link-42"));
}

// SessionContextChips.test.ts: "names the automation a session was started by"
#[test]
fn names_the_automation_a_session_was_started_by() {
    let mut rt = scene(Case::Automation);
    assert_eq!(chips(&rt).automation.as_deref(), Some("Nightly dependency sweep"));
    assert!(has(&rt, "context-automation"));
    click(&mut rt, "context-automation");
    assert_eq!(rt.state.smart_links.focus.as_ref().map(|f| f.target.as_str()), Some("origin"));
    // An automation with no title is "Automation".
    rt.update(|s, _| s.projects[0].sessions[0].row.origin.as_mut().unwrap().title = None);
    assert_eq!(chips(&rt).automation.as_deref(), Some("Automation"));
}

// SessionContextChips.test.ts: "renders nothing when the session has no context"
#[test]
fn renders_nothing_when_the_session_has_no_context() {
    let mut rt = scene(Case::Chips);
    rt.update(|s, _| {
        s.smart_links.set_links(SESSION, &[]);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(ContextChips::of(&rt.state.smart_links, SESSION, None).is_none());
    assert!(!has(&rt, "context-chips"));
    assert!(!has(&rt, "session-pr-pill"));
}

#[test]
fn the_pill_is_coloured_by_the_state_and_says_what_the_pull_request_waits_on() {
    for (case, state, words) in [
        (Case::Open, PrState::Open, "1 of 2 checks done"),
        (Case::Blocked, PrState::Blocked, "1 failing · 1 thread"),
        (Case::Draft, PrState::Draft, "Draft"),
        (Case::Merged, PrState::Merged, "Merged"),
        (Case::Closed, PrState::Closed, "Closed"),
    ] {
        let rt = scene(case);
        let pill = chips(&rt).pill.unwrap();
        assert_eq!((pill.state, pill.words.as_str()), (state, words), "{case:?}");
        assert!(has(&rt, "session-pr-words"), "{case:?}: the words show once GitHub has answered");
        assert_eq!(pill.description, format!("Pull request #187: Change 187. {words}"));
    }
}

#[test]
fn a_pill_before_github_answers_is_a_neutral_number_only() {
    let rt = scene(Case::Pending);
    let pill = chips(&rt).pill.unwrap();
    assert!(!pill.resolved);
    assert_eq!(pill.state, PrState::Draft);
    assert!(has(&rt, "session-pr-number"));
    assert!(!has(&rt, "session-pr-words"));
}

#[test]
fn only_an_open_pull_request_with_checks_gets_a_check_icon() {
    use crate::fleet::pr_state::Checks;
    assert_eq!(chips(&scene(Case::Open)).pill.unwrap().checks, Some(Checks::Pending));
    assert_eq!(chips(&scene(Case::Blocked)).pill.unwrap().checks, Some(Checks::Failing));
    assert_eq!(chips(&scene(Case::Merged)).pill.unwrap().checks, None);
}

#[test]
fn two_chips_then_a_count_of_the_rest_that_shows_the_first_one_left_out() {
    let mut rt = scene(Case::Overflow);
    let c = chips(&rt);
    assert_eq!((c.links.len(), c.overflow), (2, 2));
    assert!(has(&rt, "context-more"));
    click(&mut rt, "context-more");
    assert_eq!(rt.state.smart_links.focus.as_ref().map(|f| f.target.clone()), Some(c.overflow_target));
    assert!(rt.state.content_open);
}

#[test]
fn a_chip_with_a_failing_check_or_a_conflict_on_an_open_pull_request_is_tinted() {
    let mut rt = scene(Case::Chips);
    let failing = crate::mock::context::failing(31, "pinned");
    rt.update(move |s, _| {
        let mut wires = vec![issue(42, "origin")];
        wires.push(failing);
        s.smart_links.set_links(SESSION, &wires);
    });
    rt.frame(FRAME);
    // The pull request is the pill here, so a second one is the chip that is tinted.
    let c = chips(&rt);
    assert!(c.pill.is_some());
    let mut again = crate::mock::context::failing(32, "pinned");
    again["id"] = "link-32".into();
    rt.update(move |s, _| s.smart_links.upsert_link(&again));
    assert!(chips(&rt).links.iter().any(|l| l.attention));
}

#[test]
fn a_chip_says_what_it_is_and_what_started_the_session() {
    let rt = scene(Case::Chips);
    let c = chips(&rt);
    assert_eq!(describe(&c.origin.unwrap().link), "Issue #42: Thumbnails reload too often. Session started from this. Open");
}

#[test]
fn a_github_origin_the_server_has_not_made_a_link_yet_shows_from_the_record() {
    let mut rt = scene(Case::Open);
    let record = |kind: &str, url: &str| Origin {
        source_type: kind.into(),
        title: Some("Cache limit".into()),
        resource_url: Some(url.into()),
        resource_id: None,
        provider_id: "builtin.github".into(),
    };
    rt.update(move |s, _| {
        s.projects[0].sessions[0].row.origin = Some(record("github-pull-request", "https://github.com/example/demo/pull/55"))
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let p = chips(&rt).pending.expect("pending origin");
    assert_eq!((p.number.as_str(), p.is_pull_request, p.title.as_str()), ("55", true, "Cache limit"));
    assert!(has(&rt, "context-origin-pending"));
    // Another provider's record, or a URL with no number, shows nothing; a link that is the origin takes its place.
    rt.update(move |s, _| s.projects[0].sessions[0].row.origin = Some(record("x", "https://example.test/")));
    assert!(chips(&rt).pending.is_none());
    rt.update(move |s, _| {
        s.projects[0].sessions[0].row.origin = Some(record("github-issue", "https://github.com/example/demo/issues/42"));
        let wires = vec![issue(42, "origin")];
        s.smart_links.set_links(SESSION, &wires);
    });
    assert!(chips(&rt).pending.is_none());
    assert!(chips(&rt).origin.is_some());
}
