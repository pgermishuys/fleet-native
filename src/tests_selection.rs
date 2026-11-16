//! The Pinned group and bulk selection (piece J of docs/parity/06-session-list.md): `SessionsPanel.vue`'s Pinned group and
//! selection bar, `SessionItem.vue`'s pick (`handleSelect`), and the `session-selection` store's cases, which are in
//! `src/fleet/tests/selection.rs` under Fleet's names. The invented list takes every change at once, so no server is needed.
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, PhysicalInput, Runtime};

fn id(rt: &Runtime<crate::State>, p: usize, i: usize) -> String {
    rt.state.projects[p].sessions[i].id.clone()
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

/// A click with `mods` held, as a person Ctrl- or Shift-clicks.
fn click_with(rt: &mut Runtime<crate::State>, key: &str, mods: Mods) {
    rt.physical_input(PhysicalInput::Modifiers(mods));
    rt.frame(FRAME);
    click(rt, key);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(FRAME);
}

fn ctrl() -> Mods {
    Mods { ctrl: true, cmd: true, ..Mods::default() }
}

fn shift() -> Mods {
    Mods { shift: true, ..Mods::default() }
}

fn picked(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.selection.ids().to_vec()
}

fn pin(rt: &mut Runtime<crate::State>, at: &[(usize, usize, f64)]) {
    let at = at.to_vec();
    rt.update(move |s, _| {
        for (p, i, order) in at {
            s.projects[p].sessions[i].listing.pin_order = Some(order);
        }
    });
    rt.frame(FRAME);
}

// ---- picking rows ---------------------------------------------------------------------------------------------------------

#[test]
fn a_ctrl_click_picks_a_row_without_opening_it_and_the_bar_counts_it() {
    let mut rt = app();
    let open = rt.state.open_at();
    click_with(&mut rt, "session-0-1", ctrl());
    assert_eq!(picked(&rt), [id(&rt, 0, 1)]);
    assert_eq!(rt.state.open_at(), open, "a pick doesn't open the session");
    assert!(has(&rt, "session-selection-bar"));
    assert_eq!(rt.state.selection.count(), 1);
    // The checkbox takes the glyph's slot and the hover buttons give way.
    assert!(has(&rt, "session-check-0-1"));
    assert!(has(&rt, "session-check-0-0"), "every row of the list shows its checkbox while rows are picked");
    assert!(!has(&rt, "session-pin-0-0") && !has(&rt, "session-archive-0-0"));
    // The bar takes the footer's place.
    assert!(!has(&rt, "archived"));
}

#[test]
fn a_plain_click_while_rows_are_picked_adds_or_removes_one() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-0-2");
    assert_eq!(picked(&rt), [id(&rt, 0, 0), id(&rt, 0, 2)]);
    click(&mut rt, "session-0-0");
    assert_eq!(picked(&rt), [id(&rt, 0, 2)]);
    click(&mut rt, "session-0-2");
    assert!(!rt.state.selection.is_selecting());
    assert!(!has(&rt, "session-selection-bar") && has(&rt, "archived"));
    // Nothing is picked now, so a plain click opens a session again.
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open_at(), (0, 1));
}

#[test]
fn a_shift_click_takes_the_range_from_the_last_picked_row_in_list_order() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    click_with(&mut rt, "session-0-3", shift());
    assert_eq!(picked(&rt), (0..4).map(|i| id(&rt, 0, i)).collect::<Vec<_>>());
}

#[test]
fn a_shift_click_with_nothing_picked_starts_from_the_open_session() {
    let mut rt = app();
    click(&mut rt, "session-0-1");
    click_with(&mut rt, "session-0-3", shift());
    assert_eq!(picked(&rt), [id(&rt, 0, 1), id(&rt, 0, 2), id(&rt, 0, 3)]);
}

#[test]
fn escape_and_the_clear_button_let_the_rows_go() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(!rt.state.selection.is_selecting() && !has(&rt, "session-selection-bar"));
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-selection-clear");
    assert!(!rt.state.selection.is_selecting());
}

#[test]
fn a_picked_row_that_leaves_the_list_leaves_the_selection() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-0-1");
    // Folding the group takes its rows off the screen.
    click(&mut rt, "project-0");
    // The prune runs on the clock: a press changes the list after the frame's poll.
    for _ in 0..8 {
        rt.frame(0.05);
    }
    assert!(!rt.state.selection.is_selecting(), "both picked rows were in the folded group");
    click(&mut rt, "project-0");
    // An archive of one of two picked rows keeps the other.
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-0-1");
    let gone = id(&rt, 0, 0);
    rt.update(move |s, tx| s.archive_sessions(&[gone], None, tx));
    rt.frame(FRAME);
    assert_eq!(picked(&rt), [id(&rt, 0, 1)]);
}

#[test]
fn the_filter_lets_go_of_a_picked_row_it_hides() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-0-1");
    let keep = rt.state.projects[0].sessions[1].title.clone();
    rt.update(move |s, _| s.filter = rune::TextEdit::new(&keep));
    rt.frame(FRAME);
    assert_eq!(picked(&rt), [id(&rt, 0, 1)]);
}

#[test]
fn leaving_the_sessions_view_lets_the_rows_go() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    rt.update(|s, _| s.rail = crate::Rail::Settings);
    rt.frame(FRAME);
    assert!(!rt.state.selection.is_selecting());
}

// ---- the bar ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_bars_archive_hides_the_rows_at_once_offers_undo_and_clears_the_selection() {
    let mut rt = app();
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-0-1");
    let both = [id(&rt, 0, 0), id(&rt, 0, 1)];
    click(&mut rt, "session-selection-archive");
    assert!(!rt.state.selection.is_selecting());
    assert!(!has(&rt, "session-0-0") || id(&rt, 0, 0) != both[0] || rt.state.hides(&rt.state.projects[0].sessions[0]));
    let pending = rt.state.archive.pending.as_ref().expect("the archive waits for Undo");
    assert_eq!(pending.ids, both);
    assert_eq!(pending.message, "Archived 2 sessions");
    rt.state.undo_archive();
    rt.frame(FRAME);
    assert!(rt.state.archive.pending.is_none());
}

#[test]
fn the_bars_archive_of_one_row_names_it() {
    let mut rt = app();
    click_with(&mut rt, "session-0-2", ctrl());
    let title = rt.state.projects[0].sessions[2].title.clone();
    click(&mut rt, "session-selection-archive");
    assert_eq!(rt.state.archive.pending.as_ref().unwrap().message, format!("Archived \"{title}\""));
}

#[test]
fn the_bar_in_the_archived_view_offers_restore_for_each_picked_row() {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects[0].sessions[0].row.archived = true;
        s.projects[0].sessions[1].row.archived = true;
        s.store.retention = crate::fleet::store::Retention::All;
    });
    rt.frame(FRAME);
    click_with(&mut rt, "session-0-0", ctrl());
    click(&mut rt, "session-0-1");
    rt.update(|s, _| s.store.retention = crate::fleet::store::Retention::Archived);
    rt.frame(FRAME);
    assert!(has(&rt, "session-selection-restore") && !has(&rt, "session-selection-archive"));
    click(&mut rt, "session-selection-restore");
    assert!(!rt.state.selection.is_selecting());
    assert!(!rt.state.projects[0].sessions[0].row.archived && !rt.state.projects[0].sessions[1].row.archived);
}

// ---- the Pinned group --------------------------------------------------------------------------------------------------------

#[test]
fn the_pinned_group_is_above_the_projects_in_pin_order_and_its_rows_leave_their_project() {
    let mut rt = app();
    assert!(!has(&rt, "pinned-group"), "no pins, no group");
    let before = rt.state.home_layout().groups[0].rows.len();
    pin(&mut rt, &[(0, 2, 2.0), (1, 0, 1.0)]);
    assert!(has(&rt, "pinned-group") && has(&rt, "pinned-header"));
    let layout = rt.state.home_layout();
    assert_eq!(layout.pinned, [(1, 0), (0, 2)], "by their order, whichever project they are in");
    assert_eq!(layout.groups[0].rows.len(), before - 1, "a pinned row isn't in its project too");
    let pinned_top = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("pinned")).map(|e| e.rect[1]).unwrap();
    let project_top = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("project-0")).map(|e| e.rect[1]).unwrap();
    assert!(pinned_top < project_top);
    // The pinned session's row is drawn once, under its own key, in the group.
    assert!(has(&rt, "session-1-0") && has(&rt, "session-0-2"));
}

#[test]
fn the_pinned_group_folds_and_keeps_its_fold_under_fleets_key() {
    let mut rt = app();
    pin(&mut rt, &[(0, 0, 1.0)]);
    assert!(has(&rt, "session-0-0"));
    click(&mut rt, "pinned");
    assert!(!has(&rt, "session-0-0"));
    assert!(rt.state.sidebar.is_group_collapsed("project:home:pinned"));
    click(&mut rt, "pinned");
    assert!(has(&rt, "session-0-0"));
}

#[test]
fn the_filter_applies_to_the_pinned_group_row_by_row() {
    let mut rt = app();
    pin(&mut rt, &[(0, 0, 1.0), (0, 1, 2.0)]);
    let word = rt.state.projects[0].sessions[1].title.clone();
    rt.update(move |s, _| s.filter = rune::TextEdit::new(&word));
    rt.frame(FRAME);
    assert_eq!(rt.state.home_layout().pinned, [(0, 1)]);
    rt.update(|s, _| s.filter = rune::TextEdit::new("zzzz nothing matches"));
    rt.frame(FRAME);
    assert!(!has(&rt, "pinned-group"));
}

#[test]
fn a_session_the_archive_is_holding_back_leaves_the_pinned_group() {
    let mut rt = app();
    pin(&mut rt, &[(0, 0, 1.0), (0, 1, 2.0)]);
    let gone = id(&rt, 0, 0);
    rt.update(move |s, tx| s.archive_sessions(&[gone], None, tx));
    rt.frame(FRAME);
    assert_eq!(rt.state.home_layout().pinned, [(0, 1)]);
}

#[test]
fn the_heading_counts_pinned_rows_too() {
    let mut rt = app();
    let all = rt.state.home_layout().shown();
    pin(&mut rt, &[(0, 0, 1.0)]);
    assert_eq!(rt.state.home_layout().shown(), all, "pinning moves a row, it doesn't lose one");
}

#[test]
fn a_shift_range_runs_through_the_pinned_group_and_on_into_the_projects() {
    let mut rt = app();
    pin(&mut rt, &[(0, 2, 1.0)]);
    let order = crate::fleet::home_layout::visible_ids(&rt.state, &rt.state.home_layout());
    assert_eq!(order[0], id(&rt, 0, 2), "Pinned first");
    click_with(&mut rt, "session-0-2", ctrl());
    click_with(&mut rt, "session-0-1", shift());
    // From the pinned row to the project's second row: the pinned one, then the project's first two.
    assert_eq!(rt.state.selection.count(), 3);
    assert!(rt.state.selection.is_selected(&id(&rt, 0, 0)) && rt.state.selection.is_selected(&id(&rt, 0, 1)));
}

#[test]
fn pinned_rows_of_a_folded_pinned_group_are_not_in_the_visible_order() {
    let mut rt = app();
    pin(&mut rt, &[(0, 2, 1.0)]);
    click(&mut rt, "pinned");
    let order = crate::fleet::home_layout::visible_ids(&rt.state, &rt.state.home_layout());
    assert!(!order.contains(&id(&rt, 0, 2)));
}

// ---- another machine ---------------------------------------------------------------------------------------------------------

fn machines() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_machines(crate::mock::machines::Case::Several, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

#[test]
fn a_pick_on_another_machine_starts_again_there_and_a_pick_at_home_starts_again_at_home() {
    let mut rt = machines();
    click_with(&mut rt, "session-0-0", ctrl());
    assert_eq!(rt.state.selection.machine(), None);
    click_with(&mut rt, "machine-session-m-falcon-f-fence", ctrl());
    assert_eq!(picked(&rt), ["f-fence"], "the home pick was let go");
    assert_eq!(rt.state.selection.machine(), Some("m-falcon"));
    assert!(has(&rt, "session-check-m-falcon-f-tomato"), "the machine's rows show checkboxes, home's don't");
    assert!(!has(&rt, "session-check-0-1"));
    click_with(&mut rt, "machine-session-m-falcon-f-tomato", shift());
    assert!(rt.state.selection.is_selected("f-tomato"));
    click_with(&mut rt, "session-0-1", ctrl());
    assert_eq!(picked(&rt), [id(&rt, 0, 1)]);
    assert_eq!(rt.state.selection.machine(), None);
}

#[test]
fn a_machine_that_isnt_answering_has_rows_that_only_open() {
    let mut rt = machines();
    click_with(&mut rt, "machine-session-m-harbor-h-ledger", ctrl());
    assert!(!rt.state.selection.is_selecting(), "a read-only row is never picked");
}
