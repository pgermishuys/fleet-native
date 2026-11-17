//! Component 16, piece U: what Fleet has no test for: the status store, the way back after a restart, the wire shapes, the
//! rule preferences and the last-seen versions.
use super::feed::*;
use super::notes::Releases;
use super::rows::*;
use super::*;
use serde_json::json;

// ─── not in Fleet's tests ─────────────────────────────────────────────────────

#[test]
fn compares_versions_the_way_all_three_of_fleets_copies_do() {
    use std::cmp::Ordering::{Equal, Greater, Less};
    assert_eq!(compare_versions("v0.46.0", "0.46.0"), Equal);
    assert_eq!(compare_versions("0.59.1-beta.2", "0.59.1"), Equal, "a tag is ignored");
    assert_eq!(compare_versions("0.10.0", "0.9.0"), Greater);
    assert_eq!(compare_versions("1.0", "1.0.1"), Less);
    assert_eq!(release_notes_url("v0.59.1"), "https://github.com/pgermishuys/weave-fleet/releases/tag/v0.59.1");
}

#[test]
fn the_status_store_drops_an_answer_that_a_later_read_overtook() {
    let ok = |version: &str| UpdateStatus::from_json(&json!({ "currentVersion": version, "status": "uptodate" }));
    let mut store = StatusStore::default();
    let (first, second) = (store.begin(), store.begin());
    store.adopt(first, ok("0.59.0"), 1.0);
    assert!(store.status.is_none(), "the older read came back after the newer was sent");
    store.adopt(second, ok("0.59.1"), 2.0);
    assert_eq!(store.status.as_ref().map(|s| s.current_version.as_str()), Some("0.59.1"));
}

#[test]
fn the_status_store_reads_every_second_and_a_half_while_downloading_else_every_fifteen_minutes() {
    let mut store = StatusStore::default();
    assert!(store.due(0.0), "never read");
    let r = store.begin();
    assert!(!store.due(0.0), "one is under way");
    store.adopt(
        r,
        UpdateStatus::from_json(&json!({ "currentVersion": "0.59.0", "status": "downloading", "latestVersion": "0.59.1" })),
        10.0,
    );
    assert!(!store.due(11.0) && store.due(11.5));
    let r = store.begin();
    store.adopt(r, UpdateStatus::from_json(&json!({ "currentVersion": "0.59.0", "status": "uptodate" })), 20.0);
    assert!(!store.due(20.0 + 899.0) && store.due(20.0 + 900.0));
}

#[test]
fn the_status_store_keeps_what_it_showed_when_a_read_fails() {
    let mut store = StatusStore::default();
    let r = store.begin();
    assert!(store.loading);
    store.adopt(r, UpdateStatus::from_json(&json!({ "currentVersion": "0.59.0", "status": "staged", "latestVersion": "0.59.1" })), 0.0);
    let r = store.begin();
    assert!(!store.loading, "something is already shown");
    store.adopt(r, None, 5.0);
    let kept = store.status.as_ref().expect("kept");
    assert!(kept.is_staged() && !kept.is_available());
}

#[test]
fn the_notes_store_says_so_when_a_read_fails_and_keeps_the_ones_it_has() {
    let mut store = ReleasesStore::default();
    store.adopt(None);
    assert_eq!(store.error.as_deref(), Some("Fleet couldn't load the release notes."));
    store.adopt(Releases::from_json(
        &json!({ "releases": [{ "version": "0.59.1", "publishedAt": null, "body": "x", "url": "u" }], "fetchedAt": "t", "error": null }),
    ));
    assert_eq!((store.releases.len(), store.error.clone()), (1, None));
    store.adopt(None);
    assert_eq!(store.releases.len(), 1, "the notes it had stay");
}

#[test]
fn the_way_back_needs_the_fleet_to_go_down_and_answer_again() {
    let mut back = ComeBack::new(Some("0.59.0".into()), Some("0.59.1".into()), 100.0);
    assert!(!back.should_ask(101.0) && back.should_ask(101.5));
    assert_eq!(back.answered(true, 101.5), ComeBackStep::Waiting, "it hadn't stopped yet");
    assert_eq!(back.answered(false, 103.0), ComeBackStep::Waiting);
    assert_eq!(back.answered(true, 106.0), ComeBackStep::Back);
}

#[test]
fn the_way_back_gives_up_after_three_minutes_and_says_so() {
    let mut back = ComeBack::new(None, None, 0.0);
    assert_eq!(back.answered(false, 179.0), ComeBackStep::Waiting);
    assert_eq!(back.answered(false, 181.0), ComeBackStep::GaveUp);
    assert_eq!(DIDNT_COME_BACK, "This Fleet didn't come back within 3 minutes.");
}

#[test]
fn a_refusal_says_the_servers_words_or_the_status() {
    assert_eq!(refusal(409, r#"{"error":"A rollout is already running."}"#), "A rollout is already running.");
    assert_eq!(refusal(502, "<html>"), "The Fleet answered 502.");
}

#[test]
fn reads_the_update_block_a_machine_reports_and_none_from_an_old_fleet() {
    let full = json!({ "status": "staged", "latestVersion": "0.59.1", "stagedVersion": "0.59.1", "checkedAt": "2026-10-20T08:00:00Z", "canRestart": false,
        "restartUnavailableReason": "managed by systemd", "rolledBack": { "from": "0.59.0", "to": "0.59.2", "reason": "exit 1", "at": "t" },
        "rule": "overnight", "ruleSource": "set", "lastUpdate": { "from": "0.58.0", "to": "0.59.0", "at": "t", "trigger": "rollout" } });
    let u = MachineUpdate::from_json(&full).expect("an update");
    assert_eq!((u.status, u.can_restart, u.rule, u.rule_set), (Status::Staged, Some(false), Some(UpdateRule::Overnight), true));
    assert_eq!(u.rolled_back.as_ref().map(|r| r.to.as_str()), Some("0.59.2"));
    assert_eq!(u.last_update.as_ref().map(|l| l.trigger), Some(Trigger::Rollout));
    assert!(MachineUpdate::from_json(&json!(null)).is_none());
    let info = crate::fleet::machines::Info::from_json(
        &json!({ "id": "m", "name": "n", "version": "0.59.0", "update": { "status": "uptodate" } }),
    )
    .expect("info");
    assert_eq!(info.update.map(|u| u.status), Some(Status::UpToDate));
    let old = crate::fleet::machines::Info::from_json(&json!({ "id": "m", "name": "n", "version": "0.50.0" })).expect("info");
    assert!(old.update.is_none());
}

#[test]
fn reads_the_rollout_a_fleet_sends_and_skips_a_step_it_cant_read() {
    let r = Rollout::from_json(&json!({ "id": "r1", "status": "running", "startedAt": "t", "summary": null, "steps": [
        { "machineId": "m1", "name": "falcon", "state": "waiting", "waitingSessions": 2, "toVersion": "0.59.1" },
        { "machineId": "m2", "name": "heron", "state": "someday", "waitingSessions": 0 },
        { "machineId": "m3", "name": "kestrel", "state": "rolled-back", "detail": "x", "waitingSessions": 0 } ] }))
    .expect("a rollout");
    assert_eq!(r.steps.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["falcon", "kestrel"]);
    assert!(r.running());
    assert!(Rollout::from_json(&json!({ "id": "r", "status": "paused", "steps": [] })).is_none());
}

#[test]
fn reads_this_fleets_status_with_the_bytes_of_a_download() {
    let s = UpdateStatus::from_json(
        &json!({ "currentVersion": "0.59.0", "status": "downloading", "latestVersion": "0.59.1", "checkedAt": null, "error": null,
        "downloadBytesReceived": 1_048_576, "downloadBytesTotal": 52_428_800 }),
    )
    .expect("status");
    assert!(s.is_available() && !s.is_staged());
    assert_eq!((s.download_bytes_received, s.download_bytes_total), (Some(1_048_576), Some(52_428_800)));
}

#[test]
fn keeps_the_rule_preferences_as_fleet_writes_them() {
    let mut overrides = parse_rule_overrides(Some(r#"{"falcon":"idle"}"#));
    overrides = with_override(&overrides, "heron", Some(UpdateRule::Ask));
    assert_eq!(overrides_json(&overrides), r#"{"falcon":"idle","heron":"ask"}"#);
    assert_eq!(overrides_json(&with_override(&overrides, "falcon", None)), r#"{"heron":"ask"}"#, "Default removes the override");
    assert_eq!(parse_chosen_rule(Some("")), None);
    assert_eq!(parse_chosen_rule(Some("overnight")), Some(UpdateRule::Overnight));
    assert_eq!(
        (UPDATE_RULE_DEFAULT_KEY, UPDATE_RULE_OVERRIDES_KEY, UPDATE_RULE_NOTICE_KEY),
        ("FleetUpdateRule", "FleetUpdateRules", "FleetUpdateRuleNotice")
    );
}

#[test]
fn the_harness_update_check_is_on_unless_it_says_false() {
    let mut prefs = crate::fleet::prefs::Prefs::default();
    assert!(HARNESS_UPDATE_CHECK.enabled(&prefs));
    prefs.set("harnessUpdates.check", "false");
    assert!(!HARNESS_UPDATE_CHECK.enabled(&prefs));
    prefs.set("harnessUpdates.check", "whatever");
    assert!(HARNESS_UPDATE_CHECK.enabled(&prefs));
}

#[test]
fn keeps_the_version_each_machine_last_reported_for_one_that_goes_away() {
    let mut seen = LastSeen::parse(r#"{"falcon":"0.58.2","bad":3}"#);
    assert_eq!(seen.get("falcon"), Some("0.58.2"));
    assert_eq!(seen.get("bad"), None);
    assert!(!seen.note([("falcon", "0.58.2"), ("heron", "")]), "nothing new");
    assert!(seen.note([("falcon", "0.59.0"), ("heron", "0.59.0")]));
    assert_eq!(LastSeen::parse(&seen.to_json()), seen);
    assert_eq!(LastSeen::parse("not json"), LastSeen::default());
}

#[test]
fn names_the_operating_systems() {
    assert_eq!(
        [os_label("linux"), os_label("macos"), os_label("windows"), os_label("")],
        [Some("Linux".into()), Some("macOS".into()), Some("Windows".into()), None]
    );
    assert_eq!(os_label("freebsd").as_deref(), Some("freebsd"));
}

#[test]
fn words_the_rule_choices_and_the_hint_under_the_strip() {
    assert_eq!(RULE_OPTIONS.iter().map(|o| o.label).collect::<Vec<_>>().len(), 4);
    assert_eq!(rule_hint(None), format!("{} Update machines now doesn't wait for the rule.", RULE_OPTIONS[0].detail));
    assert!(rule_hint(Some(UpdateRule::Ask)).starts_with("Each Fleet waits here until you restart it. Update machines now"));
    assert_eq!(default_rule_label(None, MachineKind::Node), "When nothing is in use");
    assert_eq!(default_rule_label(None, MachineKind::Fleet), "Ask me");
    assert_eq!(default_rule_label(Some(UpdateRule::Overnight), MachineKind::Node), "Overnight");
}

// ─── the words of every state, for the picture beside Fleet's own ─────────────

/// `cargo test prints_the_words_of_every_state -- --ignored --nocapture` prints one `key<TAB>text` line per state. The same
/// states go through Fleet's own `fleet-updates.ts` (see `shots/parity/16-updates-words/`), and the two lists are diffed.
#[test]
#[ignore = "prints, for the pictures"]
fn prints_the_words_of_every_state() {
    use std::collections::HashSet;
    let now = 1_792_497_600_000;
    let fixed = |f: fn(&mut MachineUpdate)| update_with(f);
    type Case = (&'static str, fn(&mut MachineUpdate), &'static str, u32, bool, Option<UpdateRule>);
    let cases: [Case; 12] = [
        ("up to date", |_| {}, "0.59.1", 0, true, Some(UpdateRule::Ask)),
        ("update out", |u| u.status = Status::Available, "0.59.0", 0, true, Some(UpdateRule::Ask)),
        ("downloading", |u| u.status = Status::Downloading, "0.59.0", 0, true, Some(UpdateRule::Ask)),
        (
            "staged, idle rule",
            |u| {
                u.status = Status::Staged;
                u.staged_version = Some("0.59.1".into());
            },
            "0.59.0",
            0,
            true,
            Some(UpdateRule::Idle),
        ),
        (
            "staged, busy",
            |u| {
                u.status = Status::Staged;
                u.staged_version = Some("0.59.1".into());
            },
            "0.59.0",
            2,
            true,
            Some(UpdateRule::Idle),
        ),
        (
            "staged, ask",
            |u| {
                u.status = Status::Staged;
                u.staged_version = Some("0.59.1".into());
            },
            "0.59.0",
            0,
            true,
            Some(UpdateRule::Ask),
        ),
        (
            "staged, overnight",
            |u| {
                u.status = Status::Staged;
                u.staged_version = Some("0.59.1".into());
            },
            "0.59.0",
            0,
            true,
            Some(UpdateRule::Overnight),
        ),
        (
            "cannot restart",
            |u| {
                u.status = Status::Staged;
                u.staged_version = Some("0.59.1".into());
                u.can_restart = Some(false);
                u.restart_unavailable_reason = Some("managed by systemd".into());
            },
            "0.59.0",
            0,
            true,
            Some(UpdateRule::Ask),
        ),
        ("unreachable", |_| {}, "0.58.2", 0, false, Some(UpdateRule::Ask)),
        (
            "rolled back",
            |u| {
                u.rolled_back = Some(RolledBack {
                    from: "0.59.0".into(),
                    to: "0.59.2".into(),
                    reason: "it exited with code 1 before it was ready".into(),
                    at: "t".into(),
                })
            },
            "0.59.0",
            0,
            true,
            Some(UpdateRule::Ask),
        ),
        (
            "check failed",
            |u| {
                u.status = Status::Error;
                u.latest_version = None;
            },
            "0.59.0",
            0,
            true,
            Some(UpdateRule::Ask),
        ),
        (
            "not checked",
            |u| {
                u.status = Status::Unknown;
                u.latest_version = None;
            },
            "0.59.0",
            0,
            true,
            None,
        ),
    ];
    for (name, patch, version, working, reachable, rule) in cases {
        let u = fixed(patch);
        let state = describe_machine_update(&MachineInput { version: Some(version), update: Some(&u), working, reachable });
        let offer = restart_offer(&state, Some(&u), false);
        let mut r = row("kestrel-id", state.clone());
        r.version = Some(version.into());
        r.working = Some(working);
        r.rule = rule.map(|effective| RuleChoice { effective, overridden: None });
        r.restart = offer.clone();
        let line = describe_version_line(&r, None);
        let offer = match offer {
            Some(RestartOffer::Button { label, disabled }) => format!("{label}{}", if disabled { " (disabled)" } else { "" }),
            Some(RestartOffer::Unavailable { reason }) => format!("no button: {reason}"),
            None => "-".into(),
        };
        println!("{name}\t{} | {} | {} | menu {} | {offer}", state.label, line.version, line.text, line.rule_menu);
    }
    let sum = |waiting, downloaded| Summary { latest: Some("0.59.1".into()), waiting, downloaded, unreachable: 0 };
    for (name, s, at) in [
        ("release: none", Summary::default(), None),
        ("release: current", sum(0, 0), Some("2026-10-20T11:56:00Z")),
        ("release: none downloaded", sum(2, 0), None),
        ("release: some", sum(3, 2), None),
        ("release: all", sum(3, 3), None),
    ] {
        let line = describe_release_line(&s, at, now, 0);
        println!("{name}\t{} {}", line.lead, line.text);
    }
    let mut r = Rollout::from_json(&json!({ "id": "r", "status": "running", "startedAt": "t", "steps": [
        { "machineId": "a", "name": "heron", "state": "done", "toVersion": "0.59.1", "waitingSessions": 0 },
        { "machineId": "b", "name": "falcon", "state": "waiting", "waitingSessions": 2, "toVersion": "0.59.1" },
        { "machineId": "c", "name": "kestrel", "state": "pending", "waitingSessions": 0 },
        { "machineId": "d", "name": "osprey", "state": "pending", "waitingSessions": 0 } ] }))
    .expect("rollout");
    let narrative = describe_rollout_narrative(&r, &HashSet::from(["d".to_string()]));
    println!("rollout\t{} | active {:?} | {}", narrative.title, narrative.active, narrative.text);
    println!("chip\t{:?} | {:?}", describe_update_chip(3, None), describe_update_chip(3, Some(&r)));
    r.status = RolloutStatus::Stopped;
    r.summary = Some("kestrel didn't start on 0.59.1 and went back to 0.59.0.".into());
    println!("outcome\t{:?}", describe_rollout_outcome(Some(&r)).map(|o| o.0));
}

fn update_with(patch: fn(&mut MachineUpdate)) -> MachineUpdate {
    let mut u = MachineUpdate {
        status: Status::UpToDate,
        latest_version: Some("0.59.1".into()),
        staged_version: None,
        checked_at: Some("2026-10-20T08:00:00Z".into()),
        can_restart: Some(true),
        restart_unavailable_reason: None,
        rolled_back: None,
        rule: Some(UpdateRule::Ask),
        rule_set: false,
        last_update: None,
    };
    patch(&mut u);
    u
}

fn row(key: &str, state: UpdateState) -> UpdateRow {
    UpdateRow {
        key: key.into(),
        name: key.into(),
        kind: MachineKind::Fleet,
        os: None,
        version: None,
        latest: Some("0.59.1".into()),
        where_: key.into(),
        is_live: false,
        state,
        restart: None,
        is_home: false,
        rule: None,
        working: None,
    }
}
