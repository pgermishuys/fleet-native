//! Machine update notices (component 9, piece O): `use-fleet-update-notices.test.ts` under its names. Fleet's test hands the
//! composable a machine store, a preferences store and the watch; here the machines and preferences are the app's own state,
//! the notices are read from the store after `keep_update_notices`, and a button is pressed by telling the source its name.
//! Invented machines throughout; nothing here updates a real machine.
use crate::fleet::machines::{Info, Source};
use crate::fleet::notices::Notice;
use crate::fleet::update_notices::{CHIP_ID, RUN_PREFIX, UPDATED_PREFIX, card, chip, unpicked_rule_news};
use crate::fleet::updates::rows::describe_updated_notice;
use crate::fleet::updates::{Rollout, Trigger, UpdateRule};
use crate::fleet::work::parse_iso_ms;
use crate::tests::app;
use rune::Runtime;
use serde_json::{Value, json};

/// 2026-10-20 12:00 UTC, the moment Fleet's test fakes.
const NOW: &str = "2026-10-20T12:00:00Z";

fn update(from: &str, to: &str, trigger: &str) -> Value {
    json!({ "from": from, "to": to, "at": "2026-10-20T03:12:00Z", "trigger": trigger })
}

fn falcon(version: &str, last: &Value) -> Info {
    Info::from_json(&json!({
        "id": "falcon-id", "name": "falcon", "os": "linux", "version": version,
        "update": { "status": "uptodate", "latestVersion": version, "stagedVersion": null, "checkedAt": null, "lastUpdate": last },
    }))
    .expect("an info")
}

fn start() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.now_ms = parse_iso_ms(NOW);
        s.pickers.prefs.fetched = true;
        s.machines.source = Source::Owner;
        s.machines.home = None;
    });
    rt
}

fn machines(rt: &mut Runtime<crate::State>, info: Info) {
    rt.update(move |s, _| {
        s.machines.home = Some(info);
        s.keep_update_notices();
    });
}

fn updated(rt: &Runtime<crate::State>) -> Vec<Notice> {
    rt.state.notices.notices().iter().filter(|n| n.id.starts_with(UPDATED_PREFIX)).cloned().collect()
}

fn chips(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.notices.chips().iter().filter_map(|n| n.chip.clone()).collect()
}

/// A rollout with `steps` machines in these states.
fn rollout(states: &[&str]) -> Rollout {
    let steps: Vec<Value> =
        states.iter().enumerate().map(|(i, s)| json!({ "machineId": format!("m{i}"), "name": format!("m{i}"), "state": s })).collect();
    Rollout::from_json(&json!({ "id": "r", "status": "running", "startedAt": "", "steps": steps })).expect("a rollout")
}

fn labels(n: &Notice) -> Vec<&str> {
    n.actions.iter().map(|a| a.label.as_str()).collect()
}

fn press(rt: &mut Runtime<crate::State>, run: &str) {
    let run = run.to_string();
    rt.update(move |s, _| {
        s.notices.tell(&run);
        s.keep_update_buttons();
    });
}

// use-fleet-update-notices.test.ts: "announces a machine's update once per machine and version, not again when the machine is read again or the page reloads"
#[test]
fn announces_a_machine_s_update_once_per_machine_and_version_not_again_when_the_machine_is_read_again_or_the_page_reloads() {
    let mut rt = start();
    let last = update("0.59.0", "0.59.1", "idle");
    machines(&mut rt, falcon("0.59.1", &last));
    let cards = updated(&rt);
    assert_eq!(cards.iter().filter(|n| n.title == "falcon updated to 0.59.1").count(), 1);

    rt.update(|s, _| {
        s.notices.open_next();
        s.notices.settle();
    });
    machines(&mut rt, falcon("0.59.1", &last));
    // As in Fleet the store takes it again, but its id was shown, so it never waits for a card.
    assert_eq!(updated(&rt).len(), 1);
    assert!(rt.state.notices.next_waiting().is_none());

    // After a restart the store is empty but the settings file remembers it showed this one.
    let shown = rt.state.notices.shown().to_vec();
    assert_eq!(shown, vec![format!("{UPDATED_PREFIX}home:0.59.1")]);
    let mut again = start();
    again.update(move |s, _| s.notices.adopt_shown(&shown));
    machines(&mut again, falcon("0.59.1", &last));
    assert!(again.state.notices.next_waiting().is_none());
    assert!(again.state.notices.open().is_none());
}

// use-fleet-update-notices.test.ts: "keeps a chip that counts the Fleets that can update, and takes it away at zero"
#[test]
fn keeps_a_chip_that_counts_the_fleets_that_can_update_and_takes_it_away_at_zero() {
    let mut rt = start();
    assert!(chips(&rt).is_empty());
    let waiting = |n: usize| {
        // The count is the machines whose update is on its way: one Fleet each, here the listed ones.
        (0..n)
            .map(|i| json!({ "id": format!("m{i}"), "name": format!("m{i}"), "baseUrl": format!("http://127.0.0.1:{}", 6000 + i) }))
            .collect::<Vec<_>>()
    };
    for (count, expect) in [(2, Some("↑ 2 machines can update")), (1, Some("↑ 1 machine can update")), (0, None)] {
        rt.update(move |s, _| {
            s.machines.list = waiting(count).iter().filter_map(crate::fleet::machine::Machine::from_json).collect();
            s.machines.identities = s
                .machines
                .list
                .iter()
                .map(|m| {
                    let info = json!({ "id": m.id, "name": m.name, "version": "0.59.0",
                        "update": { "status": "available", "latestVersion": "0.59.1" } });
                    (m.id.clone(), Info::from_json(&info).expect("an info"))
                })
                .collect();
            s.keep_update_notices();
        });
        assert_eq!(chips(&rt), expect.map(String::from).into_iter().collect::<Vec<_>>());
    }
}

// use-fleet-update-notices.test.ts: "turns the one chip into the rollout's progress while machines update, and a click opens Settings → Updates & about › Machines"
#[test]
fn turns_the_one_chip_into_the_rollout_s_progress_while_machines_update_and_a_click_opens_settings_updates_and_about_machines() {
    let mut rt = start();
    rt.update(|s, _| {
        s.updates.rollout = Some(rollout(&["done", "waiting", "pending"]));
        s.keep_update_notices();
    });
    assert_eq!(chips(&rt), vec!["Updating machines · 2 of 3".to_string()]);

    // A click on the chip tells its source, which opens the page and its Machines group.
    assert_eq!(rt.state.notices.get(CHIP_ID).and_then(|n| n.on_chip_click.clone()).as_deref(), Some("fleet-update:open-machines"));
    press(&mut rt, "fleet-update:open-machines");
    assert_eq!(rt.state.rail, crate::Rail::Settings);
    assert_eq!(rt.state.settings.page, crate::fleet::settings_nav::Page::Updates);
    assert_eq!(rt.state.settings.anchor.as_deref(), Some("machines"));

    // The rollout ends: the one chip is gone.
    rt.update(|s, _| {
        s.updates.rollout = None;
        s.keep_update_notices();
    });
    assert!(chips(&rt).is_empty());
}

mod a_machine_that_restarted_itself_under_a_rule_nobody_picked {
    use super::*;

    // use-fleet-update-notices.test.ts: "tells the user once, with a way to change the rule, and never again"
    #[test]
    fn tells_the_user_once_with_a_way_to_change_the_rule_and_never_again() {
        let mut rt = start();
        let first = update("0.59.0", "0.59.1", "idle");
        machines(&mut rt, falcon("0.59.1", &first));
        let card = updated(&rt).remove(0);
        assert!(card.body.as_deref().unwrap_or("").contains("Nobody chose an update rule"));
        assert_eq!(labels(&card), vec!["What's new", "Change the rule", "Dismiss"]);
        assert_eq!(rt.state.pickers.prefs.get("FleetUpdateRuleNotice"), Some("true"));

        // The next self-update, on another version, is not announced that way.
        rt.update(|s, _| s.notices.remove(&format!("{UPDATED_PREFIX}home:0.59.1")));
        machines(&mut rt, falcon("0.59.2", &update("0.59.1", "0.59.2", "idle")));
        let next = updated(&rt).remove(0);
        assert!(!next.body.as_deref().unwrap_or("").contains("Nobody chose"));
        assert_eq!(labels(&next), vec!["What's new", "Dismiss"]);
    }

    // use-fleet-update-notices.test.ts: "says nothing about the rule when the user chose one, or the update was theirs, or preferences haven't loaded"
    #[test]
    fn says_nothing_about_the_rule_when_the_user_chose_one_or_the_update_was_theirs_or_preferences_haven_t_loaded() {
        let mut rt = start();
        rt.update(|s, _| {
            s.pickers.prefs.values.insert("FleetUpdateRules".into(), json!({ "home": "idle" }).to_string()).map(drop).unwrap_or(())
        });
        machines(&mut rt, falcon("0.59.1", &update("0.59.0", "0.59.1", "idle")));
        assert!(!updated(&rt)[0].body.as_deref().unwrap_or("").contains("Nobody chose"));
        assert_eq!(rt.state.pickers.prefs.get("FleetUpdateRuleNotice"), None);

        let mut rt = start();
        machines(&mut rt, falcon("0.59.2", &update("0.59.1", "0.59.2", "manual")));
        assert_eq!(rt.state.pickers.prefs.get("FleetUpdateRuleNotice"), None);

        let mut rt = start();
        rt.update(|s, _| s.pickers.prefs.fetched = false);
        machines(&mut rt, falcon("0.59.3", &update("0.59.2", "0.59.3", "idle")));
        assert_eq!(rt.state.pickers.prefs.get("FleetUpdateRuleNotice"), None);
    }
}

#[test]
fn the_card_says_when_and_by_what_and_news_older_than_three_days_is_none() {
    let now = parse_iso_ms(NOW).unwrap();
    let last = crate::fleet::updates::LastUpdate {
        from: "0.59.0".into(),
        to: "0.59.1".into(),
        at: "2026-10-20T03:12:00Z".into(),
        trigger: Trigger::Rollout,
    };
    let news = describe_updated_notice("m", "harbor", Some("v0.59.1"), Some(&last), now, 0).expect("news");
    assert_eq!((news.title.as_str(), news.body.as_str()), ("harbor updated to 0.59.1", "At 3:12 AM, by Update machines now."));
    assert!(describe_updated_notice("m", "harbor", Some("0.59.1"), Some(&last), now + 4 * 86_400_000, 0).is_none());
    assert!(describe_updated_notice("m", "harbor", Some("0.59.0"), Some(&last), now, 0).is_none());
    assert!(chip(0, None).is_none());
    let c = card(&news, false);
    assert!(c.chip.is_none() && c.actions.iter().all(|a| a.run.starts_with(RUN_PREFIX)));
}

#[test]
fn only_an_idle_or_overnight_restart_under_no_chosen_rule_is_news_about_the_rule() {
    let none = Default::default();
    let rules = |fetched, told, chosen| crate::fleet::update_notices::Rules { fetched, told, chosen, overrides: &none };
    assert!(unpicked_rule_news("m", Trigger::Overnight, &rules(true, false, None)));
    assert!(!unpicked_rule_news("m", Trigger::Rollout, &rules(true, false, None)));
    assert!(!unpicked_rule_news("m", Trigger::Idle, &rules(true, true, None)));
    assert!(!unpicked_rule_news("m", Trigger::Idle, &rules(true, false, Some(UpdateRule::Ask))));
}

#[test]
fn what_s_new_and_change_the_rule_open_settings_and_the_card_goes() {
    let mut rt = start();
    machines(&mut rt, falcon("0.59.1", &update("0.59.0", "0.59.1", "idle")));
    rt.update(|s, _| s.notices.open_next());
    let id = format!("{UPDATED_PREFIX}home:0.59.1");
    press(&mut rt, &format!("fleet-update:change-rule:{id}"));
    assert!(rt.state.notices.open().is_none() && !rt.state.notices.has(&id));
    assert_eq!(rt.state.settings.anchor.as_deref(), Some("machines"));
}
