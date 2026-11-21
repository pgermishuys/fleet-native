//! Component 19, piece J: the When chip and its menu. Fleet has no tests of its own for `AutomationWhenPicker.vue`, so these
//! are native: what each row makes (Repeat, Once, the time, the date, a cron, an event), "Use the words in the message", the
//! cron's five-part error, the cron applied by Enter or by leaving its box, focus going back to the box, and the server's
//! event catalog replacing the built-in list. The rows' rules first, as plain functions; then the menu driven on a fake Fleet.
use crate::fleet::automations::schedule::{Rel, When, weekday};
use crate::fleet::automations::when::{CRON_ERROR, Repeat, apply_cron, once, parse_hm, set_date, set_time, time_of};
use crate::fleet::automations::words::event_types;
use crate::mock::automations::NOW_MS;
use crate::tests::{click, press_key};
use crate::tests_automations_page::{Rig, rig};
use crate::view::automations::when::{CRON, DATE, TIME};
use rune::{Key, Mods};

const UTC: &dyn Fn(i64) -> i64 = &|_| 0;

fn weekly(days: &[u8], t: (i64, i64), ambiguous: bool) -> When {
    When::Weekly { days: days.to_vec(), t, ambiguous }
}

fn once_on(date: &str, t: (i64, i64)) -> When {
    When::Once { t, day: None, rel: None, date: Some(date.into()) }
}

// ─── The rows' rules ─────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn each_repeat_row_makes_its_schedule_at_the_time_there_was() {
    let friday_at_ten = weekly(&[5], (10, 30), true);
    let had = Some(&friday_at_ten);
    assert_eq!(Repeat::Daily.make(had, 2), weekly(&[0, 1, 2, 3, 4, 5, 6], (10, 30), false));
    assert_eq!(Repeat::Weekdays.make(had, 2), weekly(&[1, 2, 3, 4, 5], (10, 30), false));
    // The schedule's own day, and the "every week or once?" question answered.
    assert_eq!(Repeat::Weekly.make(had, 2), weekly(&[5], (10, 30), false));
    assert_eq!(Repeat::Hourly.make(had, 2), When::Hours { n: 1 });
    // Nothing yet: 09:00, and the weekly one on today.
    assert_eq!(Repeat::Weekly.make(None, 2), weekly(&[2], (9, 0), false));
    assert_eq!(Repeat::Daily.make(Some(&When::Hours { n: 3 }), 4), weekly(&[0, 1, 2, 3, 4, 5, 6], (9, 0), false));
}

#[test]
fn a_row_is_checked_for_its_own_kind_only() {
    let every_day = weekly(&[0, 1, 2, 3, 4, 5, 6], (9, 0), false);
    let weekdays = weekly(&[1, 2, 3, 4, 5], (9, 0), false);
    let mondays = weekly(&[1], (9, 0), false);
    let checked = |w: &When| Repeat::ALL.iter().filter(|r| r.is(Some(w))).copied().collect::<Vec<_>>();
    assert_eq!(checked(&every_day), vec![Repeat::Daily]);
    assert_eq!(checked(&weekdays), vec![Repeat::Weekdays]);
    assert_eq!(checked(&mondays), vec![Repeat::Weekly]);
    assert_eq!(checked(&When::Hours { n: 2 }), vec![Repeat::Hourly]);
    assert_eq!(checked(&weekly(&[1, 3], (9, 0), false)), Vec::<Repeat>::new());
}

#[test]
fn once_keeps_the_date_it_had_else_takes_tomorrow_at_the_time() {
    // NOW is Tuesday 15 September 2026.
    assert_eq!(once(None, NOW_MS, UTC), once_on("2026-09-16", (9, 0)));
    assert_eq!(once(Some(&weekly(&[1], (7, 15), false)), NOW_MS, UTC), once_on("2026-09-16", (7, 15)));
    // "tomorrow at 8" is already a one-off: its date stays.
    let tomorrow = When::Once { t: (8, 0), day: None, rel: Some(Rel::Tomorrow), date: None };
    assert_eq!(once(Some(&tomorrow), NOW_MS, UTC), once_on("2026-09-16", (8, 0)));
    let fixed = once_on("2026-10-02", (18, 0));
    assert_eq!(once(Some(&fixed), NOW_MS, UTC), fixed);
}

#[test]
fn a_time_change_keeps_the_kind_and_with_nothing_yet_makes_a_weekly_one_on_today() {
    assert_eq!(set_time(Some(&weekly(&[5], (9, 0), true)), "10:30", 2), Some(weekly(&[5], (10, 30), false)));
    assert_eq!(set_time(Some(&once_on("2026-10-02", (9, 0))), "7:05", 2), Some(once_on("2026-10-02", (7, 5))));
    assert_eq!(set_time(Some(&When::Monthly { dom: 3, t: (9, 0) }), "23:59", 2), Some(When::Monthly { dom: 3, t: (23, 59) }));
    assert_eq!(set_time(None, "06:45", 2), Some(weekly(&[2], (6, 45), false)));
    assert_eq!(set_time(Some(&When::Hours { n: 1 }), "06:45", 4), Some(weekly(&[4], (6, 45), false)));
    // Half typed, or not a time: nothing changes.
    for typed in ["", "10", "10:", "10:3", "24:00", "09:60", "ab:cd", "1:2:3"] {
        assert_eq!(set_time(None, typed, 2), None, "{typed:?}");
        assert_eq!(parse_hm(typed), None, "{typed:?}");
    }
}

#[test]
fn a_date_makes_a_one_off_on_that_day_at_the_time() {
    assert_eq!(set_date(Some(&weekly(&[1], (7, 15), false)), "2026-10-05"), Some(once_on("2026-10-05", (7, 15))));
    assert_eq!(set_date(None, "2026-10-05"), Some(once_on("2026-10-05", time_of(None))));
    for typed in ["", "2026-10-5", "2026/10/05", "05-10-2026", "2026-1x-05"] {
        assert_eq!(set_date(None, typed), None, "{typed:?}");
    }
}

#[test]
fn a_cron_needs_five_parts() {
    assert_eq!(apply_cron("  0  9 * *   1-5 "), Ok(Some(When::Cron { expr: "0 9 * * 1-5".into() })));
    assert_eq!(apply_cron("0 9 * *"), Err(CRON_ERROR));
    assert_eq!(apply_cron("0 9 * * 1 2026"), Err(CRON_ERROR));
    assert_eq!(apply_cron("   "), Ok(None));
}

// ─── The menu, on a fake Fleet ───────────────────────────────────────────────────────────────────────────────────────

/// New automation on a fake Fleet, `words` typed, and the event catalog's answer landed.
fn new_with(words: &str) -> Rig {
    let mut r = rig(vec![], vec![]);
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    click(&mut r.rt, "new-automation");
    r.frames();
    let heard = r.home.handle();
    r.wait("the event catalog asked for", move |_| heard("GET", "/api/automations/event-catalog") > 0);
    r.wait("the event catalog", |s| s.when_event_types().len() == 2);
    if !words.is_empty() {
        click(&mut r.rt, "automation-message");
        press_key(&mut r.rt, Key::Text(words.into()), Mods::default());
    }
    r.frames();
    r
}

fn when(r: &Rig) -> Option<When> {
    r.rt.state.with_composer(false, |c, _| c.when())
}

fn chip(r: &Rig) -> String {
    r.rt.elements().iter().find(|e| e.key.as_deref() == Some("automation-when-chip")).and_then(|e| e.label.clone()).unwrap_or_default()
}

fn open_menu(r: &mut Rig) {
    click(&mut r.rt, "automation-when-chip");
    r.frames();
    assert!(r.has("automation-when-pop"), "the menu is open");
}

/// Clicks a box in the menu, selects what's there and types `words` over it.
fn type_in(r: &mut Rig, key: &str, words: &str) {
    click(&mut r.rt, key);
    press_key(&mut r.rt, Key::Command('a'), Mods::default());
    press_key(&mut r.rt, Key::Text(words.into()), Mods::default());
    r.frames();
}

/// The menu is gone, and its panel done fading out: until then its children still answer by key (docs/RUNE.md).
fn closed(r: &mut Rig) -> bool {
    let gone = !r.has("automation-when-pop");
    for _ in 0..30 {
        r.rt.frame(crate::tests::FRAME);
    }
    gone
}

fn focused(r: &Rig, key: &str) -> bool {
    r.rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some(key))
}

#[test]
fn a_repeat_row_takes_over_from_the_words_drops_just_once_and_closes_the_menu() {
    let mut r = new_with("Tidy the changelog on Friday.");
    open_menu(&mut r);
    assert!(r.words().contains("Picked up from your message. Choosing here takes over."));
    assert!(r.words().contains("Fridays"), "the weekly row is the sentence's day");
    press_key(&mut r.rt, Key::Escape, Mods::default());
    assert!(closed(&mut r));
    click(&mut r.rt, "automation-plan-ask");
    r.frames();
    assert!(chip(&r).starts_with("Once"), "{}", chip(&r));
    open_menu(&mut r);

    click(&mut r.rt, "automation-when-weekdays");
    r.frames();
    assert_eq!(when(&r), Some(weekly(&[1, 2, 3, 4, 5], (9, 0), false)));
    assert!(!r.rt.state.automations_nav.draft.force_once, "Just once goes");
    assert!(closed(&mut r), "the menu closed");
    assert!(focused(&r, "automation-message"), "the box has the keys again");
    // The words no longer decide: no highlight, and the menu now offers them back.
    assert_eq!(r.rt.state.automations_nav.draft.highlight(), None);
    open_menu(&mut r);
    assert!(r.has("automation-when-use-message"));
    assert!(!r.words().contains("Picked up from your message."));
}

#[test]
fn use_the_words_in_the_message_gives_the_schedule_back_to_the_sentence() {
    let mut r = new_with("Every Monday at 9, summarise the open PRs");
    open_menu(&mut r);
    click(&mut r.rt, "automation-when-hourly");
    r.frames();
    assert_eq!(chip(&r), "Every hour");
    open_menu(&mut r);
    click(&mut r.rt, "automation-when-use-message");
    r.frames();
    assert_eq!(r.rt.state.automations_nav.draft.manual_when, None);
    assert_eq!(chip(&r), "Mondays 09:00");
    assert!(closed(&mut r));
}

#[test]
fn with_no_schedule_words_the_menu_has_neither_the_note_nor_the_words_row() {
    let mut r = new_with("Summarise the open PRs");
    assert_eq!(chip(&r), "When?");
    open_menu(&mut r);
    assert!(!r.has("automation-when-use-message"));
    assert!(!r.words().contains("Picked up from your message."));
    let today = crate::fleet::automations::schedule::day_name(weekday(NOW_MS, &|ms| crate::conv::time::local_offset_at(ms)));
    assert!(r.words().contains(&format!("{today}s")), "the weekly row is today's day");
    assert!(r.has(TIME), "a time box even with nothing chosen");
    assert!(!r.has(DATE));
}

#[test]
fn once_stays_open_and_asks_for_the_date_which_makes_it_that_day() {
    let mut r = new_with("Every Monday at 9, summarise the open PRs");
    open_menu(&mut r);
    click(&mut r.rt, "automation-when-once");
    r.frames();
    assert!(r.has("automation-when-pop"), "Once leaves the menu open");
    assert!(matches!(when(&r), Some(When::Once { t: (9, 0), date: Some(_), .. })), "{:?}", when(&r));
    assert!(r.has(DATE));
    let tomorrow = crate::fleet::automations::schedule::tomorrow(NOW_MS, &|ms| crate::conv::time::local_offset_at(ms));
    assert_eq!(r.rt.state.automations_nav.when.date.text, tomorrow);

    type_in(&mut r, DATE, "2026-10-05");
    assert_eq!(when(&r), Some(once_on("2026-10-05", (9, 0))));
    assert!(chip(&r).contains("Mon 5 Oct"), "{}", chip(&r));
    // Half a date changes nothing.
    type_in(&mut r, DATE, "2026-10-");
    assert_eq!(when(&r), Some(once_on("2026-10-05", (9, 0))));
}

#[test]
fn a_time_typed_keeps_the_kind_and_stops_asking_every_week_or_once() {
    let mut r = new_with("Tidy the changelog on Friday.");
    assert!(r.texts_of("automation-plan").concat().contains("Every Friday, or just once?"));
    open_menu(&mut r);
    type_in(&mut r, TIME, "10:30");
    assert_eq!(when(&r), Some(weekly(&[5], (10, 30), false)));
    assert!(!r.texts_of("automation-plan").concat().contains("or just once?"));
    assert!(r.has("automation-when-pop"), "the menu stays open while typing");
    assert!(r.words().contains("every week at 10:30"));
}

#[test]
fn a_cron_of_four_parts_says_so_and_five_parts_are_the_schedule() {
    let mut r = new_with("Every Monday at 9, summarise the open PRs");
    open_menu(&mut r);
    assert_eq!(r.rt.state.automations_nav.when.cron.text, "0 9 * * 1", "filled with the schedule's cron");
    type_in(&mut r, CRON, "0 9 * *");
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.frames();
    assert_eq!(r.texts_of("automation-when-cron-error"), vec![CRON_ERROR.to_string()]);
    assert!(r.has("automation-when-pop"));

    type_in(&mut r, CRON, "30 7 * * 1-5");
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.frames();
    assert_eq!(when(&r), Some(When::Cron { expr: "30 7 * * 1-5".into() }));
    assert_eq!(chip(&r), "30 7 * * 1-5");
    assert!(closed(&mut r));
    // Opening again starts clean: no error, and the time box is gone for a cron.
    open_menu(&mut r);
    assert!(r.texts_of("automation-when-cron-error").is_empty());
    assert!(!r.has(TIME));
}

#[test]
fn leaving_the_cron_box_changed_applies_it_and_unchanged_does_nothing() {
    let mut r = new_with("Every Monday at 9, summarise the open PRs");
    open_menu(&mut r);
    // In and out without a change: still the words' schedule.
    click(&mut r.rt, CRON);
    click(&mut r.rt, TIME);
    r.frames();
    assert_eq!(r.rt.state.automations_nav.draft.manual_when, None);
    assert!(r.has("automation-when-pop"));

    type_in(&mut r, CRON, "0 18 * * 5");
    click(&mut r.rt, TIME);
    r.frames();
    assert_eq!(when(&r), Some(When::Cron { expr: "0 18 * * 5".into() }));
    assert!(closed(&mut r), "applying it closes the menu");
}

#[test]
fn an_event_chosen_waits_for_it_and_the_chip_says_so() {
    let mut r = new_with("Say hello in the new session");
    open_menu(&mut r);
    click(&mut r.rt, "automation-when-event-session.completed");
    r.frames();
    assert_eq!(when(&r), Some(When::Event { event_type: "session.completed".into() }));
    assert_eq!(chip(&r), "On an event");
    open_menu(&mut r);
    assert!(!r.has(TIME), "an event has no time of day");
}

#[test]
fn the_servers_event_catalog_replaces_the_built_in_list() {
    let mut r = new_with("");
    open_menu(&mut r);
    let rows: Vec<String> =
        r.rt.elements()
            .iter()
            .filter_map(|e| e.key.clone())
            .filter(|k| k.starts_with("automation-when-event-"))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    assert_eq!(rows, vec!["automation-when-event-run.failed".to_string(), "automation-when-event-session.completed".to_string()]);
    // A type the built-in list doesn't know is shown as one that never fires (14.11).
    assert!(r.words().contains("run.failed (never fires)"));
    assert_eq!(r.home.calls("GET", "/api/automations/event-catalog"), 1, "read once");
}

#[test]
fn with_no_server_the_menu_offers_the_built_in_events() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.show_automations(true, crate::mock::automations::Scene::CreateTyped));
    rt.frame(crate::tests::FRAME);
    click(&mut rt, "automation-when-chip");
    rt.frame(crate::tests::FRAME);
    for event_type in event_types() {
        assert!(rt.find(&format!("automation-when-event-{event_type}")).is_some(), "{event_type}");
    }
    // Escape closes it, and the box has the keys.
    rt.key(rune::KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(crate::tests::FRAME);
    rt.frame(crate::tests::FRAME);
    assert!(rt.find("automation-when-pop").is_none());
    assert!(rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some("automation-message")));
}
