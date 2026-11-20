//! Component 16, piece AD: Settings → Plugins. Fleet's `PluginsPages.test.ts`, `plugin-switch.test.ts` and
//! `plugin-preference.test.ts` cases under their names, over the fixed table of three plugins, against a fake Fleet.
use crate::fleet::rails::{DISABLED_PLUGINS_KEY, plugin_is_off};
use crate::fleet::settings::plugins as p;
use crate::fleet::settings_nav::Page;
use crate::tests::click;
use crate::tests_settings_general::{Rig, rig_on};

fn rig(page: &'static str, prefs: &[(&str, &str)]) -> Rig {
    rig_on(page, prefs)
}

/// The list write has reached Fleet and come back into the preferences (a plugin switch has no saving spinner to wait on).
fn settled(r: &mut Rig, value: &str) {
    let v = value.to_string();
    r.wait_put(DISABLED_PLUGINS_KEY, value);
    r.wait("the saved list to be adopted", move |s| s.pickers.prefs.get(DISABLED_PLUGINS_KEY) == Some(v.as_str()));
    r.frames();
}

fn list(r: &Rig) -> Vec<String> {
    p::parse_disabled(r.rt.state.pickers.prefs.get(DISABLED_PLUGINS_KEY)).into_iter().collect()
}

// ─── PluginsPages.test.ts ───────────────────────────────────────────────────────

#[test]
fn lists_each_plugin_with_its_description_and_adds_to_chips_from_its_contribution_points() {
    let r = rig("plugins", &[]);
    for (id, areas) in [("github", "Rail,Settings,Folder picker,Board"), ("mods", "Settings"), ("built-in-skills", "Settings")] {
        assert!(r.info(&format!("plugin-row-{id}")).is_some(), "{id}");
        let chips: Vec<String> = p::find(&r.rt.state.places, id).unwrap().areas().iter().map(|a| a.label().to_string()).collect();
        assert_eq!(chips.join(","), areas);
    }
    assert!(r.words().contains("Pull requests, issues and checks beside your sessions."));
    assert!(r.words().contains("Agents write small add-ons for Fleet; you review each one."));
    assert!(r.words().contains("Each switch adds or removes everything below"));
    assert!(r.info("plugin-adds-github-Folder picker").is_some());
}

#[test]
fn an_empty_list_says_no_plugins_are_installed() {
    let mut r = rig("plugins", &[]);
    r.rt.update(|s, _| s.settings.plugins_none = true);
    r.frames();
    assert!(r.info("plugins-empty").is_some());
    assert!(r.words().contains("No plugins are installed"));
    assert!(r.info("plugin-row-github").is_none());
}

#[test]
fn switches_a_plugin_off_and_on_from_its_row() {
    let mut r = rig("plugins", &[]);
    assert!(r.rt.state.plugin_enabled("github"));
    click(&mut r.rt, "plugin-switch-github");
    settled(&mut r, "[\"github\"]");
    assert!(plugin_is_off(&r.rt.state.pickers.prefs, "github"));
    assert!(!r.rt.state.plugin_enabled("github"));
    click(&mut r.rt, "plugin-switch-github");
    settled(&mut r, "[]");
    assert!(!plugin_is_off(&r.rt.state.pickers.prefs, "github"));
}

#[test]
fn opens_a_plugin_s_page_from_its_name() {
    let mut r = rig("plugins", &[]);
    click(&mut r.rt, "plugin-name-github");
    r.frames();
    assert_eq!(r.rt.state.settings.page, Page::Plugin("github".into()));
    assert!(r.info("plugin-page-github").is_some());
    click(&mut r.rt, "plugin-breadcrumb-plugins");
    r.frames();
    assert_eq!(r.rt.state.settings.page, Page::Plugins);
}

#[test]
fn shows_a_plugin_s_own_settings_and_what_it_adds_and_replaces_the_settings_with_a_note_when_it_is_off() {
    let mut r = rig("plugin:github", &[]);
    assert!(r.info("plugin-config-github").is_some());
    assert!(r.info("plugin-off-note").is_none());
    assert!(r.words().contains("4 things"));
    assert!(r.words().contains("GitHub (opens /github)"));
    assert!(r.words().contains("Built in to Fleet. It ships with the app and updates with it."));
    click(&mut r.rt, "plugin-page-switch");
    settled(&mut r, "[\"github\"]");
    assert!(r.info("plugin-config-github").is_none());
    assert!(r.words().contains("GitHub is off. Its rail icon, settings and pickers are gone until you turn it on."));
    assert!(r.words().contains("4 things"), "what it adds stays");
}

#[test]
fn says_so_when_no_plugin_has_that_id() {
    let r = rig("plugin:nothing", &[]);
    assert!(r.words().contains("No plugin called \"nothing\" is installed"));
    assert_eq!(r.rt.state.settings.page.title(), "Plugin");
}

// ─── plugin-switch.test.ts / plugin-preference.test.ts ──────────────────────────

#[test]
fn removes_everything_a_plugin_adds_when_it_is_off_and_restores_it_when_it_is_on() {
    let mut r = rig("plugins", &[(DISABLED_PLUGINS_KEY, "[\"github\"]")]);
    let rails = |r: &Rig| r.rt.state.rail_list().iter().any(|d| d.label == "GitHub");
    assert!(!rails(&r));
    assert_eq!(p::count_on(&r.rt.state.places, &r.rt.state.pickers.prefs), 1, "Built-in skills only");
    assert!(p::children(&r.rt.state.places, &r.rt.state.pickers.prefs).iter().all(|c| c.id != "github"));
    click(&mut r.rt, "plugin-switch-github");
    settled(&mut r, "[]");
    assert!(rails(&r));
}

#[test]
fn never_switches_off_a_plugin_without_a_switch() {
    let mut r = rig("plugins", &[]);
    assert!(r.info("plugin-switch-built-in-skills").is_none());
    r.rt.update(|s, _| s.set_plugin_enabled("built-in-skills", false));
    r.frames();
    assert!(r.puts().is_empty());
    assert!(r.rt.state.plugin_enabled("built-in-skills"));
}

#[test]
fn shows_plugins_as_on_when_the_preferences_can_t_be_read() {
    let r = rig("plugins", &[(DISABLED_PLUGINS_KEY, "not json")]);
    assert!(r.rt.state.plugin_enabled("github"));
    let r = rig("plugins", &[(DISABLED_PLUGINS_KEY, "{\"a\":1}")]);
    assert!(r.rt.state.plugin_enabled("github"));
}

#[test]
fn lets_another_device_s_change_win_over_this_copy() {
    // This copy is stale: another device switched "other" off after it was read. Switching GitHub off keeps "other".
    let mut r = rig("plugins", &[]);
    r.fake.lock().unwrap().prefs.insert(DISABLED_PLUGINS_KEY.into(), "[\"other\"]".into());
    click(&mut r.rt, "plugin-switch-github");
    settled(&mut r, "[\"github\",\"other\"]");
    assert_eq!(list(&r), vec!["github".to_string(), "other".to_string()]);
    assert_eq!(r.puts().len(), 1);
}

#[test]
fn a_refused_save_puts_the_switch_back() {
    let mut r = rig("plugins", &[]);
    r.fake.lock().unwrap().refuse.push(DISABLED_PLUGINS_KEY.into());
    click(&mut r.rt, "plugin-switch-github");
    r.wait("the save to be refused and the switch put back", |s| !plugin_is_off(&s.pickers.prefs, "github") && s.pickers.prefs.fetched);
    assert!(r.rt.state.plugin_enabled("github"));
}

// ─── Mods, the status, the menu ─────────────────────────────────────────────────

#[test]
fn the_mods_switch_is_the_mods_preference_not_the_list() {
    let mut r = rig("plugins", &[(DISABLED_PLUGINS_KEY, "[\"mods\"]"), ("Mods", "true")]);
    assert!(r.rt.state.plugin_enabled("mods"), "the Mods preference decides, whatever plugins.disabled says");
    click(&mut r.rt, "plugin-switch-mods");
    r.wait_put("Mods", "false");
    assert!(!r.rt.state.plugin_enabled("mods"));
    assert!(r.puts().iter().all(|(k, _)| k != DISABLED_PLUGINS_KEY));
    assert!(r.info("plugin-panel-mods").is_none(), "off with nothing to say: the panel slot is empty (tests_settings_mods has the rest)");
}

#[test]
fn the_page_line_says_on_or_off_and_the_status_while_it_is_on() {
    let mut r = rig("plugin:github", &[]);
    r.rt.update(|s, _| s.plugins.statuses = vec![("github".into(), "error".into())]);
    r.frames();
    let words = r.words();
    assert!(words.contains("On") && words.contains(" \u{b7} Needs attention"));
    let r = rig("plugin:built-in-skills", &[]);
    assert!(r.info("plugin-page-switch").is_none());
    assert!(r.words().contains("Skills that come with Fleet"));
}

#[test]
fn the_menu_counts_the_plugins_that_are_on_folds_and_lists_each_one_s_page() {
    let mut r = rig("plugins", &[("Mods", "true")]);
    assert_eq!(r.text("settings-nav-count"), "3");
    for id in ["github", "mods", "built-in-skills"] {
        assert!(r.info(&format!("settings-nav-plugin:{id}")).is_some(), "{id}");
    }
    assert_eq!(r.info("settings-nav-fold").unwrap().label.as_deref(), Some("Fold Plugins"));
    click(&mut r.rt, "settings-nav-fold");
    r.frames();
    assert!(r.info("settings-nav-plugin:github").is_none());
    assert_eq!(r.info("settings-nav-fold").unwrap().label.as_deref(), Some("Unfold Plugins"));
    click(&mut r.rt, "settings-nav-plugins");
    r.frames();
    click(&mut r.rt, "settings-nav-fold");
    r.frames();
    click(&mut r.rt, "settings-nav-plugin:github");
    r.frames();
    assert_eq!(r.rt.state.settings.page, Page::Plugin("github".into()));
}

#[test]
fn an_off_plugin_has_no_child_in_the_menu_and_the_count_drops() {
    let r = rig("plugins", &[(DISABLED_PLUGINS_KEY, "[\"github\"]")]);
    assert_eq!(r.text("settings-nav-count"), "1");
    assert!(r.info("settings-nav-plugin:github").is_none());
}

#[test]
fn it_is_unfolded_only_while_one_of_its_pages_is_open_unless_folded_by_hand() {
    let r = rig("general", &[]);
    assert!(r.info("settings-nav-fold").unwrap().expanded != Some(true));
    assert!(r.info("settings-nav-plugin:github").is_none());
}
