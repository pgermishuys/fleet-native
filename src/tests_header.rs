//! The project header and the remembered fold (piece F of docs/parity/06-session-list.md): `ProjectGroup.vue`'s header,
//! `SessionsPanel.vue`'s `handleToggleProject` and `stores/sidebar.ts`. Tests use a folder of their own, never the real one.
use crate::fleet::settings::{Sidebar, project_group_key};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn with_folder(rt: &mut Runtime<crate::State>, name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-header-{name}-{}", std::process::id()));
    let d = dir.clone();
    rt.update(move |s, _| s.adopt_settings(d, Sidebar::default()));
    rt.frame(FRAME);
    dir
}

#[test]
fn folding_a_group_is_remembered_by_its_project_id_and_comes_back_after_a_restart() {
    let mut rt = app();
    let dir = with_folder(&mut rt, "fold");
    let id = rt.state.projects[1].id.clone();
    click(&mut rt, "project-1");
    assert!(!rt.state.projects[1].open);
    // The press already handed the write to its thread, so what it holds is read from the state.
    let json = rt.state.sidebar.to_json();
    assert!(json.contains(&project_group_key("home", &id)));
    // Only the folded group is kept.
    assert_eq!(json.matches("project:").count(), 1);

    let mut next = app();
    let stored = Sidebar::parse(&json);
    next.update(move |s, _| s.adopt_settings(dir, stored));
    next.frame(FRAME);
    assert!(!next.state.projects[1].open, "folded again after a restart");
    assert!(next.state.projects[0].open);
    assert!(next.find("session-1-0").is_none());
    click(&mut next, "project-1");
    assert!(next.state.projects[1].open);
}

#[test]
fn a_folded_group_stays_folded_while_the_filter_is_typed() {
    // `SessionsPanel` hands `isProjectExpanded(id)` to the group and nothing about the query.
    let mut rt = app();
    click(&mut rt, "project-1");
    let name = rt.state.projects[1].sessions[0].title.clone();
    rt.update(move |s, _| s.filter = rune::TextEdit::new(name));
    rt.frame(FRAME);
    assert!(rt.find("project-1").is_some(), "its header still matches");
    assert!(rt.find("session-1-0").is_none(), "its rows stay folded");
}

#[test]
fn a_refresh_keeps_a_folded_group_folded_by_its_id() {
    let mut rt = app();
    click(&mut rt, "project-1");
    let mut again = crate::mock::projects();
    again.reverse();
    rt.update(move |s, tx| s.on_list(again, tx));
    rt.frame(FRAME);
    let folded: Vec<&str> = rt.state.projects.iter().filter(|p| !p.open).map(|p| p.name.as_str()).collect();
    assert_eq!(folded.len(), 1, "one group folded: {folded:?}");
}

#[test]
fn the_pinned_header_has_a_pin_and_the_others_do_not() {
    let mut rt = app();
    assert!(rt.find("pinned-header").is_none());
    rt.update(|s, _| s.sample_headers = true);
    rt.frame(FRAME);
    assert!(rt.find("sample-pinned").is_some() && rt.find("pinned-header").is_some());
    assert!(rt.find("sample-ungrouped").is_some() && rt.find("sample-project").is_some());
}

#[test]
fn the_hidden_list_is_remembered_and_comes_back_hidden() {
    let mut rt = app();
    with_folder(&mut rt, "hidden");
    rt.update(|s, _| s.set_sidebar_open(false));
    let json = rt.state.sidebar.to_json();
    let dir = std::env::temp_dir().join("fleet-native-header-hidden-next");
    let mut next = app();
    next.update(move |s, _| s.adopt_settings(dir, Sidebar::parse(&json)));
    next.frame(FRAME);
    assert!(!next.state.sidebar_open);
}

#[test]
fn the_right_panel_is_remembered_per_session() {
    let mut rt = app();
    with_folder(&mut rt, "right");
    // The invented sessions have no server id, so they get ones of their own for a choice to be kept under.
    rt.update(|s, _| {
        s.set_session_id((0, 0), "sa");
        s.set_session_id((0, 1), "sb");
        s.set_content_open(true);
    });
    let first = rt.state.open_id().unwrap();
    rt.update(|s, tx| s.open_session(0, 1, tx));
    // A session never toggled takes the last choice made anywhere: open.
    assert!(rt.state.content_open);
    rt.update(|s, _| s.set_content_open(false));
    rt.update(move |s, tx| {
        let at = s.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|x| x.id == first).map(|i| (p, i))).unwrap();
        s.open_session(at.0, at.1, tx);
    });
    assert!(rt.state.content_open, "the first session kept its own choice");
}

#[test]
fn a_change_reaches_the_file_on_disk_without_the_ui_thread_waiting_for_it() {
    let mut rt = app();
    let dir = with_folder(&mut rt, "disk");
    click(&mut rt, "project-0");
    let key = project_group_key("home", &rt.state.projects[0].id);
    let result = std::panic::catch_unwind(|| {
        crate::test_wait::until_true("the folded group to be written", || crate::fleet::settings::load(&dir).is_group_collapsed(&key))
    });
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
