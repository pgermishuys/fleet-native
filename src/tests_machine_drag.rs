//! Dragging a row of another machine's tree (`MachineSessionsGroup.test.ts` "gives its rows what the live machine's rows have:
//! pin, archive, drag, rename and picking", the drag part; `SessionsPanel.test.ts` "sends another machine's row actions to that
//! machine, and lists that machine again"). A drag never leaves its machine, and every call goes to it with its own token.
use crate::fleet::groups::{ProjectInfo, ProjectKind};
use crate::fleet::machines::parse_machine_list;
use crate::fleet::row_drag::Over;
use crate::tests::FRAME;
use crate::tests_archive_pins::{Fake, Rig};
use rune::Runtime;
use serde_json::json;

const MINI: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn project(id: &str, name: &str, position: i64) -> ProjectInfo {
    ProjectInfo { id: id.into(), name: name.into(), kind: ProjectKind::User, position }
}

/// Home at one fake, "mini" at another, with two sessions in two projects of its own.
fn rig() -> (Rig, Fake, Fake) {
    let mini = Fake::start(|_, _| (200, r#"{"pinOrder":3}"#.into()));
    let body = json!({ "machines": [{"id": MINI, "name": "mini", "baseUrl": mini.base, "token": "token-of-mini"}] }).to_string();
    let listed = body.clone();
    let home = Fake::start(move |_, path| if path == "/api/machines" { (200, listed.clone()) } else { (200, "{}".into()) });
    let mut r = Rig::home(&home);
    r.rt.update(move |s, _| {
        s.machines.apply_list(parse_machine_list(200, &body));
        s.toggle_machine(crate::fleet::settings::HOME_MACHINE_KEY);
        for (id, title, p) in [("mini-1", "Water the tomatoes", ("proj-a", "Garden")), ("mini-2", "Paint the fence", ("proj-b", "Fence"))] {
            let row = crate::mock::machines::listed(id, title, crate::mock::Status::Idle, "now", Some(p), |_| {});
            s.machines.upsert_session(MINI, row);
        }
        if let Some(m) = s.machines.others.get_mut(MINI) {
            m.projects = vec![project("proj-a", "Garden", 0), project("proj-b", "Fence", 1)];
            m.loaded_at = Some(1);
        }
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    (r, mini, home)
}

fn row(id: &str) -> String {
    format!("machine-session-{MINI}-{id}")
}

fn header(project: &str) -> String {
    format!("machine-project-{MINI}-{project}")
}

/// Press on `from`, move off it so the drag begins, then to `to` and stay (the mouse is still down).
fn grab(rt: &mut Runtime<crate::State>, from: &str, to: &str) -> (f32, f32) {
    let (x, y) = rt.find(from).unwrap_or_else(|| panic!("no element keyed {from:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 12.0, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let end = rt.find(to).unwrap_or_else(|| panic!("no element keyed {to:?}"));
    rt.pointer_move(end.0, end.1);
    rt.frame(FRAME);
    end
}

fn release(rt: &mut Runtime<crate::State>, at: (f32, f32)) {
    rt.pointer_up(at.0, at.1);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn dropping_a_row_on_another_project_of_its_machine_moves_it_there_on_that_machine() {
    let (mut r, mini, home) = rig();
    let to = grab(&mut r.rt, &row("mini-1"), &header("proj-b"));
    assert_eq!(r.state().drag_over(), Over::MProject(MINI.into(), "proj-b".into()));
    release(&mut r.rt, to);
    let moved = r.state().machines.others.get(MINI).unwrap().sessions.iter().find(|s| s.id == "mini-1").unwrap().listing.project_id.clone();
    assert_eq!(moved.as_deref(), Some("proj-b"), "the list takes it at once");
    r.wait("the move on its machine", |_| !mini.calls("/project").is_empty());
    let call = &mini.calls("/project")[0];
    assert_eq!((call.0.as_str(), call.1.as_str(), call.3.as_deref()), ("PATCH", "/api/sessions/mini-1/project", Some("token-of-mini")));
    assert_eq!(call.2, json!({ "projectId": "proj-b" }));
    assert!(home.calls("mini-").is_empty(), "nothing reaches home");
}

#[test]
fn a_drop_on_its_own_project_changes_nothing() {
    let (mut r, mini, _home) = rig();
    let to = grab(&mut r.rt, &row("mini-1"), &header("proj-a"));
    assert_eq!(r.state().drag_over(), Over::Nothing);
    release(&mut r.rt, to);
    assert!(mini.calls("/project").is_empty());
}

#[test]
fn a_drag_shows_its_machines_pinned_group_empty_and_no_archive_box_and_a_drop_pins_there() {
    let (mut r, mini, home) = rig();
    assert!(r.rt.find("pinned-empty").is_none(), "nothing shows at rest");
    grab(&mut r.rt, &row("mini-1"), &header("proj-b"));
    assert!(r.rt.find("pinned-empty").is_some(), "the machine's Pinned group shows while its row drags");
    assert!(r.rt.find("drop-archive").is_none(), "the Archive box is home's tree only");
    let pin = r.rt.find("pinned-empty").unwrap();
    r.rt.pointer_move(pin.0, pin.1);
    r.rt.frame(FRAME);
    assert_eq!(r.state().drag_over(), Over::MPinned(MINI.into(), None));
    release(&mut r.rt, pin);
    assert!(r.state().is_pinned("mini-1"));
    r.wait("the pin on its machine", |_| !mini.calls("/pin").is_empty());
    assert_eq!(mini.calls("/pin")[0].3.as_deref(), Some("token-of-mini"));
    assert!(r.rt.find("pinned-empty").is_none(), "and it is gone when the drag ends");
    assert!(home.calls("mini-").is_empty(), "nothing reaches home");
}

#[test]
fn a_pinned_row_dropped_on_its_own_project_unpins_there() {
    let (mut r, mini, _home) = rig();
    r.rt.update(|s, _| s.set_pin_order("mini-1", Some(1.0)));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    let to = grab(&mut r.rt, &row("mini-1"), &header("proj-a"));
    assert_eq!(r.state().drag_over(), Over::MProject(MINI.into(), "proj-a".into()));
    release(&mut r.rt, to);
    assert!(!r.state().is_pinned("mini-1"));
    r.wait("the unpin on its machine", |_| !mini.calls("/pin").is_empty());
    assert_eq!(mini.calls("/pin")[0].0, "DELETE");
}

#[test]
fn a_machine_that_isnt_answering_has_no_drag() {
    let (mut r, _mini, _home) = rig();
    r.rt.update(|s, _| {
        if let Some(m) = s.machines.others.get_mut(MINI) {
            m.error = Some("Can't reach mini.".into());
        }
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    grab(&mut r.rt, &row("mini-1"), &header("proj-b"));
    assert!(r.state().row_drag.is_none());
}

// ---- the project's menu and its + on another machine ---------------------------------------------------------------------------

fn right_click(rt: &mut Runtime<crate::State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn a_projects_menu_on_another_machine_moves_it_down_on_that_machine_and_offers_no_move_up_for_the_first() {
    let (mut r, mini, home) = rig();
    right_click(&mut r.rt, &header("proj-a"));
    assert!(r.rt.find("pctx-menu").is_some());
    // New session, separator, Rename, Move down, separator, Delete…: no Move up for the first project.
    assert!(r.rt.find("pctx-item3").is_some() && r.rt.find("pctx-item5").is_some() && r.rt.find("pctx-item6").is_none());
    crate::tests::click(&mut r.rt, "pctx-item3");
    r.wait("the new order on its machine", |_| mini.calls("/reorder").len() == 2);
    let calls = mini.calls("/reorder");
    assert_eq!(
        (calls[0].0.as_str(), calls[0].1.as_str(), calls[0].3.as_deref()),
        ("PATCH", "/api/projects/proj-b/reorder", Some("token-of-mini"))
    );
    assert_eq!(calls[0].2, json!({ "position": 1 }));
    assert_eq!(calls[1].1, "/api/projects/proj-a/reorder");
    assert!(home.calls("/api/projects").is_empty(), "nothing reaches home");
}

#[test]
fn deleting_a_project_of_another_machine_asks_with_its_count_and_sends_the_delete_to_that_machine() {
    let (mut r, mini, home) = rig();
    right_click(&mut r.rt, &header("proj-b"));
    // Move up is there for the second one: New session, separator, Rename, Move up, separator, Delete….
    crate::tests::click(&mut r.rt, "pctx-item5");
    let dialog = r.state().proj.delete.as_ref().expect("the dialog opens");
    assert_eq!((dialog.name.as_str(), dialog.count), ("Fence", 1));
    crate::tests::click(&mut r.rt, "delete-project-all");
    crate::tests::click(&mut r.rt, "delete-project-confirm");
    r.wait("the delete on its machine", |_| !mini.calls("/api/projects/proj-b").is_empty());
    let call = &mini.calls("/api/projects/proj-b")[0];
    assert_eq!(
        (call.0.as_str(), call.1.as_str(), call.3.as_deref()),
        ("DELETE", "/api/projects/proj-b?mode=delete_sessions", Some("token-of-mini"))
    );
    assert!(home.calls("/api/projects").is_empty(), "nothing reaches home");
}

#[test]
fn the_plus_on_another_machines_project_opens_the_new_session_page_on_that_machine_and_project() {
    let (mut r, _mini, _home) = rig();
    crate::tests::click(&mut r.rt, &format!("machine-project-new-{MINI}-proj-b"));
    r.rt.frame(FRAME);
    assert!(r.state().on_new_session_page());
    assert_eq!(r.state().new_draft.machine(), Some(MINI));
    assert_eq!(r.state().new_draft.draft().and_then(|d| d.project_id.clone()).as_deref(), Some("proj-b"));
}
