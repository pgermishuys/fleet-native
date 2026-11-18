//! Component 13, piece D: the drawer. Fleet's `TerminalDrawer.test.ts` (the seven cases of this piece) and
//! `TerminalToggleButton.test.ts`, under their names, against a fake Fleet that answers the terminal calls over HTTP
//! (a tab's socket never connects to it, which these cases don't need), and ours.
use crate::tests::{FRAME, click};
use crate::tests_terminal_tabs::{asked, enable, entry, rig, serving, tabs};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn posts(fake: &crate::tests_archive_pins::Fake) -> Vec<crate::tests_archive_pins::Call> {
    asked(fake).into_iter().filter(|c| c.0 == "POST").collect()
}

fn open_drawer(r: &mut crate::tests_archive_pins::Rig) {
    r.rt.update(|s, _| s.terminals.set_open("s1", true));
    r.rt.frame(FRAME);
}

fn live_id(r: &crate::tests_archive_pins::Rig) -> Option<String> {
    r.state().terminal.live.as_ref().map(|l| l.id.clone())
}

// ---- TerminalDrawer.test.ts --------------------------------------------------------------------------------------------------

#[test]
fn renders_nothing_and_starts_nothing_until_the_drawer_is_opened() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    r.rt.frame(FRAME);

    assert!(r.rt.find("terminal-drawer").is_none());
    assert!(r.state().terminal.live.is_none(), "no view is made, so no socket");
    assert!(posts(&fake).is_empty());
}

#[test]
fn starts_a_shell_when_opened_with_no_terminals_once_the_list_has_loaded() {
    let fake = serving(vec![], |method, path| {
        if method == "POST" && path.ends_with("/terminals") { (201, entry("t1", "zsh").to_string()) } else { (204, String::new()) }
    });
    let mut r = rig(&fake);
    r.rt.update(|s, _| {
        s.terminals_allowed = Some(true);
        s.terminal.session = s.open_id();
        s.terminals.set_open("s1", true);
    });
    r.rt.frame(FRAME);
    // The list hasn't been read, so nothing is started yet.
    assert!(posts(&fake).is_empty());
    assert!(!r.state().terminals.is_loaded("s1"));

    // Fleet's case sets the list directly; a drawer left open on a list read as empty closes, one asked for stays.
    r.rt.update(|s, _| s.terminals.set_terminals("s1", vec![]));
    r.rt.frame(FRAME);
    r.wait("the new shell", |s| s.terminals.active_for("s1").is_some_and(|t| t.id == "t1"));

    assert_eq!(posts(&fake).len(), 1);
    assert_eq!(tabs(&r, "s1"), ["t1"]);
}

#[test]
fn shows_why_a_shell_couldnt_start_and_tries_again_on_request() {
    let tries = Arc::new(AtomicUsize::new(0));
    let counted = tries.clone();
    let fake = serving(vec![], move |method, path| {
        if method == "POST" && path.ends_with("/terminals") {
            if counted.fetch_add(1, Ordering::SeqCst) == 0 {
                return (409, json!({"error": "The session's folder /gone doesn't exist any more."}).to_string());
            }
            return (201, entry("t1", "zsh").to_string());
        }
        (204, String::new())
    });
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    open_drawer(&mut r);

    r.wait("the refusal", |s| s.terminal.error.is_some());
    r.rt.frame(FRAME);
    assert!(r.state().terminal.error.as_deref().unwrap().contains("/gone"));
    assert!(r.rt.find("terminal-message").is_some() && r.rt.find("terminal-retry").is_some());
    assert_eq!(posts(&fake).len(), 1, "it doesn't try again by itself");

    click(&mut r.rt, "terminal-retry");
    r.wait("the second shell", |s| s.terminals.active_for("s1").is_some_and(|t| t.id == "t1"));
    assert_eq!(posts(&fake).len(), 2);
    assert!(r.state().terminal.error.is_none());
}

#[test]
fn mounts_a_tabs_terminal_the_first_time_it_shows_and_keeps_it() {
    let fake = serving(vec![entry("t1", "zsh"), entry("t2", "zsh 2")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    open_drawer(&mut r);
    assert_eq!(live_id(&r).as_deref(), Some("t1"));
    assert!(r.state().terminal.parked.is_empty(), "t2 has not been shown, so it has no view");
    let first = Arc::as_ptr(&r.state().terminal.live.as_ref().unwrap().term);

    click(&mut r.rt, "terminal-tab-t2");
    assert_eq!(live_id(&r).as_deref(), Some("t2"));
    let parked: Vec<_> = r.state().terminal.parked.iter().map(|l| l.id.clone()).collect();
    assert_eq!(parked, ["t1"], "the view that was showing is kept");

    click(&mut r.rt, "terminal-tab-t1");
    assert_eq!(live_id(&r).as_deref(), Some("t1"));
    assert_eq!(Arc::as_ptr(&r.state().terminal.live.as_ref().unwrap().term), first, "the same view and connection came back");
}

#[test]
fn closes_a_tab_through_the_server_and_clears_the_one_showing() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    open_drawer(&mut r);
    r.rt.update(|s, _| s.terminal.live.as_ref().unwrap().term.feed(b"one\r\ntwo\r\n"));
    r.rt.frame(FRAME);
    assert_eq!(crate::term::text_of(&r.state().terminal.live.as_ref().unwrap().term)[..2], ["one", "two"]);

    click(&mut r.rt, "terminal-clear");
    assert!(crate::term::text_of(&r.state().terminal.live.as_ref().unwrap().term).iter().all(String::is_empty), "the screen is clear");

    click(&mut r.rt, "terminal-tab-close-t1");
    r.wait("the close", |_| fake.calls("/terminals/t1").iter().any(|c| c.0 == "DELETE"));
    assert!(tabs(&r, "s1").is_empty());
    assert!(!r.state().terminals.is_open("s1"), "the last tab closing closes the drawer");
}

#[test]
fn hides_without_ending_the_shells() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    open_drawer(&mut r);

    click(&mut r.rt, "terminal-hide");

    assert!(!r.state().terminals.is_open("s1"));
    assert!(r.rt.find("terminal-drawer").is_none());
    assert!(asked(&fake).iter().all(|c| c.0 != "DELETE"));
    assert_eq!(tabs(&r, "s1"), ["t1"]);
    assert_eq!(live_id(&r).as_deref(), Some("t1"), "its view is kept, hidden");
}

#[test]
fn shows_the_session_folder() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    r.rt.update(|s, _| s.projects[0].sessions[0].listing.workspace_directory = "/home/dev/lantern".into());
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    open_drawer(&mut r);

    assert!(r.rt.find("terminal-cwd").is_some());
    assert!(r.rt.find("terminal-size").is_some(), "the active size too");
}

// ---- TerminalToggleButton.test.ts --------------------------------------------------------------------------------------------

#[test]
fn isnt_there_when_fleet_has_terminals_turned_off() {
    let fake = serving(vec![], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, false);
    r.rt.frame(FRAME);
    assert!(r.rt.find("terminal").is_none());

    enable(&mut r, true);
    r.rt.frame(FRAME);
    assert!(r.rt.find("terminal").is_some());
}

#[test]
fn shows_and_hides_the_sessions_drawer() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    assert!(r.state().terminal_toggle_words().starts_with("Show terminal"));

    click(&mut r.rt, "terminal");

    assert!(r.state().terminals.is_open("s1"));
    assert!(r.state().terminal_toggle_words().starts_with("Hide terminal"));
    click(&mut r.rt, "terminal");
    assert!(!r.state().terminals.is_open("s1"));
}

#[test]
fn marks_shells_still_running_while_the_drawer_is_hidden() {
    let fake = serving(vec![entry("t1", "zsh")], |_, _| (204, String::new()));
    let mut r = rig(&fake);
    enable(&mut r, true);
    r.wait("the list", |s| s.terminals.is_loaded("s1"));
    r.rt.frame(FRAME);
    assert!(r.rt.find("terminal-dot").is_some());
    assert!(r.state().terminal_toggle_words().contains("shells are running"));

    r.rt.update(|s, _| s.terminals.set_open("s1", true));
    r.rt.frame(FRAME);
    assert!(r.rt.find("terminal-dot").is_none());
}

// ---- ours --------------------------------------------------------------------------------------------------------------------

#[test]
fn the_toggle_words_name_the_key() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.set_session_id((0, 0), "s1"));
    let key = if cfg!(target_os = "macos") { "\u{2318} J" } else { "Ctrl J" };
    assert_eq!(rt.state.terminal_toggle_words(), format!("Show terminal ({key})"));
    rt.update(|s, tx| s.toggle_terminal(tx));
    assert_eq!(rt.state.terminal_toggle_words(), format!("Hide terminal ({key})"));
}

#[test]
fn with_no_server_the_toggle_opens_a_pretend_shell_and_new_adds_another_tab() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.set_session_id((0, 0), "s1"));
    rt.frame(FRAME);
    click(&mut rt, "terminal");
    rt.frame(FRAME);
    assert!(rt.find("terminal-drawer").is_some());
    let id = rt.state.open_id().unwrap();
    assert_eq!(rt.state.terminals.terminals_for(&id).len(), 1);
    assert!(rt.state.terminal.live.as_ref().is_some_and(|l| l.pretend));

    click(&mut rt, "terminal-new");
    assert_eq!(rt.state.drawer_tabs().iter().map(|t| t.1.clone()).collect::<Vec<_>>(), ["zsh", "zsh 2"]);
    assert_eq!(rt.state.terminal.live.as_ref().map(|l| l.id.as_str()), Some("mock-2"));

    // Off once the server says so.
    rt.update(|s, _| s.terminals_allowed = Some(false));
    rt.frame(FRAME);
    assert!(rt.find("terminal").is_none());
}

#[test]
fn the_drawer_is_kept_under_60_percent_of_the_column_and_never_under_140() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| {
        s.sheet_h = 700.0;
        s.terminals.set_height(600.0);
    });
    assert_eq!(crate::view::terminal_drawer::height(&rt.state), 420.0);
    rt.update(|s, _| {
        s.sheet_h = 200.0;
        s.terminals.set_height(272.0);
    });
    assert_eq!(crate::view::terminal_drawer::height(&rt.state), 140.0);
}

#[test]
fn clear_keeps_the_cursors_row_and_drops_the_scrollback() {
    let term = crate::term::Terminal::new(20, 4);
    term.feed(b"a\r\nb\r\nc\r\nd\r\ne\r\nprompt $ ");
    term.clear();
    assert_eq!(crate::term::text_of(&term), ["prompt $", "", "", ""]);
}
