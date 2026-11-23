//! The "Fleet restarted" notice (component 9, piece N): `use-restart-notice.test.ts` under its names. Fleet's test hands the
//! composable a sessions store and fake calls; here the sessions are rows of the open list, the notice is read from the
//! store after `keep_restart_notice`, and a button is pressed by telling the notice's source what the button says.
use crate::fleet::notices::Notice;
use crate::fleet::restart_notice::{PREFIX, RUN_PREFIX, other_machine_lines, title};
use crate::fleet::rows::Interruption;
use crate::tests::app;
use rune::Runtime;

fn cut(id: &str, title: &str, waiting: bool, is_cut: bool, auto: bool) -> crate::mock::Session {
    let mut s = crate::mock::Session {
        id: id.into(),
        title: title.into(),
        status: crate::mock::Status::Idle,
        ago_secs: None,
        branch: String::new(),
        messages: Vec::new(),
        tokens: 0,
        row: Default::default(),
        listing: Default::default(),
    };
    if is_cut {
        s.row.interrupted = Some(Interruption {
            at: "2026-11-06T09:00:00Z".into(),
            waiting,
            last_call: (!waiting).then(|| "bash".into()),
            changed_files: Some(2),
            auto,
        });
    }
    s
}

fn set(rt: &mut Runtime<crate::State>, sessions: Vec<crate::mock::Session>) {
    rt.update(move |s, _| {
        s.projects[0].sessions = sessions;
        s.keep_restart_notice();
    });
}

fn notice(rt: &Runtime<crate::State>) -> Option<Notice> {
    rt.state.notices.notices().iter().find(|n| n.id.starts_with(PREFIX)).cloned()
}

fn press(rt: &mut Runtime<crate::State>, run: &str) {
    let run = run.to_string();
    rt.update(move |s, _| {
        s.notices.tell(&run);
        s.keep_restart_buttons();
        s.keep_restart_notice();
    });
}

fn labels(n: &Notice) -> Vec<&str> {
    n.actions.iter().map(|a| a.label.as_str()).collect()
}

// use-restart-notice.test.ts: "says Fleet restarted and lists the cut-off sessions, noting a lost question"
#[test]
fn says_fleet_restarted_and_lists_the_cut_off_sessions_noting_a_lost_question() {
    let mut rt = app();
    set(
        &mut rt,
        vec![
            cut("a", "Fix login", false, true, false),
            cut("b", "Write docs", true, true, false),
            cut("c", "Idle one", false, false, false),
        ],
    );
    let n = notice(&rt).unwrap();
    assert_eq!(n.title, "Fleet restarted");
    assert_eq!(n.body.as_deref(), Some("2 sessions were cut off."));
    assert_eq!(n.items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), ["Fix login", "Write docs: the question was lost. Ask again."]);
    assert_eq!(labels(&n), ["Continue all", "Dismiss all"]);
    assert_eq!(n.hold_secs, Some(30.0));
}

// use-restart-notice.test.ts: "names the version when the restart was an update"
#[test]
fn names_the_version_when_the_restart_was_an_update() {
    assert_eq!(title(Some("v0.66.0")), "Fleet updated to v0.66.0");
    assert_eq!(title(Some("0.66.0")), "Fleet updated to v0.66.0");
    let mut rt = app();
    set(&mut rt, vec![cut("a", "Fix login", false, true, false)]);
    assert_eq!(notice(&rt).unwrap().body.as_deref(), Some("1 session was cut off."));
    // The title the card really draws follows the version this run updated to.
    assert_eq!(notice(&rt).unwrap().title, "Fleet restarted");
    rt.state.after_update.updated_to = Some("0.66.0".into());
    rt.state.keep_restart_notice();
    assert_eq!(notice(&rt).unwrap().title, "Fleet updated to v0.66.0");
}

// use-restart-notice.test.ts: "Continue all continues every cut-off session and removes the notice"
// The requests themselves (POST …/interruption/continue for each) go out on a thread; with no server the rows are cleared, as
// Fleet's own answer does, and the notice goes.
#[test]
fn continue_all_continues_every_cut_off_session_and_removes_the_notice() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, false), cut("b", "Two", false, true, false)]);
    let run = notice(&rt).unwrap().actions[0].run.clone();
    assert!(run.starts_with(RUN_PREFIX));
    press(&mut rt, &run);
    assert!(notice(&rt).is_none());
    assert!(rt.state.projects[0].sessions.iter().all(|s| s.row.interrupted.is_none()));
}

// use-restart-notice.test.ts: "Dismiss all sends nothing to the agents"
#[test]
fn dismiss_all_sends_nothing_to_the_agents() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, false), cut("b", "Two", false, true, false)]);
    let (cont, dismiss) = {
        let n = notice(&rt).unwrap();
        (n.actions[0].run.clone(), n.actions[1].run.clone())
    };
    assert_ne!(cont, dismiss, "its own request: a DELETE, never the POST that continues");
    press(&mut rt, &dismiss);
    assert!(notice(&rt).is_none());
}

// use-restart-notice.test.ts: "goes away when the last session is continued from its own card"
#[test]
fn goes_away_when_the_last_session_is_continued_from_its_own_card() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, false)]);
    assert!(notice(&rt).is_some());
    set(&mut rt, vec![cut("a", "One", false, false, false)]);
    assert!(notice(&rt).is_none());
}

// use-restart-notice.test.ts: "offers the automatic switch in one line while it is off, and the line turns it on"
#[test]
fn offers_the_automatic_switch_in_one_line_while_it_is_off_and_the_line_turns_it_on() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, false)]);
    let link = notice(&rt).unwrap().link.unwrap();
    assert_eq!(link.label, "Next time, continue these by itself?");
    press(&mut rt, &link.run.unwrap());
    assert_eq!(rt.state.pickers.prefs.get("RestartContinueAuto"), Some("true"));
}

// use-restart-notice.test.ts: "doesn't offer the switch once it is on"
#[test]
fn doesnt_offer_the_switch_once_it_is_on() {
    let mut rt = app();
    rt.update(|s, _| {
        s.pickers.prefs.set("RestartContinueAuto", "true");
    });
    set(&mut rt, vec![cut("a", "One", false, true, false)]);
    assert!(notice(&rt).unwrap().link.is_none());
}

// use-restart-notice.test.ts: "in automatic mode says Fleet is continuing, with Stop all instead of Continue all"
#[test]
fn in_automatic_mode_says_fleet_is_continuing_with_stop_all_instead_of_continue_all() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, true), cut("b", "Two", false, true, true), cut("c", "Three", true, true, false)]);
    let n = notice(&rt).unwrap();
    assert_eq!(n.body.as_deref(), Some("Continuing 2 sessions."));
    assert_eq!(labels(&n), ["Stop all"]);
    assert!(n.link.is_none());
    // Stop all forgets the count; the next look at the rows takes it up again only for sessions still marked automatic.
    let run = n.actions[0].run.clone();
    rt.update(move |s, _| {
        s.notices.tell(&run);
        s.keep_restart_buttons();
    });
    assert_eq!(rt.state.restart.continuing(), 0);
}

// use-restart-notice.test.ts: "Esc on the automatic notice stops the sends too"
#[test]
fn esc_on_the_automatic_notice_stops_the_sends_too() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, true)]);
    let n = notice(&rt).unwrap();
    assert_eq!(n.on_escape, Some(n.actions[0].run.clone()), "Esc tells what Stop all tells");
    // Esc on a card in manual mode stops nothing.
    set(&mut rt, vec![cut("a", "One", false, true, false)]);
    assert_eq!(notice(&rt).unwrap().on_escape, None);
}

// use-restart-notice.test.ts: "keeps saying how many it is continuing while the sends go out"
#[test]
fn keeps_saying_how_many_it_is_continuing_while_the_sends_go_out() {
    let mut rt = app();
    set(&mut rt, vec![cut("a", "One", false, true, true), cut("b", "Two", false, true, true)]);
    set(&mut rt, vec![cut("a", "One", false, false, false), cut("b", "Two", false, true, true)]);
    assert_eq!(notice(&rt).unwrap().body.as_deref(), Some("Continuing 2 sessions."));
}

// use-restart-notice.test.ts: "says one line per other machine that restarted"
#[test]
fn says_one_line_per_other_machine_that_restarted() {
    let lines = other_machine_lines(&[("build-box".into(), 1), ("laptop".into(), 2), ("quiet".into(), 0)]);
    assert_eq!(lines, ["build-box restarted: 1 session cut off", "laptop restarted: 2 sessions cut off"]);
}

// Not in Fleet's file: only another machine restarted, and more than five cut off.
#[test]
fn a_machine_only_restart_has_no_buttons_and_a_long_list_says_how_many_more() {
    use crate::fleet::restart_notice::{Cut, Inputs, build};
    let lines = vec!["build-box restarted: 1 session cut off".to_string()];
    let mut continuing = 0;
    let n = build(&Inputs { cut: &[], lines: &lines, auto_pref: false, updated_to: None }, &mut continuing).unwrap();
    assert_eq!((n.title.as_str(), n.actions.len(), n.items.len()), ("A machine restarted", 0, 1));
    let many: Vec<Cut> =
        (0..7).map(|i| Cut { id: i.to_string(), title: format!("Task {i}"), waiting: false, at: "t".into(), auto: false }).collect();
    let n = build(&Inputs { cut: &many, lines: &lines, auto_pref: false, updated_to: None }, &mut continuing).unwrap();
    assert_eq!((n.items.len(), n.more.as_deref()), (6, Some("+2 more")), "five sessions and the machine's line");
}
