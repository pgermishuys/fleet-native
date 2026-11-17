//! Component 16, piece T: Settings → Labs. Fleet's nine `LabsSection.test.ts` cases under their names, against a fake Fleet that
//! answers `GET /api/preferences` and records `PUT /api/preferences/{key}`; then the wording of every row, the adds lines and
//! where Open goes.
use crate::fleet::settings::switches as sw;
use crate::mock::machines::Case;
use crate::tests::click;
use crate::tests_settings_general::{Rig, rig_on};
use crate::{Rail, fleet::settings_nav::Page};

fn rig(prefs: &[(&str, &str)]) -> Rig {
    rig_on("labs", prefs)
}

/// The same with other machines listed.
fn rig_with_machines(prefs: &[(&str, &str)]) -> Rig {
    let mut r = rig(prefs);
    r.rt.update(|s, _| s.show_machines_only(Case::Several));
    r.frames();
    r
}

// ─── LabsSection.test.ts ────────────────────────────────────────────────────────

#[test]
fn doesn_t_offer_to_keep_machines_live_with_only_this_one() {
    let r = rig(&[]);
    assert!(r.info("labs-section").is_some());
    assert!(r.info("live-machines-switch").is_none());
    assert!(!r.words().contains("Keep every machine live"));
}

#[test]
fn offers_it_off_once_there_s_another_machine_and_saves_the_choice() {
    let mut r = rig_with_machines(&[]);
    assert_eq!(r.info("live-machines-switch").unwrap().label.as_deref(), Some("Keep every machine live"));
    assert!(!r.on(sw::LIVE_MACHINES), "off by default");
    click(&mut r.rt, "live-machines-switch");
    r.wait_put("LiveMachines", "true");
    assert!(r.on(sw::LIVE_MACHINES));
}

#[test]
fn has_no_github_copilot_switch_copilot_is_a_harness_like_the_others() {
    let r = rig_with_machines(&[]);
    assert!(!r.words().contains("Copilot"));
}

#[test]
fn has_no_updates_in_machines_switch_machine_updates_are_always_in_updates_and_about() {
    let r = rig_with_machines(&[]);
    assert!(!r.words().contains("Updates in Machines"));
}

#[test]
fn has_no_mods_switch_mods_is_a_plugin_switched_in_settings_plugins() {
    let r = rig_with_machines(&[]);
    assert!(r.info("mods-switch").is_none());
    assert!(!r.words().contains("Mods"));
}

#[test]
fn has_no_dreaming_row_dreaming_lives_in_memory() {
    let r = rig_with_machines(&[]);
    assert!(!r.words().contains("Dreaming"));
}

#[test]
fn holds_the_workflows_switch_which_saves_the_workflows_preference() {
    let mut r = rig(&[]);
    assert_eq!(r.info("workflows-switch").unwrap().label.as_deref(), Some("Enable workflows"));
    click(&mut r.rt, "workflows-switch");
    r.wait_put("Workflows", "true");
    assert!(r.on(sw::WORKFLOWS));
    assert!(r.rt.state.rail_list().iter().any(|e| e.rail == Rail::Workflows), "the rail follows at once");
}

#[test]
fn won_t_turn_on_search_older_notes_while_memory_is_off() {
    let mut r = rig(&[]);
    assert!(r.info("memory-search-switch").unwrap().opacity < 1.0);
    click(&mut r.rt, "memory-search-switch");
    r.frames();
    assert!(r.puts().is_empty(), "nothing is sent");
    assert!(!r.on(sw::MEMORY_SEARCH));
    assert!(r.words().contains("Needs Memory on. Sessions read a short list of notes and search the rest."));
}

#[test]
fn saves_the_search_older_notes_choice_once_memory_is_on() {
    let mut r = rig(&[("Memory", "true")]);
    assert!(r.info("memory-search-switch").unwrap().opacity >= 1.0);
    assert!(!r.words().contains("Needs Memory on. Sessions"));
    click(&mut r.rt, "memory-search-switch");
    r.wait_put("MemorySearch", "true");
    assert!(r.on(sw::MEMORY_SEARCH));
}

// ─── ours ───────────────────────────────────────────────────────────────────────

#[test]
fn a_search_switch_that_is_on_can_still_be_turned_off_while_memory_is_off() {
    let mut r = rig(&[("MemorySearch", "true")]);
    assert!(r.info("memory-search-switch").unwrap().opacity >= 1.0);
    click(&mut r.rt, "memory-search-switch");
    r.wait_put("MemorySearch", "false");
}

#[test]
fn every_row_has_its_words_its_switch_name_and_its_own_key() {
    let mut r = rig_with_machines(&[]);
    let rows: [(&str, &str, &str, &str, &str); 5] = [
        (
            "live-machines",
            "Other machines update as they change, not every 15 seconds.",
            "Other machines' sessions update as they change, instead of every 15 seconds, and open here.",
            "live-machines-switch",
            "Keep every machine live",
        ),
        (
            "jobs",
            "One queue; Fleet starts each job wherever a seat is free.",
            "One queue of jobs. Fleet starts each on whichever of your machines has a free seat. Give machines seats in Settings → Machines.",
            "jobs-switch",
            "Enable Jobs",
        ),
        (
            "mission-control",
            "One screen for every running agent. It becomes the first screen.",
            "One screen for every running agent on every machine: what needs you, what each is doing right now, and a nudge box. It becomes the first screen when Fleet opens.",
            "mission-control-switch",
            "Enable Mission control",
        ),
        (
            "memory-search",
            "Needs Memory on. Sessions read a short list of notes and search the rest.",
            "Every session reads 15 notes about its repository and 8 about the machine, and finds the rest with a search tool when it starts on a new area. A new note goes in the short list and the least used one moves down to search. Nothing is deleted, and turning this off puts every note back in the prompt. A weaker model may never search, so this stays here until it has been checked on every harness. Needs Memory on.",
            "memory-search-switch",
            "Enable searching older notes",
        ),
        (
            "workflows",
            "Workflows in the rail; Fleet runs their steps and stops where one asks you.",
            "Show Workflows in the rail, and let Fleet run workflows: it moves between steps and stops where a step asks you. Off: no runner, no step tool, nothing changes.",
            "workflows-switch",
            "Enable workflows",
        ),
    ];
    let page = r.words();
    assert!(page.contains("Experiments") && page.contains("Each is off until you turn it on"));
    for (id, summary, more, switch, label) in rows {
        assert!(page.contains(summary), "{id}'s summary");
        assert_eq!(r.info(switch).unwrap().label.as_deref(), Some(label), "{id}'s switch");
        click(&mut r.rt, &format!("setting-more-{id}"));
        assert_eq!(r.text(&format!("setting-more-text-{id}")), more, "{id}'s More text");
    }
    // The live row says this app keeps no feeds yet; it has no adds line, and nothing has one while everything is off.
    assert!(r.info("live-machines-later").is_some());
    for id in ["jobs", "mission-control", "memory-search", "workflows", "live-machines"] {
        assert!(r.info(&format!("setting-adds-line-{id}")).is_none(), "{id} adds nothing while off");
    }
}

#[test]
fn a_row_says_what_it_adds_only_while_its_switch_is_on() {
    let r = rig(&[
        ("Jobs", "true"),
        ("features.mission-control.enabled", "true"),
        ("Workflows", "true"),
        ("Memory", "true"),
        ("MemorySearch", "true"),
    ]);
    let adds = |id: &str| r.text(&format!("setting-adds-{id}"));
    assert!(adds("jobs").contains("Jobs in the rail, seats on each machine"));
    assert!(adds("mission-control").contains("Fleet in the rail, the first screen"));
    assert!(adds("memory-search").contains("Two groups in Memory"));
    assert!(adds("workflows").contains("Workflows in the rail"));
    // Search older notes adds its groups only while Memory is on too.
    let r = rig(&[("MemorySearch", "true")]);
    assert!(r.info("setting-adds-line-memory-search").is_none());
}

#[test]
fn open_goes_to_the_place_a_switch_adds_and_leaves_the_switch_alone() {
    let cases = [
        ("jobs", "Jobs", Rail::Jobs),
        ("mission-control", "features.mission-control.enabled", Rail::Fleet),
        ("workflows", "Workflows", Rail::Workflows),
    ];
    for (id, key, rail) in cases {
        let mut r = rig(&[(key, "true")]);
        click(&mut r.rt, &format!("setting-open-{id}"));
        assert_eq!(r.rt.state.rail, rail, "{id}");
        assert_eq!(r.rt.state.pickers.prefs.get(key), Some("true"), "{id} stays on");
    }
    let mut r = rig(&[("Memory", "true"), ("MemorySearch", "true")]);
    click(&mut r.rt, "setting-open-memory-search");
    assert_eq!((r.rt.state.rail, r.rt.state.settings.page.clone()), (Rail::Settings, Page::Memory));
}

#[test]
fn the_page_reads_the_preferences_each_time_labs_opens() {
    let mut r = rig(&[("Jobs", "true")]);
    r.fake.lock().unwrap().prefs.insert("Jobs".into(), "false".into());
    r.rt.update(|s, _| s.open_settings("labs", None));
    r.wait("the new value", |s| !sw::JOBS.enabled(&s.pickers.prefs));
}

#[test]
fn a_refused_save_puts_the_switch_back() {
    let mut r = rig(&[]);
    r.fake.lock().unwrap().refuse.push("Jobs".into());
    click(&mut r.rt, "jobs-switch");
    r.wait("the switch to come back", |s| !s.setting_saving("Jobs") && !sw::JOBS.enabled(&s.pickers.prefs));
}
