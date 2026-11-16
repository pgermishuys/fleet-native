//! The home page (component 9, piece D): written from reading `FleetDashboard.vue` and `DashboardSessionRow.vue`. A row opens
//! its session on a click, Enter and Space; the sections hold what the rules say; the states and the banner show.
use crate::fleet::home::Scene;
use crate::tests::{FRAME, app, click};
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};

fn home(scene: Scene) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_home_scene(scene, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn info(rt: &Runtime<crate::State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key))
}

fn id_of(rt: &mut Runtime<crate::State>, title: &str) -> String {
    rt.update(|s, _| {
        let mut n = 0;
        for p in &mut s.projects {
            for x in &mut p.sessions {
                n += 1;
                x.id = format!("s{n}");
            }
        }
    });
    rt.frame(FRAME);
    let s = &rt.state;
    s.projects.iter().flat_map(|p| &p.sessions).find(|x| x.title == title).unwrap().id.clone()
}

#[test]
fn home_has_no_row_selected_in_the_list() {
    let rt = home(Scene::Three);
    assert!(rt.state.on_home());
    assert!(!rt.state.has_session());
}

#[test]
fn sections_hold_what_needs_you_what_works_and_the_rest() {
    let mut rt = home(Scene::Three);
    let working = id_of(&mut rt, "Rebuild the index");
    let needs = id_of(&mut rt, "Pick a page size");
    let quiet = id_of(&mut rt, "Fix pagination");
    let [n, w, r] = crate::view::dashboard::listed(&rt.state);
    assert_eq!((n, w, r), (vec![needs], vec![working], vec![quiet]));
    assert!(info(&rt, "home-needs-you").is_some() && info(&rt, "home-working").is_some() && info(&rt, "home-recent").is_some());
}

#[test]
fn a_click_opens_the_session() {
    let mut rt = home(Scene::Three);
    let needs = id_of(&mut rt, "Pick a page size");
    click(&mut rt, &format!("dash-row-{needs}"));
    assert!(!rt.state.on_home());
    assert_eq!(rt.state.session().title, "Pick a page size");
}

fn open_with(key: Key) -> String {
    let mut rt = home(Scene::Three);
    let quiet = id_of(&mut rt, "Fix pagination");
    let row = format!("dash-row-{quiet}");
    rt.update(|_, tx| tx.focus("dash-row-s3", true));
    rt.frame(FRAME);
    assert!(info(&rt, &row).is_some());
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
    if rt.state.on_home() { String::new() } else { rt.state.session().title.clone() }
}

#[test]
fn enter_opens_the_session() {
    assert_eq!(open_with(Key::Enter), "Fix pagination");
}

#[test]
fn space_opens_the_session() {
    assert_eq!(open_with(Key::Text(" ".into())), "Fix pagination");
}

#[test]
fn the_status_label_takes_the_rows_tone() {
    let mut rt = home(Scene::Three);
    let needs = id_of(&mut rt, "Pick a page size");
    let quiet = id_of(&mut rt, "Fix pagination");
    assert!(info(&rt, &format!("dash-row-{needs}-status")).is_some(), "Needs input is drawn");
    assert!(info(&rt, &format!("dash-row-{quiet}-status")).is_some(), "the age is drawn");
    rt.update(|s, _| s.projects[0].sessions[1].status = crate::mock::Status::Idle);
    rt.frame(FRAME);
    assert!(info(&rt, "home-needs-you").is_none(), "nothing needs the user now");
}

#[test]
fn the_empty_loading_and_error_states_say_so() {
    let rt = home(Scene::Empty);
    assert!(info(&rt, "home-empty").is_some() && info(&rt, "home-recent").is_none());
    let rt = home(Scene::Loading);
    assert!(info(&rt, "home-loading").is_some() && info(&rt, "home-empty").is_none());
    let rt = home(Scene::Error);
    assert!(info(&rt, "home-load-error").is_some() && info(&rt, "home-empty").is_none());
}

#[test]
fn the_banner_shows_fleets_reason_and_opens_setup() {
    let mut rt = home(Scene::Mock);
    assert!(info(&rt, "home-harness-banner").is_none());
    let mut rt2 = home(Scene::Harness);
    assert!(info(&rt2, "home-harness-banner").is_some() && info(&rt2, "home-harness-reason").is_some());
    click(&mut rt2, "home-harness-setup");
    assert!(rt2.state.harness_setup.is_open);
    rt.frame(FRAME);
}

#[test]
fn new_session_goes_to_the_new_session_page() {
    let mut rt = home(Scene::Three);
    click(&mut rt, "home-new-session");
    assert!(rt.state.on_new_session_page());
}
