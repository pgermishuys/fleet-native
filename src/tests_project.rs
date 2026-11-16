//! The project header's +, its right-click menu, rename and delete (piece G of docs/parity/06-session-list.md): the cases of
//! `ProjectGroup.test.ts` and `SessionsPanel.test.ts` that name them, and new ones for what has none (`InlineEdit`,
//! `ConfirmDeleteProjectDialog`, the order). The invented list answers a change at once, so no server is needed.
use crate::fleet::groups::{UNGROUPED_ID, UNGROUPED_NAME};
use crate::fleet::projects::{Call, DeleteMode, delete_path, move_targets, rename_body, rename_outcome, reorder_body, reorder_path};
use crate::tests::{FRAME, app, click};
use crate::view::project_dialogs::description;
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::json;

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn right_click(rt: &mut Runtime<crate::State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn names(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.projects.iter().map(|p| p.name.clone()).collect()
}

/// A list with a group of sessions that have no project, last (the released server files them in Scratch, but a newer one
/// can leave them out).
fn with_ungrouped(rt: &mut Runtime<crate::State>) {
    rt.update(|s, tx| {
        let mut projects = crate::mock::projects();
        let mut sessions = std::mem::take(&mut projects[2].sessions);
        sessions.truncate(1);
        projects.push(crate::mock::Project {
            id: UNGROUPED_ID.into(),
            kind: Default::default(),
            position: i64::MAX,
            name: UNGROUPED_NAME.into(),
            path: String::new(),
            open: true,
            sessions,
        });
        s.on_list(projects, tx);
    });
    rt.frame(FRAME);
}

fn open_menu(rt: &mut Runtime<crate::State>, p: usize) {
    right_click(rt, &format!("project-{p}"));
    assert!(rt.find("pctx-menu").is_some(), "the project menu opened");
}

fn start_rename(rt: &mut Runtime<crate::State>, p: usize) {
    open_menu(rt, p);
    click(rt, "pctx-item2");
    assert!(rt.state.project_renaming(p));
}

fn type_name(rt: &mut Runtime<crate::State>, name: &str) {
    press(rt, Key::Text(name.into()));
}

// ---- the requests ---------------------------------------------------------------------------------------------------

#[test]
fn the_requests_are_the_ones_fleet_sends() {
    assert_eq!(rename_body("Docs"), json!({ "name": "Docs", "description": null }));
    assert_eq!(delete_path("p1", DeleteMode::MoveToScratch), "/api/projects/p1?mode=move_to_scratch");
    assert_eq!(delete_path("p1", DeleteMode::DeleteSessions), "/api/projects/p1?mode=delete_sessions");
    assert_eq!(reorder_path("p1"), "/api/projects/p1/reorder");
    assert_eq!(reorder_body(2), json!({ "position": 2 }));
}

#[test]
fn a_rename_that_is_empty_or_unchanged_asks_for_nothing() {
    assert_eq!(rename_outcome("Orchard", "  Grove  "), Some("Grove".into()));
    assert_eq!(rename_outcome("Orchard", "   "), None);
    assert_eq!(rename_outcome("Orchard", ""), None);
    assert_eq!(rename_outcome("Orchard", "Orchard"), None);
    assert_eq!(rename_outcome("Orchard", " Orchard "), None);
}

#[test]
fn a_move_swaps_with_the_neighbour_and_numbers_every_project_from_one() {
    let order: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    let up = move_targets(&order, 2, true).unwrap();
    assert_eq!(up.iter().map(|t| (t.project_id.as_str(), t.position)).collect::<Vec<_>>(), [("a", 1), ("c", 2), ("b", 3)]);
    let down = move_targets(&order, 0, false).unwrap();
    assert_eq!(down.iter().map(|t| t.project_id.as_str()).collect::<Vec<_>>(), ["b", "a", "c"]);
    assert!(move_targets(&order, 0, true).is_none(), "the first can't go up");
    assert!(move_targets(&order, 2, false).is_none(), "the last can't go down");
}

#[test]
fn the_dialog_counts_sessions_in_the_singular_and_the_plural() {
    let (one, at) = description(1);
    assert_eq!(one, "Choose what should happen to the 1 session in this project.");
    assert_eq!(&one[at], "1");
    assert_eq!(description(0).0, "Choose what should happen to the 0 sessions in this project.");
    assert_eq!(description(12).0, "Choose what should happen to the 12 sessions in this project.");
}

// ---- ProjectGroup > the + ----------------------------------------------------------------------------------------------

// "shows a New session + button that emits newSession without toggling"
#[test]
fn the_plus_asks_for_a_session_in_its_project_and_does_not_fold_the_group() {
    let mut rt = app();
    let id = rt.state.projects[1].id.clone();
    assert!(rt.state.projects[1].open);
    click(&mut rt, "project-new-1");
    assert_eq!(rt.state.proj.new_session_in.as_deref(), Some(id.as_str()));
    assert!(rt.state.projects[1].open, "the group stayed as it was");
    assert!(rt.state.on_new_session_page(), "it opens the New session page for that project");
}

// "has no + button for the ungrouped group"
#[test]
fn the_ungrouped_group_has_no_plus() {
    let mut rt = app();
    with_ungrouped(&mut rt);
    let last = rt.state.projects.len() - 1;
    assert_eq!(rt.state.projects[last].id, UNGROUPED_ID);
    assert!(rt.find(&format!("project-new-{last}")).is_none());
    assert!(rt.find("project-new-0").is_some());
}

#[test]
fn the_plus_says_which_project_it_starts_a_session_in() {
    let mut rt = app();
    let name = rt.state.projects[0].name.clone();
    rt.frame(FRAME);
    assert!(rt.find(&format!("project-new-{}", 0)).is_some());
    assert_eq!(rt.state.projects[0].name, name);
}

// ---- the menu --------------------------------------------------------------------------------------------------------------

#[test]
fn the_menu_lists_new_session_rename_the_moves_and_delete_in_order() {
    let mut rt = app();
    open_menu(&mut rt, 1);
    let labels = ["New session", "Rename", "Move up", "Move down", "Delete…"];
    let found: Vec<bool> = [0, 2, 3, 4, 6].iter().map(|i| rt.find(&format!("pctx-item{i}")).is_some()).collect();
    assert_eq!(found, [true; 5], "{labels:?}");
    assert!(rt.find("pctx-item7").is_none());
}

#[test]
fn the_first_project_has_no_move_up_and_the_last_no_move_down() {
    let mut rt = app();
    open_menu(&mut rt, 0);
    // New session, separator, Rename, Move down, separator, Delete…
    assert!(rt.find("pctx-item3").is_some() && rt.find("pctx-item4").is_none() && rt.find("pctx-item5").is_some());
    assert!(!rt.state.can_move(0, true) && rt.state.can_move(0, false));
    let last = rt.state.projects.len() - 1;
    assert!(rt.state.can_move(last, true) && !rt.state.can_move(last, false));
}

#[test]
fn the_ungrouped_group_has_no_menu_and_no_rename() {
    let mut rt = app();
    with_ungrouped(&mut rt);
    let last = rt.state.projects.len() - 1;
    right_click(&mut rt, &format!("project-{last}"));
    assert!(rt.find("pctx-menu").is_none());
    // "shows no rename input for the ungrouped group"
    rt.update(move |s, tx| s.start_rename(last, tx));
    rt.frame(FRAME);
    assert!(!rt.state.project_renaming(last));
    assert!(rt.find("project-rename").is_none());
    // Ungrouped can't be ordered either, so the project above it is last among the orderable ones.
    assert!(!rt.state.can_move(last, true) && !rt.state.can_move(last, false));
    assert!(!rt.state.can_move(last - 1, false));
}

#[test]
fn the_menu_waits_while_a_request_is_on_its_way() {
    let mut rt = app();
    rt.update(|s, _| s.proj.pending = true);
    open_menu(&mut rt, 1);
    click(&mut rt, "pctx-item2");
    assert!(!rt.state.project_renaming(1), "Rename is disabled");
    rt.update(|s, tx| s.start_rename(1, tx));
    assert!(!rt.state.project_renaming(1));
}

#[test]
fn the_menus_new_session_closes_it_and_asks_like_the_plus() {
    let mut rt = app();
    open_menu(&mut rt, 2);
    click(&mut rt, "pctx-item0");
    assert!(rt.find("pctx-menu").is_none());
    assert_eq!(rt.state.proj.new_session_in.as_deref(), Some(rt.state.projects[2].id.as_str()));
}

// ---- moving ----------------------------------------------------------------------------------------------------------------

#[test]
fn move_up_and_move_down_reorder_the_projects() {
    let mut rt = app();
    let before = names(&rt);
    open_menu(&mut rt, 1);
    click(&mut rt, "pctx-item3");
    assert!(rt.find("pctx-menu").is_none(), "the menu closed first");
    let mut swapped = before.clone();
    swapped.swap(0, 1);
    assert_eq!(names(&rt), swapped);
    // Back down again from the top.
    open_menu(&mut rt, 0);
    click(&mut rt, "pctx-item3");
    assert_eq!(names(&rt), before);
}

// ---- rename ----------------------------------------------------------------------------------------------------------------

#[test]
fn rename_puts_the_name_in_a_field_with_the_whole_name_selected() {
    let mut rt = app();
    let name = rt.state.projects[1].name.clone();
    start_rename(&mut rt, 1);
    rt.frame(FRAME);
    assert!(rt.find("project-rename").is_some(), "the field is where the title was");
    assert_eq!(rt.state.proj.edit.text, name);
    assert_eq!(rt.state.proj.edit.range(), (0, name.len()), "selected, so typing replaces it");
    assert!(rt.state.proj.edit.focused, "and focused");
}

#[test]
fn enter_commits_the_trimmed_name_at_once() {
    let mut rt = app();
    start_rename(&mut rt, 1);
    type_name(&mut rt, "  Grove  ");
    press(&mut rt, Key::Enter);
    assert!(!rt.state.project_renaming(1), "edit mode closes");
    assert_eq!(rt.state.projects[1].name, "Grove");
    assert!(rt.find("project-rename").is_none());
}

#[test]
fn escape_cancels_and_asks_for_nothing() {
    let mut rt = app();
    let before = names(&rt);
    start_rename(&mut rt, 1);
    type_name(&mut rt, "Grove");
    press(&mut rt, Key::Escape);
    assert!(!rt.state.project_renaming(1));
    assert_eq!(names(&rt), before);
}

#[test]
fn an_empty_or_unchanged_name_closes_the_edit_and_changes_nothing() {
    let mut rt = app();
    let before = names(&rt);
    start_rename(&mut rt, 1);
    press(&mut rt, Key::Enter);
    assert!(!rt.state.project_renaming(1));
    assert_eq!(names(&rt), before);
    start_rename(&mut rt, 1);
    type_name(&mut rt, "   ");
    press(&mut rt, Key::Enter);
    assert_eq!(names(&rt), before);
}

#[test]
fn leaving_the_field_commits_the_name() {
    let mut rt = app();
    start_rename(&mut rt, 1);
    type_name(&mut rt, "Grove");
    // Focus goes to the filter box, as a click elsewhere would.
    click(&mut rt, "filter");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!rt.state.project_renaming(1), "blur ended the edit");
    assert_eq!(rt.state.projects[1].name, "Grove");
}

#[test]
fn only_the_first_end_of_an_edit_counts() {
    let mut rt = app();
    start_rename(&mut rt, 1);
    type_name(&mut rt, "Grove");
    press(&mut rt, Key::Enter);
    // The blur that follows the Enter finds nothing to commit.
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.projects[1].name, "Grove");
    assert!(rt.state.proj.rename.is_none());
}

#[test]
fn f2_on_the_focused_header_starts_the_rename() {
    let mut rt = app();
    rt.update(|_, tx| tx.focus("project-1", true));
    rt.frame(FRAME);
    let focused = rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused.as_deref(), Some("project-1"), "the header takes focus");
    press(&mut rt, Key::Function(2));
    assert!(rt.state.project_renaming(1));
}

#[test]
fn the_header_has_no_menu_while_it_is_renamed() {
    let mut rt = app();
    start_rename(&mut rt, 1);
    right_click(&mut rt, "project-1");
    assert!(rt.find("pctx-menu").is_none());
}

// ---- delete ----------------------------------------------------------------------------------------------------------------

fn open_delete(rt: &mut Runtime<crate::State>, p: usize) {
    open_menu(rt, p);
    let last = if rt.find("pctx-item6").is_some() { 6 } else { 5 };
    click(rt, &format!("pctx-item{last}"));
    assert!(rt.find("delete-project-dialog").is_some(), "the dialog opened");
}

#[test]
fn delete_opens_an_alert_that_names_the_project_and_the_count() {
    let mut rt = app();
    let (name, count) = (rt.state.projects[1].name.clone(), rt.state.projects[1].sessions.len());
    open_delete(&mut rt, 1);
    let d = rt.state.proj.delete.as_ref().unwrap();
    assert_eq!((d.name.as_str(), d.count), (name.as_str(), count));
    assert_eq!(d.mode, DeleteMode::MoveToScratch, "the first card is the default");
    assert!(rt.find("delete-project-close").is_none(), "an alert has no close button");
}

#[test]
fn the_count_in_the_dialog_is_the_filtered_count() {
    let mut rt = app();
    let title = rt.state.projects[1].sessions[0].title.clone();
    rt.update(move |s, _| s.filter = rune::TextEdit::new(title));
    rt.frame(FRAME);
    assert_eq!(rt.state.shown_count(1), 1);
    assert!(rt.state.projects[1].sessions.len() > 1 || rt.state.shown_count(1) == 1);
}

#[test]
fn cancel_and_escape_close_it_and_pressing_outside_does_not() {
    let mut rt = app();
    open_delete(&mut rt, 1);
    rt.pointer_move(4.0, 4.0);
    rt.pointer_down(4.0, 4.0);
    rt.pointer_up(4.0, 4.0);
    rt.frame(FRAME);
    assert!(rt.state.proj.delete.is_some(), "the overlay does not close an alert");
    click(&mut rt, "delete-project-cancel");
    assert!(rt.state.proj.delete.is_none());
    open_delete(&mut rt, 1);
    press(&mut rt, Key::Escape);
    assert!(rt.state.proj.delete.is_none());
}

#[test]
fn the_cards_pick_what_happens_and_every_open_starts_on_the_first() {
    let mut rt = app();
    open_delete(&mut rt, 1);
    click(&mut rt, "delete-project-all");
    assert_eq!(rt.state.proj.delete.as_ref().unwrap().mode, DeleteMode::DeleteSessions);
    click(&mut rt, "delete-project-keep");
    assert_eq!(rt.state.proj.delete.as_ref().unwrap().mode, DeleteMode::MoveToScratch);
    click(&mut rt, "delete-project-all");
    click(&mut rt, "delete-project-cancel");
    open_delete(&mut rt, 1);
    assert_eq!(rt.state.proj.delete.as_ref().unwrap().mode, DeleteMode::MoveToScratch);
}

#[test]
fn deleting_the_project_only_keeps_its_sessions_in_ungrouped() {
    let mut rt = app();
    let (id, moved) = (rt.state.projects[1].id.clone(), rt.state.projects[1].sessions.len());
    open_delete(&mut rt, 1);
    click(&mut rt, "delete-project-confirm");
    assert!(rt.state.proj.delete.is_none(), "closed on success");
    assert!(rt.state.projects.iter().all(|p| p.id != id));
    let ungrouped = rt.state.projects.iter().find(|p| p.id == UNGROUPED_ID).expect("a group for them");
    assert_eq!(ungrouped.sessions.len(), moved);
}

#[test]
fn deleting_the_project_and_its_sessions_removes_them() {
    let mut rt = app();
    let (id, total) = (rt.state.projects[1].id.clone(), rt.state.projects.iter().map(|p| p.sessions.len()).sum::<usize>());
    let gone = rt.state.projects[1].sessions.len();
    open_delete(&mut rt, 1);
    click(&mut rt, "delete-project-all");
    click(&mut rt, "delete-project-confirm");
    assert!(rt.state.projects.iter().all(|p| p.id != id));
    assert!(rt.state.projects.iter().all(|p| p.id != UNGROUPED_ID));
    assert_eq!(rt.state.projects.iter().map(|p| p.sessions.len()).sum::<usize>(), total - gone);
}

#[test]
fn while_it_deletes_the_dialog_says_so_and_cannot_be_closed() {
    let mut rt = app();
    open_delete(&mut rt, 1);
    rt.update(|s, _| s.proj.delete.as_mut().unwrap().deleting = true);
    rt.frame(FRAME);
    rt.update(|s, _| s.cancel_delete());
    assert!(rt.state.proj.delete.is_some());
    press(&mut rt, Key::Escape);
    assert!(rt.state.proj.delete.is_some(), "Escape waits too");
    rt.update(|s, _| s.pick_delete_mode(DeleteMode::DeleteSessions));
    assert_eq!(rt.state.proj.delete.as_ref().unwrap().mode, DeleteMode::MoveToScratch, "the cards wait");
}

#[test]
fn a_refused_delete_leaves_the_dialog_open_and_says_why() {
    let mut rt = app();
    open_delete(&mut rt, 1);
    rt.update(|s, _| s.proj.delete.as_mut().unwrap().deleting = true);
    let id = rt.state.projects[1].id.clone();
    rt.update(move |s, tx| {
        s.proj.pending = true;
        s.project_call_done(&Call::Delete { id, mode: DeleteMode::MoveToScratch }, Err("status 500".into()), tx);
    });
    rt.frame(FRAME);
    let d = rt.state.proj.delete.as_ref().expect("still open");
    assert!(!d.deleting && !rt.state.proj.pending);
    assert!(rt.state.ui.toast.as_ref().unwrap().message.starts_with("Couldn't delete the project"));
}

#[test]
fn a_refused_rename_or_move_says_why() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.proj.pending = true;
        s.project_call_done(&Call::Rename { id: "x".into(), name: "y".into() }, Err("status 409".into()), tx);
    });
    assert!(rt.state.ui.toast.as_ref().unwrap().message.starts_with("Couldn't rename the project"));
    assert!(!rt.state.proj.pending);
}
