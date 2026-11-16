//! Dragging a session row (piece S of docs/parity/06-session-list.md): `ProjectGroup.test.ts` and `SessionItem.test.ts`
//! drag cases under their names where the behaviour is the same, and the targets' keys. The first test is the small Rune case:
//! a row dragged over keyed elements inside the scroll box finds the key under the pointer.
use crate::fleet::row_drag::Over;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

type Rt = Runtime<crate::State>;

/// Press on `from`, move off it (the drag begins and the list changes: the Pinned group and the Archive box appear), then to
/// where `to` is now and stay there (the mouse is still down).
fn grab(rt: &mut Rt, from: &str, to: &str) -> (f32, f32) {
    let (x, y) = rt.find(from).unwrap_or_else(|| panic!("no element keyed {from:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 12.0, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let end = at(rt, to);
    rt.pointer_move(end.0, end.1);
    rt.frame(FRAME);
    end
}

fn release(rt: &mut Rt, at: (f32, f32)) {
    rt.pointer_up(at.0, at.1);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn at(rt: &Rt, key: &str) -> (f32, f32) {
    rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn drag_to(rt: &mut Rt, from: &str, key: &str) {
    let to = grab(rt, from, key);
    release(rt, to);
}

fn project_of(rt: &Rt, id: &str) -> Option<String> {
    rt.state.projects.iter().flat_map(|p| &p.sessions).find(|s| s.id == id).and_then(|s| s.listing.project_id.clone())
}

#[test]
fn a_dragged_row_finds_the_key_under_the_pointer_inside_the_scroll_box() {
    let mut rt = app();
    grab(&mut rt, "session-0-0", "project-1");
    assert_eq!(rt.state.drag_over(), Over::Project(1), "the group under the pointer");
    let above = at(&rt, "new-session");
    rt.pointer_move(above.0, above.1);
    rt.frame(FRAME);
    assert_eq!(rt.state.drag_over(), Over::Nothing, "the New session button is no target");
    release(&mut rt, above);
    assert!(rt.state.row_drag.is_none(), "the release ends it");
}

#[test]
fn a_click_still_opens_the_row_and_does_not_start_a_drag() {
    let mut rt = app();
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open_at(), (0, 1));
    assert!(rt.state.row_drag.is_none());
}

#[test]
fn dropping_on_another_project_moves_the_session_there() {
    let mut rt = app();
    let id = rt.state.projects[0].sessions[1].id.clone();
    let target = rt.state.projects[1].id.clone();
    drag_to(&mut rt, "session-0-1", "project-1");
    assert_eq!(project_of(&rt, &id).as_deref(), Some(target.as_str()));
}

#[test]
fn dropping_on_its_own_project_changes_nothing() {
    let mut rt = app();
    let id = rt.state.projects[0].sessions[1].id.clone();
    let before = project_of(&rt, &id);
    drag_to(&mut rt, "session-0-1", "project-0");
    assert_eq!(project_of(&rt, &id), before);
    assert!(rt.state.ui.toast.is_none());
}

#[test]
fn escape_puts_the_drag_down_without_moving_anything() {
    let mut rt = app();
    let id = rt.state.projects[0].sessions[1].id.clone();
    let before = project_of(&rt, &id);
    let to = grab(&mut rt, "session-0-1", "project-1");
    rt.key(rune::KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() });
    rt.frame(FRAME);
    rt.pointer_up(to.0, to.1);
    rt.frame(FRAME);
    assert_eq!(project_of(&rt, &id), before);
    assert!(rt.state.row_drag.is_none());
}

#[test]
fn a_drag_shows_the_pinned_group_empty_and_the_archive_box_and_a_drop_pins() {
    let mut rt = app();
    assert!(rt.find("pinned-empty").is_none() && rt.find("drop-archive").is_none(), "nothing shows at rest");
    let id = rt.state.projects[0].sessions[1].id.clone();
    grab(&mut rt, "session-0-1", "session-0-0");
    assert!(rt.find("pinned-empty").is_some() && rt.find("drop-archive").is_some(), "the targets show while it drags");
    let pin = at(&rt, "pinned-empty");
    rt.pointer_move(pin.0, pin.1);
    rt.frame(FRAME);
    assert_eq!(rt.state.drag_over(), Over::Pinned(None));
    release(&mut rt, pin);
    assert!(rt.state.is_pinned(&id));
    assert!(rt.find("pinned-empty").is_none() && rt.find("drop-archive").is_none(), "and are gone when it ends");
}

#[test]
fn a_drop_in_pinned_goes_before_the_row_it_is_dropped_on() {
    let mut rt = app();
    let (a, b) = (rt.state.projects[0].sessions[0].id.clone(), rt.state.projects[0].sessions[1].id.clone());
    rt.update(|s, _| {
        s.pin_session(&a, None);
    });
    rt.frame(FRAME);
    let row = rt.state.home_layout().pinned[0];
    let key = format!("session-{}-{}", row.0, row.1);
    let to = grab(&mut rt, "session-0-1", &key);
    assert!(rt.state.pin_line_before(&a), "the line shows before the row under the pointer");
    release(&mut rt, to);
    let order = |id: &str| rt.state.projects[0].sessions.iter().find(|s| s.id == id).and_then(|s| s.listing.pin_order).unwrap();
    assert!(order(&b) < order(&a), "pinned before it");
}

#[test]
fn dropping_on_the_archive_box_archives_the_session_with_undo() {
    let mut rt = app();
    let id = rt.state.projects[0].sessions[1].id.clone();
    grab(&mut rt, "session-0-1", "session-0-0");
    let zone = at(&rt, "drop-archive");
    rt.pointer_move(zone.0, zone.1);
    rt.frame(FRAME);
    assert_eq!(rt.state.drag_over(), Over::Archive);
    release(&mut rt, zone);
    assert_eq!(rt.state.archive.pending.as_ref().map(|p| p.ids.clone()), Some(vec![id]));
}

#[test]
fn a_filter_turns_the_project_drops_off() {
    let mut rt = app();
    rt.update(|s, _| s.filter.text = "e".into());
    rt.frame(FRAME);
    rt.frame(FRAME);
    let from = rt.state.home_layout().groups[0].rows[0];
    let g = rt.state.home_layout().groups[0].p;
    let to = grab(&mut rt, &format!("session-{g}-{from}"), "project-1");
    assert_eq!(rt.state.drag_over(), Over::Nothing);
    release(&mut rt, to);
}

#[test]
fn a_pinned_row_dropped_on_its_own_project_unpins() {
    let mut rt = app();
    let id = rt.state.projects[0].sessions[1].id.clone();
    rt.update(|s, _| s.pin_session(&id, None));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.is_pinned(&id));
    let (p, i) = rt.state.row_of(&id).unwrap();
    let to = grab(&mut rt, &format!("session-{p}-{i}"), "project-0");
    assert_eq!(rt.state.drag_over(), Over::Project(0));
    release(&mut rt, to);
    assert!(!rt.state.is_pinned(&id));
}
