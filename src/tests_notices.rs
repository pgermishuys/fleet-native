//! Notices (component 9, piece K): `stores/__tests__/notices.test.ts` under its names, the quiet moment of
//! `NoticeCard.test.ts` 1-3 as gate cases, and the same moment seen through the running app's clock.
use crate::fleet::notices::{Notice, Notices, Quiet, STARTUP_QUIET_SECS, TYPING_QUIET_SECS};
use crate::tests::{FRAME, app};

fn update(version: &str) -> Notice {
    Notice::new(format!("update:app:{version}"), format!("Fleet {version} is ready")).chip(format!("Fleet {version} ready"))
}

fn chip_texts(store: &Notices) -> Vec<String> {
    store.chips().iter().filter_map(|n| n.chip.clone()).collect()
}

fn chip_ids(store: &Notices) -> Vec<String> {
    store.chips().iter().map(|n| n.id.clone()).collect()
}

// notices.test.ts: "waits to open a new notice until the host opens it, then settles it into its chip"
#[test]
fn waits_to_open_a_new_notice_until_the_host_opens_it_then_settles_it_into_its_chip() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    assert_eq!(store.next_waiting(), Some("update:app:0.37.0"));
    assert!(store.open().is_none());
    assert!(store.chips().is_empty());

    store.open_next();
    assert_eq!(store.open().map(|n| n.id.as_str()), Some("update:app:0.37.0"));
    assert!(store.chips().is_empty());

    store.settle();
    assert!(store.open().is_none());
    assert_eq!(chip_texts(&store), ["Fleet 0.37.0 ready"]);
}

// notices.test.ts: "never opens the same notice twice, even after a restart"
#[test]
fn never_opens_the_same_notice_twice_even_after_a_restart() {
    let mut first = Notices::default();
    first.post(update("0.37.0"), 0.0);
    first.open_next();
    first.settle();
    let saved = first.take_shown_save().expect("the shown id is saved");

    // A new run: a fresh store, the same settings file.
    let mut second = Notices::default();
    second.adopt_shown(&saved);
    second.post(update("0.37.0"), 0.0);
    assert_eq!(second.next_waiting(), None);
    assert_eq!(chip_ids(&second), ["update:app:0.37.0"]);

    second.post(update("0.37.1"), 0.0);
    assert_eq!(second.next_waiting(), Some("update:app:0.37.1"));
}

// notices.test.ts: "opens one card at a time, the next once the first settles"
#[test]
fn opens_one_card_at_a_time_the_next_once_the_first_settles() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    store.post(Notice::new("update:server:0.37.0", "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready"), 0.0);

    store.open_next();
    store.open_next();
    assert_eq!(store.open().map(|n| n.id.as_str()), Some("update:app:0.37.0"));

    store.settle();
    store.open_next();
    assert_eq!(store.open().map(|n| n.id.as_str()), Some("update:server:0.37.0"));
}

// notices.test.ts: "reopens a settled card from its chip, pinned open"
#[test]
fn reopens_a_settled_card_from_its_chip_pinned_open() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    store.open_next();
    assert!(!store.pinned());
    store.settle();

    store.reopen("update:app:0.37.0");
    assert_eq!(store.open().map(|n| n.id.as_str()), Some("update:app:0.37.0"));
    assert!(store.pinned());
}

// notices.test.ts: "pins a card whose content changes, so a question stays until it's answered"
#[test]
fn pins_a_card_whose_content_changes_so_a_question_stays_until_its_answered() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    store.open_next();

    store.update("update:app:0.37.0", |n| n.title = "2 sessions are working".into());

    assert_eq!(store.open().map(|n| n.title.as_str()), Some("2 sessions are working"));
    assert!(store.pinned());
}

// notices.test.ts: "settles a card that asks something when it's dismissed, keeping its chip so the question can be reopened"
#[test]
fn settles_a_card_that_asks_something_when_its_dismissed_keeping_its_chip_so_the_question_can_be_reopened() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    store.open_next();
    store.update("update:app:0.37.0", |n| n.title = "2 sessions are working".into());
    assert!(store.pinned());

    store.settle();
    assert!(store.open().is_none());
    assert!(!store.pinned());
    assert_eq!(chip_ids(&store), ["update:app:0.37.0"]);

    store.reopen("update:app:0.37.0");
    assert_eq!(store.open().map(|n| n.title.as_str()), Some("2 sessions are working"));
}

// notices.test.ts: "drops a notice without a chip when it settles"
#[test]
fn drops_a_notice_without_a_chip_when_it_settles() {
    let mut store = Notices::default();
    store.post(Notice::new("hello", "Hello"), 0.0);
    store.open_next();
    store.settle();
    assert!(store.notices().is_empty());
}

// notices.test.ts: "keeps a quiet notice as a chip only, and lets it expire"
#[test]
fn keeps_a_quiet_notice_as_a_chip_only_and_lets_it_expire() {
    let mut store = Notices::default();
    store.post(Notice::new("updated:app:0.37.0", "Updated").chip("Updated to 0.37.0").quiet().expires(5.0), 10.0);
    assert_eq!(store.next_waiting(), None);
    assert_eq!(store.chips().len(), 1);

    assert!(!store.expire(14.9));
    assert_eq!(store.chips().len(), 1);
    assert!(store.expire(15.0));
    assert!(store.chips().is_empty());
}

// notices.test.ts: "removes a family of notices, keeping one"
#[test]
fn removes_a_family_of_notices_keeping_one() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    store.post(update("0.37.1"), 0.0);
    store.post(Notice::new("update:server:0.37.0", "Server").chip("Server"), 0.0);

    store.remove_where("update:app:", Some("update:app:0.37.1"));

    let ids: Vec<&str> = store.notices().iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["update:app:0.37.1", "update:server:0.37.0"]);
    assert_eq!(store.next_waiting(), Some("update:app:0.37.1"));
}

#[test]
fn only_the_last_fifty_shown_ids_are_kept() {
    let mut store = Notices::default();
    for i in 0..60 {
        store.post(Notice::new(format!("n{i}"), "x"), 0.0);
        store.open_next();
        store.settle();
    }
    let saved = store.take_shown_save().unwrap();
    assert_eq!(saved.len(), 50);
    assert_eq!(saved.first().map(String::as_str), Some("n10"));
    assert_eq!(saved.last().map(String::as_str), Some("n59"));
    assert!(store.take_shown_save().is_none(), "nothing changed since");
}

#[test]
fn a_posted_id_with_the_same_name_replaces_the_notice_in_place() {
    let mut store = Notices::default();
    store.post(update("0.37.0"), 0.0);
    let mut again = update("0.37.0");
    again.title = "Changed".into();
    store.post(again, 1.0);
    assert_eq!(store.notices().len(), 1);
    assert_eq!(store.notices()[0].title, "Changed");
    assert_eq!(store.next_waiting(), Some("update:app:0.37.0"));
}

#[test]
fn the_shown_ids_survive_the_settings_file() {
    use crate::fleet::settings::Sidebar;
    let mut sidebar = Sidebar::default();
    sidebar.save_notices_shown(&["a".to_string(), "b".to_string()]);
    let back = Sidebar::parse(&sidebar.to_json());
    assert_eq!(back.notices_shown(), ["a", "b"]);
    assert!(Sidebar::parse(r#"{"noticesShown": [1, "c", null]}"#).notices_shown() == ["c"]);
}

// NoticeCard.test.ts: "opens a few seconds after Fleet opens, not on the first paint"
#[test]
fn opens_a_few_seconds_after_fleet_opens_not_on_the_first_paint() {
    let gate = Quiet::default();
    assert!(!gate.is_quiet(STARTUP_QUIET_SECS - 0.6));
    assert!(gate.is_quiet(STARTUP_QUIET_SECS + 0.4));
}

// NoticeCard.test.ts: "waits while you type and opens once you pause"
#[test]
fn waits_while_you_type_and_opens_once_you_pause() {
    let mut gate = Quiet::default();
    let mut now = STARTUP_QUIET_SECS;
    gate.typed(now);
    now += TYPING_QUIET_SECS - 0.6;
    gate.typed(now);
    now += TYPING_QUIET_SECS - 0.6;
    assert!(!gate.is_quiet(now));
    assert!(gate.is_quiet(now + 1.2));
}

// NoticeCard.test.ts: "waits until you're back in Fleet"
#[test]
fn waits_until_youre_back_in_fleet() {
    let mut gate = Quiet { focused: false, ..Quiet::default() };
    assert!(!gate.is_quiet(STARTUP_QUIET_SECS + 2.0));
    gate.focused = true;
    assert!(gate.is_quiet(STARTUP_QUIET_SECS + 2.0));
    gate.visible = false;
    assert!(!gate.is_quiet(STARTUP_QUIET_SECS + 2.0));
}

fn open_id(rt: &rune::Runtime<crate::State>) -> Option<String> {
    rt.state.notices.open().map(|n| n.id.clone())
}

fn run_until(rt: &mut rune::Runtime<crate::State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

// The same moment through the app's own clock: nothing for 3 s, then the card.
#[test]
fn the_running_app_shows_a_posted_notice_only_after_the_startup_quiet() {
    let mut rt = app();
    rt.update(|s, _| s.notices.post(update("0.37.0"), 0.0));
    rt.frame(FRAME);
    assert!(rt.find("notice-card").is_none());

    run_until(&mut rt, STARTUP_QUIET_SECS - 0.5);
    assert!(open_id(&rt).is_none(), "still the first seconds");
    assert!(rt.find("notice-card").is_none());

    run_until(&mut rt, 1.5);
    assert_eq!(open_id(&rt).as_deref(), Some("update:app:0.37.0"));
    assert!(rt.find("notice-card").is_some());
    assert_eq!(rt.state.notices.shown(), ["update:app:0.37.0"]);
}

#[test]
fn the_running_app_holds_a_card_while_the_window_has_no_focus_and_opens_it_on_return() {
    let mut rt = app();
    rt.update(|s, _| {
        s.quiet.focused = false;
        s.notices.post(update("0.37.0"), 0.0);
    });
    run_until(&mut rt, STARTUP_QUIET_SECS + 2.0);
    assert!(open_id(&rt).is_none());

    rt.update(|s, _| s.quiet.focused = true);
    run_until(&mut rt, 1.0);
    assert!(open_id(&rt).is_some());
}
