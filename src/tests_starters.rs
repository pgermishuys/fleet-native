//! Piece E of the New session page: the starters (`NewSessionComposer.test.ts` > starters, A21.30-35, and `StarterChips`'s
//! rows) and the strip's chips (A11: which chips there are, and what the machine and folder chips say).
use crate::fleet::new_session::feeds::{STARTERS_PREFERENCE_KEY, WORKFLOWS_PREFERENCE_KEY};
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::new_session::starters::{BUILT_IN, FIX_A_BUG_WORKFLOW_ID, WorkflowRequest, starter_selection};
use crate::fleet::new_session::strip::Chips;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn on_page() -> Runtime<crate::State> {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt
}

fn set_message(rt: &mut Runtime<crate::State>, words: &str) {
    // `update` marks the state changed, so the view is built again.
    rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").message = words.into());
    rt.frame(FRAME);
}

fn shown(rt: &Runtime<crate::State>) -> bool {
    rt.find("new-session-starters").is_some()
}

const ROCKET: &str = crate::mock::feeds::ROCKET;

#[test]
fn starters_shows_fleets_three_starters_on_the_empty_page_by_default() {
    let rt = on_page();
    assert!(shown(&rt));
    // The chips' text list is the three labels, then the icon-only menu button (B9.2).
    for starter in &BUILT_IN {
        assert!(rt.find(&format!("new-session-starter-{}", starter.id)).is_some(), "{} is drawn", starter.label);
    }
    let labels: Vec<_> = BUILT_IN.iter().map(|s| s.label).collect();
    assert_eq!(labels, ["Explore the code", "Plan a change", "Review my changes"]);
    assert!(rt.find("new-session-starters-menu").is_some(), "the options button is last");
    assert!(rt.find("new-session-starter-fix-a-bug").is_none(), "Workflows is off");
}

#[test]
fn starters_fills_the_box_and_selects_the_ellipsis_without_sending() {
    let mut rt = on_page();
    click(&mut rt, "new-session-starter-explore");
    let draft = rt.state.new_draft.draft().expect("a draft");
    assert_eq!(draft.message, "Explore this repository and explain how … works. Don't change any files.");
    assert!(!draft.is_starting, "nothing was sent");
    let (start, end) = rt.state.starters.pending_selection.expect("the … is selected");
    assert_eq!(&draft.message[start..end], "…");
    assert!(rt.state.starters.focus_box);
    assert!(rt.state.on_new_session_page(), "still on the page");
    // A starter with no … leaves the caret at the end.
    assert_eq!(starter_selection("No placeholder"), (14, 14));
}

#[test]
fn starters_fill_each_text_as_fleet_words_it() {
    let mut rt = on_page();
    for (id, text) in [
        ("plan", "I want to … Read the code and propose a plan. Don't change any files yet."),
        ("review", "Review my uncommitted changes. List bugs and risks, most serious first. Don't edit anything."),
    ] {
        rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").message.clear());
        rt.frame(FRAME);
        click(&mut rt, &format!("new-session-starter-{id}"));
        assert_eq!(rt.state.new_draft.draft().expect("a draft").message, text);
    }
    let review = rt.state.starters.pending_selection.expect("a selection");
    assert_eq!(review, (BUILT_IN[2].text.len(), BUILT_IN[2].text.len()), "no … in it: the caret goes to the end");
}

#[test]
fn starters_hides_while_the_box_has_text_and_comes_back_when_its_cleared() {
    let mut rt = on_page();
    set_message(&mut rt, "Fix the");
    assert!(!shown(&rt));
    assert!(rt.find("new-session-empty").is_some(), "the headline stays");
    set_message(&mut rt, "");
    assert!(shown(&rt));
}

#[test]
fn starters_hide_while_the_session_is_starting() {
    let mut rt = on_page();
    rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").is_starting = true);
    rt.frame(FRAME);
    assert!(!shown(&rt));
}

#[test]
fn starters_hide_starters_turns_them_off_for_good() {
    let mut rt = on_page();
    click(&mut rt, "new-session-starters-menu");
    rt.frame(FRAME);
    assert!(rt.find("new-session-starters-hide").is_some(), "the menu has Hide starters");
    click(&mut rt, "new-session-starters-hide");
    rt.frame(FRAME);
    assert_eq!(rt.state.pickers.prefs.get(STARTERS_PREFERENCE_KEY), Some("false"));
    assert!(!shown(&rt));
    assert!(rt.state.ui.open.is_none(), "the menu closed");
    // The box takes the request (piece L) and the keyboard with it.
    let focused = rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused.as_deref(), Some("new-session-message"), "focus goes back to the box");
    assert!(rt.find("new-session-empty").is_some());
}

#[test]
fn starters_stays_away_when_turned_off_and_until_preferences_are_known() {
    let mut rt = on_page();
    rt.update(|s, _| s.pickers.prefs.fetched = false);
    rt.frame(FRAME);
    assert!(!shown(&rt), "no flash before the preferences are known");
    rt.update(|s, _| s.pickers.prefs.fetched = true);
    rt.frame(FRAME);
    assert!(shown(&rt));
    rt.update(|s, _| {
        s.pickers.prefs.set(STARTERS_PREFERENCE_KEY, "false");
    });
    rt.frame(FRAME);
    assert!(!shown(&rt), "off in Settings");
    rt.update(|s, _| {
        s.pickers.prefs.set(STARTERS_PREFERENCE_KEY, "true");
    });
    rt.frame(FRAME);
    assert!(shown(&rt), "anything but \"false\" is on");
}

#[test]
fn starters_offers_fix_a_bug_only_with_workflows_on_and_opens_its_run_box_in_this_folder() {
    let mut rt = on_page();
    assert!(rt.find("new-session-starter-fix-a-bug").is_none());
    rt.update(|s, _| {
        s.pickers.prefs.set(WORKFLOWS_PREFERENCE_KEY, "true");
        s.set_folder(Folder::Repository(ROCKET.into()), true);
    });
    rt.frame(FRAME);
    assert!(rt.find("new-session-starter-fix-a-bug").is_some());
    click(&mut rt, "new-session-starter-fix-a-bug");
    assert_eq!(
        rt.state.starters.workflow_request,
        Some(WorkflowRequest { workflow: FIX_A_BUG_WORKFLOW_ID.into(), folder: Some(ROCKET.into()) })
    );
    assert!(rt.state.new_draft.draft().expect("a draft").message.is_empty(), "it fills nothing");
    assert!(!rt.state.new_draft.draft().expect("a draft").is_starting, "and starts nothing");
    // A plain folder sends no folder.
    rt.update(|s, _| {
        s.set_folder(Folder::Directory("/home/ada/notes".into()), true);
        s.starters.workflow_request = None;
        s.open_fix_a_bug();
    });
    assert_eq!(rt.state.starters.workflow_request.as_ref().map(|r| r.folder.clone()), Some(None));
    // Workflows run on this machine: another machine's folder can't go to its Run box.
    rt.update(|s, _| {
        s.new_draft.set_machine(Some("m-2"), "home");
    });
    rt.frame(FRAME);
    assert!(rt.find("new-session-starter-fix-a-bug").is_none());
}

#[test]
fn starters_say_what_a_click_does_and_the_menu_says_where_they_come_back() {
    let mut rt = on_page();
    click(&mut rt, "new-session-starters-menu");
    rt.frame(FRAME);
    assert!(rt.find("new-session-starters-pop").is_some());
    assert!(rt.find("new-session-starters-scrim").is_some(), "a press outside closes it");
    click(&mut rt, "new-session-starters-scrim");
    assert!(rt.state.ui.open.is_none());
}

// A11: which chips there are.
#[test]
fn chips_shows_the_workspace_chip_for_a_repository() {
    let chips = Chips::of(Some(&Folder::Repository(ROCKET.into())), &Workspace::Current);
    assert!(chips.machine && chips.folder && chips.workspace);
    assert!(!chips.base, "the base chip is for a new worktree only");
}

#[test]
fn chips_hides_the_workspace_chip_for_a_plain_folder_and_for_no_folder() {
    for folder in [Some(Folder::Directory("/home/ada/notes".into())), Some(Folder::None), None] {
        let chips = Chips::of(folder.as_ref(), &Workspace::New);
        assert!(!chips.workspace && !chips.base, "{folder:?}");
        assert!(chips.machine && chips.folder);
    }
}

#[test]
fn chips_shows_the_base_chip_for_a_new_worktree_only() {
    let repo = Folder::Repository(ROCKET.into());
    assert!(Chips::of(Some(&repo), &Workspace::New).base);
    assert!(!Chips::of(Some(&repo), &Workspace::Existing("/x".into())).base);
}

#[test]
fn chips_draws_the_machine_chip_and_the_folder_chip_on_a_desktop_with_one_machine() {
    let rt = on_page();
    assert!(rt.find("new-session-machine").is_some());
    assert!(rt.find("new-session-folder-chip").is_some());
    assert!(rt.find("new-session-strip").is_some());
}

#[test]
fn chips_the_folder_chip_asks_for_a_folder_until_one_is_chosen() {
    let mut rt = on_page();
    assert_eq!(rt.state.folder_label(None), "Choose a folder");
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    rt.frame(FRAME);
    let chosen = rt.state.new_draft.draft().and_then(|d| d.folder.clone());
    assert_eq!(rt.state.folder_label(chosen.as_ref()), "rocket", "a repository by its name");
    assert_eq!(rt.state.folder_label(Some(&Folder::None)), "No folder");
    assert_eq!(rt.state.folder_label(Some(&Folder::Directory("/home/ada/notes".into()))), "~/notes");
    assert_eq!(rt.state.folder_label(Some(&Folder::Repository("/srv/other".into()))), "other", "not scanned: its folder name");
}

#[test]
fn chips_set_folder_forgets_the_base_when_the_folder_changes() {
    let mut rt = on_page();
    rt.update(|s, _| {
        s.set_folder(Folder::Repository(ROCKET.into()), true);
        let d = s.new_draft.draft_mut().expect("a draft");
        d.base_branch = Some("main".into());
        d.fetch_origin = false;
        d.branch_name = "mine".into();
        // The same folder again keeps them.
        s.set_folder(Folder::Repository(ROCKET.into()), true);
        let d = s.new_draft.draft().expect("a draft");
        assert_eq!((d.base_branch.as_deref(), d.fetch_origin, d.branch_name.as_str()), (Some("main"), false, "mine"));
        s.set_folder(Folder::Directory("/home/ada/notes".into()), true);
        let d = s.new_draft.draft().expect("a draft");
        assert_eq!((d.base_branch.as_deref(), d.fetch_origin, d.branch_name.as_str()), (None, true, ""));
        assert_eq!(d.workspace, Workspace::New);
        assert!(d.has_chosen_folder);
    });
}

#[test]
fn chips_the_machine_chip_names_the_machine() {
    let rt = on_page();
    assert_eq!(rt.state.target_key(), "home");
    assert_eq!(rt.state.target_name(), "This machine", "until home says its name");
    assert!(!rt.state.target_is_down());
}
