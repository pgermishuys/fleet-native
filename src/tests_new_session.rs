//! The New session page's route and every way in (piece D of docs/parity/07-new-session-and-first-run.md): the cases of
//! `NewSessionComposer.test.ts` that name the page's own behaviour, and one per entry point. The message box is piece L's, so
//! text is put in the draft directly.
use crate::fleet::new_session::page::{created_session_id, new_in_folder_path};
use crate::fleet::store::Route;
use crate::tests::{FRAME, app, click};
use crate::ui::commands::Run;
use rune::{Key, KeyEvent, Mods, Runtime, TextEdit};
use serde_json::json;

fn on_page(rt: &Runtime<crate::State>, project: Option<&str>) {
    assert_eq!(rt.state.route, Route::NewSession { project_id: project.map(str::to_string) });
    assert!(!rt.state.has_session(), "the open session is cleared on entry (A2.2)");
    assert!(rt.find("new-session-form").is_some(), "the page is drawn");
}

fn type_in_draft(rt: &mut Runtime<crate::State>, words: &str) {
    rt.state.new_draft.draft_mut().expect("the page has a draft").message = words.into();
    rt.frame(FRAME);
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn command(rt: &mut Runtime<crate::State>, c: char, mods: Mods) {
    rt.key(KeyEvent { key: Key::Command(c), mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn palette(rt: &mut Runtime<crate::State>, query: &str) {
    rt.update(|s, tx| {
        crate::ui::palette::open(s, tx);
        s.ui.palette.query = TextEdit::new(query);
    });
    rt.frame(FRAME);
    press(rt, Key::Enter);
}

#[test]
fn the_lists_button_opens_the_page() {
    let mut rt = app();
    assert!(rt.state.has_session());
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    on_page(&rt, None);
}

#[test]
fn the_shortcut_opens_the_page_and_shift_does_not() {
    let mut rt = app();
    command(&mut rt, 'n', Mods { shift: true, ..Mods::default() });
    assert!(rt.state.has_session(), "Shift+N is not the shortcut");
    command(&mut rt, 'n', Mods::default());
    on_page(&rt, None);
}

#[test]
fn the_palette_row_opens_the_page_and_has_no_waiting_tail() {
    let mut rt = app();
    let row = crate::ui::commands::all(&rt.state).into_iter().find(|c| c.id == "new-session").expect("the row");
    assert!(matches!(row.run, Run::Now(_)), "it runs, it doesn't wait on component 7");
    assert_eq!(row.description, "Create and open a new session.");
    palette(&mut rt, "new session");
    on_page(&rt, None);
    assert!(rt.state.ui.toast.is_none(), "no 'isn't in the native app yet' toast");
}

#[test]
fn the_page_replaces_the_content_panel_and_adds_no_gutter() {
    let mut rt = app();
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    assert!(rt.find("content").is_some());
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    assert!(rt.find("content").is_none() && rt.find("content-rail").is_none(), "no content panel on the page (A1.6)");
    assert!(rt.find("sidebar-gutter").is_some(), "the list's gutter stays; there is no second one");
    assert!(rt.find("session-header").is_none(), "no session header");
}

#[test]
fn the_page_has_its_header_and_the_empty_block() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    for key in ["new-session-header", "new-session-title", "new-session-pill", "new-session-stage", "new-session-empty"] {
        assert!(rt.find(key).is_some(), "{key} is drawn");
    }
}

#[test]
fn a_projects_plus_opens_the_page_for_that_project() {
    let mut rt = app();
    let id = rt.state.projects[1].id.clone();
    click(&mut rt, "project-new-1");
    rt.frame(FRAME);
    on_page(&rt, Some(&id));
    assert_eq!(rt.state.new_draft.draft().and_then(|d| d.project_id.clone()).as_deref(), Some(id.as_str()));
}

// "a project's + moves a kept draft to that project"
#[test]
fn a_projects_plus_moves_a_kept_draft_to_that_project() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    type_in_draft(&mut rt, "Half a thought");
    click(&mut rt, "session-0-1");
    assert!(rt.state.new_draft.draft().is_some(), "a draft with a message stays when you leave");
    let id = rt.state.projects[1].id.clone();
    click(&mut rt, "project-new-1");
    rt.frame(FRAME);
    on_page(&rt, Some(&id));
    let draft = rt.state.new_draft.draft().expect("the draft");
    assert_eq!(draft.message, "Half a thought");
    assert_eq!(rt.state.new_draft.row().and_then(|r| r.project_id).as_deref(), Some(id.as_str()));
}

#[test]
fn a_kept_draft_keeps_its_own_project_when_none_is_asked_for() {
    let mut rt = app();
    let id = rt.state.projects[1].id.clone();
    click(&mut rt, "project-new-1");
    rt.frame(FRAME);
    type_in_draft(&mut rt, "Half a thought");
    click(&mut rt, "session-0-1");
    rt.update(|s, _| s.open_new_session_page(None));
    rt.frame(FRAME);
    let draft = rt.state.new_draft.draft().expect("the draft");
    assert_eq!(draft.project_id.as_deref(), Some(id.as_str()), "a restored draft keeps its own project (A3.5)");
    assert_eq!(draft.message, "Half a thought");
}

// "Esc never leaves the page"
#[test]
fn esc_never_leaves_the_page() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    type_in_draft(&mut rt, "Half a thought");
    press(&mut rt, Key::Escape);
    on_page(&rt, None);
    assert_eq!(rt.state.new_draft.draft().expect("draft").message, "Half a thought");
}

#[test]
fn opening_a_session_leaves_the_page_and_drops_an_empty_draft() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    assert!(rt.find("new-session-draft-row").is_some(), "the list shows the draft's row while the page is open");
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open_at(), (0, 1));
    assert!(rt.state.new_draft.draft().is_none(), "a draft with nothing in it goes (A3.4)");
    assert!(rt.find("new-session-draft-row").is_none());
    assert!(!rt.state.new_draft.is_page_open());
}

#[test]
fn the_draft_row_follows_typing_and_opens_the_page_again() {
    let mut rt = app();
    let id = rt.state.projects[1].id.clone();
    click(&mut rt, "project-new-1");
    rt.frame(FRAME);
    assert_eq!(rt.state.new_draft.row().expect("row").title, "");
    type_in_draft(&mut rt, "Fix the login redirect\nand the logout one");
    assert_eq!(rt.state.new_draft.row().expect("row").title, "Fix the login redirect");
    click(&mut rt, "session-0-1");
    assert!(rt.find("new-session-draft-row").is_some(), "a draft with a message keeps its row");
    assert!(!rt.state.on_new_session_page());
    click(&mut rt, "new-session-draft-row");
    rt.frame(FRAME);
    on_page(&rt, Some(&id));
}

#[test]
fn the_draft_row_lands_in_its_project_group_before_the_sessions() {
    let mut rt = app();
    click(&mut rt, "project-new-1");
    rt.frame(FRAME);
    let (_, row_y) = rt.find("new-session-draft-row").expect("draft row");
    let (_, header_y) = rt.find("project-1").expect("project header");
    let (_, first_y) = rt.find("session-1-0").expect("first session");
    assert!(header_y < row_y && row_y < first_y, "header {header_y}, draft {row_y}, first session {first_y}");
}

#[test]
fn a_list_refresh_leaves_the_page_where_it_is() {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt.update(|s, tx| s.on_list(crate::mock::projects(), tx));
    rt.frame(FRAME);
    on_page(&rt, None);
}

#[test]
fn an_empty_list_opens_the_page_instead_of_a_session() {
    let mut rt = app();
    rt.update(|s, tx| s.on_list(Vec::new(), tx));
    rt.frame(FRAME);
    on_page(&rt, None);
    // The same when groups come back with nothing in them.
    let mut rt = app();
    rt.update(|s, tx| {
        let mut projects = crate::mock::projects();
        projects.iter_mut().for_each(|p| p.sessions.clear());
        s.on_list(projects, tx);
    });
    rt.frame(FRAME);
    on_page(&rt, None);
}

#[test]
fn a_list_with_sessions_goes_home_with_nothing_selected() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.route = Route::None;
        s.on_list(crate::mock::projects(), tx);
    });
    assert_eq!(rt.state.route, Route::Home);
    assert!(!rt.state.has_session());
}

#[test]
fn new_session_in_this_folder_is_for_the_open_session() {
    let mut rt = app();
    let row = || crate::ui::commands::all(&rt.state).into_iter().find(|c| c.id == "new-session-in-folder").expect("the row");
    let row_open = row();
    assert!(!row_open.disabled);
    assert!(matches!(row_open.run, Run::Now(_)), "it runs, it doesn't wait on component 7");
    assert_eq!(row_open.description, "An empty session in the active session's folder, on the same harness.");
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    let row_none = crate::ui::commands::all(&rt.state).into_iter().find(|c| c.id == "new-session-in-folder").expect("the row");
    assert!(row_none.disabled, "no session is open on the page");
    assert_eq!(row_none.description, "No active session selected.");
}

#[test]
fn new_session_in_this_folder_asks_the_server_and_opens_what_it_made() {
    assert_eq!(new_in_folder_path("ses_1"), "/api/sessions/ses_1/new-in-folder");
    assert_eq!(created_session_id(&json!({ "instanceId": "i", "session": { "id": "ses_2" } })).as_deref(), Some("ses_2"));
    assert_eq!(created_session_id(&json!({ "session": {} })), None);
    assert_eq!(created_session_id(&json!({ "session": { "id": "" } })), None);

    let mut rt = app();
    rt.update(|s, tx| {
        s.set_session_id((0, 0), "ses_2");
        s.new_draft.open_after_list = Some("ses_2".into());
        s.route = Route::None;
        s.open_made_session(tx);
    });
    assert_eq!(rt.state.open_at(), (0, 0));
    assert_eq!(rt.state.new_draft.open_after_list, None);
    // Not in the list yet: it waits for the list.
    rt.update(|s, tx| {
        s.new_draft.open_after_list = Some("later".into());
        s.open_made_session(tx);
    });
    assert_eq!(rt.state.new_draft.open_after_list.as_deref(), Some("later"));
}

#[test]
fn puts_the_cursor_in_the_message_box() {
    // A21.1 / A2.15: the box has the keyboard as the page opens.
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt.frame(FRAME);
    let focused = rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused.as_deref(), Some("new-session-message"));
}
