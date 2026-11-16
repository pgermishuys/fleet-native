//! The status bar's hints (component 9, piece B): `StatusBar.test.ts` ("StatusBar hints" and "command store") under its
//! names, `StatusBar.characterise.test.ts` as view tests of its two states, and the rail's Help menu (`IconRail.vue`).
//! A hint is read from the element tree: its tooltip (`help`), its label and its dimming.
use crate::fleet::status_hints::{command_hint, interrupt_hint, terminal_shown};
use crate::mock::Status;
use crate::tests::{FRAME, app, click};
use crate::ui::{Open, commands};
use rune::{ElementInfo, Runtime};

fn info(rt: &Runtime<crate::State>, key: &str) -> ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn dim(rt: &Runtime<crate::State>, key: &str) -> bool {
    info(rt, key).opacity < 1.0
}

fn set_status(rt: &mut Runtime<crate::State>, status: Status) {
    rt.update(move |s, _| {
        let (p, i) = s.open_at();
        s.projects[p].sessions[i].status = status;
    });
    rt.frame(FRAME);
}

/// The bar with one session in the list: nothing to go to.
fn alone() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects.truncate(1);
        s.projects[0].sessions.truncate(1);
    });
    rt.frame(FRAME);
    rt
}

// ---- StatusBar.test.ts, "StatusBar hints" ------------------------------------------------------------------------------------

#[test]
fn opens_and_closes_the_command_palette_the_way_ctrl_k_does() {
    let mut rt = app();
    click(&mut rt, "palette-hint");
    assert_eq!(rt.state.ui.open, Some(Open::Palette));
    // The palette's scrim is over the bar now, so the second press is the click that closes it; Ctrl K's toggle is the
    // same function the hint runs.
    rt.update(crate::ui::palette::toggle);
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn runs_the_previous_and_next_session_commands_from_their_own_buttons() {
    let mut rt = app();
    assert_eq!(info(&rt, "status-hint-prev-session").label.as_deref(), Some("Previous session"));
    assert_eq!(info(&rt, "status-hint-next-session").label.as_deref(), Some("Next session"));
    assert!(info(&rt, "status-hint-next-session").help.as_deref().unwrap().contains("Next session"));
    let order = rt.state.order();
    let here = rt.state.open_at();
    let at = order.iter().position(|&o| o == here).unwrap();
    click(&mut rt, "status-hint-next-session");
    assert_eq!(rt.state.open_at(), order[(at + 1) % order.len()], "next went forward and nothing else ran");
    click(&mut rt, "status-hint-prev-session");
    assert_eq!(rt.state.open_at(), here, "previous came back");
}

#[test]
fn disables_previous_and_next_with_the_commands_reason_when_theres_no_other_session() {
    let mut rt = alone();
    for key in ["status-hint-prev-session", "status-hint-next-session"] {
        let hint = info(&rt, key);
        assert_eq!(hint.help.as_deref(), Some("A second session is required."), "{key}");
        assert!(dim(&rt, key), "{key} is dimmed");
    }
    let before = rt.state.open_at();
    click(&mut rt, "status-hint-next-session");
    assert_eq!(rt.state.open_at(), before);
}

#[test]
fn runs_the_sidebar_command_and_names_what_it_will_do() {
    let mut rt = app();
    assert_eq!(info(&rt, "status-hint-sidebar").label.as_deref(), Some("Hide sidebar"));
    click(&mut rt, "status-hint-sidebar");
    assert!(!rt.state.sidebar_open, "one press ran the command");
    assert_eq!(info(&rt, "status-hint-sidebar").label.as_deref(), Some("Show sidebar"));
}

#[test]
fn interrupts_only_while_the_session_is_working() {
    let mut rt = app();
    set_status(&mut rt, Status::Idle);
    let hint = info(&rt, "status-hint-interrupt");
    assert_eq!(hint.help.as_deref(), Some("Nothing to interrupt: the session isn't working"));
    assert!(dim(&rt, "status-hint-interrupt"));
    click(&mut rt, "status-hint-interrupt");
    assert!(rt.state.aborting.is_none(), "nothing was interrupted");

    set_status(&mut rt, Status::Working);
    assert_eq!(info(&rt, "status-hint-interrupt").help.as_deref(), Some("Interrupt the session (Esc)"));
    assert!(!dim(&rt, "status-hint-interrupt"));
    click(&mut rt, "status-hint-interrupt");
    assert_eq!(rt.state.session().status, Status::Idle, "the press interrupted the turn");
}

#[test]
fn keeps_interrupt_in_place_disabled_when_no_session_is_open() {
    let mut rt = app();
    rt.update(|s, _| s.projects.clear());
    rt.frame(FRAME);
    assert!(!rt.state.has_session());
    let hint = info(&rt, "status-hint-interrupt");
    assert_eq!(hint.help.as_deref(), Some("Open a session to interrupt it"));
    assert!(dim(&rt, "status-hint-interrupt"));
}

#[test]
fn shows_the_terminal_button_only_when_terminals_are_on_and_runs_the_terminal_command() {
    let mut rt = app();
    rt.update(|s, _| s.terminals_allowed = Some(false));
    rt.frame(FRAME);
    assert!(!has(&rt, "status-hint-terminal"));

    rt.update(|s, _| s.terminals_allowed = Some(true));
    rt.frame(FRAME);
    assert!(has(&rt, "status-hint-terminal"));
    click(&mut rt, "status-hint-terminal");
    // The terminal is component 13's: the command says so, the way the palette's row does.
    assert!(rt.state.ui.toast.as_ref().is_some_and(|t| t.message.contains("Show terminal")));
}

#[test]
fn the_terminal_hint_is_drawn_in_the_scenes_and_waits_for_a_server_to_say() {
    assert!(terminal_shown(None, false), "the invented data draws the full bar");
    assert!(!terminal_shown(None, true), "a server that hasn't said keeps it away (Fleet's default is off)");
    assert!(terminal_shown(Some(true), true));
    assert!(!terminal_shown(Some(false), false));
}

#[test]
fn doesnt_take_the_keyboard_when_a_hint_is_clicked() {
    let mut rt = app();
    set_status(&mut rt, Status::Working);
    click(&mut rt, "composer");
    let focused = |rt: &Runtime<crate::State>| rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    let before = focused(&rt);
    assert!(before.is_some(), "the composer has the keyboard");
    // (Next and prev open another session, which gives its own composer the keyboard: not what this checks.)
    for key in ["status-hint-sidebar", "status-hint-terminal"] {
        click(&mut rt, key);
        assert_eq!(focused(&rt), before, "{key} left the keyboard where it was");
    }
}

// ---- StatusBar.test.ts, "command store" --------------------------------------------------------------------------------------

#[test]
fn runs_a_command_unless_its_missing_or_disabled() {
    let mut rt = alone();
    let (before, sidebar) = (rt.state.open_at(), rt.state.sidebar_open);
    rt.update(|s, tx| {
        commands::run(s, tx, "toggle-sidebar");
        commands::run(s, tx, "nav-next-session");
        commands::run(s, tx, "no-such-command");
    });
    assert_ne!(rt.state.sidebar_open, sidebar, "an enabled command ran");
    assert_eq!(rt.state.open_at(), before, "a disabled one did not");
}

#[test]
fn a_missing_command_leaves_its_hint_disabled_as_not_available_yet() {
    let hint = command_hint(None, "Ctrl B");
    assert!(hint.disabled);
    assert_eq!(hint.title, "Not available yet");
    assert!(interrupt_hint(None, true, true).disabled);
}

// ---- StatusBar.characterise.test.ts ------------------------------------------------------------------------------------------

#[test]
fn draws_the_same_with_no_session() {
    let mut rt = app();
    rt.update(|s, _| s.go_live(false));
    rt.frame(FRAME);
    let texts = bar_texts(&rt);
    assert!(texts.iter().any(|t| t == "Command palette") && texts.iter().any(|t| t == "Interrupt"));
    assert!(!texts.iter().any(|t| t.ends_with(" tokens")), "no session, no token count: {texts:?}");
}

#[test]
fn draws_the_same_with_a_session() {
    let rt = app();
    let texts = bar_texts(&rt);
    assert!(texts.iter().any(|t| t == "opencode"));
    assert!(texts.iter().any(|t| t.ends_with(" tokens")), "{texts:?}");
    // Hints first, then the session's part: the end cluster keeps its order (row 4.8).
    let x = |key: &str| rt.find(key).unwrap().0;
    assert!(x("status-hint-sidebar") < x("status-hint-interrupt"));
}

fn bar_texts(rt: &Runtime<crate::State>) -> Vec<String> {
    let bar = info(rt, "status-bar").rect;
    rt.elements().into_iter().filter(|e| e.rect[1] >= bar[1] && e.rect[1] < bar[1] + bar[3]).filter_map(|e| e.text).collect()
}

// ---- IconRail.vue: the Help menu ---------------------------------------------------------------------------------------------

#[test]
fn the_help_button_opens_a_menu_of_two_rows() {
    let mut rt = app();
    click(&mut rt, "rail-help");
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert_eq!(rt.state.ui.open, Some(Open::Menu("help")));
    assert!(has(&rt, "help-pop-menu"));
    let labels: Vec<_> = crate::ui::menus::help()
        .into_iter()
        .filter_map(|e| match e {
            crate::ui::menu::Entry::Item(i) => Some(i.label),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["Report a problem…", "What's new"]);
    // To the right of the button, its bottom edge level with the button's (side right, align end).
    let (menu, button) = (info(&rt, "help-pop-menu").rect, info(&rt, "rail-help").rect);
    assert!(menu[0] >= button[0] + button[2], "right of the button");
    assert!((menu[1] + menu[3] - (button[1] + button[3])).abs() < 2.0, "ends level with the button");
    assert!((menu[2] - 196.0).abs() < 1.0, "w-56 at Fleet's 14 px root: {menu:?}");
}

#[test]
fn report_a_problem_runs_the_command_and_closes_the_menu() {
    let mut rt = app();
    click(&mut rt, "rail-help");
    click(&mut rt, "help-pop-item0");
    assert_eq!(rt.state.ui.open, None);
    assert!(rt.state.ui.toast.as_ref().is_some_and(|t| t.message.contains("Report a problem")), "the stub until the dialog is built");
}

#[test]
fn whats_new_opens_settings_at_updates_and_about() {
    let mut rt = app();
    click(&mut rt, "rail-help");
    click(&mut rt, "help-pop-item2");
    assert_eq!(rt.state.ui.open, None);
    assert_eq!(rt.state.rail, crate::Rail::Settings);
    assert_eq!(rt.state.settings_section, Some("updates"));
}
