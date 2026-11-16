//! The machine part of the session list (component 17, piece C): `MachineSessionsGroup.test.ts` and the machine cases of
//! `SessionsPanel.test.ts` that a read-only list can answer, under Fleet's names. Scenes use invented machines.
use crate::fleet::groups::{Listing, ProjectInfo, ProjectKind};
use crate::fleet::machine_tree::{self, build};
use crate::fleet::machines::MachineSessions;
use crate::fleet::settings::{machine_group_key, project_group_key};
use crate::mock::machines::Case;
use crate::mock::{Session, Status};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn scene(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_machines(case, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn projects() -> Vec<ProjectInfo> {
    let p = |id: &str, name: &str, kind, position| ProjectInfo { id: id.into(), name: name.into(), kind, position };
    vec![
        p("p-scratch", "Scratch", ProjectKind::Scratch, 9),
        p("p-garden", "Garden", ProjectKind::User, 2),
        p("p-lighthouse", "Lighthouse", ProjectKind::User, 1),
        p("p-empty", "Products", ProjectKind::User, 3),
    ]
}

fn item(id: &str, title: &str, project: &str, edit: impl FnOnce(&mut Listing)) -> Session {
    let mut s = crate::mock::machines::listed(id, title, Status::Idle, "now", Some((project, project)), edit);
    s.listing.project_name = None;
    s
}

fn names(t: &machine_tree::Tree) -> Vec<&str> {
    t.groups.iter().map(|g| g.name.as_str()).collect()
}

fn ids(sessions: &[Session], t: &machine_tree::Tree) -> Vec<String> {
    t.groups.iter().flat_map(|g| g.rows.iter().map(|r| sessions[r.index].id.clone())).collect()
}

#[test]
fn machinesessionsgroup_groups_its_sessions_like_the_live_machine_pinned_first_then_its_projects_in_their_order() {
    let sessions = vec![
        item("garden-1", "Water the tomatoes", "p-garden", |_| {}),
        item("pin-1", "Plan the spring planting", "p-lighthouse", |l| l.pin_order = Some(1.0)),
        item("lh-1", "Paint the fence", "p-lighthouse", |_| {}),
        item("scratch-1", "Quick regex", "p-scratch", |_| {}),
    ];
    let t = build(&sessions, &projects(), "");
    assert_eq!(names(&t), ["Pinned", "Lighthouse", "Garden", "Products", "Scratch"]);
    assert_eq!(ids(&sessions, &t), ["pin-1", "lh-1", "garden-1", "scratch-1"]);
    assert_eq!(t.count, 4, "the heading counts what the groups hold");
}

#[test]
fn machinesessionsgroup_puts_forks_and_started_sessions_under_the_top_level_session_they_came_from() {
    let sessions = vec![
        item("fork", "Fork of the mapper", "p-lighthouse", |l| {
            l.forked_from = Some("parent".into());
            l.spawn_kind = Some("fork".into());
        }),
        item("parent", "Capture subagents", "p-lighthouse", |_| {}),
        item("other", "Something else", "p-lighthouse", |_| {}),
    ];
    let t = build(&sessions, &projects(), "");
    assert_eq!(ids(&sessions, &t), ["parent", "fork", "other"]);
    let rows = &t.groups[0].rows;
    assert_eq!((rows[0].depth, rows[1].depth, rows[2].depth), (0, 1, 0));
    assert_eq!(rows[1].kind, Some(crate::fleet::groups::LineageKind::Fork));
}

#[test]
fn machinesessionsgroup_counts_what_runs_on_a_row() {
    let mut rt = scene(Case::Several);
    rt.update(|s, _| {
        let m = s.machines.others.get_mut("m-falcon").unwrap();
        m.sessions.iter_mut().find(|x| x.id == "f-plan").unwrap().row.running_work = 2;
    });
    rt.frame(FRAME);
    assert!(has(&rt, "session-running-chip-m-falcon-f-plan"));
}

#[test]
fn machinesessionsgroup_filters_by_the_search_box_keeping_a_project_whose_name_matches() {
    let sessions =
        vec![item("garden-1", "Water the tomatoes", "p-garden", |_| {}), item("lh-1", "Paint the fence", "p-lighthouse", |_| {})];
    assert_eq!(ids(&sessions, &build(&sessions, &projects(), "tomatoes")), ["garden-1"]);
    assert_eq!(names(&build(&sessions, &projects(), "light")), ["Lighthouse"]);
    assert_eq!(ids(&sessions, &build(&sessions, &projects(), "light")), ["lh-1"], "a project whose name matches stays whole");
    assert!(build(&sessions, &projects(), "nothing like it").groups.is_empty());
}

#[test]
fn machinesessionsgroup_folds_its_projects_under_its_own_keys() {
    let mut rt = scene(Case::Several);
    assert!(has(&rt, "machine-session-m-falcon-f-tomato"));
    click(&mut rt, "machine-project-m-falcon-p-garden");
    assert!(rt.state.sidebar.is_group_collapsed(&project_group_key("m-falcon", "p-garden")));
    assert!(!has(&rt, "machine-session-m-falcon-f-tomato"), "its rows fold");
    assert!(has(&rt, "machine-session-m-falcon-f-fence"), "another project stays");
    assert!(!rt.state.sidebar.is_group_collapsed(&project_group_key("home", "p-garden")), "no other machine's key");
}

#[test]
fn machinesessionsgroup_has_no_draft_row_for_a_draft_on_another_machine_and_says_when_it_has_no_sessions() {
    let rt = scene(Case::Empty);
    assert!(has(&rt, "machine-empty-m-quiet"), "No sessions yet");
    assert!(has(&rt, "machine-new-session-m-quiet"));
    assert!(!has(&rt, "sessions-draft"), "no draft row");
    let mut rt = scene(Case::Empty);
    rt.update(|s, _| s.filter = rune::TextEdit::new("zzz"));
    rt.frame(FRAME);
    assert!(has(&rt, "machine-empty-m-quiet") && !has(&rt, "machine-new-session-m-quiet"), "No sessions found has no button");
}

#[test]
fn sessionspanel_lists_home_first_and_the_other_machines_in_the_order_they_were_added() {
    let rt = scene(Case::Several);
    let y = |k: &str| rt.find(k).unwrap_or_else(|| panic!("no {k}")).1;
    assert!(y("machine-home") < y("machine-m-falcon") && y("machine-m-falcon") < y("machine-m-harbor"));
    assert!(y("machine-group-m-harbor") < y("sessions-add-machine"), "the Add a machine row ends the groups");
    assert_eq!(rt.state.machines.entries().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["atlas", "falcon", "harbor", "lantern"]);
}

#[test]
fn sessionspanel_shows_no_machine_headings_and_its_rows_act_on_this_machine_as_before() {
    // Row 5.1 (17.80): with no machine besides home nothing of this component is drawn.
    let rt = app();
    assert!(!rt.state.machines.has_machines());
    for key in ["machine-home", "sessions-add-machine"] {
        assert!(!has(&rt, key), "{key}");
    }
    assert!(has(&rt, "project-0") && has(&rt, "session-0-0"));
    assert!(!rt.state.sidebar.is_group_collapsed(&machine_group_key("home")));
}

#[test]
fn sessionspanel_ends_the_machine_groups_with_an_add_a_machine_row() {
    let mut rt = scene(Case::Down);
    click(&mut rt, "sessions-add-machine");
    assert!(rt.state.add_machine_asked);
    assert_eq!(rt.state.rail, crate::Rail::Settings);
}

#[test]
fn the_live_machines_heading_folds_its_tree_and_counts_what_the_filter_leaves() {
    let mut rt = scene(Case::Several);
    assert!(has(&rt, "session-0-0"));
    click(&mut rt, "machine-home");
    assert!(rt.state.sidebar.is_group_collapsed(&machine_group_key("home")));
    assert!(!has(&rt, "session-0-0") && has(&rt, "machine-m-falcon"), "only home's tree folds");
    click(&mut rt, "machine-m-falcon");
    assert!(!has(&rt, "machine-session-m-falcon-f-fence"));
}

#[test]
fn a_machine_that_isnt_answering_says_why_and_keeps_its_rows_dimmed_and_unactionable() {
    let rt = scene(Case::Down);
    assert!(has(&rt, "machine-error-m-harbor"), "the reason under the heading");
    assert!(has(&rt, "machine-m-harbor-note"));
    assert!(!rt.state.machines.is_answering("m-harbor"));
    assert!(has(&rt, "machine-session-m-harbor-h-crane"), "the last list stays");
    assert!(!has(&rt, "machine-glyph-m-harbor-h-crane"), "a working session shows no working glyph");
    assert!(has(&rt, "session-status-m-harbor-h-crane"), "it says Not answering instead");
}

#[test]
fn the_note_is_an_ellipsis_while_loading_and_unreachable_with_or_without_a_cache() {
    use crate::view::machine_group::note_of;
    let loading = MachineSessions { loading: true, ..Default::default() };
    assert_eq!(note_of(Some(&loading)), Some("\u{2026}"));
    assert_eq!(note_of(None), None);
    let none = MachineSessions { error: Some("x".into()), ..Default::default() };
    assert_eq!(note_of(Some(&none)), Some("unreachable"));
    let cached = MachineSessions { error: Some("x".into()), loaded_at: Some(1), ..Default::default() };
    assert_eq!(note_of(Some(&cached)), Some("unreachable \u{b7} cached"));
    let done = MachineSessions { loaded_at: Some(1), ..Default::default() };
    assert_eq!(note_of(Some(&done)), None, "the count shows");
    assert!(has(&scene(Case::Loading), "machine-m-lantern-note"));
}

#[test]
fn a_parent_opens_its_children_by_itself_while_one_works_and_its_caret_toggles_them() {
    let mut rt = scene(Case::Several);
    assert!(has(&rt, "machine-session-m-falcon-f-fork"), "the fork works, so its parent is open");
    click(&mut rt, "machine-caret-m-falcon-f-plan");
    assert!(!has(&rt, "machine-session-m-falcon-f-fork"));
    click(&mut rt, "machine-caret-m-falcon-f-plan");
    assert!(has(&rt, "machine-session-m-falcon-f-fork"));
    assert!(has(&rt, "session-kind-m-falcon-f-fork"), "the kind label");
}

// Piece E: opening another machine's session in place, and the machine you're on.

#[test]
fn sessionspanel_opens_another_machines_session_in_place_without_a_reload() {
    let mut rt = scene(Case::Several);
    click(&mut rt, "machine-session-m-falcon-f-plan");
    rt.state.open_id().expect("a session is open");
    assert_eq!(rt.state.open_id().as_deref(), Some("f-plan"));
    assert_eq!(rt.state.machine_key_of("f-plan"), "m-falcon");
    assert_eq!(rt.state.store.machine_of("f-plan"), Some("m-falcon"), "kept in elsewhere with its machine");
    assert_eq!(rt.state.session().title, "Plan the lamp rewiring", "the conversation shows the machine's row");
    assert_eq!(rt.state.machine_name().as_deref(), Some("falcon"));
}

#[test]
fn sessionspanel_marks_the_open_session_s_row_on_the_machine_its_on_and_only_that_row() {
    let mut rt = scene(Case::Several);
    let open_rows = |s: &crate::State| -> Vec<String> {
        let mut rows = Vec::new();
        for (key, m) in &s.machines.others {
            rows.extend(m.sessions.iter().filter(|x| s.is_open_on(key, &x.id)).map(|x| format!("{key}/{}", x.id)));
        }
        rows
    };
    assert!(open_rows(&rt.state).is_empty(), "home's row is open, so none of another machine's");
    click(&mut rt, "machine-session-m-falcon-f-plan");
    assert_eq!(open_rows(&rt.state), ["m-falcon/f-plan"]);
    assert_eq!(rt.state.open_at().0, crate::fleet::store::ELSEWHERE, "home's list has no row open");
    // And back to one of home's.
    rt.update(|s, tx| s.open_session(0, 0, tx));
    assert!(open_rows(&rt.state).is_empty());
    assert_eq!(rt.state.machine_name(), None, "home has no chip");
}

#[test]
fn sessionspanel_keeps_another_machines_session_open_when_home_lists_again() {
    // The route names the session, so a refresh of home's list leaves it where it is (the part of "after a reload" that
    // applies without a page).
    let mut rt = scene(Case::Several);
    click(&mut rt, "machine-session-m-falcon-f-plan");
    rt.update(|s, tx| {
        let list = std::mem::take(&mut s.projects);
        s.on_list(list, tx);
    });
    assert_eq!(rt.state.open_id().as_deref(), Some("f-plan"));
    assert_eq!(rt.state.session().title, "Plan the lamp rewiring");
}

#[test]
fn machines_switch_is_on_the_live_machine_until_a_session_on_another_machine_is_open_and_stays_on_it_afterwards() {
    let mut rt = scene(Case::Several);
    assert_eq!(rt.state.machines.current_key(), "home");
    click(&mut rt, "machine-session-m-falcon-f-plan");
    assert_eq!(rt.state.machines.current_key(), "m-falcon");
    assert_eq!(rt.state.machines.list.iter().find(|m| m.id == "m-falcon").unwrap().name, "falcon");
    rt.update(|s, tx| s.open_session(0, 0, tx));
    // Home's own session is open: back on the live machine.
    assert_eq!(rt.state.machines.current_key(), "home");
}

#[test]
fn machines_switch_ignores_a_machine_thats_gone() {
    let mut rt = scene(Case::Several);
    rt.update(|s, _| s.machines.set_current("cccccccccccccccccccccccccccccccc"));
    assert_eq!(rt.state.machines.current_key(), "home");
    rt.update(|s, _| s.machines.set_current("m-harbor"));
    assert_eq!(rt.state.machines.current_key(), "m-harbor");
}

#[test]
fn a_session_on_a_machine_that_isnt_answering_refuses_a_message_naming_the_machine() {
    let mut rt = scene(Case::Down);
    click(&mut rt, "machine-session-m-harbor-h-crane");
    assert_eq!(rt.state.machine_refusal().as_deref(), Some("harbor isn't answering."));
    rt.update(|s, tx| {
        s.draft.text = "hello".into();
        crate::send(s, tx);
    });
    assert_eq!(rt.state.composer_error.as_deref(), Some("harbor isn't answering."));
    assert_eq!(rt.state.draft.text, "hello", "the draft stays");
}

// Piece F: another machine's rows: actions and read-only.

fn right_click(rt: &mut Runtime<crate::State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn falcon_title(rt: &Runtime<crate::State>, id: &str) -> String {
    rt.state.machines.others["m-falcon"].sessions.iter().find(|s| s.id == id).unwrap().title.clone()
}

fn type_over(rt: &mut Runtime<crate::State>, text: &str) {
    rt.update(|s, _| s.act.edit = rune::TextEdit::new("x".to_string()));
    let text = text.to_string();
    rt.update(move |s, _| s.act.edit = rune::TextEdit::new(text));
}

#[test]
fn machinesessionsgroup_gives_its_rows_what_the_live_machines_rows_have_rename_and_the_menu() {
    let mut rt = scene(Case::Several);
    // The title says how, as this machine's rows do.
    assert!(rt.find("machine-session-m-falcon-f-fence").is_some());
    // F2 starts the rename on the row: the title is a field.
    rt.update(|s, tx| s.start_machine_row_rename("m-falcon", "f-fence", tx));
    rt.frame(FRAME);
    assert!(rt.state.machine_row_renaming("f-fence"));
    assert!(rt.find(crate::fleet::actions::RENAME_FIELD).is_some(), "the field is where the title was");
    type_over(&mut rt, "Paint the gate");
    rt.update(|s, tx| s.finish_row_rename(true, tx));
    rt.frame(FRAME);
    assert_eq!(falcon_title(&rt, "f-fence"), "Paint the gate", "the new title shows at once on that machine's list");
    assert!(rt.find("machine-title-m-falcon-f-fence").is_some(), "and the row is a row again");
}

#[test]
fn machinesessionsgroup_only_opens_a_session_no_pin_archive_drag_rename_or_picking() {
    let mut rt = scene(Case::Down);
    rt.update(|s, tx| s.start_machine_row_rename("m-harbor", "h-ledger", tx));
    assert!(!rt.state.machine_row_renaming("h-ledger"), "a machine that isn't answering renames nothing");
    right_click(&mut rt, "machine-session-m-harbor-h-ledger");
    assert!(rt.find("ctx-menu").is_none(), "no menu");
    assert!(rt.state.ui.confirm.is_none());
    rt.update(|s, tx| {
        s.ask_machine_delete_session("m-harbor", "h-ledger", tx);
        s.ask_machine_clear_attachments("m-harbor", "h-ledger", tx);
    });
    assert!(rt.state.ui.confirm.is_none(), "nor a question about it");
    click(&mut rt, "machine-session-m-harbor-h-ledger");
    assert_eq!(rt.state.open_id().as_deref(), Some("h-ledger"), "but it opens");
}

#[test]
fn machinesessionsgroup_offers_copy_path_not_open_in_on_another_machines_row() {
    let mut rt = scene(Case::Several);
    right_click(&mut rt, "machine-session-m-falcon-f-fence");
    assert!(rt.find("ctx-menu").is_some());
    // Open, Rename, Archive, Copy path, Clear attachments, a separator, Delete permanently: no Open in, no schedule.
    for n in [0, 1, 2, 3, 4, 6] {
        assert!(rt.find(&format!("ctx-item{n}")).is_some(), "item {n}");
    }
    assert!(rt.find("ctx-item7").is_none());
    assert_eq!(rt.state.machines.others["m-falcon"].sessions[2].listing.workspace_directory, "/srv/work", "the folder Copy path copies");
}

#[test]
fn sessionspanel_the_menus_rename_and_questions_act_on_that_machines_session() {
    let mut rt = scene(Case::Several);
    right_click(&mut rt, "machine-session-m-falcon-f-fence");
    click(&mut rt, "ctx-item1");
    assert!(rt.state.machine_row_renaming("f-fence"));
    assert!(rt.find("ctx-menu").is_none(), "the menu closed before the field opened");
    rt.update(|s, tx| s.finish_row_rename(false, tx));
    rt.frame(FRAME);
    right_click(&mut rt, "machine-session-m-falcon-f-fence");
    assert!(rt.find("ctx-menu").is_some(), "{:?} {:?}", rt.state.ui.open, rt.state.act.rename.is_some());
    click(&mut rt, "ctx-item6");
    rt.frame(FRAME);
    let c = rt.state.ui.confirm.as_ref().expect("a question");
    assert_eq!((c.title.as_str(), c.subject.as_str()), ("Delete this session?", "Paint the fence"));
    click(&mut rt, "confirm-action");
    assert!(rt.state.ui.confirm.is_none());
    assert!(rt.state.machines.others["m-falcon"].sessions.iter().all(|s| s.id != "f-fence"), "out of that machine's list at once");
    assert_eq!(rt.state.machines.others["m-falcon"].sessions.len(), 3);
}

#[test]
fn sessionspanel_renames_another_machines_project_on_that_machine_and_lists_that_machine_again() {
    let mut rt = scene(Case::Several);
    let name = |rt: &Runtime<crate::State>| {
        rt.state.machines.others["m-falcon"].projects.iter().find(|p| p.id == "p-lighthouse").unwrap().name.clone()
    };
    right_click(&mut rt, "machine-project-m-falcon-p-lighthouse");
    assert!(rt.find("pctx-menu").is_some());
    click(&mut rt, "pctx-item0");
    assert!(rt.state.machine_project_renaming("m-falcon", "p-lighthouse"));
    assert!(!rt.state.project_renaming(0), "not home's project of the same place in the list");
    rt.update(|s, _| s.proj.edit = rune::TextEdit::new("Lamp house".to_string()));
    rt.update(|s, tx| s.finish_rename(true, tx));
    rt.frame(FRAME);
    assert_eq!(name(&rt), "Lamp house");
    assert!(!rt.state.proj.pending);
    // Pinned and a down machine's projects have no rename.
    assert!(rt.find("machine-project-m-falcon-pinned").is_none() || !rt.state.machine_project_renaming("m-falcon", "pinned"));
    rt.update(|s, tx| s.start_machine_rename("m-harbor", "p-docks", tx));
    assert!(!rt.state.machine_project_renaming("m-harbor", "p-docks"));
}
