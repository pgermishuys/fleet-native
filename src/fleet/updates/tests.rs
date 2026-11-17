//! Component 16, piece U: `fleet-updates.test.ts` under Fleet's names (as snake case) except `describeDesktopUpdate`, which is
//! piece V's (native's updater, decision 4), then the cases Fleet has no test for: the status store, the way back after a
//! restart, the wire shapes, the rule preferences and the last-seen versions.
use super::rows::*;
use super::*;
use std::collections::HashSet;

fn update(patch: impl FnOnce(&mut MachineUpdate)) -> MachineUpdate {
    let mut u = MachineUpdate {
        status: Status::UpToDate,
        latest_version: Some("0.59.1".into()),
        staged_version: None,
        checked_at: Some("2026-10-20T08:00:00Z".into()),
        can_restart: None,
        restart_unavailable_reason: None,
        rolled_back: None,
        rule: None,
        rule_set: false,
        last_update: None,
    };
    patch(&mut u);
    u
}

fn machine_at(patch: impl FnOnce(&mut MachineUpdate), version: &str, working: u32, reachable: bool) -> UpdateState {
    describe_machine_update(&MachineInput { version: Some(version), update: Some(&update(patch)), working, reachable })
}

fn machine(patch: impl FnOnce(&mut MachineUpdate)) -> UpdateState {
    machine_at(patch, "0.59.0", 0, true)
}

fn staged(u: &mut MachineUpdate) {
    u.status = Status::Staged;
    u.staged_version = Some("0.59.1".into());
}

// ─── isNewerVersion ───────────────────────────────────────────────────────────

#[test]
fn compares_numerically_not_as_text() {
    assert!(is_newer_version(Some("0.59.1"), Some("0.59.0")));
    assert!(is_newer_version(Some("0.60.0"), Some("0.59.9")));
    assert!(!is_newer_version(Some("0.9.0"), Some("0.10.0")));
    assert!(!is_newer_version(Some("0.59.0"), Some("0.59.0")));
    assert!(!is_newer_version(None, Some("0.59.0")));
}

// ─── describeMachineUpdate ────────────────────────────────────────────────────

#[test]
fn says_up_to_date_when_it_runs_the_newest_release() {
    let s = machine_at(|_| {}, "0.59.1", 0, true);
    assert_eq!((s.kind, s.label.as_str(), s.tone, s.target), (Kind::Current, "up to date", Tone::Ok, None));
}

#[test]
fn says_an_update_is_out_when_it_hasnt_downloaded_it() {
    let s = machine(|u| u.status = Status::Available);
    assert_eq!((s.kind, s.target.as_deref()), (Kind::Available, Some("0.59.1")));
}

#[test]
fn says_its_downloading() {
    let s = machine(|u| u.status = Status::Downloading);
    assert_eq!((s.kind, s.label.as_str()), (Kind::Downloading, "downloading 0.59.1…"));
}

#[test]
fn says_its_downloaded_and_waiting_for_a_restart() {
    let s = machine(staged);
    assert_eq!((s.kind, s.label.as_str(), s.target.as_deref()), (Kind::Ready, "0.59.1 downloaded · waiting for a restart", Some("0.59.1")));
}

#[test]
fn knows_about_a_download_staged_before_the_last_restart_even_though_it_hasnt_checked_since() {
    let s = machine(|u| {
        u.status = Status::Unknown;
        u.latest_version = None;
        u.staged_version = Some("0.59.1".into());
    });
    assert_eq!((s.kind, s.target.as_deref()), (Kind::Ready, Some("0.59.1")));
}

#[test]
fn says_busy_with_how_many_sessions_are_working_when_the_download_waits_on_a_busy_machine() {
    assert_eq!(machine_at(staged, "0.59.0", 2, true).label, "0.59.1 downloaded · busy: 2 sessions working or waiting for you");
    let one = machine_at(staged, "0.59.0", 1, true);
    assert_eq!((one.kind, one.label.as_str()), (Kind::Busy, "0.59.1 downloaded · busy: 1 session working or waiting for you"));
}

#[test]
fn says_it_cant_be_reached_whatever_it_last_said() {
    let s = machine_at(staged, "0.59.0", 0, false);
    assert_eq!((s.kind, s.label.as_str()), (Kind::Unreachable, "can't be reached"));
}

#[test]
fn says_the_desktop_app_manages_it() {
    let s = machine(|u| {
        u.status = Status::Managed;
        u.latest_version = None;
    });
    assert_eq!((s.kind, s.label.as_str()), (Kind::Managed, "managed by the desktop app"));
}

#[test]
fn says_a_failed_check_and_one_that_hasnt_run() {
    let failed = machine(|u| {
        u.status = Status::Error;
        u.latest_version = None;
    });
    assert_eq!((failed.kind, failed.tone), (Kind::Error, Tone::Bad));
    let unchecked = machine(|u| {
        u.status = Status::Unknown;
        u.latest_version = None;
    });
    assert_eq!(unchecked.kind, Kind::Unchecked);
}

#[test]
fn says_a_fleet_too_old_to_report_updates() {
    let s = describe_machine_update(&MachineInput { version: Some("0.50.0"), update: None, working: 0, reachable: true });
    assert_eq!(s.kind, Kind::Unsupported);
}

// ─── summarizeRows ────────────────────────────────────────────────────────────

fn row(key: &str, state: UpdateState) -> UpdateRow {
    UpdateRow {
        key: key.into(),
        name: key.into(),
        kind: MachineKind::Node,
        os: None,
        version: Some("0.59.0".into()),
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

#[test]
fn counts_the_fleets_with_an_update_waiting_the_ones_that_have_it_downloaded_and_the_ones_it_cant_reach() {
    let rows = vec![
        row("kestrel", machine_at(|_| {}, "0.59.1", 0, true)),
        row("falcon", machine(staged)),
        row("heron", machine_at(staged, "0.59.0", 2, true)),
        row("osprey", machine_at(|_| {}, "0.59.0", 0, false)),
        row("wren", machine(|u| u.status = Status::Available)),
    ];
    assert_eq!(summarize_rows(&rows), Summary { latest: Some("0.59.1".into()), waiting: 3, downloaded: 2, unreachable: 1 });
    assert_eq!(rows.iter().map(|r| has_update_waiting(&r.state)).collect::<Vec<_>>(), [false, true, true, false, true]);
}

// ─── restart offers ───────────────────────────────────────────────────────────

fn can_restart(u: &mut MachineUpdate) {
    staged(u);
    u.can_restart = Some(true);
}

#[test]
fn offers_a_button_on_a_machine_that_has_the_update_and_can_restart() {
    let state = machine(can_restart);
    assert_eq!(
        restart_offer(&state, Some(&update(can_restart)), false),
        Some(RestartOffer::Button { label: "Restart into 0.59.1".into(), disabled: false })
    );
}

#[test]
fn lets_a_busy_machine_in_line_since_the_rollout_waits_for_its_sessions_but_not_this_one() {
    let state = machine_at(can_restart, "0.59.0", 2, true);
    let u = update(can_restart);
    assert_eq!(restart_offer(&state, Some(&u), false), Some(RestartOffer::Button { label: "Restart when idle".into(), disabled: false }));
    assert!(matches!(restart_offer(&state, Some(&u), true), Some(RestartOffer::Button { disabled: true, .. })));
}

#[test]
fn says_why_a_machine_cant_restart_itself_instead_of_offering_a_button() {
    let why = "Restart Fleet once by hand; after that it can restart itself.";
    let state = machine(|u| {
        staged(u);
        u.can_restart = Some(false);
        u.restart_unavailable_reason = Some(why.into());
    });
    let u = update(|u| {
        u.can_restart = Some(false);
        u.restart_unavailable_reason = Some(why.into());
    });
    assert_eq!(restart_offer(&state, Some(&u), false), Some(RestartOffer::Unavailable { reason: why.into() }));
    match restart_offer(&state, Some(&update(|_| {})), false) {
        Some(RestartOffer::Unavailable { reason }) => assert!(reason.contains("too old")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn offers_nothing_to_a_machine_with_no_update_waiting() {
    let state = machine_at(|_| {}, "0.59.1", 0, true);
    assert_eq!(restart_offer(&state, Some(&update(|u| u.can_restart = Some(true))), false), None);
}

#[test]
fn reports_an_update_that_was_undone_until_nothing_newer_is_on_its_way() {
    let undone = |to: &str| RolledBack {
        from: "0.59.0".into(),
        to: to.into(),
        reason: "it exited with code 1 before it was ready".into(),
        at: "2026-10-20T09:00:00Z".into(),
    };
    let state = machine(|u| u.rolled_back = Some(undone("0.59.2")));
    assert_eq!((state.kind, state.tone), (Kind::RolledBack, Tone::Bad));
    assert!(state.label.contains("0.59.2 didn't start here"));
    assert!(!has_update_waiting(&state));
    // A newer release staged since is shown as waiting; it isn't the one that failed.
    let newer = machine(|u| {
        u.status = Status::Staged;
        u.staged_version = Some("0.59.3".into());
        u.rolled_back = Some(undone("0.59.2"));
    });
    assert_eq!(newer.kind, Kind::Ready);
}

// ─── rollout progress ─────────────────────────────────────────────────────────

fn step(patch: impl FnOnce(&mut RolloutStep)) -> RolloutStep {
    let mut s = RolloutStep {
        machine_id: "m1".into(),
        name: "falcon".into(),
        state: StepState::Pending,
        detail: None,
        from_version: None,
        to_version: None,
        waiting_sessions: 0,
    };
    patch(&mut s);
    s
}

fn run(status: RolloutStatus, steps: Vec<RolloutStep>, summary: Option<&str>) -> Rollout {
    Rollout {
        id: "r1".into(),
        status,
        started_at: "2026-10-20T09:00:00Z".into(),
        finished_at: None,
        steps,
        summary: summary.map(str::to_string),
    }
}

#[test]
fn maps_each_step_to_the_state_its_row_shows() {
    let waiting = describe_rollout_step(&step(|s| {
        s.state = StepState::Waiting;
        s.detail = Some("waiting for 2 sessions to finish".into());
        s.to_version = Some("0.59.1".into());
    }));
    assert_eq!((waiting.kind, waiting.label.as_str(), waiting.tone), (Kind::Waiting, "waiting for 2 sessions to finish", Tone::Warn));
    let restarting = describe_rollout_step(&step(|s| {
        s.state = StepState::Restarting;
        s.detail = Some("restarting into 0.59.1".into());
    }));
    assert_eq!((restarting.kind, restarting.tone), (Kind::Restarting, Tone::Info));
    let done = describe_rollout_step(&step(|s| {
        s.state = StepState::Done;
        s.to_version = Some("0.59.1".into());
    }));
    assert_eq!((done.kind, done.label.as_str(), done.tone), (Kind::Updated, "updated to 0.59.1 · healthy", Tone::Ok));
    let skipped = describe_rollout_step(&step(|s| {
        s.state = StepState::Skipped;
        s.detail = Some("can't be reached".into());
    }));
    assert_eq!((skipped.kind, skipped.label.as_str()), (Kind::Skipped, "skipped · can't be reached"));
    let pending = describe_rollout_step(&step(|_| {}));
    assert_eq!((pending.kind, pending.label.as_str()), (Kind::Pending, "waiting its turn"));
}

#[test]
fn says_a_failed_machine_stopped_the_rollout() {
    let back = describe_rollout_step(&step(|s| {
        s.state = StepState::RolledBack;
        s.detail = Some("went back to 0.59.0: it exited with code 1 before it was ready".into());
    }));
    assert!(back.label.contains("the rollout stopped here"));
    let gone = describe_rollout_step(&step(|s| {
        s.state = StepState::NoReturn;
        s.detail = Some("didn't come back within 3 minutes".into());
    }));
    assert_eq!((gone.kind, gone.tone), (Kind::NoReturn, Tone::Bad));
}

fn named(name: &str, state: StepState) -> RolloutStep {
    step(|s| {
        s.name = name.into();
        s.state = state;
    })
}

#[test]
fn has_no_outcome_while_it_runs_and_names_who_was_updated_or_skipped_once_it_is_done() {
    assert_eq!(describe_rollout_outcome(Some(&run(RolloutStatus::Running, vec![step(|_| {})], None))), None);
    let done = run(
        RolloutStatus::Done,
        vec![named("heron", StepState::Done), named("falcon", StepState::Done), named("kite", StepState::Skipped)],
        None,
    );
    assert_eq!(describe_rollout_outcome(Some(&done)), Some(("Updated heron and falcon. Skipped kite.".into(), Tone::Ok)));
    assert_eq!(
        describe_rollout_outcome(Some(&run(RolloutStatus::Done, vec![], None))),
        Some(("Nothing needed updating.".into(), Tone::Ok))
    );
}

#[test]
fn says_why_it_stopped_and_who_it_left_alone() {
    let stopped = run(
        RolloutStatus::Stopped,
        vec![named("heron", StepState::RolledBack), named("falcon", StepState::Pending)],
        Some("heron didn't start on 0.59.2 and went back to 0.59.0."),
    );
    assert_eq!(
        describe_rollout_outcome(Some(&stopped)),
        Some(("heron didn't start on 0.59.2 and went back to 0.59.0. falcon was left alone.".into(), Tone::Bad))
    );
    let plain = run(RolloutStatus::Cancelled, vec![named("a", StepState::Pending), named("b", StepState::Pending)], None);
    assert_eq!(describe_rollout_outcome(Some(&plain)), Some(("The rollout stopped. a and b were left alone.".into(), Tone::Muted)));
}

#[test]
fn a_session_waiting_on_an_answer_keeps_the_row_busy_like_a_working_one() {
    assert_eq!(busy_sessions(0, 1), 1);
    assert_eq!(busy_sessions(2, 1), 3);
    let s = machine_at(staged, "0.59.0", busy_sessions(0, 1), true);
    assert_eq!((s.kind, s.label.as_str()), (Kind::Busy, "0.59.1 downloaded · busy: 1 session working or waiting for you"));
}

// ─── update rules ─────────────────────────────────────────────────────────────

#[test]
fn a_machine_follows_its_own_rule_then_the_one_chosen_for_all_then_what_suits_its_kind() {
    assert_eq!(effective_rule(None, None, MachineKind::Node), UpdateRule::Idle);
    assert_eq!(effective_rule(None, None, MachineKind::Fleet), UpdateRule::Ask);
    assert_eq!(effective_rule(None, Some(UpdateRule::Overnight), MachineKind::Node), UpdateRule::Overnight);
    assert_eq!(effective_rule(Some(UpdateRule::Ask), Some(UpdateRule::Overnight), MachineKind::Node), UpdateRule::Ask);
}

#[test]
fn keeps_only_the_overrides_that_name_a_known_rule() {
    let kept = parse_rule_overrides(Some(r#"{"falcon":"idle","heron":"whenever","kestrel":3}"#));
    assert_eq!(kept.into_iter().collect::<Vec<_>>(), [("falcon".to_string(), UpdateRule::Idle)]);
    assert!(parse_rule_overrides(Some("not json")).is_empty());
    assert!(parse_rule_overrides(None).is_empty());
}

// ─── a Fleet that updated itself ──────────────────────────────────────────────

const NOW: i64 = 1_792_497_600_000; // 2026-10-20T12:00:00Z

fn last() -> LastUpdate {
    LastUpdate { from: "0.59.0".into(), to: "0.59.1".into(), at: "2026-10-20T03:12:00Z".into(), trigger: Trigger::Idle }
}

#[test]
fn says_so_for_the_machine_and_version_it_is_running_now() {
    let news = describe_updated_notice("falcon-id", "falcon", Some("0.59.1"), Some(&last()), NOW, 0).expect("news");
    assert_eq!(news.title, "falcon updated to 0.59.1");
    assert_eq!(news.id, "fleet-updated:falcon-id:0.59.1");
    assert_eq!(news.body, "At 3:12 AM, when nothing was working.");
    // The clock is the reader's zone.
    let east = describe_updated_notice("f", "falcon", Some("0.59.1"), Some(&last()), NOW, 2 * 3600).expect("news");
    assert_eq!(east.body, "At 5:12 AM, when nothing was working.");
}

#[test]
fn stays_quiet_when_it_runs_another_version_has_no_record_or_updated_days_ago() {
    assert_eq!(describe_updated_notice("f", "falcon", Some("0.59.0"), Some(&last()), NOW, 0), None);
    assert_eq!(describe_updated_notice("f", "falcon", Some("0.59.1"), None, NOW, 0), None);
    assert_eq!(describe_updated_notice("f", "falcon", Some("0.59.1"), Some(&last()), NOW + 10 * 86_400_000, 0), None);
}

#[test]
fn names_a_different_notice_for_the_next_version() {
    let next = LastUpdate { to: "0.60.0".into(), ..last() };
    let news = describe_updated_notice("falcon-id", "falcon", Some("0.60.0"), Some(&next), NOW, 0).expect("news");
    assert_eq!(news.id, "fleet-updated:falcon-id:0.60.0");
}

// ─── the version line on a machine card ───────────────────────────────────────

fn mrow(patch: impl FnOnce(&mut UpdateRow), state: UpdateState) -> UpdateRow {
    let mut r = row("falcon-id", state);
    r.name = "falcon".into();
    r.where_ = "falcon".into();
    r.rule = Some(RuleChoice { effective: UpdateRule::Idle, overridden: None });
    patch(&mut r);
    r
}

fn rollout_of(steps: Vec<(&str, &str, StepState, u32)>) -> Rollout {
    run(
        RolloutStatus::Running,
        steps
            .into_iter()
            .map(|(id, name, state, waiting)| RolloutStep {
                machine_id: id.into(),
                name: name.into(),
                state,
                detail: None,
                from_version: None,
                to_version: None,
                waiting_sessions: waiting,
            })
            .collect(),
        None,
    )
}

fn ready() -> UpdateState {
    machine(staged)
}

#[test]
fn says_a_downloaded_update_waits_for_the_machines_rule() {
    let busy = describe_version_line(&mrow(|r| r.working = Some(2), machine_at(staged, "0.59.0", 2, true)), None);
    assert_eq!(
        (busy.version.as_str(), busy.target.as_deref(), busy.text.as_str(), busy.tone, busy.rule_menu),
        ("Fleet 0.59.0", Some("0.59.1"), "0.59.1 downloaded · updates itself when its 2 sessions finish", Tone::Warn, true)
    );
    assert!(
        describe_version_line(&mrow(|r| r.working = Some(1), machine_at(staged, "0.59.0", 1, true)), None)
            .text
            .contains("when its session finishes")
    );
    assert_eq!(describe_version_line(&mrow(|_| {}, ready()), None).text, "0.59.1 downloaded · updates itself in a minute");
    let overnight =
        mrow(|r| r.rule = Some(RuleChoice { effective: UpdateRule::Overnight, overridden: Some(UpdateRule::Overnight) }), ready());
    assert_eq!(describe_version_line(&overnight, None).text, "0.59.1 downloaded · updates itself tonight, 02:00–05:00");
    let ask = mrow(|r| r.rule = Some(RuleChoice { effective: UpdateRule::Ask, overridden: None }), ready());
    assert_eq!(describe_version_line(&ask, None).text, "0.59.1 downloaded · waiting for you to restart it");
}

#[test]
fn puts_the_reason_a_machine_cant_restart_itself_in_the_line() {
    let r = mrow(|r| r.restart = Some(RestartOffer::Unavailable { reason: "managed by systemd".into() }), ready());
    assert_eq!(describe_version_line(&r, None).text, "0.59.1 downloaded · managed by systemd");
}

#[test]
fn says_where_a_rollout_has_the_machine() {
    let r = mrow(|_| {}, ready());
    let first = rollout_of(vec![
        ("m0", "heron", StepState::Done, 0),
        ("falcon-id", "falcon", StepState::Waiting, 2),
        ("m2", "kestrel", StepState::Pending, 0),
    ]);
    assert_eq!(describe_version_line(&r, Some(&first)).text, "Updating: waiting for 2 sessions to finish");
    let next = rollout_of(vec![("m0", "falcon", StepState::Waiting, 0), ("falcon-id", "kestrel", StepState::Pending, 0)]);
    assert_eq!(describe_version_line(&r, Some(&next)).text, "Updating: next, after falcon");
    let queued = rollout_of(vec![
        ("m0", "falcon", StepState::Waiting, 0),
        ("m1", "heron", StepState::Pending, 0),
        ("falcon-id", "kestrel", StepState::Pending, 0),
    ]);
    assert_eq!(describe_version_line(&r, Some(&queued)).text, "Updating: next, after heron");
    assert_eq!(
        describe_version_line(&r, Some(&rollout_of(vec![("falcon-id", "x", StepState::Pending, 0)]))).text,
        "Updating: waiting its turn"
    );
    assert_eq!(
        describe_version_line(&r, Some(&rollout_of(vec![("falcon-id", "x", StepState::Restarting, 0)]))).text,
        "Updating: restarting…"
    );
    let mut done = rollout_of(vec![("falcon-id", "x", StepState::Done, 0)]);
    done.steps[0].to_version = Some("0.59.1".into());
    let line = describe_version_line(&r, Some(&done));
    assert_eq!((line.version.as_str(), line.text.as_str(), line.tone), ("Fleet 0.59.1", "updated to 0.59.1 · healthy", Tone::Ok));
}

#[test]
fn says_when_it_cant_be_reached_with_the_version_last_seen_and_no_rule_menu() {
    let line = describe_version_line(&mrow(|r| r.version = Some("0.58.2".into()), machine_at(|_| {}, "0.58.2", 0, false)), None);
    assert_eq!(
        (line.version.as_str(), line.text.as_str(), line.rule_menu),
        ("Fleet 0.58.2", "can't be reached · last seen on 0.58.2", false)
    );
}

#[test]
fn says_up_to_date_and_reuses_the_other_states_words() {
    let line = describe_version_line(&mrow(|r| r.version = Some("0.59.1".into()), machine_at(|_| {}, "0.59.1", 0, true)), None);
    assert_eq!((line.version.as_str(), line.target, line.text.as_str(), line.tone), ("Fleet 0.59.1", None, "up to date", Tone::Ok));
    assert_eq!(describe_version_line(&mrow(|_| {}, machine(|u| u.status = Status::Downloading)), None).text, "downloading 0.59.1…");
    assert_eq!(
        describe_version_line(&mrow(|_| {}, machine(|u| u.status = Status::Available)), None).text,
        "0.59.1 is out, not downloaded yet"
    );
}

#[test]
fn offers_no_rule_menu_for_a_fleet_too_old_to_take_one() {
    assert!(!describe_version_line(&mrow(|r| r.rule = None, machine_at(|_| {}, "0.59.1", 0, true)), None).rule_menu);
}

// ─── the release line ─────────────────────────────────────────────────────────

fn sum(latest: Option<&str>, waiting: usize, downloaded: usize) -> Summary {
    Summary { latest: latest.map(str::to_string), waiting, downloaded, unreachable: 0 }
}

#[test]
fn says_how_many_machines_have_the_release_downloaded() {
    let line = describe_release_line(&sum(Some("0.59.1"), 3, 3), None, NOW, 0);
    assert_eq!(
        line,
        ReleaseLine {
            kind: ReleaseKind::Available,
            lead: "Fleet 0.59.1 is out.".into(),
            text: "3 machines have it downloaded and switch when they restart.".into()
        }
    );
    assert_eq!(
        describe_release_line(&sum(Some("0.59.1"), 1, 1), None, NOW, 0).text,
        "1 machine has it downloaded and switches when it restarts."
    );
    assert_eq!(
        describe_release_line(&sum(Some("0.59.1"), 3, 2), None, NOW, 0).text,
        "2 of 3 machines that can update have it downloaded and switch when they restart."
    );
    assert_eq!(describe_release_line(&sum(Some("0.59.1"), 2, 0), None, NOW, 0).text, "2 machines can update; they aren't downloaded yet.");
    assert_eq!(describe_release_line(&sum(Some("0.59.1"), 1, 0), None, NOW, 0).text, "1 machine can update; it isn't downloaded yet.");
    assert_eq!(
        describe_release_line(&sum(Some("0.59.1"), 3, 1), None, NOW, 0).text,
        "1 of 3 machines that can update has it downloaded and switch when they restart."
    );
}

#[test]
fn is_one_quiet_line_when_there_is_nothing_to_do_or_no_release() {
    let line = describe_release_line(&sum(Some("0.59.1"), 0, 0), Some("2026-10-20T11:56:00Z"), NOW, 0);
    assert_eq!(
        line,
        ReleaseLine {
            kind: ReleaseKind::Current,
            lead: String::new(),
            text: "Every machine that can be reached is on Fleet 0.59.1. Checked 4 minutes ago.".into()
        }
    );
    let none = describe_release_line(&sum(None, 0, 0), None, NOW, 0);
    assert_eq!((none.kind, none.text.as_str()), (ReleaseKind::None, "Fleet hasn't found a release yet."));
}

#[test]
fn finds_the_newest_check_across_machines() {
    let times = [None, Some("2026-10-20T08:00:00Z"), None, Some("2026-10-20T11:00:00Z"), Some("nonsense")];
    assert_eq!(newest_checked_at(times), Some("2026-10-20T11:00:00Z".into()));
    assert_eq!(newest_checked_at([None]), None);
}

#[test]
fn writes_a_time_the_way_fleet_does() {
    let at = |ago_ms: i64| format_relative_words(NOW - ago_ms, NOW, 0);
    assert_eq!([at(10_000), at(60_000), at(3_600_000), at(7_200_000)], ["just now", "1 minute ago", "1 hour ago", "2 hours ago"]);
    assert_eq!(
        [at(86_400_000), at(3 * 86_400_000), at(7 * 86_400_000), at(14 * 86_400_000)],
        ["yesterday", "3 days ago", "last week", "2 weeks ago"]
    );
    assert_eq!(format_relative_words(NOW + 120_000, NOW, 0), "in 2 minutes");
    assert_eq!(at(70 * 86_400_000), "Aug 11");
}

// ─── the rollout narrative ────────────────────────────────────────────────────

#[test]
fn tells_each_machines_part_in_a_sentence() {
    let mut r = rollout_of(vec![
        ("a", "heron", StepState::Done, 0),
        ("b", "falcon", StepState::Waiting, 2),
        ("c", "kestrel", StepState::Pending, 0),
        ("d", "osprey", StepState::Skipped, 0),
    ]);
    r.steps[0].to_version = Some("0.59.1".into());
    r.steps[3].detail = Some("can't be reached".into());
    assert_eq!(
        describe_rollout_narrative(&r, &HashSet::new()),
        Narrative {
            title: "Updating machines to 0.59.1, one at a time.".into(),
            active: Some("falcon".into()),
            text: "heron is done. falcon is waiting for 2 sessions; kestrel is next. osprey can't be reached, so it's skipped.".into(),
        }
    );
}

#[test]
fn has_nobody_active_before_the_first_machine_starts() {
    let r = rollout_of(vec![("a", "a", StepState::Pending, 0), ("b", "b", StepState::Pending, 0)]);
    assert_eq!(describe_rollout_narrative(&r, &HashSet::new()).active, None);
}

#[test]
fn says_a_machine_that_cant_be_reached_will_be_skipped_and_the_next_one_is_the_next_reachable_machine() {
    let r = rollout_of(vec![
        ("m0", "falcon", StepState::Waiting, 1),
        ("osprey-id", "osprey", StepState::Pending, 0),
        ("m2", "heron", StepState::Pending, 0),
    ]);
    let text = describe_rollout_narrative(&r, &HashSet::from(["osprey-id".to_string()])).text;
    assert_eq!(text, "falcon is waiting for 1 session. osprey can't be reached, so it'll be skipped. heron is next.");
}

// ─── the status bar chip ──────────────────────────────────────────────────────

#[test]
fn counts_the_machines_that_can_update() {
    assert_eq!(describe_update_chip(0, None), None);
    assert_eq!(describe_update_chip(1, None).as_deref(), Some("↑ 1 machine can update"));
    assert_eq!(describe_update_chip(3, None).as_deref(), Some("↑ 3 machines can update"));
}

#[test]
fn says_where_a_rollout_is_while_it_runs_one_step_ahead_of_the_finished_ones_and_never_past_the_last() {
    let steps = vec![("a", "a", StepState::Done, 0), ("b", "b", StepState::Waiting, 0), ("c", "c", StepState::Pending, 0)];
    assert_eq!(describe_update_chip(3, Some(&rollout_of(steps.clone()))).as_deref(), Some("Updating machines · 2 of 3"));
    let all = rollout_of(vec![("a", "a", StepState::Done, 0), ("b", "b", StepState::Skipped, 0), ("c", "c", StepState::Done, 0)]);
    assert_eq!(describe_update_chip(0, Some(&all)).as_deref(), Some("Updating machines · 3 of 3"));
    let mut over = rollout_of(steps);
    over.status = RolloutStatus::Done;
    assert_eq!(describe_update_chip(2, Some(&over)).as_deref(), Some("↑ 2 machines can update"));
}

// ─── the idle rule's text ─────────────────────────────────────────────────────

#[test]
fn says_it_waits_15_quiet_minutes_with_no_turn_question_prompt_or_answer_from_any_device_and_that_restart_is_immediate() {
    let idle = UpdateRule::Idle.info().detail;
    for part in ["15 minutes", "no turn", "any device", "Restart is immediate"] {
        assert!(idle.contains(part), "{part}");
    }
    assert!(!idle.contains("a minute or two"));
}
