//! `components/layout/__tests__/rails.test.ts` and `ContextPanel.test.ts` under their own names (component 9, piece A).
//! Fleet's mount-and-click becomes a headless runtime with the preferences set; its `useLocation` pathname becomes
//! `rail_for_path`, because this app has no address bar: the rule is the same function the window follows.
//! `stores/__tests__/sidebar.test.ts` "tracks the active rail and panel visibility" and "persists both panel states"
//! are `fleet::tests::settings` (6F); here they are checked again through the window's own Ctrl/Cmd B.
use crate::fleet::rails::{self, Inputs, Placement, RailDef};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;
use serde_json::json;

const LABS: [(&str, &str, &str, Rail); 4] = [
    ("Fleet", "/fleet", rails::FLEET_KEY, Rail::Fleet),
    ("Board", "/board", rails::BOARD_KEY, Rail::Board),
    ("Workflows", "/workflows", rails::WORKFLOWS_KEY, Rail::Workflows),
    ("Jobs", "/jobs", rails::JOBS_KEY, Rail::Jobs),
];

fn with(keys: &[&str]) -> Runtime<State> {
    let mut rt = app();
    set(&mut rt, keys);
    rt
}

fn set(rt: &mut Runtime<State>, keys: &[&str]) {
    let body: serde_json::Map<String, serde_json::Value> = keys.iter().map(|k| ((*k).to_string(), json!("true"))).collect();
    rt.update(|s, _| s.pickers.prefs.adopt(&body.into()));
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// `railLabels`: the rail buttons in the order they are drawn, found by their keys.
fn labels(rt: &Runtime<State>) -> Vec<&'static str> {
    let mut at: Vec<(f32, &'static str)> =
        ["Fleet", "Sessions", "Board", "Workflows", "Jobs", "Automations", "Analytics", "GitHub", "Settings", "Help"]
            .into_iter()
            .filter_map(|l| rt.find(&format!("rail-{}", l.to_lowercase())).map(|(_, y)| (y, l)))
            .collect();
    at.sort_by(|a, b| a.0.total_cmp(&b.0));
    at.into_iter().map(|(_, l)| l).collect()
}

fn def(list: &[RailDef], rail: Rail) -> RailDef {
    *list.iter().find(|d| d.rail == rail).unwrap()
}

fn list_with(keys: &[&str]) -> Vec<RailDef> {
    let mut p = crate::fleet::prefs::Prefs::default();
    p.adopt(&json!(keys.iter().map(|k| ((*k).to_string(), "true")).collect::<std::collections::HashMap<_, _>>()));
    rails::rails(&Inputs::from_prefs(&p))
}

// ─── the icon rail ──────────────────────────────────────────────────────────────────────────────

#[test]
fn lists_every_rail_icon_only_and_no_labs_feature_while_its_off() {
    let rt = app();
    assert_eq!(labels(&rt), ["Sessions", "Automations", "Analytics", "GitHub", "Settings", "Help"]);
    let list = rt.state.rail_list();
    assert!(list.iter().all(|d| !d.off));
    assert!(rt.find("rail-off-dot").is_none());
}

#[test]
fn puts_fleet_first_once_mission_control_is_on_and_opens_it() {
    let mut rt = with(&[rails::FLEET_KEY]);
    assert_eq!(labels(&rt)[..3], ["Fleet", "Sessions", "Automations"]);
    click(&mut rt, "rail-fleet");
    assert_eq!(rt.state.rail, Rail::Fleet);
    assert_eq!(def(&rt.state.rail_list(), Rail::Fleet).to, "/fleet");
}

#[test]
fn adds_board_after_sessions_unmarked_once_its_on() {
    let rt = with(&[rails::BOARD_KEY]);
    assert_eq!(labels(&rt)[..3], ["Sessions", "Board", "Automations"]);
    assert!(rt.find("rail-off-dot").is_none());
}

#[test]
fn the_labs_features_leave_the_rail_while_off() {
    for (label, _, _, _) in LABS {
        assert!(!labels(&app()).contains(&label), "{label} is in the rail while it's off");
    }
}

#[test]
fn the_labs_features_are_in_the_rail_unmarked_while_on_and_open_their_page() {
    for (label, to, key, rail) in LABS {
        let mut rt = with(&[key]);
        assert!(labels(&rt).contains(&label), "{label}");
        assert!(rt.find("rail-off-dot").is_none(), "{label} is marked");
        click(&mut rt, &format!("rail-{}", label.to_lowercase()));
        assert_eq!(rt.state.rail, rail, "{label}");
        assert_eq!(def(&rt.state.rail_list(), rail).to, to);
    }
}

#[test]
fn the_labs_features_still_own_their_address_while_off_so_their_notice_fills_the_context_area() {
    for (label, to, _, rail) in LABS {
        let list = list_with(&[]);
        assert_eq!(rails::rail_for_path(to, &list), Some(rail), "{label}");
        assert!(!rails::has_panel(Some(rail), &list), "{label} keeps a panel beside its notice");
        // On the window: the rail is the one reached by the address, the list of sessions is gone, and nothing else's
        // panel is drawn in its place.
        let mut rt = app();
        rt.update(|s, _| s.rail = rail);
        rt.frame(FRAME);
        assert!(rt.find("session-list").is_none(), "{label}");
        assert!(!rt.state.context_panel_shown() || rail == Rail::Sessions);
    }
}

#[test]
fn a_rail_marked_off_is_drawn_dimmed_with_a_dot_and_only_it_is_marked() {
    let mut rt = app();
    rt.update(|s, _| s.rail_marked_off = vec![Rail::Automations]);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("rail-off-dot").is_some());
    let list = rt.state.rail_list();
    let off: Vec<_> = list.iter().filter(|d| d.off).map(|d| d.label).collect();
    assert_eq!(off, ["Automations"]);
    assert_eq!(rails::tooltip(&def(&list, Rail::Automations)), "Automations · switched off");
    assert_eq!(rails::tooltip(&def(&list, Rail::Sessions)), "Sessions");
}

#[test]
fn a_rail_marked_off_still_opens_its_page() {
    let mut rt = app();
    rt.update(|s, _| s.rail_marked_off = vec![Rail::Automations]);
    rt.frame(FRAME);
    click(&mut rt, "rail-automations");
    assert_eq!(rt.state.rail, Rail::Automations);
}

#[test]
fn makes_a_clicked_rail_active_and_navigates_when_it_has_a_path() {
    let mut rt = app();
    click(&mut rt, "rail-automations");
    assert_eq!(rt.state.rail, Rail::Automations);
    click(&mut rt, "rail-github");
    assert_eq!(rt.state.rail, Rail::GitHub);
    assert_eq!(def(&rt.state.rail_list(), Rail::GitHub).to, "/github");
}

#[test]
fn follows_the_route_to_a_rail_and_to_no_rail_on_a_route_none_owns() {
    let list = list_with(&[]);
    assert_eq!(rails::rail_for_path("/analytics", &list), Some(Rail::Analytics));
    assert_eq!(rails::rail_for_path("/analytics/models", &list), Some(Rail::Analytics));
    // The last rail's panel must not stay behind on /welcome or /repositories.
    for orphan in ["/welcome", "/repositories", "/somewhere-else"] {
        assert_eq!(rails::rail_for_path(orphan, &list), None, "{orphan}");
    }
    let mut rt = app();
    rt.update(|s, _| s.open_repositories_page());
    rt.frame(FRAME);
    assert_eq!(rt.state.active_rail(), None);
}

#[test]
fn keeps_jobs_active_on_a_jobs_page_and_lists_jobs_only_while_its_on() {
    assert!(!labels(&app()).contains(&"Jobs"));
    assert!(labels(&with(&[rails::JOBS_KEY])).contains(&"Jobs"));
    assert_eq!(rails::rail_for_path("/jobs/j1", &list_with(&[])), Some(Rail::Jobs));
}

#[test]
fn keeps_settings_active_on_its_sub_pages() {
    assert_eq!(rails::rail_for_path("/settings/plugins/some-plugin", &list_with(&[])), Some(Rail::Settings));
    // A name that only starts like a rail's address is not it.
    assert_eq!(rails::rail_for_path("/settingsish", &list_with(&[])), None);
}

#[test]
fn highlights_fleet_not_sessions_on_mission_control() {
    assert_eq!(rails::rail_for_path("/fleet", &list_with(&[rails::FLEET_KEY])), Some(Rail::Fleet));
    let mut rt = with(&[rails::FLEET_KEY]);
    click(&mut rt, "rail-fleet");
    assert_eq!(rt.state.active_rail(), Some(Rail::Fleet));
}

#[test]
fn session_pages_are_the_sessions_rail() {
    let list = list_with(&[]);
    assert_eq!(rails::rail_for_path("/", &list), Some(Rail::Sessions));
    assert_eq!(rails::rail_for_path("/sessions/s1", &list), Some(Rail::Sessions));
}

#[test]
fn follows_a_plugins_path_to_the_plugins_rail() {
    let list = list_with(&[]);
    assert_eq!(rails::rail_for_path("/github/pulls", &list), Some(Rail::GitHub));
    // With the plugin switched off nobody owns it.
    let mut p = crate::fleet::prefs::Prefs::default();
    p.adopt(&json!({ "plugins.disabled": "[\"github\"]" }));
    assert_eq!(rails::rail_for_path("/github/pulls", &rails::rails(&Inputs::from_prefs(&p))), None);
}

#[test]
fn marks_the_active_rail() {
    let mut rt = app();
    click(&mut rt, "rail-settings");
    assert_eq!(rt.state.active_rail(), Some(Rail::Settings));
    assert_ne!(rt.state.active_rail(), Some(Rail::Sessions));
}

#[test]
fn places_each_rail_in_its_group_and_in_order() {
    let list = list_with(&[rails::FLEET_KEY, rails::BOARD_KEY, rails::WORKFLOWS_KEY, rails::JOBS_KEY]);
    let names = |v: Vec<RailDef>| v.into_iter().map(|d| d.label).collect::<Vec<_>>();
    assert_eq!(names(rails::top(&list)), ["Fleet", "Sessions", "Board", "Workflows", "Jobs", "Automations"]);
    assert_eq!(names(rails::secondary(&list)), ["Analytics"]);
    // Plugin rails sit above Fleet's own bottom rails.
    assert_eq!(names(rails::bottom(&list)), ["GitHub", "Settings"]);
    assert_eq!(def(&list, Rail::GitHub).placement, Placement::Plugin);
}

#[test]
fn github_is_there_while_its_plugin_is_not_switched_off() {
    let mut rt = app();
    assert!(labels(&rt).contains(&"GitHub"));
    rt.update(|s, _| s.pickers.prefs.adopt(&json!({ "plugins.disabled": "[\"github\"]" })));
    rt.frame(FRAME);
    assert!(!labels(&rt).contains(&"GitHub"));
    // Not a list: nothing is off.
    rt.update(|s, _| s.pickers.prefs.adopt(&json!({ "plugins.disabled": "nonsense" })));
    rt.frame(FRAME);
    assert!(labels(&rt).contains(&"GitHub"));
}

#[test]
fn shows_a_tooltip_beside_the_hovered_button_and_not_the_systems() {
    let mut rt = app();
    let (x, y) = rt.find("rail-automations").unwrap();
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.rail_tip, Some(Rail::Automations));
    let (tx, ty) = rt.find("rail-tip").expect("the tooltip is drawn at once, with no delay");
    assert!(tx > x + 16.0 && (ty - y).abs() < 2.0, "right of the button, centred down it: {tx},{ty} vs {x},{y}");
    rt.pointer_move(900.0, 400.0);
    rt.frame(FRAME);
    assert_eq!(rt.state.rail_tip, None);
}

// ─── the context panel per rail ────────────────────────────────────────────────────────────────

fn shown(rail: Rail, keys: &[&str]) -> bool {
    rails::has_panel(Some(rail), &list_with(keys))
}

#[test]
fn shows_the_rails_own_panel() {
    for rail in [Rail::Sessions, Rail::Analytics, Rail::Automations, Rail::Settings, Rail::GitHub] {
        assert!(shown(rail, &[]), "{rail:?}");
    }
}

#[test]
fn shows_each_labs_panel_only_while_it_is_on_and_none_while_its_off() {
    for (label, _, key, rail) in LABS {
        assert!(!shown(rail, &[]), "{label} off");
        assert!(shown(rail, &[key]), "{label} on");
    }
}

#[test]
fn gives_every_rail_that_can_be_switched_off_no_panel_while_its_off() {
    for d in list_with(&[]).into_iter().filter(|d| d.hidden || d.off) {
        assert!(d.no_panel, "{:?} needs no_panel", d.rail);
        assert!(!rails::has_panel(Some(d.rail), &[d]));
    }
}

#[test]
fn never_borrows_another_views_panel_a_rail_nobody_knows_or_no_rail_shows_none() {
    let list = list_with(&[]);
    assert!(!rails::has_panel(None, &list));
    assert!(!rails::has_panel(Some(Rail::Fleet), &list), "Fleet is not contributed while off... and has no panel then");
    // A rail missing from the list (a plugin that went away) has none.
    let without: Vec<_> = list.into_iter().filter(|d| d.rail != Rail::GitHub).collect();
    assert!(!rails::has_panel(Some(Rail::GitHub), &without));
}

#[test]
fn a_rail_nobody_contributes_redirects_to_sessions() {
    let mut rt = app();
    rt.update(|s, _| s.rail = Rail::GitHub);
    rt.update(|s, _| s.pickers.prefs.adopt(&json!({ "plugins.disabled": "[\"github\"]" })));
    rt.update(|s, _| s.settle_rail());
    assert_eq!(rt.state.rail, Rail::Sessions);
    assert_eq!(rt.state.route, crate::fleet::store::Route::Home, "and to /");
}

#[test]
fn a_rail_nobody_contributes_leaves_a_known_rail_and_the_no_rail_state_alone() {
    let list = list_with(&[]);
    assert_eq!(rails::stale_redirect(Some(Rail::Analytics), &list), None);
    assert_eq!(rails::stale_redirect(None, &list), None);
    let mut rt = app();
    rt.update(|s, _| s.rail = Rail::Analytics);
    rt.update(|s, _| s.settle_rail());
    assert_eq!(rt.state.rail, Rail::Analytics);
    rt.update(|s, _| s.open_welcome_page());
    rt.update(|s, _| s.settle_rail());
    assert_eq!(rt.state.route, crate::fleet::store::Route::Welcome);
}

// ─── the panel's collapse (stores/__tests__/sidebar.test.ts) ────────────────────────────────────

#[test]
fn use_sidebar_store_tracks_the_active_rail_and_panel_visibility() {
    let mut rt = app();
    assert!(rt.state.context_panel_shown());
    rt.update(|s, _| s.set_sidebar_open(false));
    assert!(!rt.state.context_panel_shown());
    rt.update(|s, _| s.set_sidebar_open(true));
    click(&mut rt, "rail-settings");
    assert_eq!(rt.state.active_rail(), Some(Rail::Settings));
    assert!(rt.state.context_panel_shown(), "Settings has a panel");
    rt.update(|s, _| s.open_repositories_page());
    assert!(!rt.state.context_panel_shown(), "no rail owns /repositories");
}

#[test]
fn ctrl_b_collapses_whichever_rails_panel_and_it_is_remembered() {
    let mut rt = app();
    click(&mut rt, "rail-automations");
    assert!(rt.state.context_panel_shown());
    rt.key(rune::KeyEvent { key: rune::Key::Command('b'), mods: rune::Mods::default() });
    rt.frame(FRAME);
    assert!(!rt.state.context_panel_shown());
    assert!(rt.state.sidebar.panel_collapsed(), "written to the settings that persist it");
    // Sessions follows the same switch.
    click(&mut rt, "rail-sessions");
    assert!(!rt.state.context_panel_shown());
}

#[test]
fn keeps_what_the_plugin_poll_answers_and_the_last_answer_when_a_request_fails() {
    let mut p = rails::PluginStatuses::default();
    p.adopt(Ok(&json!({ "statuses": [{ "pluginId": "github", "status": "connected" }, { "status": "x" }] })));
    assert_eq!(p.statuses, [("github".to_string(), "connected".to_string())]);
    p.adopt(Err("HTTP 500".into()));
    assert_eq!(p.error.as_deref(), Some("HTTP 500"));
    assert_eq!(p.statuses.len(), 1, "the last answer stays");
}
