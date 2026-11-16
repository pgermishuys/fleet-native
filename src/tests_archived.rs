//! The Archived view (piece K of docs/parity/06-session-list.md): `SessionsPanel.vue`'s `showArchived`, `isArchivedView`, its
//! header and footer, and the list read with `retentionStatus=archived`. Fleet has no test of its own for the view, so these
//! take the rows (1.77-1.81, 1.53, 1.70) by name. The invented list takes a change at once; the request test uses a fake server.
use crate::fleet::store::Retention;
use crate::tests::{FRAME, app, click};
use crate::tests_archive_pins::{Fake, Rig};
use rune::Runtime;
use serde_json::json;

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn archive_rows(rt: &mut Runtime<crate::State>, rows: &[(usize, usize)]) {
    let rows = rows.to_vec();
    rt.update(move |s, _| {
        for (p, i) in rows {
            s.projects[p].sessions[i].row.archived = true;
        }
    });
    rt.frame(FRAME);
}

fn open_view(rt: &mut Runtime<crate::State>) {
    click(rt, "archived");
    rt.frame(FRAME);
}

#[test]
fn the_footer_link_opens_the_archived_view_with_its_own_header_and_no_footer() {
    let mut rt = app();
    assert!(has(&rt, "new-session") && has(&rt, "add-folder") && has(&rt, "archived") && !has(&rt, "archived-back"));
    open_view(&mut rt);
    assert_eq!(rt.state.store.retention, Retention::Archived);
    assert!(has(&rt, "archived-back"), "the header names the view");
    assert!(!has(&rt, "new-session") && !has(&rt, "add-folder"), "it takes the place of New session and New project");
    assert!(!has(&rt, "archived"), "the Archived link is the active view's footer");
    assert!(has(&rt, "filter"), "the filter stays");
}

#[test]
fn the_header_goes_back_to_the_active_list() {
    let mut rt = app();
    archive_rows(&mut rt, &[(0, 1)]);
    open_view(&mut rt);
    click(&mut rt, "archived-back");
    rt.frame(FRAME);
    assert_eq!(rt.state.store.retention, Retention::Active);
    assert!(has(&rt, "new-session") && has(&rt, "archived") && !has(&rt, "archived-back"));
    assert!(has(&rt, "session-0-0") && !has(&rt, "session-0-1"), "the archived row is out of the active list again");
}

#[test]
fn the_view_shows_only_archived_sessions_and_drops_projects_without_any() {
    let mut rt = app();
    let projects = rt.state.projects.len();
    assert!(projects > 1);
    archive_rows(&mut rt, &[(0, 1)]);
    open_view(&mut rt);
    assert!(has(&rt, "session-0-1"));
    assert!(!has(&rt, "session-0-0"), "an active row isn't listed");
    assert_eq!(rt.state.home_layout().groups.len(), 1, "only the project that holds an archived session shows");
}

#[test]
fn the_view_has_no_pinned_group_and_a_pin_doesnt_lift_a_row_out_of_its_project() {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects[0].sessions[1].listing.pin_order = Some(1.0);
        s.projects[0].sessions[1].row.archived = true;
    });
    rt.frame(FRAME);
    open_view(&mut rt);
    assert!(!has(&rt, "pinned-group") && !has(&rt, "pinned"));
    assert!(has(&rt, "session-0-1"));
    assert!(rt.state.home_layout().pinned.is_empty());
}

#[test]
fn the_view_has_no_machine_headings_and_no_add_a_machine_row() {
    let mut rt = app();
    rt.update(|s, _| s.show_machines(crate::mock::machines::Case::Several, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    assert!(has(&rt, "machine-home") && has(&rt, "sessions-add-machine"));
    open_view(&mut rt);
    assert!(!has(&rt, "machine-home") && !has(&rt, "sessions-add-machine") && !has(&rt, "machine-m-falcon"));
    assert!(rt.state.home_tree_open(), "with no heading to fold it, the live tree is always open");
}

#[test]
fn switching_to_or_from_the_view_lets_the_picked_rows_go() {
    let mut rt = app();
    archive_rows(&mut rt, &[(0, 1)]);
    rt.update(|s, _| s.selection.toggle("a-row", None));
    assert!(rt.state.selection.is_selecting());
    rt.update(|s, _| s.show_archived(true));
    assert!(!rt.state.selection.is_selecting(), "into the view");
    rt.update(|s, _| s.selection.toggle("a-row", None));
    rt.update(|s, _| s.show_archived(false));
    assert!(!rt.state.selection.is_selecting(), "and out of it");
}

#[test]
fn an_empty_view_says_no_archived_sessions_and_a_filter_says_no_sessions_found() {
    let mut rt = app();
    open_view(&mut rt);
    assert!(has(&rt, "sessions-empty"));
    let words = crate::fleet::panel::empty_words(false, true);
    assert_eq!(words, ("No archived sessions", "Sessions you archive show up here."));
    assert_eq!(crate::fleet::panel::empty_words(true, true).0, "No sessions found");
}

#[test]
fn restoring_a_row_in_the_view_takes_it_out_of_the_view() {
    let mut rt = app();
    archive_rows(&mut rt, &[(0, 1), (0, 2)]);
    open_view(&mut rt);
    click(&mut rt, "session-restore-0-1");
    rt.frame(FRAME);
    assert!(!rt.state.projects[0].sessions[1].row.archived);
    assert!(!has(&rt, "session-0-1") && has(&rt, "session-0-2"), "the restored row leaves, the other stays");
    // With a row picked the bar's Restore does the same.
    let picked = rt.state.projects[0].sessions[2].id.clone();
    rt.update(move |s, _| s.selection.toggle(&picked, None));
    rt.frame(FRAME);
    assert!(has(&rt, "session-selection-restore") && !has(&rt, "archived"));
}

#[test]
fn the_archived_view_reads_the_list_with_retention_status_archived_and_keeps_what_comes_back() {
    let row = |id: &str, title: &str, archived: bool| {
        json!({"session": {"id": id, "title": title, "time": {"created": 1, "updated": 2}}, "projectName": "Harbor",
               "retentionStatus": if archived { "archived" } else { "active" }})
    };
    let fake = Fake::start(move |_, path| {
        if path.contains("retentionStatus=archived") {
            (200, json!([row("old-1", "Retire the ferry", true)]).to_string())
        } else {
            (200, json!([row("live-1", "Paint the pier", false)]).to_string())
        }
    });
    let mut r = Rig::home(&fake);
    // The fake has no hub to connect to, so the list is asked for the way "Try again" does.
    r.rt.update(|s, _| s.retry_list());
    r.wait("the active list", |s| s.projects.iter().any(|p| p.sessions.iter().any(|x| x.id == "live-1")));
    let ask = |fake: &Fake| fake.calls("/api/sessions?").iter().map(|c| c.1.clone()).collect::<Vec<_>>();
    assert!(ask(&fake).iter().all(|p| !p.contains("retentionStatus")), "the active list leaves the filter out");
    r.rt.update(|s, _| s.show_archived(true));
    r.wait("the archived list", |s| s.projects.iter().any(|p| p.sessions.iter().any(|x| x.id == "old-1")));
    assert!(ask(&fake).iter().any(|p| p.ends_with("&retentionStatus=archived")));
    r.rt.frame(FRAME);
    let ids: Vec<String> = r
        .state()
        .home_layout()
        .groups
        .iter()
        .flat_map(|g| g.rows.iter().map(|&i| r.state().projects[g.p].sessions[i].id.clone()))
        .collect();
    assert_eq!(ids, ["old-1"], "the archived row shows and the active one doesn't");
    r.rt.update(|s, _| s.show_archived(false));
    r.wait("the active list again", |s| s.projects.iter().any(|p| p.sessions.iter().any(|x| x.id == "live-1")));
}
