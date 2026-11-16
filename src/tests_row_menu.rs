//! The session row's right-click menu (piece L of docs/parity/06-session-list.md): the rows, their conditions and hints, in
//! Fleet's order. `SessionItem.test.ts` cases under their names ("shows_context_actions_enabled_by_session_capabilities",
//! "says_what_fork_does", "has_no_fork_on_an_archived_session", the "moving it out of its parent" cases), the rest from the
//! source (3.90-3.109). The invented list answers each action at once, so no server is needed.
use crate::fleet::rows::Capabilities;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn menu_on(rt: &mut Runtime<crate::State>, p: usize, i: usize) {
    let (x, y) = rt.find(&format!("session-{p}-{i}")).expect("the row");
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(has(rt, "ctx-menu"), "the menu opened");
}

fn change(rt: &mut Runtime<crate::State>, edit: impl FnOnce(&mut crate::mock::Session)) {
    rt.update(|s, _| edit(&mut s.projects[0].sessions[1]));
    rt.frame(FRAME);
}

fn id_of(rt: &Runtime<crate::State>, p: usize, i: usize) -> String {
    rt.state.projects[p].sessions[i].id.clone()
}

/// The keys of the menu's rows, top to bottom.
fn order(rt: &Runtime<crate::State>) -> Vec<String> {
    let mut rows: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter_map(|e| e.key.filter(|k| k.starts_with("session-context-") || k.starts_with("session-repeat")).map(|k| (e.rect[1], k)))
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, k)| k.trim_start_matches("session-context-").to_string()).collect()
}

#[test]
fn shows_context_actions_enabled_by_session_capabilities() {
    let mut rt = app();
    change(&mut rt, |s| {
        s.row.capabilities = Capabilities { can_archive: Some(true), can_fork: Some(true), can_delete: Some(true), ..Default::default() }
    });
    menu_on(&mut rt, 0, 1);
    assert!(has(&rt, "session-context-archive") && !has(&rt, "session-context-restore"));
    assert!(has(&rt, "session-context-fork") && has(&rt, "session-context-delete"));
}

#[test]
fn hides_context_actions_disabled_by_session_capabilities() {
    let mut rt = app();
    change(&mut rt, |s| {
        s.row.capabilities = Capabilities { can_archive: Some(false), can_fork: Some(false), can_delete: Some(false), ..Default::default() }
    });
    menu_on(&mut rt, 0, 1);
    assert!(!has(&rt, "session-context-archive") && !has(&rt, "session-context-delete"));
}

#[test]
fn says_what_fork_does() {
    let mut rt = app();
    menu_on(&mut rt, 0, 1);
    let (x, y) = rt.find("session-context-fork").expect("Fork");
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.ui.hint.as_deref(), Some("A new session with a copy of this conversation."));
    assert!(!has(&rt, "session-context-fork-note"), "nothing extra on the row");
}

#[test]
fn shows_fork_off_with_the_reason_on_a_harness_that_cant_fork() {
    let mut rt = app();
    let reason = "Claude Code can't copy a conversation, so its sessions can't be forked.";
    change(&mut rt, |s| {
        s.row.capabilities = Capabilities { can_fork: Some(false), fork_disabled_reason: Some(reason.into()), ..Default::default() }
    });
    menu_on(&mut rt, 0, 1);
    let fork = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-context-fork")).expect("Fork stays");
    assert!(!fork.focusable, "it is off");
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some(reason)), "its reason is under it");
    // With no reason given, the default words.
    change(&mut rt, |s| s.row.capabilities = Capabilities { can_fork: Some(false), ..Default::default() });
    rt.update(|s, _| s.ui.open = None);
    rt.frame(FRAME);
    menu_on(&mut rt, 0, 1);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("This session can't be forked.")));
}

#[test]
fn offers_a_new_session_in_the_folder_even_where_fork_is_off() {
    let mut rt = app();
    change(&mut rt, |s| {
        s.row.capabilities = Capabilities { can_fork: Some(false), fork_disabled_reason: Some("No.".into()), ..Default::default() }
    });
    menu_on(&mut rt, 0, 1);
    let item = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-context-new-in-folder")).expect("offered");
    assert!(item.focusable, "and on");
}

#[test]
fn has_no_fork_on_an_archived_session() {
    let mut rt = app();
    change(&mut rt, |s| {
        s.row.archived = true;
        s.row.capabilities = Capabilities {
            can_fork: Some(false),
            fork_disabled_reason: Some("Archived sessions are read-only.".into()),
            ..Default::default()
        };
        s.listing.forked_from = Some("a".into());
        s.listing.spawn_kind = Some("fork".into());
    });
    click(&mut rt, "archived");
    rt.frame(FRAME);
    menu_on(&mut rt, 0, 1);
    assert!(!has(&rt, "session-context-move-out"), "an archived one can't be moved out");
    assert!(!has(&rt, "session-context-fork") && !has(&rt, "session-context-new-in-folder"));
    assert!(has(&rt, "session-context-restore") && !has(&rt, "session-context-archive"), "Restore, not Archive");
}

#[test]
fn the_menu_has_fleets_rows_in_fleets_order() {
    let mut rt = app();
    menu_on(&mut rt, 0, 1);
    assert_eq!(
        order(&rt),
        [
            "rename",
            "pin",
            "archive",
            "fork",
            "new-in-folder",
            "session-repeat-on-schedule",
            "open-in",
            "move",
            "copy-id",
            "clear-attachments",
            "delete"
        ]
        .map(String::from)
    );
    assert!(!has(&rt, "session-context-open"), "no Open row: a click on the row opens it");
}

#[test]
fn a_pinned_row_offers_unpin_and_the_moves_which_stop_at_the_ends() {
    let mut rt = app();
    let (a, b) = (id_of(&rt, 0, 0), id_of(&rt, 0, 1));
    rt.update(move |s, _| {
        s.pin_session(&a, None);
        s.pin_session(&b, None);
    });
    rt.frame(FRAME);
    let first = rt.state.projects[0].sessions.iter().position(|s| s.id == id_of(&rt, 0, 0)).unwrap();
    menu_on(&mut rt, 0, first);
    assert!(has(&rt, "session-context-pin-up") && has(&rt, "session-context-pin-down"));
    let up = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-context-pin-up")).unwrap();
    let down = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-context-pin-down")).unwrap();
    assert!(!up.focusable, "first in Pinned: nothing above");
    assert!(down.focusable);
    click(&mut rt, "session-context-pin");
    assert!(!rt.state.is_pinned(&id_of(&rt, 0, first)), "Unpin unpins");
}

#[test]
fn a_new_session_in_this_folder_is_made_and_opened() {
    let mut rt = app();
    let before = rt.state.projects.iter().map(|p| p.sessions.len()).sum::<usize>();
    menu_on(&mut rt, 0, 1);
    click(&mut rt, "session-context-new-in-folder");
    rt.frame(FRAME);
    assert!(!has(&rt, "ctx-menu"), "the menu closed first");
    assert_eq!(rt.state.projects.iter().map(|p| p.sessions.len()).sum::<usize>(), before + 1);
    let made = rt.state.session();
    assert!(made.listing.forked_from.is_none() && made.listing.spawn_kind.is_none(), "not a fork");
}

#[test]
fn fork_makes_a_fork_under_its_origin_and_opens_it() {
    let mut rt = app();
    let source = id_of(&rt, 0, 1);
    menu_on(&mut rt, 0, 1);
    click(&mut rt, "session-context-fork");
    rt.frame(FRAME);
    let made = rt.state.session();
    assert_eq!(made.listing.forked_from.as_deref(), Some(source.as_str()));
    assert_eq!(made.listing.spawn_kind.as_deref(), Some("fork"));
}

#[test]
fn copy_session_id_is_in_the_menu_and_the_footer_says_where_it_is() {
    let mut rt = app();
    menu_on(&mut rt, 0, 1);
    assert!(has(&rt, "session-context-copy-id"));
    assert!(has(&rt, "ctx-hint"), "the footer");
    // Nothing hovered: where the session is, with the missing parts dropped.
    let shown = rt.elements().into_iter().any(|e| e.key.as_deref() == Some("ctx-hint"));
    assert!(shown);
}

// ---- lineage ----------------------------------------------------------------------------------------------------------

#[test]
fn moving_it_out_of_its_parent_names_the_session_it_really_came_from() {
    let mut rt = app();
    let parent = id_of(&rt, 0, 0);
    let title = rt.state.projects[0].sessions[0].title.clone();
    change(&mut rt, move |s| {
        s.listing.forked_from = Some(parent);
        s.listing.spawn_kind = Some("fork".into());
    });
    menu_on(&mut rt, 0, 1);
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-context-move-out")).and_then(|e| e.label);
    assert_eq!(label.as_deref(), Some(format!("Move out of \"{title}\"").as_str()));
    assert!(!has(&rt, "session-context-move-back"));
    click(&mut rt, "session-context-move-out");
    assert!(rt.state.projects[0].sessions[1].listing.detached, "the list takes it at once");
}

#[test]
fn moving_it_out_is_there_for_a_session_an_agent_started_not_for_a_subagents_or_the_users() {
    let mut rt = app();
    change(&mut rt, |s| {
        s.listing.spawned_by = Some("elsewhere".into());
        s.listing.spawn_kind = Some("api".into());
    });
    menu_on(&mut rt, 0, 1);
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-context-move-out")).and_then(|e| e.label);
    assert_eq!(label.as_deref(), Some("Move out of its parent"), "its parent isn't in the list");
    rt.update(|s, _| s.ui.open = None);
    change(&mut rt, |s| {
        s.listing.spawned_by = None;
        s.listing.parent = Some("parent".into());
    });
    menu_on(&mut rt, 0, 1);
    assert!(!has(&rt, "session-context-move-out"), "a subagent's session");
    rt.update(|s, _| s.ui.open = None);
    change(&mut rt, |s| s.listing.parent = None);
    menu_on(&mut rt, 0, 1);
    assert!(!has(&rt, "session-context-move-out"), "one the user started");
}

#[test]
fn it_offers_to_move_a_session_thats_out_back_under_its_parent() {
    let mut rt = app();
    change(&mut rt, |s| {
        s.listing.forked_from = Some("elsewhere".into());
        s.listing.spawn_kind = Some("fork".into());
        s.listing.detached = true;
    });
    menu_on(&mut rt, 0, 1);
    assert!(!has(&rt, "session-context-move-out"));
    click(&mut rt, "session-context-move-back");
    assert!(!rt.state.projects[0].sessions[1].listing.detached);
}

// ---- Move to project --------------------------------------------------------------------------------------------------

#[test]
fn move_to_project_opens_its_submenu_with_a_row_per_project_and_moves_the_session() {
    let mut rt = app();
    menu_on(&mut rt, 0, 1);
    click(&mut rt, "session-context-move");
    rt.frame(FRAME);
    assert!(has(&rt, "session-context-move-sub-menu"), "the submenu is open");
    let target = rt
        .state
        .projects
        .iter()
        .find(|p| p.id != rt.state.projects[0].id && p.id != "ungrouped")
        .map(|p| (p.id.clone(), p.name.clone()))
        .expect("another project");
    let rows =
        rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("session-context-move-sub-item"))).count();
    assert!(rows >= 2, "{rows} rows");
    let id = id_of(&rt, 0, 1);
    rt.update(move |s, _| s.move_to_project(&id, Some(&target.0), &target.1));
    assert_ne!(rt.state.projects[0].sessions[1].listing.project_id.as_deref(), Some(rt.state.projects[0].id.as_str()));
}

#[test]
fn a_menu_row_on_another_machine_that_isnt_answering_offers_only_copy_session_id() {
    let mut rt = app();
    let machines = crate::mock::machines::Case::Several;
    rt.update(move |s, _| s.show_machines(machines, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt.update(|s, _| s.machines.others.get_mut("m-falcon").expect("a machine").error = Some("not answering".into()));
    let (x, y) = rt.find("machine-session-m-falcon-f-fence").expect("row");
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    assert_eq!(order(&rt), ["copy-id"]);
}
