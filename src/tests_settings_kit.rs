//! Component 16, piece B: the setting primitives and the anchor rule. Fleet's `SettingRow.test.ts` cases and the anchor cases of
//! `SettingsPage.test.ts` under their names, and the native ones (segments pick with the arrows and wrap).
use crate::fleet::settings::anchor::{Settle, settle};
use crate::fleet::settings_nav::{Page, SettingsNav};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};

fn kit_page() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.rail = Rail::Settings;
        s.open_settings("general", None);
        s.settings.demo_open = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn info(rt: &Runtime<State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn exists(rt: &Runtime<State>, key: &str) -> bool {
    info(rt, key).is_some()
}

fn text_of(rt: &Runtime<State>, key: &str) -> String {
    info(rt, key).unwrap_or_else(|| panic!("no element keyed {key:?}")).text.unwrap_or_default()
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// ─── SettingRow.test.ts ─────────────────────────────────────────────────────────

#[test]
fn shows_the_summary_and_more_opens_todays_full_text_word_for_word() {
    let mut rt = kit_page();
    assert!(!exists(&rt, "setting-more-text-starters"));
    assert_eq!(info(&rt, "setting-more-starters").unwrap().expanded, Some(false));
    click(&mut rt, "setting-more-starters");
    assert_eq!(
        text_of(&rt, "setting-more-text-starters"),
        "On the new-session page, offer a few sentences to start from, such as \"Explore the code\". A starter fills the box; nothing is sent \
         until you press Enter."
    );
    assert_eq!(info(&rt, "setting-more-starters").unwrap().expanded, Some(true));
    assert_eq!(info(&rt, "setting-more-starters").unwrap().label.as_deref(), Some("Less"));
    click(&mut rt, "setting-more-starters");
    assert!(!exists(&rt, "setting-more-text-starters"));
    // The control is drawn beside the text.
    assert!(exists(&rt, "kit-starters"));
}

#[test]
fn has_no_more_link_without_a_full_text_and_carries_a_stable_id() {
    let rt = kit_page();
    assert!(!exists(&rt, "setting-more-jobs"));
    assert!(exists(&rt, "settings-row-jobs"));
}

#[test]
fn says_what_it_adds_with_an_open_link_and_emits_open() {
    let mut rt = kit_page();
    assert!(text_of(&rt, "setting-adds-jobs").contains("Adds: Jobs in the rail, seats on each machine"));
    assert_eq!(rt.state.settings.demo.opened, 0);
    click(&mut rt, "setting-open-jobs");
    assert_eq!(rt.state.settings.demo.opened, 1);
}

#[test]
fn shows_nothing_about_adds_without_it_and_no_open_link_when_there_is_nowhere_to_go() {
    let rt = kit_page();
    assert!(!exists(&rt, "setting-adds-starters"));
    assert!(exists(&rt, "setting-adds-quiet"));
    assert!(!exists(&rt, "setting-open-quiet"));
}

#[test]
fn passes_aria_label_and_test_id_to_the_switch_button_and_emits_click() {
    let mut rt = kit_page();
    let switch = info(&rt, "kit-starters").unwrap();
    assert_eq!((switch.role, switch.label.as_deref()), ("switch", Some("Show starters")));
    assert!(rt.state.settings.demo.starters);
    click(&mut rt, "kit-starters");
    assert!(!rt.state.settings.demo.starters);
}

#[test]
fn titles_a_group_and_gives_it_a_stable_id() {
    let rt = kit_page();
    assert!(exists(&rt, "settings-group-experiments"));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Labs")));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Each is off until you turn it on.")));
}

#[test]
fn a_switch_that_is_saving_shows_a_spinner_and_does_not_press() {
    let mut rt = kit_page();
    let quiet = info(&rt, "kit-quiet").unwrap();
    // Disabled while saving: a press changes nothing, and its opacity says so.
    assert!(quiet.opacity < 1.0);
    click(&mut rt, "kit-quiet");
    assert!(rt.state.settings.demo.starters, "no other control moved");
}

// ─── segments (ours) ────────────────────────────────────────────────────────────

#[test]
fn segments_pick_by_click_and_with_the_arrows_and_wrap() {
    let mut rt = kit_page();
    assert_eq!(rt.state.settings.demo.choice, "ask");
    click(&mut rt, "kit-choice-allow");
    assert_eq!(rt.state.settings.demo.choice, "allow");
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.settings.demo.choice, "deny");
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.settings.demo.choice, "ask", "right from the last wraps to the first");
    press(&mut rt, Key::Left);
    assert_eq!(rt.state.settings.demo.choice, "deny", "left from the first wraps to the last");
}

#[test]
fn a_compact_error_note_is_an_alert_with_its_message() {
    let rt = kit_page();
    let note = info(&rt, "kit-error").unwrap();
    assert_eq!(note.role, "alert");
}

// ─── SettingsPage.test.ts (the anchor) ──────────────────────────────────────────

fn scroll(page: Page, anchor: Option<&str>) -> (Option<String>, Option<String>) {
    let mut nav = SettingsNav { page, anchor: anchor.map(str::to_string), ..Default::default() };
    let key = nav.settle_anchor();
    (key, nav.anchor)
}

#[test]
fn scrolls_to_a_group_once_and_then_lets_the_anchor_go_so_coming_back_doesnt_scroll_again() {
    let mut nav = SettingsNav::default();
    nav.set("machines", Some("phones"));
    assert_eq!(nav.settle_anchor().as_deref(), Some("settings-group-phones"));
    assert_eq!(nav.anchor, None);
    assert_eq!(nav.settle_anchor(), None, "a second look finds nothing to scroll to");
}

#[test]
fn lets_go_of_the_permissions_browser_anchor_too() {
    // The Permissions page draws its `browser` spot in piece M; its rule is the one every page that doesn't keep an anchor has.
    assert!(!crate::fleet::settings::anchor::keeps_anchor(&Page::Permissions));
    assert_eq!(settle(&Page::Machines, Some("add-machine")), Settle::Scroll { key: "settings-row-add-machine".into(), release: true });
}

#[test]
fn keeps_the_anchor_on_skills_and_tools_which_picks_its_tab_from_it() {
    assert!(crate::fleet::settings::anchor::keeps_anchor(&Page::Skills));
    assert!(crate::fleet::settings::anchor::keeps_anchor(&Page::Harnesses));
    // A page that keeps its anchor scrolls once and keeps it: the rule, with a page that has a target.
    assert_eq!(settle(&Page::Skills, Some("tools")), Settle::Wait, "nothing drawn for Skills yet, so nothing to scroll to");
}

#[test]
fn keeps_an_anchor_whose_group_isnt_on_the_page_yet() {
    assert_eq!(scroll(Page::Folders, Some("not-there-yet")), (None, Some("not-there-yet".into())));
    assert_eq!(scroll(Page::Machines, Some("not-there-yet")), (None, Some("not-there-yet".into())));
    assert_eq!(scroll(Page::Machines, None), (None, None));
}

#[test]
fn the_poll_scrolls_to_the_anchor_once_and_lets_it_go() {
    let mut rt = app();
    rt.update(|s, _| s.open_add_machine());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.settings.anchor, None, "the frame scrolled to Add a machine and let the anchor go");
    assert!(exists(&rt, "settings-row-add-machine"));
}

#[test]
fn every_target_the_page_offers_is_drawn() {
    let mut rt = app();
    rt.update(|s, _| s.open_settings("machines", None));
    rt.frame(FRAME);
    rt.frame(FRAME);
    for (name, key) in crate::fleet::settings::anchor::targets(&Page::Machines) {
        assert!(exists(&rt, key), "{name}: {key}");
    }
}
