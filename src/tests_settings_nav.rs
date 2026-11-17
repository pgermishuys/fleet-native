//! Component 16, piece A: the Settings shell. Fleet's `use-settings-nav.test.ts` and `SettingsNavPanel.test.ts` cases under their
//! names, and the entry points that used to say "Settings isn't in the native app yet".
use crate::fleet::settings_nav::{Page, SettingsNav, Target, resolve};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;

fn shown() -> Runtime<State> {
    let mut rt = app();
    click(&mut rt, "rail-settings");
    rt.frame(FRAME);
    rt
}

fn exists(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn target(page: Page, anchor: Option<&str>) -> Target {
    Target { page, anchor: anchor.map(str::to_string) }
}

// ─── use-settings-nav.test.ts ───────────────────────────────────────────────────

#[test]
fn old_settings_ids_land_on_their_page() {
    for (old, page, anchor) in [
        ("features", Page::General, None),
        ("workspace", Page::Folders, None),
        ("storage", Page::Folders, Some("storage")),
        ("browser", Page::Permissions, Some("browser")),
        ("tools", Page::Skills, Some("tools")),
        ("mods", Page::Skills, Some("mods")),
        ("system", Page::Updates, None),
        ("weave", Page::Harnesses, Some("weave")),
        ("workflows", Page::Labs, Some("workflows")),
    ] {
        assert_eq!(resolve(old, None), target(page, anchor), "{old}");
    }
}

#[test]
fn sends_an_old_integration_id_to_that_plugins_page_and_keeps_plugins_as_the_overview() {
    assert_eq!(resolve("integration:github", None), target(Page::Plugin("github".into()), None));
    assert_eq!(resolve("plugins", None), target(Page::Plugins, None));
    assert_eq!(resolve("plugin:github", None), target(Page::Plugin("github".into()), None));
}

#[test]
fn leaves_current_ids_alone() {
    assert_eq!(resolve("labs", None), target(Page::Labs, None));
    assert_eq!(resolve("harnesses", None), target(Page::Harnesses, None));
    assert_eq!(resolve("skills", Some("mods")), target(Page::Skills, Some("mods")));
}

#[test]
fn stores_the_new_page_and_anchor_when_given_an_old_id() {
    let mut nav = SettingsNav::default();
    nav.set("storage", None);
    assert_eq!((nav.page.clone(), nav.anchor.clone()), (Page::Folders, Some("storage".into())));
    nav.set("memory", None);
    assert_eq!(nav.anchor, None);
}

#[test]
fn opens_harnesses_on_a_harnesss_tab_at_a_group() {
    let mut s = State::default();
    s.open_harness("opencode2", Some("may-do"));
    assert_eq!(s.settings.page, Page::Harnesses);
    assert_eq!(s.settings.harness.as_deref(), Some("opencode2"));
    assert_eq!(s.settings.anchor.as_deref(), Some("may-do"));
    assert_eq!(s.rail, Rail::Settings);
}

#[test]
fn sends_the_old_weave_id_to_harnesses_at_its_weave_group_leaving_the_tab_to_be_chosen() {
    let mut s = State::default();
    s.open_harness("pi", None);
    s.settings.set("weave", None);
    assert_eq!(
        (s.settings.page.clone(), s.settings.anchor.clone(), s.settings.harness.clone()),
        (Page::Harnesses, Some("weave".into()), None)
    );
}

#[test]
fn opens_memory_with_the_repository() {
    let mut s = State::default();
    s.open_memory("/r/demo");
    assert_eq!(s.settings.page, Page::Memory);
    assert_eq!(s.settings.repository.as_deref(), Some("/r/demo"));
}

#[test]
fn goes_to_machines_at_the_add_a_machine_row_and_asks_for_the_form() {
    let mut s = State::default();
    s.open_add_machine();
    assert_eq!((s.settings.page.clone(), s.settings.anchor.clone()), (Page::Machines, Some("add-machine".into())));
    assert!(s.add_machine_asked);
}

// ─── SettingsNavPanel.test.ts ───────────────────────────────────────────────────

#[test]
fn groups_the_pages_under_you_agents_work_and_fleet_and_has_no_features_page() {
    let rt = shown();
    for group in ["you", "agents", "work", "fleet", "plugins"] {
        assert!(exists(&rt, &format!("settings-group-{group}")), "{group}");
    }
    for page in [
        "general",
        "appearance",
        "harnesses",
        "permissions",
        "skills",
        "memory",
        "credentials",
        "folders",
        "machines",
        "labs",
        "updates",
        "plugins",
    ] {
        assert!(exists(&rt, &format!("settings-nav-{page}")), "{page}");
    }
    assert!(!exists(&rt, "settings-nav-features"));
}

#[test]
fn has_no_mods_entry_of_its_own_mods_is_a_tab_of_skills_and_tools() {
    let rt = shown();
    assert!(!exists(&rt, "settings-nav-mods"));
    assert!(!exists(&rt, "settings-nav-integrations"));
}

// ─── the shell ──────────────────────────────────────────────────────────────────

#[test]
fn the_settings_rail_draws_the_menu_beside_the_page_and_opens_on_general() {
    let rt = shown();
    assert!(exists(&rt, "settings-menu") && exists(&rt, "settings-detail") && exists(&rt, "sidebar-gutter"));
    assert!(!exists(&rt, "session-list"), "the session list belongs to Sessions");
    assert_eq!(rt.state.settings.page, Page::General);
    assert!(exists(&rt, "general-section"));
}

#[test]
fn a_menu_entry_opens_its_page_with_its_title_and_line_and_the_note() {
    let mut rt = shown();
    for page in [
        Page::Harnesses,
        Page::Permissions,
        Page::Skills,
        Page::Memory,
        Page::Credentials,
        Page::Folders,
        Page::Labs,
        Page::Updates,
        Page::Plugins,
    ] {
        click(&mut rt, &format!("settings-nav-{}", page.id()));
        rt.frame(FRAME);
        assert_eq!(rt.state.settings.page, page);
        let words: String = rt.elements().iter().filter_map(|e| e.text.clone()).collect();
        assert!(words.contains(page.title()) && words.contains(page.line()), "{}", page.title());
        assert!(words.contains(&format!("{} isn't in the native app yet.", page.title())));
    }
}

#[test]
fn machines_is_component_17s_page_and_has_no_note() {
    let mut rt = shown();
    click(&mut rt, "settings-nav-machines");
    rt.frame(FRAME);
    assert!(exists(&rt, "machines-page-body") && !exists(&rt, "settings-not-yet"));
}

#[test]
fn the_rail_remembers_the_last_page_while_the_app_runs() {
    let mut rt = shown();
    click(&mut rt, "settings-nav-labs");
    click(&mut rt, "rail-sessions");
    click(&mut rt, "rail-settings");
    assert_eq!(rt.state.settings.page, Page::Labs);
}

#[test]
fn the_settings_panel_collapses_with_the_session_lists_switch() {
    let mut rt = shown();
    rt.update(|s, _| s.set_sidebar_open(false));
    rt.frame(FRAME);
    assert!(!exists(&rt, "settings-menu") && exists(&rt, "settings-detail"));
}

// ─── entry points ───────────────────────────────────────────────────────────────

#[test]
fn entry_points_open_their_page_and_show_no_waiting_toast() {
    let mut rt = app();
    for (page, go) in [
        (Page::Machines, (|s: &mut State| s.open_add_machine()) as fn(&mut State)),
        (Page::Folders, |s| s.open_settings("folders", None)),
        (Page::Plugin("github".into()), |s| s.connect_github()),
        (Page::Updates, |s| s.open_settings("updates", None)),
    ] {
        rt.update(|s, _| s.rail = Rail::Sessions);
        rt.update(|s, _| go(s));
        assert_eq!((rt.state.rail, rt.state.settings.page.clone()), (Rail::Settings, page));
        assert!(rt.state.ui.toast.is_none());
    }
}

#[test]
fn the_palettes_add_a_machine_and_open_plugins_open_their_pages() {
    let mut rt = app();
    rt.update(|s, tx| crate::ui::commands::run(s, tx, "add-machine"));
    assert_eq!((rt.state.settings.page.clone(), rt.state.add_machine_asked), (Page::Machines, true));
    rt.update(|s, tx| crate::ui::commands::run(s, tx, "open-marketplace-panel"));
    assert_eq!(rt.state.settings.page, Page::Plugins);
}
