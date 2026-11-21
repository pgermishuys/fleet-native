//! Component 20, piece C: the page and the menu. Fleet's `AnalyticsNavPanel.test.ts` (its first three cases, under their
//! names; the fourth waits for the filters and Export, pieces D and E), then the rules the native page keeps: every way in opens
//! Overview, a stale address opens Overview, Ctrl+B hides the menu, and leaving Analytics forgets the machine pick.
use crate::fleet::analytics::sections::{SECTIONS, Section, section_path};
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;

fn page(scene: Scene) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics(true, scene));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

/// The text of a keyed element, or none when it isn't drawn.
fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

#[test]
fn lists_spend_overview_daily_and_breakdown_projects_models_sessions_with_counts() {
    let rt = page(Scene::Menu);
    assert!(shown(&rt, "analytics-group-spend") && shown(&rt, "analytics-group-breakdown"));
    // Overview and Daily have no count; the others have the length of what they list.
    for id in ["overview", "daily"] {
        assert!(shown(&rt, &format!("analytics-section-{id}")));
        assert!(!shown(&rt, &format!("analytics-count-{id}")), "{id} has no count");
    }
    let counts: Vec<Option<usize>> = SECTIONS.iter().map(|x| crate::view::analytics_page::menu::count(&rt.state, *x)).collect();
    assert_eq!(counts, [None, None, Some(5), Some(5), Some(18)]);
    for id in ["projects", "models", "sessions"] {
        assert!(shown(&rt, &format!("analytics-count-{id}")), "{id} shows its count");
    }
    assert!(!shown(&rt, "analytics-section-harness"), "no By harness");
}

#[test]
fn marks_the_section_the_address_is_on() {
    let mut rt = page(Scene::Menu);
    rt.update(|s, _| s.open_analytics_address("/analytics/models"));
    assert_eq!(rt.state.analytics_nav.section, Section::Models);
    assert_eq!(rt.state.analytics_nav.path(), "/analytics/models");
}

#[test]
fn opens_a_section_by_its_own_route_and_does_nothing_for_the_one_already_open() {
    let mut rt = page(Scene::Menu);
    click(&mut rt, "analytics-section-sessions");
    assert_eq!(rt.state.analytics_nav.section, Section::Sessions);
    assert_eq!(rt.state.analytics_nav.path(), section_path(Section::Sessions));
    assert!(shown(&rt, "analytics-body-sessions"), "the page swaps its body");
    click(&mut rt, "analytics-section-overview");
    assert_eq!(rt.state.analytics_nav.path(), "/analytics");
    // Pressing the open one changes nothing.
    click(&mut rt, "analytics-section-overview");
    assert_eq!(rt.state.analytics_nav.section, Section::Overview);
}

#[test]
fn the_rail_the_palette_and_the_sparkline_all_open_overview() {
    let mut rt = page(Scene::Models);
    rt.update(|s, _| s.rail = Rail::Sessions);
    click(&mut rt, "rail-analytics");
    assert_eq!((rt.state.rail, rt.state.analytics_nav.section), (Rail::Analytics, Section::Overview), "the rail");

    rt.update(|s, _| s.open_analytics_section(Section::Daily));
    rt.update(|s, tx| crate::ui::commands::run(s, tx, "nav-analytics"));
    assert_eq!(rt.state.analytics_nav.section, Section::Overview, "the palette");

    rt.update(|s, _| {
        s.open_analytics_section(Section::Projects);
        s.rail = Rail::Sessions;
        s.go(crate::fleet::store::Route::Home);
    });
    rt.frame(FRAME);
    if shown(&rt, "summary-spend") {
        click(&mut rt, "summary-spend");
        assert_eq!((rt.state.rail, rt.state.analytics_nav.section), (Rail::Analytics, Section::Overview), "the sparkline");
    } else {
        rt.update(|s, _| s.press_rail(Rail::Analytics));
        assert_eq!(rt.state.analytics_nav.section, Section::Overview, "the sparkline's own press");
    }
}

#[test]
fn a_stale_address_opens_overview() {
    let mut rt = page(Scene::Models);
    for stale in ["/analytics/harness", "/analytics/", "/analytics"] {
        rt.update(move |s, _| s.open_analytics_address(stale));
        assert_eq!(rt.state.analytics_nav.section, Section::Overview, "{stale}");
    }
}

#[test]
fn ctrl_b_hides_the_menu_and_leaves_the_page_alone() {
    let mut rt = page(Scene::Menu);
    assert!(shown(&rt, "analytics-menu") && shown(&rt, "analytics-detail"));
    rt.key(rune::KeyEvent { key: rune::Key::Command('b'), mods: rune::Mods::default() });
    rt.frame(FRAME);
    assert!(!shown(&rt, "analytics-menu"), "the menu is hidden");
    assert!(shown(&rt, "analytics-detail"), "the page stays");
}

#[test]
fn the_page_has_its_title_and_line_and_the_menu_only_the_machine_switch_when_another_machine_is_listed() {
    let rt = page(Scene::Menu);
    assert!(shown(&rt, "analytics-header") && shown(&rt, "analytics-menu-header"));
    assert!(!shown(&rt, "analytics-machine-switch"), "one machine, no switch");
    let rt = page(Scene::Machines);
    assert!(shown(&rt, "analytics-machine-switch"));
}

#[test]
fn leaving_analytics_forgets_the_machine_pick_and_moving_between_sections_keeps_it() {
    let mut rt = page(Scene::Machines);
    rt.update(|s, _| s.analytics.choice.pick("m-falcon"));
    click(&mut rt, "analytics-section-daily");
    assert_eq!(rt.state.analytics_machine(), "m-falcon", "a section change keeps it");
    click(&mut rt, "rail-sessions");
    click(&mut rt, "rail-analytics");
    assert_eq!(rt.state.analytics_machine(), rt.state.machines.current_key(), "the next visit opens on the machine you're on");
}
