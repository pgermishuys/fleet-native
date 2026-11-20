//! Component 18, piece A: `lib/__tests__/workflows.test.ts` under its names, then the shapes, the model roles and the store's
//! rules. The run is the invented "Keyboard shortcut sheet" one Fleet's own test builds.
use crate::fleet::workflows::helpers::*;
use crate::fleet::workflows::runs::{StartRequest, WorkflowRuns, error_from, extract_api_error, library_path, run_path};
use crate::fleet::workflows::*;
use serde_json::{Value, json};
use std::collections::HashSet;

#[allow(clippy::too_many_arguments)]
fn step(id: &str, title: &str, kind: &str, state: &str, visits: u32, optional: bool, enabled: bool, role: Option<&str>) -> Value {
    json!({
        "id": id, "title": title, "kind": kind, "state": state, "visits": visits, "optional": optional, "enabled": enabled,
        "outcome": null, "sessionId": null, "model": null, "role": role, "skill": null, "maxLoops": null,
        "finishYou": false, "finishAgent": false, "withYou": false, "outcomes": []
    })
}

fn session(id: &str, step: &str, visit: u32, summary: &str) -> Value {
    json!({
        "sessionId": id, "stepId": step, "visit": visit, "outcome": "ready", "summary": summary, "note": null, "withYou": false,
        "files": [], "filesChecked": false, "filesCommit": null, "filesCommitError": null, "promptMessageId": null,
        "wrapUpMessageId": null, "handOffNote": null
    })
}

fn run_json() -> Value {
    json!({
        "id": "run-1", "workflowId": "builtin:build-a-feature", "workflowName": "Build a feature", "title": "Keyboard shortcut sheet",
        "request": "Press ? to see every keyboard shortcut", "status": "running", "currentStepId": "implement", "result": null,
        "repositoryPath": "/repo", "branch": "fleet/keyboard-shortcut-sheet", "baseBranch": null,
        "worktreePath": "/repo-worktrees/keyboard-shortcut-sheet", "harnessType": "opencode", "createdAt": "2026-09-23T10:00:00Z",
        "updatedAt": "2026-09-23T10:10:00Z", "endedAt": null,
        "steps": [
            step("design", "Design", "agent", "skipped", 0, true, false, Some("strong")),
            step("plan", "Plan", "agent", "done", 2, false, true, Some("strong")),
            step("ok-plan", "Approve the plan", "you", "done", 1, false, true, None),
            step("implement", "Implement", "agent", "running", 1, false, true, Some("standard")),
        ],
        "sessions": [session("s1", "plan", 1, "v1"), session("s2", "plan", 2, "v2"), session("s3", "implement", 1, "")],
        "waiting": null
    })
}

fn run() -> Run {
    Run::from_json(&run_json()).expect("a run")
}

struct Item {
    id: &'static str,
    run: Option<&'static str>,
    title: &'static str,
}

impl RunMember for Item {
    fn id(&self) -> &str {
        self.id
    }
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }
    fn workflow_run_id(&self) -> Option<&str> {
        self.run
    }
}

fn item(id: &'static str, run: Option<&'static str>) -> Item {
    Item { id, run, title: id }
}

// ─── lib/__tests__/workflows.test.ts, "workflows" ───────────────────────────────────────────────────────────────

#[test]
fn names_a_step_session_by_its_step_with_the_pass_when_it_ran_again() {
    let run = run();
    assert_eq!(step_label(&run, "s1").as_deref(), Some("Plan"));
    assert_eq!(step_label(&run, "s2").as_deref(), Some("Plan \u{b7} 2nd pass"));
    assert_eq!(step_label(&run, "s3").as_deref(), Some("Implement"));
    assert_eq!(step_label(&run, "elsewhere"), None);
}

#[test]
fn counts_a_step_among_the_steps_that_run_in_this_run() {
    // Design is off, so Implement is the third of three.
    assert_eq!(step_position(&run(), "s3"), Some((3, 3)));
}

#[test]
fn groups_a_run_s_sessions_under_one_row_where_its_newest_session_was_in_the_order_they_ran() {
    let current = run();
    let sessions = [item("a", None), item("s3", Some("run-1")), item("b", None), item("s1", Some("run-1")), item("s2", Some("run-1"))];

    let entries = group_run_sessions(&sessions, |id| current.sessions.iter().any(|s| s.session_id == id).then_some(&current));

    let names: Vec<String> = entries
        .iter()
        .map(|e| match e {
            ListEntry::Run { run_id, .. } => format!("run:{run_id}"),
            ListEntry::Session(s) => s.id.to_string(),
        })
        .collect();
    assert_eq!(names, ["a", "run:run-1", "b"]);
    let ListEntry::Run { steps, .. } = &entries[1] else { panic!("a run") };
    assert_eq!(steps.iter().map(|(_, label)| label.as_str()).collect::<Vec<_>>(), ["Plan", "Plan \u{b7} 2nd pass", "Implement"]);
}

#[test]
fn groups_a_run_the_store_doesn_t_know_yet_by_the_sessions_run_id_and_titles() {
    let sessions = [Item { id: "x", run: Some("run-9"), title: "Shortcut sheet \u{b7} Plan" }];
    let entries = group_run_sessions(&sessions, |_| None);
    assert_eq!(entries.len(), 1);
    let ListEntry::Run { steps, run, .. } = &entries[0] else { panic!("a run") };
    assert!(run.is_none());
    assert_eq!(steps[0].1, "Plan");
}

#[test]
fn says_what_a_run_is_doing() {
    let mut r = run();
    r.status = RunStatus::Waiting;
    assert_eq!(run_status_label(&r), ("Needs you".to_string(), Tone::Wait));
    r.status = RunStatus::Done;
    r.result = Some("PR #12 opened".into());
    assert_eq!(run_status_label(&r), ("PR #12 opened".to_string(), Tone::Done));
    r.result = None;
    assert_eq!(run_status_label(&r), ("Done".to_string(), Tone::Done));
    r.status = RunStatus::Failed;
    assert_eq!(run_status_label(&r), ("Failed".to_string(), Tone::Fail));
    r.status = RunStatus::Ended;
    assert_eq!(run_status_label(&r), ("Ended".to_string(), Tone::Done));
}

#[test]
fn lists_loops_and_the_skills_the_enabled_steps_use() {
    let workflow = Workflow::from_json(&json!({
        "id": "x", "name": "x",
        "steps": [
            {"id": "design", "title": "Design", "kind": "agent", "optional": true, "skill": "fleet-mockups"},
            {"id": "implement", "title": "Implement", "kind": "agent"},
            {"id": "review", "title": "Review", "kind": "agent", "skill": "fleet-code-review", "routes": {"changes": "implement"}, "maxLoops": 2},
        ]
    }));

    assert_eq!(loop_notes(&workflow), ["Review \u{2192} Implement on changes, at most 2\u{d7}"]);
    let skills = |on: &[&str]| -> Vec<&str> {
        let on: HashSet<String> = on.iter().map(|s| s.to_string()).collect();
        skills_in_use(&workflow, &on).into_iter().map(|(_, skill)| skill).collect()
    };
    assert_eq!(skills(&[]), ["fleet-code-review"]);
    assert_eq!(skills(&["design"]), ["fleet-mockups", "fleet-code-review"]);
}

#[test]
fn knows_the_prompt_a_step_started_with_by_fleet_s_footer() {
    assert!(is_step_prompt(
        "Plan it.\n\nThis is one step of a Fleet workflow. When the step is finished, call fleet_step_done once\u{2026}"
    ));
    assert!(!is_step_prompt("Plan it."));
}

fn mv(outcome: &str, to_title: &str, back: bool, used: Option<u32>, max: Option<u32>, allowed: bool) -> Move {
    Move { outcome: outcome.into(), to: Some("x".into()), to_title: Some(to_title.into()), back, loops_used: used, max_loops: max, allowed }
}

#[test]
fn says_a_run_with_a_step_the_user_finishes_open_is_with_you_not_needs_you() {
    let mut r = run();
    r.with_you = Some(WithYou {
        step_id: "implement".into(),
        step_title: "Implement".into(),
        session_id: "s3".into(),
        moves: vec![mv("ready", "Plan", false, None, None, true)],
        ..Default::default()
    });
    assert_eq!(run_status_label(&r), ("With you".to_string(), Tone::With));
    r.with_you = None;
    assert_eq!(run_status_label(&r), ("Working".to_string(), Tone::Run));
}

#[test]
fn names_the_ways_to_move_on_the_way_the_bar_shows_them() {
    let on = mv("pass", "Open the pull request", false, None, None, true);
    let back = mv("changes", "Implement", true, Some(0), Some(2), true);
    assert_eq!(move_label(&mv("ready", "Plan", false, None, None, true), true), "Move on to Plan");
    assert_eq!(move_label(&Move { to_title: None, ..on.clone() }, true), "Finish the run");
    assert_eq!(move_label(&on, false), "Pass: on to Open the pull request");
    assert_eq!(move_label(&back, false), "Changes: back to Implement (1 of 2)");
    assert_eq!(move_label(&Move { loops_used: Some(1), ..back.clone() }, false), "Changes: back to Implement (2 of 2)");
    let spent = Move { loops_used: Some(2), allowed: false, ..back.clone() };
    assert_eq!(move_label(&spent, false), "Changes: back to Implement (2 of 2 used)");
    assert_eq!(move_blocked_reason("Review", &spent).as_deref(), Some("Review has sent the work back twice, the most this run allows."));
    assert_eq!(move_blocked_reason("Review", &back), None);
    assert!(move_blocked_reason("Review", &Move { max_loops: Some(1), ..spent.clone() }).unwrap().contains("back once,"));
    assert!(move_blocked_reason("Review", &Move { max_loops: Some(3), ..spent }).unwrap().contains("3 times"));
}

#[test]
fn marks_what_fleet_sent_into_a_step_session_the_first_prompt_by_its_id_and_the_wrap_up() {
    let mut json = run_json();
    json["sessions"] = json!([{
        "sessionId": "s3", "stepId": "implement", "visit": 1, "outcome": null, "summary": null, "note": null, "withYou": true,
        "files": [], "filesChecked": false, "filesCommit": null, "filesCommitError": null,
        "promptMessageId": "msg_first", "wrapUpMessageId": "msg_wrap", "handOffNote": null
    }]);
    let current = Run::from_json(&json).unwrap();
    let label = |id: &str, text: &str| workflow_message_label(Some(&current), "s3", id, text);
    assert_eq!(label("msg_first", "Build the plan.").as_deref(), Some("Workflow \u{b7} step 3 of 3 \u{b7} you finish this step"));
    assert_eq!(label("msg_wrap", "The user is moving on to Review.").as_deref(), Some("Fleet \u{b7} you pressed Move on"));
    assert_eq!(label("msg_user", "Make the status bar read from the same list."), None);
    assert_eq!(workflow_message_label(None, "s3", "msg_first", ""), None);
    // Fleet's footer finds a prompt whose id the run doesn't hold.
    assert!(label("msg_other", &format!("Go.\n\n{STEP_FOOTER_START}")).is_some());
    assert_eq!(workflow_message_key(Some(&current), "s3").as_deref(), Some("3/3|msg_first|msg_wrap|true"));
    assert_eq!(workflow_message_key(Some(&current), "nope"), None);
}

#[test]
fn says_check_with_me_changes_only_the_steps_after_the_running_one() {
    let mut running = run();
    running.check_with_me = true;
    assert_eq!(
        check_with_me_note(&running).as_deref(),
        Some(
            "Check with me is on from the next step. This step started on its own, so it still moves on when the agent reports it's done."
        )
    );
    let mut off = run();
    off.check_with_me = false;
    assert_eq!(check_with_me_note(&off), None);

    // The step began with the user and the switch went off: the running step stays theirs.
    let mut began_with_you = run();
    began_with_you.steps[3].with_you = true;
    assert_eq!(
        check_with_me_note(&began_with_you).as_deref(),
        Some("Check with me is off from the next step. You still finish this step, because it started with you.")
    );

    // Design says finish: you, so the switch has nothing to say about it.
    let mut design = run();
    design.steps[3].finish_you = true;
    design.steps[3].with_you = true;
    assert_eq!(check_with_me_note(&design), None);

    // The push says finish: agent, so it runs on its own with Check with me on, and that's not news.
    let mut push = run();
    push.check_with_me = true;
    push.steps[3].finish_agent = true;
    assert_eq!(check_with_me_note(&push), None);
}

// ─── The rest of lib/workflows.ts ────────────────────────────────────────────────────────────────────────────────

#[test]
fn counts_the_next_step_and_the_running_step_the_way_the_note_needs_them() {
    let mut r = run();
    assert_eq!(running_step(&r).map(|s| s.id.as_str()), Some("implement"));
    assert_eq!(next_step_title(&r), None, "Implement is the last enabled step");
    r.current_step_id = Some("plan".into());
    assert_eq!(next_step_title(&r), Some("Approve the plan"));
    assert!(running_step(&r).is_none(), "Plan is done");
    r.current_step_id = Some("implement".into());
    r.status = RunStatus::Done;
    assert!(running_step(&r).is_none(), "a finished run has no running step");
}

#[test]
fn says_ordinals_and_files_and_model_names() {
    let said: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 101, 111].into_iter().map(ordinal).collect();
    assert_eq!(said, ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st", "111th"]);
    assert_eq!(file_name("docs/design/sheet.md"), "sheet.md");
    assert_eq!(file_name("sheet.md"), "sheet.md");
    let files = |l: &[&str]| list_files(&l.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(files(&[]), "");
    assert_eq!(files(&["a.md"]), "a.md");
    assert_eq!(files(&["a.md", "b.html"]), "a.md and b.html");
    assert_eq!(files(&["a.md", "b.html", "c.css"]), "a.md, b.html and c.css");
    assert_eq!(model_short_name(Some("github-copilot/claude-opus-5.5")), "claude-opus-5.5");
    assert_eq!(model_short_name(Some("opus")), "opus");
    assert_eq!(model_short_name(None), "Default model");
}

// ─── The shapes ──────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn reads_a_run_only_when_it_has_an_id_a_status_and_both_lists() {
    assert!(Run::from_json(&run_json()).is_some());
    for field in ["id", "status", "steps", "sessions"] {
        let mut json = run_json();
        json.as_object_mut().unwrap().remove(field);
        assert!(Run::from_json(&json).is_none(), "no {field}");
    }
    let mut wrong = run_json();
    wrong["steps"] = json!("none");
    assert!(Run::from_json(&wrong).is_none());
    assert!(Run::from_json(&Value::Null).is_none());
    assert!(Run::from_json(&json!([])).is_none());
}

#[test]
fn reads_what_a_run_waits_on_and_the_step_the_user_finishes() {
    let mut json = run_json();
    json["status"] = json!("waiting");
    json["checkWithMe"] = json!(true);
    json["startedBy"] = json!({"automationId": "a1", "automationName": "Nightly tidy"});
    json["waiting"] = json!({
        "kind": "you", "stepId": "ok-plan", "stepTitle": "Approve the plan", "message": "Is the plan right?", "question": "Approve?",
        "sessionId": "s2", "files": ["docs/plan.md"], "thenAlone": ["Implement"], "nextYouTitle": "Approve the pull request",
        "choices": [{"id": "approve", "label": "Approve", "note": false, "to": "implement", "involvement": true}, {"id": "redo", "label": "Redo", "note": true, "to": null}]
    });
    json["withYou"] = json!({
        "stepId": "implement", "stepTitle": "Implement", "sessionId": "s3", "files": [], "wrappingUp": true, "outcome": null,
        "moves": [{"outcome": "ready", "to": null, "toTitle": null, "back": false, "loopsUsed": null, "maxLoops": null, "allowed": true}]
    });
    let run = Run::from_json(&json).unwrap();
    let waiting = run.waiting.as_ref().unwrap();
    assert_eq!(waiting.kind, WaitingKind::You);
    assert_eq!(waiting.choices.len(), 2);
    assert!(waiting.choices[0].involvement && !waiting.choices[1].involvement);
    assert_eq!(waiting.choices[1].to, None);
    assert_eq!(
        (waiting.then_alone.as_slice(), waiting.next_you_title.as_deref()),
        (&["Implement".to_string()][..], Some("Approve the pull request"))
    );
    assert!(run.check_with_me);
    assert_eq!(run.started_by.as_ref().unwrap().automation_name, "Nightly tidy");
    let with_you = run.with_you.as_ref().unwrap();
    assert!(with_you.wrapping_up);
    assert_eq!(with_you.moves[0].to, None);
}

#[test]
fn reads_the_library_with_a_workflow_s_steps_loops_and_errors() {
    let library = Library::from_json(&json!({
        "repository": "/repo", "repositoryName": "pager-service",
        "workflows": [
            {"id": "builtin:fix-a-bug", "builtIn": true, "file": null, "name": "Fix a bug", "description": "Find it, fix it", "placeholder": "What is broken?",
             "startsFrom": "main", "runsIn": "worktree", "errors": [],
             "steps": [{"id": "diagnose", "title": "Diagnose", "kind": "agent", "model": "strong", "effort": "high", "skill": "fleet-debug",
                        "outcomes": ["found", "unclear"], "routes": {"unclear": "diagnose"}, "maxLoops": 2, "writes": ["docs/{{slug}}.md"], "prompt": "Find the cause."},
                       {"id": "ok", "title": "Is it right?", "kind": "you", "ask": "Fix it?", "choices": [{"label": "Yes", "to": "end", "note": true}], "finishYou": true}]},
            {"id": "repo:tidy", "builtIn": false, "file": ".weave/workflows/tidy-up-a-flaky-test.yaml", "name": "Tidy up", "errors": ["tidy.yaml:3: no steps"], "steps": []}
        ]
    }));
    assert_eq!((library.repository.as_deref(), library.repository_name.as_deref()), (Some("/repo"), Some("pager-service")));
    let fix = &library.workflows[0];
    assert!(fix.built_in && fix.file.is_none() && fix.errors.is_empty());
    assert_eq!(fix.steps[0].routes, [("unclear".to_string(), "diagnose".to_string())]);
    assert_eq!((fix.steps[0].max_loops, fix.steps[0].kind), (Some(2), StepKind::Agent));
    assert_eq!(fix.steps[1].kind, StepKind::You);
    assert_eq!(fix.steps[1].choices, [Choice { label: "Yes".into(), to: "end".into(), note: true }]);
    assert!(fix.steps[1].finish_you);
    let tidy = &library.workflows[1];
    assert_eq!(tidy.file.as_deref(), Some(".weave/workflows/tidy-up-a-flaky-test.yaml"));
    assert_eq!(tidy.errors.len(), 1);
    assert_eq!(Library::from_json(&json!({})), Library::default());
}

// ─── The model roles (row 6.5) and the model picker's keys (6.10) ────────────────────────────────────────────────

#[test]
fn reads_the_model_roles_preference_and_unreadable_json_as_empty() {
    let roles = parse_model_roles(Some(
        r#"{"opencode":{"strong":{"model":"anthropic/opus-5.5","effort":"high"},"fast":{"model":null}},"claude-code":{"standard":{"model":"sonnet"}}}"#,
    ));
    let choice = &roles["opencode"][&Role::Strong];
    assert_eq!((choice.model.as_deref(), choice.effort.as_deref()), (Some("anthropic/opus-5.5"), Some("high")));
    assert_eq!(roles["opencode"][&Role::Fast], ModelChoice { model: None, effort: None });
    assert!(!roles["opencode"].contains_key(&Role::Standard));
    assert_eq!(roles["claude-code"][&Role::Standard].model.as_deref(), Some("sonnet"));

    for unreadable in [None, Some(""), Some("{"), Some("[]"), Some("\"text\""), Some("null"), Some("3")] {
        assert!(parse_model_roles(unreadable).is_empty(), "{unreadable:?}");
    }
}

#[test]
fn knows_the_roles_and_a_step_s_exact_model() {
    assert_eq!(Role::parse(Some("strong")), Some(Role::Strong));
    assert_eq!(Role::parse(Some("fast")), Some(Role::Fast));
    assert_eq!(Role::parse(Some("anthropic/opus-5.5")), None);
    assert_eq!(Role::parse(None), None);
    assert_eq!(ROLES.map(Role::name), ["Strong", "Standard", "Fast"]);
    assert_eq!(Role::Standard.description(), "Doing the work: writing and running code");
}

#[test]
fn turns_a_model_path_into_the_picker_s_key_and_back() {
    let key = key_from_path(Some("anthropic/opus-5.5"));
    assert_eq!(key, r#"["anthropic","opus-5.5"]"#);
    assert_eq!(path_from_key(&key).as_deref(), Some("anthropic/opus-5.5"));
    // A model id may hold a slash of its own: the path splits at the first.
    assert_eq!(path_from_key(&key_from_path(Some("openrouter/meta/llama-4"))).as_deref(), Some("openrouter/meta/llama-4"));
    for none in [None, Some(""), Some("opus"), Some("/opus"), Some("anthropic/")] {
        assert_eq!(key_from_path(none), "", "{none:?}");
    }
    assert_eq!(path_from_key(""), None);
    assert_eq!(path_from_key("not a key"), None);
}

// ─── The store's rules (rows 11.2, 11.5) ─────────────────────────────────────────────────────────────────────────

fn at(id: &str, created: &str, updated: &str) -> Run {
    let mut r = run();
    r.id = id.into();
    r.created_at = created.into();
    r.updated_at = updated.into();
    r
}

#[test]
fn keeps_the_newer_copy_of_a_run_and_orders_newest_first() {
    let mut store = WorkflowRuns::default();
    assert!(store.upsert(at("a", "2026-09-23T10:00:00Z", "2026-09-23T10:30:00Z")));
    assert!(!store.upsert(at("a", "2026-09-23T10:00:00Z", "2026-09-23T10:10:00Z")), "an older copy is refused");
    assert_eq!(store.get("a").unwrap().updated_at, "2026-09-23T10:30:00Z");
    assert!(store.upsert(at("a", "2026-09-23T10:00:00Z", "2026-09-23T10:30:00Z")), "the same moment replaces");
    store.upsert(at("b", "2026-09-23T11:00:00Z", "2026-09-23T11:00:00Z"));
    store.upsert(at("c", "2026-09-23T09:00:00Z", "2026-09-23T09:00:00Z"));
    assert_eq!(store.ordered().iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["b", "a", "c"]);
}

#[test]
fn finds_a_session_s_run_and_the_sessions_holding_a_card_that_waits() {
    let mut store = WorkflowRuns::default();
    let mut waiting = at("w", "2026-09-23T10:00:00Z", "2026-09-23T10:00:00Z");
    waiting.sessions = vec![RunSession { session_id: "w1".into(), step_id: "plan".into(), ..Default::default() }];
    waiting.status = RunStatus::Waiting;
    waiting.waiting = Some(Waiting {
        kind: WaitingKind::NoOutcome,
        step_id: "plan".into(),
        step_title: "Plan".into(),
        message: "The agent stopped.".into(),
        question: None,
        session_id: Some("w1".into()),
        choices: vec![],
        files: vec![],
        then_alone: vec![],
        next_you_title: None,
    });
    let mut running = at("r", "2026-09-23T11:00:00Z", "2026-09-23T11:00:00Z");
    running.waiting = waiting.waiting.clone();
    store.upsert(waiting);
    store.upsert(running);
    assert_eq!(store.run_for_session("w1").map(|r| r.id.as_str()), Some("w"));
    assert_eq!(store.run_for_session("s3").map(|r| r.id.as_str()), Some("r"));
    assert!(store.run_for_session("").is_none() && store.run_for_session("nobody").is_none());
    assert_eq!(store.waiting_session_ids(), HashSet::from(["w1"]), "only a run whose status is waiting counts");
}

#[test]
fn leaves_undefined_out_of_a_start_request_as_json_stringify_does() {
    let bare = StartRequest {
        workflow_id: "builtin:fix-a-bug".into(),
        directory: "/work/repo".into(),
        request: "Fix it".into(),
        ..Default::default()
    };
    assert_eq!(bare.body(), json!({"workflowId": "builtin:fix-a-bug", "directory": "/work/repo", "request": "Fix it"}));
    let full = StartRequest {
        base_branch: Some("main".into()),
        harness_type: Some("opencode".into()),
        harness_profile_id: Some("p1".into()),
        optional_steps: Some(vec!["design".into()]),
        role_overrides: Some(vec![(Role::Strong, ModelChoice { model: Some("anthropic/opus-5.5".into()), effort: None })]),
        check_with_me: Some(true),
        project_id: Some("proj-1".into()),
        ..bare
    };
    let body = full.body();
    assert_eq!(body["roleOverrides"], json!({"strong": {"model": "anthropic/opus-5.5", "effort": null}}));
    assert_eq!(
        (body["checkWithMe"].clone(), body["projectId"].clone(), body["optionalSteps"].clone()),
        (json!(true), json!("proj-1"), json!(["design"]))
    );
}

#[test]
fn builds_the_routes_and_reads_a_refusal_the_way_fleet_does() {
    assert_eq!(library_path(Some("/work/repo")), "/api/workflows?directory=%2Fwork%2Frepo");
    assert_eq!(library_path(None), "/api/workflows");
    assert_eq!(library_path(Some("")), "/api/workflows");
    assert_eq!(run_path("run 1/x", "/move-on"), "/api/workflows/runs/run%201%2Fx/move-on");

    assert_eq!(error_from(r#"{"error":"Already ended."}"#, "Couldn't end the run."), "Already ended.");
    assert_eq!(error_from(r#"{"detail":"Not allowed","title":"Forbidden"}"#, "x"), "Not allowed");
    assert_eq!(error_from(r#"{"title":"Forbidden"}"#, "x"), "Forbidden");
    assert_eq!(error_from(r#"{"error":{"message":"Nested"}}"#, "x"), "Nested");
    assert_eq!(error_from(r#"{"error":""}"#, "x"), "x", "an empty error says nothing");
    assert_eq!(error_from("not json", "Couldn't end the run."), "Couldn't end the run.");
    assert_eq!(extract_api_error(&json!("plain"), "x"), "plain");
    assert_eq!(extract_api_error(&Value::Null, "x"), "x");
}
