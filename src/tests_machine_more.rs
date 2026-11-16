//! Piece K of the New session page: the machine chip and its menu, and the "…" chip's project, title and tags
//! (`MachinePicker.vue`, `MoreOptions.vue`; rows B5, B8, A18, A21.95-105, A21.11), driven with no window. Fleet's cases are in
//! `NewSessionComposer.test.ts` (`machine`, `chips`) and keep their names here. The machines are the invented ones of
//! `mock::machines`: home "atlas", "falcon" that answers, "harbor" that stopped answering, "lantern" not read yet.
//! Cases that start a session (A21.101, A21.104, A21.105) wait for piece L, which sends it; the checklist says so.
use crate::fleet::groups::{ProjectInfo, ProjectKind};
use crate::fleet::new_session::request::Folder;
use crate::mock::machines::Case;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::{Key, KeyEvent, Mods, Runtime};

type Rt = Runtime<State>;

fn page_with(case: Option<Case>) -> Rt {
    let mut rt = app();
    if let Some(case) = case {
        rt.update(move |s, _| s.show_machines_only(case));
    }
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt
}

fn exists(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn words(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let it = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    all.iter().filter(|e| e.path.starts_with(&it.path) && e.path != it.path).filter_map(|e| e.text.clone()).collect::<String>()
}

fn key(rt: &mut Rt, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn type_in(rt: &mut Rt, field: &str, text: &str) {
    click(rt, field);
    key(rt, Key::Text(text.into()));
}

fn message(rt: &mut Rt, text: &str) {
    rt.update(|s, _| {
        if let Some(d) = s.new_draft.draft_mut() {
            d.message = text.to_string();
        }
    });
}

fn project(id: &str, name: &str, kind: ProjectKind) -> ProjectInfo {
    ProjectInfo { id: id.into(), name: name.into(), kind, position: 1 }
}

/// Home has a Scratch group and two of the user's.
fn with_projects(rt: &mut Rt) {
    rt.update(|s, _| {
        s.project_load.projects = vec![
            project("scratch", "Scratch", ProjectKind::Scratch),
            project("p-garden", "Garden", ProjectKind::User),
            project("p-lighthouse", "Lighthouse", ProjectKind::User),
        ];
    });
    rt.frame(FRAME);
}

// ─── machine ────────────────────────────────────────────────────────────────────

#[test]
fn machine_shows_the_machine_chip_on_a_desktop_with_one_machine_and_its_menu_ends_with_add_a_machine() {
    let mut rt = page_with(None);
    assert!(exists(&rt, "new-session-machine"));
    click(&mut rt, "new-session-machine");
    assert!(rt.state.machine_menu_open());
    assert!(exists(&rt, "new-session-machine-home") && exists(&rt, "new-session-machine-add"));
    let rows: Vec<_> = rt.state.machine_rows().into_iter().map(|r| r.key).collect();
    assert_eq!(rows, ["home"]);
    assert_eq!(words(&rt, "new-session-machine-add"), "Add a machine…Settings → Machines");
}

#[test]
fn machine_shows_the_machine_first_and_lists_every_machine_with_whether_it_can_be_reached() {
    let mut rt = page_with(Some(Case::Several));
    assert_eq!(words(&rt, "new-session-machine"), "atlas");
    click(&mut rt, "new-session-machine");
    let rows = rt.state.machine_rows();
    let named: Vec<_> = rows.iter().map(|r| (r.name.as_str(), r.reachable)).collect();
    assert_eq!(named, [("atlas", true), ("falcon", true), ("harbor", false), ("lantern", true)]);
    assert_eq!(rows[0].detail, "Working here · Linux");
    assert!(rows[1].detail.starts_with("Linux · reached "), "{}", rows[1].detail);
    assert!(rows[2].detail.starts_with("Unreachable · last seen "), "{}", rows[2].detail);
    assert!(rows[1].heard_at.is_some() && rows[0].heard_at.is_none());
    assert!(words(&rt, "new-session-machine-m-harbor").contains("Unreachable"));
}

#[test]
fn machine_moves_the_draft_to_the_machine_picked_keeping_the_message_and_dropping_this_machines_folder() {
    let mut rt = page_with(Some(Case::Several));
    message(&mut rt, "Fix the lamp wiring");
    rt.update(|s, _| s.set_folder(Folder::Directory("/srv/atlas/notes".into()), true));
    click(&mut rt, "new-session-machine");
    click(&mut rt, "new-session-machine-m-falcon");
    assert!(!rt.state.machine_menu_open());
    assert_eq!(rt.state.target_key(), "m-falcon");
    assert_eq!(words(&rt, "new-session-machine"), "falcon");
    let d = rt.state.new_draft.draft().expect("a draft");
    assert_eq!(d.message, "Fix the lamp wiring");
    assert_eq!(d.folder, None);
}

#[test]
fn machine_add_a_machine_opens_settings_machines() {
    let mut rt = page_with(None);
    click(&mut rt, "new-session-machine");
    click(&mut rt, "new-session-machine-add");
    assert_eq!(rt.state.rail, Rail::Settings);
    assert!(rt.state.add_machine_asked);
    assert!(!rt.state.machine_menu_open());
}

#[test]
fn machine_on_the_new_session_page_keeps_the_message_when_the_box_moves_to_another_machine() {
    let mut rt = page_with(Some(Case::Several));
    message(&mut rt, "Water the tomatoes");
    rt.update(|s, _| s.pick_machine("m-falcon"));
    rt.update(|s, _| s.pick_machine("home"));
    assert_eq!(rt.state.new_draft.draft().map(|d| d.message.as_str()), Some("Water the tomatoes"));
    assert!(rt.state.on_new_session_page());
}

#[test]
fn machine_on_the_new_session_page_keeps_the_live_machine_on_the_chip_with_no_other_machines_listed() {
    let rt = page_with(None);
    assert_eq!(rt.state.target_key(), "home");
    assert_eq!(rt.state.target_name(), "This machine");
    assert!(!rt.state.target_is_down());
}

#[test]
fn machine_on_another_machine_remembers_its_own_folders() {
    let mut rt = page_with(Some(Case::Several));
    rt.update(|s, _| s.pick_machine("m-falcon"));
    rt.update(|s, _| s.set_folder(Folder::Directory("/srv/falcon/tools".into()), true));
    assert_eq!(rt.state.target_key(), "m-falcon");
    // Its remembered choices are kept under its own key (A21.103); the folder is the page's until a session is made.
    assert_eq!(crate::fleet::new_session::defaults::defaults_key("m-falcon"), "weave:new-session:defaults:m-falcon");
}

#[test]
fn machine_the_chip_turns_red_while_the_machine_picked_cant_be_reached() {
    let mut rt = page_with(Some(Case::Down));
    // An unreachable machine is listed but can't be picked (B5.13).
    click(&mut rt, "new-session-machine");
    click(&mut rt, "new-session-machine-m-harbor");
    assert_eq!(rt.state.target_key(), "home");
    // A machine that stops answering after it was picked turns the dot and the name red (B5.3, B5.4).
    rt.update(|s, _| {
        s.machines.others.get_mut("m-harbor").expect("harbor").error = None;
        s.pick_machine("m-harbor");
        s.machines.others.get_mut("m-harbor").expect("harbor").error = Some("Can't reach harbor.".into());
    });
    assert!(rt.state.target_is_down());
}

#[test]
fn machine_an_unreachable_machine_cannot_be_selected_with_the_keyboard_either() {
    let mut rt = page_with(Some(Case::Down));
    rt.update(|s, _| s.open_machine_menu());
    rt.frame(FRAME);
    rt.update(|s, _| s.pick_machine("m-harbor"));
    assert_eq!(rt.state.target_key(), "home");
}

#[test]
fn machine_escape_closes_the_menu_and_the_chip_keeps_the_machine() {
    let mut rt = page_with(Some(Case::Several));
    click(&mut rt, "new-session-machine");
    key(&mut rt, Key::Escape);
    assert!(!rt.state.machine_menu_open());
    assert_eq!(rt.state.target_key(), "home");
}

#[test]
fn machine_the_chip_is_disabled_while_the_session_starts() {
    let mut rt = page_with(Some(Case::Several));
    rt.update(|s, _| {
        if let Some(d) = s.new_draft.draft_mut() {
            d.is_starting = true;
        }
    });
    rt.frame(FRAME);
    click(&mut rt, "new-session-machine");
    assert!(!rt.state.machine_menu_open());
}

// ─── the "…" chip ───────────────────────────────────────────────────────────────

#[test]
fn chips_shows_the_project_on_the_more_chip_when_it_isnt_scratch() {
    let mut rt = page_with(None);
    with_projects(&mut rt);
    assert_eq!(words(&rt, "new-session-more-chip"), "");
    assert_eq!(rt.state.more_chip_label(), "More options: project, title, tags");
    rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").project_id = Some("p-garden".into()));
    rt.frame(FRAME);
    assert_eq!(words(&rt, "new-session-more-chip"), "Garden");
    assert_eq!(rt.state.more_chip_label(), "More options (project Garden): project, title, tags");
    assert_eq!(rt.state.more_chip_help(), "Project Garden: the sidebar group it's filed under");
    // The Scratch group is the default, so a draft filed under it shows no name.
    rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").project_id = Some("scratch".into()));
    rt.frame(FRAME);
    assert_eq!(words(&rt, "new-session-more-chip"), "");
}

#[test]
fn more_options_offers_the_project_field_only_when_the_user_has_a_project() {
    let mut rt = page_with(None);
    click(&mut rt, "new-session-more-chip");
    assert!(rt.state.more_menu_open());
    assert!(!exists(&rt, "new-session-project") && exists(&rt, TITLE) && exists(&rt, TAGS));
    key(&mut rt, Key::Escape);
    with_projects(&mut rt);
    click(&mut rt, "new-session-more-chip");
    assert!(exists(&rt, "new-session-project"));
    assert_eq!(words(&rt, "new-session-project"), "Scratch");
}

const TITLE: &str = "session-title";
const TAGS: &str = "new-session-tags";

#[test]
fn more_options_picks_a_project_or_scratch_from_the_list_and_keeps_the_menu_open() {
    let mut rt = page_with(None);
    with_projects(&mut rt);
    click(&mut rt, "new-session-more-chip");
    click(&mut rt, "new-session-project");
    assert!(rt.state.pickers_ns.more.select_open);
    // Scratch first, then each project; the Scratch group of the server is not listed twice.
    assert!(exists(&rt, "new-session-project-select-opt0") && exists(&rt, "new-session-project-select-opt2"));
    assert!(!exists(&rt, "new-session-project-select-opt3"));
    click(&mut rt, "new-session-project-select-opt2");
    assert!(rt.state.more_menu_open() && !rt.state.pickers_ns.more.select_open);
    assert_eq!(rt.state.new_draft.draft().and_then(|d| d.project_id.clone()).as_deref(), Some("p-lighthouse"));
    click(&mut rt, "new-session-project");
    click(&mut rt, "new-session-project-select-opt0");
    assert_eq!(rt.state.new_draft.draft().and_then(|d| d.project_id.clone()), None);
}

#[test]
fn more_options_title_and_tags_go_to_the_draft_and_enter_closes_the_menu() {
    let mut rt = page_with(None);
    click(&mut rt, "new-session-more-chip");
    type_in(&mut rt, TITLE, "Tidy the lamp room");
    type_in(&mut rt, TAGS, "review-requested, deploy");
    let d = rt.state.new_draft.draft().expect("a draft");
    assert_eq!((d.title.as_str(), d.tags.as_str()), ("Tidy the lamp room", "review-requested, deploy"));
    key(&mut rt, Key::Enter);
    assert!(!rt.state.more_menu_open());
    // Opening again shows what the draft holds.
    click(&mut rt, "new-session-more-chip");
    assert_eq!(rt.state.pickers_ns.more.title.text, "Tidy the lamp room");
}

#[test]
fn more_options_moving_to_another_machine_drops_the_project_but_keeps_title_and_tags() {
    let mut rt = page_with(Some(Case::Several));
    with_projects(&mut rt);
    rt.update(|s, _| {
        let d = s.new_draft.draft_mut().expect("a draft");
        d.project_id = Some("p-garden".into());
        d.title = "Keep me".into();
        d.tags = "a, b".into();
        s.pick_machine("m-falcon");
    });
    let d = rt.state.new_draft.draft().expect("a draft");
    assert_eq!((d.project_id.clone(), d.title.as_str(), d.tags.as_str()), (None, "Keep me", "a, b"));
    // Falcon's own projects are the ones offered now.
    let names: Vec<_> = rt.state.more_projects().into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["Lighthouse", "Garden"]);
}

#[test]
fn more_options_the_chip_is_disabled_while_the_session_starts() {
    let mut rt = page_with(None);
    rt.update(|s, _| {
        if let Some(d) = s.new_draft.draft_mut() {
            d.is_starting = true;
        }
    });
    rt.frame(FRAME);
    click(&mut rt, "new-session-more-chip");
    assert!(!rt.state.more_menu_open());
}
