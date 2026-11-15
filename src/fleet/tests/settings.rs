//! `stores/__tests__/sidebar.test.ts` under its own names, over the settings file instead of `localStorage`: "a reload"
//! is `parse(to_json())`. The cases that need the sessions store's open session pass the session id themselves.
use crate::fleet::settings::*;
use serde_json::{Value, json};

fn reload(s: &Sidebar) -> Sidebar {
    Sidebar::parse(&s.to_json())
}

#[test]
fn use_sidebar_store_tracks_the_active_rail_and_panel_visibility() {
    // The rail is the window's (`State.rail`); what is left of the case is the two panels.
    let mut s = Sidebar::default();
    assert!(!s.panel_collapsed());
    assert!(!s.right_collapsed(None));
    s.set_panel_collapsed(!s.panel_collapsed());
    s.set_right_collapsed(None, !s.right_collapsed(None));
    assert!(s.panel_collapsed());
    assert!(s.right_collapsed(None));
    s.set_panel_collapsed(false);
    s.set_right_collapsed(None, false);
    assert!(!s.panel_collapsed());
    assert!(!s.right_collapsed(None));
}

#[test]
fn use_sidebar_store_persists_both_panel_states() {
    let mut s = Sidebar::default();
    s.set_panel_collapsed(true);
    s.set_right_collapsed(None, true);
    let again = reload(&s);
    assert!(again.panel_collapsed());
    assert!(again.right_collapsed(None));
}

#[test]
fn use_sidebar_store_remembers_which_session_list_groups_are_collapsed_across_a_reload() {
    let mut s = Sidebar::default();
    let machine = machine_group_key("mac");
    let project = project_group_key("mac", "p1");
    assert!(!s.is_group_collapsed(&machine));
    s.toggle_group_collapsed(&machine);
    s.set_group_collapsed(&project, true);
    s.set_group_collapsed(&project_group_key("mac", "p2"), true);
    s.set_group_collapsed(&project_group_key("mac", "p2"), false);

    let again = reload(&s);
    assert!(again.is_group_collapsed(&machine));
    assert!(again.is_group_collapsed(&project));
    assert!(!again.is_group_collapsed(&project_group_key("mac", "p2")));
    // The same project id on another machine is its own group.
    assert!(!again.is_group_collapsed(&project_group_key("linux", "p1")));
    // Only collapsed groups are kept.
    let saved: Value = serde_json::from_str(&s.to_json()).unwrap();
    assert_eq!(saved["collapsedGroups"], json!({ machine: true, project: true }));
}

#[test]
fn use_sidebar_store_keeps_the_right_panel_collapsed_for_a_session_with_nothing_to_show_and_opens_it_on_the_first_change() {
    let mut s = Sidebar::default();
    s.note_session_content("empty", false);
    assert!(s.right_collapsed(Some("empty")));
    s.note_session_content("empty", true);
    assert!(!s.right_collapsed(Some("empty")));
}

#[test]
fn use_sidebar_store_lets_a_remembered_choice_win_over_the_empty_session_default() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(Some("mine"), false);
    s.note_session_content("mine", false);
    assert!(!s.right_collapsed(Some("mine")));
    s.set_right_collapsed(Some("mine"), true);
    s.note_session_content("mine", true);
    assert!(s.right_collapsed(Some("mine")));
}

#[test]
fn use_sidebar_store_doesnt_open_a_session_that_already_had_changes_when_the_app_loaded() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(None, true);
    // A session never toggled takes the last choice (collapsed) rather than opening.
    s.note_session_content("older", true);
    assert!(s.right_collapsed(Some("older")));
}

#[test]
fn use_sidebar_store_remembers_the_right_panel_per_session() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(Some("a"), true);
    s.set_right_collapsed(Some("b"), false);
    assert!(s.right_collapsed(Some("a")));
    assert!(!s.right_collapsed(Some("b")));
    // A session never toggled takes the last choice made anywhere.
    assert!(!s.right_collapsed(Some("c")));
    assert!(reload(&s).right_collapsed(Some("a")));
}

#[test]
fn use_sidebar_store_keeps_the_boards_right_panel_out_of_the_session_choices() {
    let mut s = Sidebar::default();
    s.set_right_collapsed(Some("a"), true);
    assert!(s.right_collapsed(None));
    s.set_right_collapsed(None, false);
    assert!(!s.right_collapsed(None));
    assert!(s.right_collapsed(Some("a")));
}

#[test]
fn use_sidebar_store_knows_whether_any_sessions_list_is_mounted() {
    let mut s = Sidebar::default();
    assert!(!s.list_shown());
    let desktop = s.register_list();
    let drawer = s.register_list();
    s.release_list(desktop);
    s.release_list(desktop);
    assert!(s.list_shown());
    s.release_list(drawer);
    assert!(!s.list_shown());
}

// ── the file, which localStorage did for Fleet ─────────────────────────────

#[test]
fn a_corrupt_file_or_a_value_of_the_wrong_kind_reads_as_nothing_kept() {
    assert!(!Sidebar::parse("{not json").is_group_collapsed("x"));
    assert!(!Sidebar::parse("[1,2]").panel_collapsed());
    let s = Sidebar::parse(
        r#"{"collapsedGroups":{"a":true,"b":"yes","c":false,"d":1},"leftCollapsed":"true","rightCollapsedBySession":[["x",true],["y",1],5]}"#,
    );
    assert!(s.is_group_collapsed("a"));
    assert!(!s.is_group_collapsed("b") && !s.is_group_collapsed("c") && !s.is_group_collapsed("d"));
    assert!(!s.panel_collapsed());
    assert!(s.right_collapsed(Some("x")));
    assert_eq!(s.right_choice("y"), None);
}

#[test]
fn the_newest_two_hundred_per_session_choices_are_kept() {
    let mut s = Sidebar::default();
    for i in 0..205 {
        s.set_right_collapsed(Some(&format!("s{i}")), true);
    }
    s.set_right_collapsed(Some("s100"), false);
    let s = reload(&s);
    assert_eq!(s.right_choice("s0"), None);
    assert_eq!(s.right_choice("s4"), None);
    assert_eq!(s.right_choice("s5"), Some(true));
    assert_eq!(s.right_choice("s100"), Some(false));
}

#[test]
fn the_file_is_written_whole_into_a_folder_of_its_own_and_read_back() {
    let dir = std::env::temp_dir().join(format!("fleet-native-settings-{}", std::process::id())).join("nested");
    let mut s = Sidebar::default();
    s.adopt(dir.clone(), Sidebar::default());
    assert!(s.take_save().is_none(), "nothing changed, nothing to write");
    s.set_group_collapsed(&project_group_key(HOME_MACHINE_KEY, "p1"), true);
    let (to, json) = s.take_save().expect("a change is written");
    assert_eq!(to, dir);
    assert!(s.take_save().is_none());
    write_file(&to, &json).unwrap();
    assert!(load(&dir).is_group_collapsed("project:home:p1"));
    assert!(!dir.join(format!("{FILE}.tmp")).exists());
    assert!(load(&dir.join("missing")).collapsed_is_empty());
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
