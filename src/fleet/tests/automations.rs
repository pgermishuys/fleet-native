//! `lib/__tests__/automations.test.ts` under its names (the 13 of it), then the data: an automation and its runs read from
//! the answers a scratch Fleet gave (`tests/fixtures/automations-*.json`), unknown fields ignored, missing ones defaulted.
use super::super::automations::model::*;
use super::super::automations::schedule::{parse_schedule, prompt_from};
use super::super::automations::words::*;
use crate::fleet::new_session::request::Folder;
use crate::mock::automations::{NOW_MS, automations, runs};
use serde_json::json;

fn utc(_: i64) -> i64 {
    0
}

#[derive(Default)]
struct Extra {
    worktree: Option<bool>,
    folder: Option<Option<Folder>>,
    same_session: bool,
    saved_time_zone: Option<Option<&'static str>>,
    workflow: Option<Option<&'static str>>,
}

/// `planFor`: the line as one string, Fleet's way of comparing it.
fn plan_for(text: &str, extra: Extra) -> String {
    let hit = parse_schedule(text);
    let prompt = prompt_from(text, hit.as_ref());
    let folder = extra.folder.unwrap_or(Some(Folder::Repository("/home/me/source/weave-fleet".into())));
    let plan = describe_automation_plan(&PlanInput {
        text,
        when: hit.as_ref().map(|h| &h.when),
        hit: hit.as_ref(),
        prompt: &prompt,
        force_once: false,
        folder: folder.as_ref(),
        worktree: extra.worktree.unwrap_or(true),
        base: Some("origin/main"),
        same_session: extra.same_session,
        workflow: extra.workflow,
        time_zone: "Europe/London",
        saved_time_zone: extra.saved_time_zone,
        legacy_folderless: false,
        from: NOW_MS,
        tz: &utc,
    });
    plan_sentence(&plan)
}

fn base() -> Automation {
    Automation {
        id: "a1".into(),
        is_enabled: true,
        trigger_type: "schedule".into(),
        next_run_at: Some("2026-09-21T09:00:00Z".into()),
        ..Automation::default()
    }
}

fn status(a: &Automation) -> RowStatus {
    automation_row_status(a, NOW_MS, &utc)
}

fn last(state: RunState) -> Option<AutomationRun> {
    Some(AutomationRun { state, ..AutomationRun::default() })
}

// ── describeAutomationPlan ────────────────────────────────────────────────────

#[test]
fn says_when_it_runs_where_each_run_happens_and_what_the_agent_gets() {
    assert_eq!(
        plan_for("Every Monday at 9, summarise the open PRs", Extra::default()),
        "Runs every Monday at 09:00 (Europe/London time), next Mon 21 Sep, 09:00. \
         Each run: a new session in a new worktree of ~/source/weave-fleet from origin/main. \
         The agent gets “Summarise the open PRs”"
    );
}

#[test]
fn says_a_one_off_switches_itself_off() {
    assert!(
        plan_for("Once on Friday at 4pm, tidy the changelog", Extra::default())
            .contains("Runs once, on Fri 18 Sep at 16:00 (Europe/London time), then it switches off.")
    );
}

#[test]
fn asks_about_on_friday() {
    assert!(plan_for("Tidy the changelog on Friday", Extra::default()).contains("Every Friday, or just once? [Just once]"));
}

#[test]
fn describes_the_folder_as_it_is_no_folder_and_continuing_one_session() {
    let daily = "Daily at 8, check CI";
    assert!(
        plan_for(daily, Extra { worktree: Some(false), ..Extra::default() })
            .contains("Each run: a new session in ~/source/weave-fleet as it is.")
    );
    assert!(
        plan_for(daily, Extra { folder: Some(Some(Folder::None)), ..Extra::default() }).contains("Each run: a new session with no folder.")
    );
    assert!(plan_for(daily, Extra { folder: Some(None), ..Extra::default() }).contains("Choose where it runs."));
    assert!(plan_for(daily, Extra { same_session: true, ..Extra::default() }).contains("Later runs continue the first run's session."));
}

#[test]
fn asks_for_the_schedule_or_for_what_to_do() {
    assert!(plan_for("Summarise the open PRs", Extra::default()).contains("When should it run?"));
    assert!(plan_for("Every Monday at 9", Extra::default()).contains("Say what it should do."));
}

#[test]
fn warns_when_saving_moves_a_schedule_to_another_time_zone() {
    let digest = "Every Monday at 9, digest";
    assert!(
        plan_for(digest, Extra { saved_time_zone: Some(None), ..Extra::default() })
            .contains("Saving moves it from UTC time to Europe/London.")
    );
    assert!(!plan_for(digest, Extra { saved_time_zone: Some(Some("Europe/London")), ..Extra::default() }).contains("Saving moves it"));
}

// ── automationRowStatus ───────────────────────────────────────────────────────

#[test]
fn shows_the_next_run_running_failed_or_off() {
    let s = status(&base());
    assert_eq!((s.label.as_str(), s.tone, s.glyph), ("Mon 09:00", Tone::Quiet, None));
    assert_eq!(status(&Automation { last_run: last(RunState::Running), ..base() }).label, "Running");
    let s = status(&Automation { last_run: last(RunState::Failed), ..base() });
    assert_eq!((s.label.as_str(), s.tone, s.glyph), ("Failed", Tone::Error, Some(Glyph::Error)));
    assert_eq!(status(&Automation { is_enabled: false, next_run_at: None, ..base() }).label, "Off");
    assert_eq!(status(&Automation { trigger_type: "event".into(), next_run_at: None, ..base() }).label, "On event");
}

#[test]
fn says_needs_you_while_the_workflow_run_its_latest_run_started_waits_on_the_user_even_when_its_off() {
    let s = status(&Automation { is_enabled: false, last_run: last(RunState::Waiting), ..base() });
    assert_eq!((s.label.as_str(), s.tone, s.glyph), ("Needs you", Tone::Warn, None));
}

// ── describeRunState ──────────────────────────────────────────────────────────

fn state_of(state: RunState, machine: Option<&str>) -> (String, Tone) {
    describe_run_state(&AutomationRun { state, machine_name: machine.map(str::to_string), ..AutomationRun::default() })
}

#[test]
fn follows_a_workflow_run_needs_you_while_it_waits_ended_once_its_ended() {
    assert_eq!(state_of(RunState::Waiting, None), ("Needs you".to_string(), Tone::Warn));
    assert_eq!(state_of(RunState::Ended, None), ("Ended".to_string(), Tone::Quiet));
    assert_eq!(state_of(RunState::Skipped, None), ("Skipped".to_string(), Tone::Warn));
}

#[test]
fn says_a_runs_machine_didnt_answer_rather_than_done() {
    assert_eq!(state_of(RunState::Unanswered, Some("atlas")), ("atlas didn't answer".to_string(), Tone::Warn));
    assert_eq!(state_of(RunState::Unanswered, None).0, "Its machine didn't answer");
}

// ── describeAutomationPlan for a workflow ─────────────────────────────────────

#[test]
fn names_the_workflow_where_it_runs_and_that_check_with_me_is_off() {
    assert_eq!(
        plan_for("Every Monday at 9am, bump the dependencies", Extra { workflow: Some(Some("Build a feature")), ..Extra::default() }),
        "Runs every Monday at 09:00 (Europe/London time), next Mon 21 Sep, 09:00. Each run: Build a feature in a new worktree of \
         ~/source/weave-fleet from origin/main, with your message as the request. Check with me is off; the run stops only where \
         the workflow asks you."
    );
}

#[test]
fn asks_for_the_workflow_and_for_a_repository() {
    let text = "Every Monday at 9am, bump it";
    assert!(plan_for(text, Extra { workflow: Some(None), ..Extra::default() }).contains("Pick the workflow it runs."));
    assert!(
        plan_for(text, Extra { workflow: Some(Some("Build a feature")), folder: Some(None), ..Extra::default() })
            .contains("Each run: Build a feature. Pick the repository it runs in.")
    );
}

// ── describeRunTrigger ────────────────────────────────────────────────────────

#[test]
fn names_what_started_a_run_and_nothing_for_the_schedule() {
    assert_eq!(describe_run_trigger("schedule"), "");
    assert_eq!(describe_run_trigger("catch_up"), "Caught up");
    assert_eq!(describe_run_trigger("manual"), "Run now");
    assert_eq!(describe_run_trigger("session_created"), "A session starts");
    assert_eq!(describe_run_trigger("once"), "Once");
}

// ── What the native port adds ─────────────────────────────────────────────────

#[test]
fn event_types_in_words() {
    assert_eq!(event_types(), ["session_created", "session_archived", "session_deleted", "delegation.created", "delegation.updated"]);
    assert_eq!(describe_event_type("delegation.updated"), "A sub agent's status changes");
    assert_eq!(describe_event_type("session_renamed"), "session_renamed (never fires)");
    assert_eq!(event_type_of("{\"eventType\":\"session_created\"}"), "session_created");
    assert_eq!(event_type_of("not json"), "not json");
    assert_eq!(event_type_of("{\"eventType\":3}"), "{\"eventType\":3}");
    assert_eq!(event_type_of("null"), "null");
}

#[test]
fn a_schedule_saved_without_a_zone_runs_in_utc() {
    assert_eq!(schedule_time_zone(None), "UTC");
    assert_eq!(schedule_time_zone(Some("")), "UTC");
    assert_eq!(schedule_time_zone(Some("Europe/London")), "Europe/London");
}

#[test]
fn an_event_plan_says_when_it_runs_and_a_long_prompt_is_cut() {
    let when = super::super::automations::schedule::When::Event { event_type: "session_created".into() };
    let plan = describe_automation_plan(&PlanInput {
        text: "an automation",
        when: Some(&when),
        hit: None,
        prompt: &"x".repeat(120),
        force_once: false,
        folder: Some(&Folder::Directory("/work/kettle".into())),
        worktree: false,
        base: None,
        same_session: false,
        workflow: None,
        time_zone: "UTC",
        saved_time_zone: None,
        legacy_folderless: false,
        from: NOW_MS,
        tz: &utc,
    });
    assert_eq!(plan.schedule.iter().map(|p| p.text.as_str()).collect::<String>(), "Runs when a session starts.");
    let rest: String = plan.rest.iter().map(|p| p.text.as_str()).collect();
    assert!(rest.ends_with(&format!(" The agent gets “{}…”", "x".repeat(108))), "{rest}");
    assert!(plan.rest.iter().any(|p| p.code && p.text == "/work/kettle"));
}

#[test]
fn a_run_off_with_nothing_to_say_has_an_empty_label() {
    assert_eq!(status(&Automation { next_run_at: None, ..base() }).label, "");
}

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[test]
fn reads_the_list_a_server_answered() {
    let list = Automation::list_from_json(&fixture("automations-list.json"));
    assert_eq!(list.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["On new session", "Nightly tidy", "Weekly digest"]);
    let digest = &list[2];
    assert_eq!(digest.trigger_config, "0 9 * * 1");
    assert_eq!((digest.max_concurrent_runs, digest.max_runs_per_hour, digest.timeout_minutes), (1, 10, 30));
    assert_eq!(digest.time_zone.as_deref(), Some("UTC"));
    assert_eq!(digest.isolation.as_deref(), Some("worktree"));
    assert_eq!(digest.base_branch.as_deref(), Some("origin/main"));
    assert_eq!(digest.next_run_at.as_deref(), Some("2026-11-23T09:00:00.0000000Z"));
    assert_eq!(digest.target_type.as_deref(), Some("new_session"));
    assert!(digest.model.is_none() && digest.builtin.is_none() && digest.workflow_steps.is_none() && digest.target_tags.is_empty());
    let run = digest.last_run.as_ref().unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert_eq!(run.trigger, "manual");
    assert!(run.error.as_deref().unwrap().starts_with("Couldn't start"));
    assert!(list[0].last_run.is_none() && list[0].workspace_id.is_none() && list[0].is_enabled);
}

#[test]
fn reads_the_runs_a_server_answered() {
    let runs = AutomationRun::list_from_json(&fixture("automations-runs.json"));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].automation_id, "01M7WR9X0HH4Y3B7WEZ02JKMDX");
    assert_eq!(runs[0].started_at, "2026-11-20T15:13:22.5085636Z");
    assert!(runs[0].session_id.is_none() && runs[0].scheduled_for.is_none());
    assert_eq!(AutomationRun::list_from_json(&json!({})), []);
}

#[test]
fn ignores_unknown_fields_and_defaults_missing_ones() {
    let a = Automation::from_json(&json!({"id": "x", "name": "n", "somethingNew": {"deep": [1]}, "lastRun": null, "state": "odd"}));
    assert_eq!((a.id.as_str(), a.name.as_str(), a.prompt.as_str()), ("x", "n", ""));
    assert_eq!((a.max_concurrent_runs, a.max_runs_per_hour, a.timeout_minutes, a.is_enabled), (1, 0, 0, false));
    assert!(a.last_run.is_none() && a.target_tags.is_empty() && a.time_zone.is_none());
    assert_eq!(AutomationRun::from_json(&json!({"state": "something-new"})).state, RunState::Done);
    assert_eq!(AutomationRun::from_json(&json!({})).state, RunState::Done);
    assert!(Automation::list_from_json(&json!([1, 2])).is_empty());
    for s in ["starting", "running", "waiting", "done", "ended", "failed", "skipped", "unanswered"] {
        assert_eq!(RunState::parse(s).word(), s);
    }
}

#[test]
fn reads_a_draft_from_a_session() {
    let d = AutomationDraft::from_json(&json!({"prompt": "p", "folder": null, "isolation": "worktree"}));
    assert_eq!((d.prompt.as_str(), d.folder, d.isolation.as_str()), ("p", None, "worktree"));
    assert_eq!(AutomationDraft::from_json(&json!({"prompt": "p", "folder": "/work/kettle", "isolation": "?"})).isolation, "existing");
}

#[test]
fn a_request_leaves_out_what_was_not_set_and_sends_null_to_clear() {
    let r = AutomationRequest {
        name: "n".into(),
        prompt: "p".into(),
        trigger_type: "schedule".into(),
        trigger_config: "0 9 * * 1".into(),
        time_zone: Some(Some("UTC".into())),
        base_branch: Some(None),
        workflow_steps: Some(vec!["review".into()]),
        ..AutomationRequest::default()
    };
    assert_eq!(
        r.to_json(),
        json!({"name": "n", "prompt": "p", "triggerType": "schedule", "triggerConfig": "0 9 * * 1", "timeZone": "UTC", "baseBranch": null,
               "workflowSteps": ["review"]})
    );
}

#[test]
fn the_mock_automations_say_what_their_rows_should() {
    let words: Vec<String> = automations().iter().map(|a| status(a).label).collect();
    assert_eq!(words, ["Mon 09:00", "Wed 02:00", "Off", "On event", "Fri 16:00", "Running", "Failed", "Needs you"]);
    assert_eq!(runs("a").len(), 6);
}
