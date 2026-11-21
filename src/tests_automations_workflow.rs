//! Component 19, piece N: a workflow target. Fleet's eight cases of `AutomationWorkflowTarget.test.ts` under their names,
//! against a fake Fleet whose machine has OpenCode (the default) and OpenCode 2, which run workflows, and Claude Code, which
//! doesn't; the repository's Library holds Build a feature (Design and Check it runs optional), the repo's own Weekly
//! dependency bump and a file with errors. Then native cases: the notes the workflow menu gives, a workflow run's row opening
//! where the run waits now, and the harness picked for a workflow being sent.
use crate::fleet::automations::workflow::{PICK_REPOSITORY, TARGET_CHIP, WORKFLOW_CHIP, WorkflowSeed};
use crate::fleet::new_session::harness::HARNESS_CHIP;
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::runs::library_path;
use crate::tests::click;
use crate::tests_automations_agent_model::chip_text;
use crate::tests_automations_composer::{KETTLE, plan, submit_shown, type_over};
use crate::tests_automations_page::{Rig, rig};
use crate::tests_automations_store::automation;
use serde_json::{Value, json};

fn step(id: &str, title: &str, extra: Value) -> Value {
    let mut s = json!({"id": id, "title": title, "kind": "agent", "model": "strong", "optional": false, "outcomes": ["done"]});
    for (k, v) in extra.as_object().into_iter().flatten() {
        s[k] = v.clone();
    }
    s
}

/// The repository's Library, as Fleet's test has it.
fn library() -> Value {
    let build = json!({"id": "builtin:build-a-feature", "builtIn": true, "file": null, "name": "Build a feature",
        "description": "Turns a sentence into a reviewed pull request.", "startsFrom": "sentence", "runsIn": "new-worktree",
        "steps": [step("design", "Design", json!({"optional": true, "optionalHint": "For UI and new features"})),
                  step("plan", "Plan", json!({})), step("verify", "Check it runs", json!({"optional": true}))],
        "errors": []});
    let deps = json!({"id": "repo:deps", "builtIn": false, "file": ".weave/workflows/deps.yaml", "name": "Weekly dependency bump",
        "steps": [step("bump", "Bump", json!({}))], "errors": []});
    let broken = json!({"id": "repo:broken", "builtIn": false, "file": ".weave/workflows/broken.yaml", "name": "Broken",
        "steps": [step("bump", "Bump", json!({}))], "errors": [".weave/workflows/broken.yaml, line 3: steps is empty."]});
    json!({"repository": KETTLE, "repositoryName": "kettle", "workflows": [build, deps, broken]})
}

fn harnesses() -> Value {
    let one = |kind: &str, name: &str, default: bool, steps: bool| {
        json!({"type": kind, "displayName": name, "available": true, "userEnabled": true, "isDefault": default, "state": "ready",
            "capabilities": {"supportsAgents": true, "supportsModelSelection": true, "supportsWorkflowSteps": steps}})
    };
    json!([
        one("opencode", "OpenCode", true, true),
        one("opencode2", "OpenCode 2", false, true),
        one("claude-code", "Claude Code", false, false)
    ])
}

/// The automation that runs Build a feature every Monday with Design on (Fleet's test's `bump`).
fn bump() -> Value {
    automation(
        "a-wf",
        json!({"name": "Weekly dependency bump", "prompt": "Bump the client's dependencies", "workspaceId": KETTLE,
            "targetType": "workflow", "isolation": "worktree", "baseBranch": null, "harnessType": null,
            "workflowId": "builtin:build-a-feature", "workflowSteps": ["design"]}),
    )
}

/// The page against the fake, Workflows on or off, the repository remembered as New session's folder.
fn page(list: Vec<Value>, workflows_on: bool) -> Rig {
    let mut r = rig(list, vec![]);
    *r.home.repositories.lock().unwrap() = vec![json!({"name": "kettle", "path": KETTLE, "parentRoot": "/work"})];
    *r.home.workflows.harnesses.lock().unwrap() = Some(harnesses());
    *r.home.workflows.library.lock().unwrap() = Some(library());
    r.rt.update(move |s, _| {
        s.pickers.prefs.set("Workflows", if workflows_on { "true" } else { "false" });
        s.sidebar.update_new_session_defaults(HOME_MACHINE_KEY, |d| d.remember(&Folder::Repository(KETTLE.into()), &Workspace::New, None));
    });
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    r
}

/// New automation, the folder New session would start in, and the sentence typed.
fn start_new(r: &mut Rig, words: &str) {
    click(&mut r.rt, "new-automation");
    r.frames();
    r.wait("the folder", |s| s.automations_nav.draft.folder.is_some());
    r.frames();
    type_over(r, words);
}

fn open(r: &mut Rig, id: &str) {
    click(&mut r.rt, &format!("automation-row-{id}"));
    r.frames();
    r.frames();
}

/// Opens a chip's menu and picks one of its rows (`choose`).
fn choose(r: &mut Rig, chip: &str, item: &str) {
    click(&mut r.rt, chip);
    r.frames();
    assert!(r.has(item), "{item} in {chip}");
    click(&mut r.rt, item);
    r.frames();
}

fn library_loaded(r: &mut Rig) {
    r.wait("the Library", |s| s.automation_library().workflows.is_some());
    r.frames();
}

fn harnesses_loaded(r: &mut Rig) {
    r.wait("the harnesses that run workflows", |s| s.automation_workflow_harnesses().len() == 2);
    r.frames();
}

/// What a switch says it is (its label: "Design: on").
fn switch(r: &Rig, key: &str) -> String {
    r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label).unwrap_or_else(|| panic!("no {key}"))
}

fn libraries(r: &Rig) -> Vec<String> {
    r.home.workflows.libraries.lock().unwrap().clone()
}

fn send(r: &mut Rig, method: &'static str, path: &'static str) -> Value {
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the request", move |_| heard(method, path) > 0);
    r.home.body_of(method, path)
}

// ─── AutomationWorkflowTarget.test.ts: "An automation that runs a workflow" ────────────────────────────────────────

#[test]
fn switches_the_composer_to_a_workflow_lists_the_folder_s_library_and_sends_the_message_as_the_request() {
    let mut r = page(vec![], true);
    start_new(&mut r, "Every Monday at 9am, bump the client's dependencies");
    assert!(chip_text(&r, TARGET_CHIP).contains("a session"), "{}", chip_text(&r, TARGET_CHIP));
    choose(&mut r, TARGET_CHIP, "automation-target-workflow");
    library_loaded(&mut r);
    harnesses_loaded(&mut r);

    // The Library for the chosen folder, with Build a feature picked first; its optional steps are switches.
    assert_eq!(libraries(&r), [library_path(Some(KETTLE))]);
    assert!(chip_text(&r, TARGET_CHIP).contains("a workflow"));
    assert!(chip_text(&r, WORKFLOW_CHIP).contains("Build a feature"));
    assert!(!r.has("automation-runs-in-chip"));
    assert_eq!(switch(&r, "automation-workflow-step-design"), "Design: off");
    click(&mut r.rt, "automation-workflow-step-design");
    r.frames();
    assert_eq!(switch(&r, "automation-workflow-step-design"), "Design: on");
    assert!(r.has(HARNESS_CHIP));

    let plan = plan(&r);
    assert!(plan.contains("Each run: Build a feature in a new worktree of"), "{plan}");
    assert!(plan.contains("with your message as the request."), "{plan}");
    assert!(plan.contains("Check with me is off"), "{plan}");

    let body = send(&mut r, "POST", "/api/automations");
    let sent = json!({"prompt": body["prompt"], "triggerType": body["triggerType"], "triggerConfig": body["triggerConfig"],
        "targetType": body["targetType"], "workflowId": body["workflowId"], "workflowSteps": body["workflowSteps"],
        "workspaceId": body["workspaceId"], "isolation": body["isolation"], "model": body["model"], "agent": body["agent"],
        "harnessType": body["harnessType"]});
    assert_eq!(
        sent,
        json!({"prompt": "Bump the client's dependencies", "triggerType": "schedule", "triggerConfig": "0 9 * * 1",
            "targetType": "workflow", "workflowId": "builtin:build-a-feature", "workflowSteps": ["design"], "workspaceId": KETTLE,
            "isolation": "worktree", "model": null, "agent": null, "harnessType": null})
    );
}

#[test]
fn lists_the_repo_s_own_workflows_after_the_built_ins_and_a_file_with_errors_can_t_be_picked() {
    let mut r = page(vec![], true);
    start_new(&mut r, "Every Monday at 9am, bump the client's dependencies");
    choose(&mut r, TARGET_CHIP, "automation-target-workflow");
    library_loaded(&mut r);
    choose(&mut r, WORKFLOW_CHIP, "automation-workflow-repo:deps");

    assert!(chip_text(&r, WORKFLOW_CHIP).contains("Weekly dependency bump"));
    // Bump has no optional steps, and the ones Design had are gone with it.
    assert!(!r.has("automation-workflow-step-design"));
    click(&mut r.rt, WORKFLOW_CHIP);
    r.frames();
    let order: Vec<String> =
        r.rt.elements()
            .into_iter()
            .filter_map(|e| e.key.filter(|k| k.starts_with("automation-workflow-") && k.contains(':') && !k.contains("-pop")))
            .collect();
    assert_eq!(order, ["automation-workflow-builtin:build-a-feature", "automation-workflow-repo:deps", "automation-workflow-repo:broken"]);
    assert!(r.texts_of_inside("automation-workflow-repo:broken").contains("The file has errors"));
    click(&mut r.rt, "automation-workflow-repo:broken");
    r.frames();
    assert_eq!(r.rt.state.automations_nav.draft.workflow_id.as_deref(), Some("repo:deps"), "a file with errors can't be picked");
}

#[test]
fn doesn_t_offer_a_workflow_when_workflows_are_off() {
    let mut r = page(vec![], false);
    start_new(&mut r, "Every Monday at 9am, bump the client's dependencies");

    assert!(!r.has(TARGET_CHIP));
    assert!(r.has("automation-runs-in-chip"));
    assert_eq!(libraries(&r), Vec::<String>::new());
}

#[test]
fn warns_on_an_automation_that_runs_a_workflow_while_workflows_are_off() {
    let mut r = page(vec![bump()], false);
    *r.home.runs.lock().unwrap() = vec![json!({"id": "r1", "automationId": "a-wf", "trigger": "schedule",
        "scheduledFor": "2026-09-14T07:00:00.0000000Z", "startedAt": "2026-09-14T07:00:01.0000000Z", "state": "skipped",
        "error": "Skipped: Workflows are turned off in Settings."})];
    open(&mut r, "a-wf");
    r.wait("the runs", |s| s.automations_nav.runs.list.first().is_some_and(|x| x.id == "r1"));
    r.frames();

    let warning = r.texts_of("automation-workflows-off-text").concat();
    assert!(
        warning.contains(
            "This automation runs a workflow, and Workflows are turned off in Settings. Its runs are skipped until they're back on."
        ),
        "{warning}"
    );
    let row = r.texts_of_inside("automation-run-r1");
    assert!(row.contains("Skipped"), "{row}");
    assert!(row.contains("Skipped: Workflows are turned off in Settings."), "{row}");
    // Its workflow still shows by name on the chip, though the Library can't load.
    assert!(chip_text(&r, WORKFLOW_CHIP).contains("build-a-feature"));
}

#[test]
fn opens_a_saved_workflow_automation_with_its_choices_and_offers_save_only_once_one_changes() {
    let mut r = page(vec![bump()], true);
    open(&mut r, "a-wf");
    library_loaded(&mut r);

    assert!(chip_text(&r, WORKFLOW_CHIP).contains("Build a feature"));
    assert_eq!(switch(&r, "automation-workflow-step-design"), "Design: on");
    assert!(!submit_shown(&r), "nothing changed yet");

    click(&mut r.rt, "automation-workflow-step-verify");
    r.frames();
    let body = send(&mut r, "PUT", "/api/automations/a-wf");

    let sent = json!({"targetType": body["targetType"], "workflowSteps": body["workflowSteps"], "harnessType": body["harnessType"]});
    assert_eq!(sent, json!({"targetType": "workflow", "workflowSteps": ["design", "verify"], "harnessType": null}));
}

#[test]
fn says_why_run_now_was_skipped_rather_than_that_it_started() {
    let mut r = page(vec![bump()], false);
    *r.home.run_answer.lock().unwrap() = Some(json!({"state": "skipped", "error": "Skipped: Workflows are turned off in Settings."}));
    open(&mut r, "a-wf");
    click(&mut r.rt, "automation-run-now");
    r.wait("the note", |s| s.automations_nav.notice.is_some());
    r.frames();

    assert_eq!(r.texts_of("automation-action-message-text"), ["Skipped: Workflows are turned off in Settings."]);
}

#[test]
fn shows_a_run_that_waits_on_you_as_needs_you_and_opens_it() {
    let mut r = page(vec![bump()], true);
    let step = r.rt.state.projects[0].sessions[0].id.clone();
    *r.home.runs.lock().unwrap() = vec![
        json!({"id": "r1", "automationId": "a-wf", "trigger": "schedule", "scheduledFor": "2026-09-14T07:00:00.0000000Z",
            "startedAt": "2026-09-14T07:00:01.0000000Z", "state": "waiting", "sessionId": step, "workflowRunId": "run-1"}),
        json!({"id": "r2", "automationId": "a-wf", "trigger": "schedule", "scheduledFor": "2026-09-21T07:00:00.0000000Z",
            "startedAt": "2026-09-21T07:00:01.0000000Z", "state": "skipped",
            "error": "Skipped: the last run is still waiting on you (Approve the plan)."}),
    ];
    open(&mut r, "a-wf");
    r.wait("the runs", |s| s.automations_nav.runs.list.len() == 2);
    r.frames();

    let waiting = r.texts_of_inside("automation-run-r1");
    assert!(waiting.contains("Needs you"), "{waiting}");
    assert!(waiting.contains("Open run →"), "{waiting}");
    assert!(r.texts_of_inside("automation-run-r2").contains("Skipped: the last run is still waiting on you (Approve the plan)."));
    click(&mut r.rt, "automation-run-r1");
    assert_eq!(r.rt.state.rail, crate::Rail::Sessions);
    assert_eq!(r.rt.state.route, crate::fleet::store::Route::Session(step));
}

#[test]
fn starts_from_the_library_s_repeat_on_a_schedule_with_the_run_box_s_choices() {
    let mut r = rig(vec![], vec![]);
    *r.home.repositories.lock().unwrap() = vec![json!({"name": "kettle", "path": KETTLE, "parentRoot": "/work"})];
    *r.home.workflows.harnesses.lock().unwrap() = Some(harnesses());
    *r.home.workflows.library.lock().unwrap() = Some(library());
    r.rt.update(|s, _| {
        s.pickers.prefs.set("Workflows", "true");
        s.start_create_from_workflow(&WorkflowSeed {
            workflow_id: "builtin:build-a-feature".into(),
            folder: KETTLE.into(),
            request: "Bump the client's dependencies".into(),
            optional_steps: vec!["verify".into()],
            base_branch: Some("main".into()),
            harness_type: Some("opencode2".into()),
        });
    });
    r.open();
    library_loaded(&mut r);
    harnesses_loaded(&mut r);

    assert_eq!(r.rt.state.automations_nav.draft_box.text, "Bump the client's dependencies");
    assert!(chip_text(&r, TARGET_CHIP).contains("a workflow"));
    assert!(chip_text(&r, WORKFLOW_CHIP).contains("Build a feature"));
    assert_eq!(switch(&r, "automation-workflow-step-verify"), "Check it runs: on");
    assert_eq!(switch(&r, "automation-workflow-step-design"), "Design: off");
    assert!(chip_text(&r, HARNESS_CHIP).contains("OpenCode 2"));
    // When is still to add.
    assert!(plan(&r).contains("When should it run?"), "{}", plan(&r));
}

// ─── native ─────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_workflow_menu_says_to_pick_a_repository_first_and_asks_no_library() {
    let mut r = page(vec![], true);
    start_new(&mut r, "Every Monday at 9am, bump the client's dependencies");
    r.rt.update(|s, _| s.automations_nav.draft.set_folder(Folder::None, true, false));
    choose(&mut r, TARGET_CHIP, "automation-target-workflow");
    click(&mut r.rt, WORKFLOW_CHIP);
    r.frames();

    assert_eq!(chip_text(&r, WORKFLOW_CHIP), "Pick a workflow");
    assert!(r.words().contains(PICK_REPOSITORY), "{}", r.words());
    assert!(libraries(&r).is_empty());
}

#[test]
fn a_workflow_run_opens_the_session_it_waits_in_now() {
    let mut r = page(vec![bump()], true);
    let (started, waits) = (r.rt.state.projects[0].sessions[0].id.clone(), r.rt.state.projects[0].sessions[1].id.clone());
    // The run moved on since: it waits at a later step, in another session.
    *r.home.workflows.runs.lock().unwrap() = Some(json!([{"id": "run-1", "workflowId": "builtin:build-a-feature",
        "workflowName": "Build a feature", "status": "waiting", "request": "Bump", "directory": KETTLE, "steps": [], "sessions": [],
        "createdAt": "2026-09-14T07:00:01Z", "waiting": {"kind": "you", "stepId": "plan", "stepTitle": "Approve the plan",
        "sessionId": waits, "choices": []}}]));
    *r.home.runs.lock().unwrap() = vec![json!({"id": "r1", "automationId": "a-wf", "trigger": "schedule",
        "startedAt": "2026-09-14T07:00:01.0000000Z", "state": "waiting", "sessionId": started, "workflowRunId": "run-1"})];
    // The page was entered before the fake had the run: read the workflow runs again.
    r.rt.update(|s, _| s.workflows = Default::default());
    r.rt.update(|s, _| s.workflow_runs_ensure_loaded(HOME_MACHINE_KEY));
    r.wait("the workflow run", |s| s.workflow_runs(HOME_MACHINE_KEY).get("run-1").is_some());
    open(&mut r, "a-wf");
    r.wait("the runs", |s| !s.automations_nav.runs.list.is_empty());
    r.frames();

    click(&mut r.rt, "automation-run-r1");
    assert_eq!(r.rt.state.route, crate::fleet::store::Route::Session(waits));
}

#[test]
fn the_harness_picked_for_a_workflow_is_sent_and_only_those_that_run_workflows_are_offered() {
    let mut r = page(vec![], true);
    start_new(&mut r, "Every Monday at 9am, bump the client's dependencies");
    choose(&mut r, TARGET_CHIP, "automation-target-workflow");
    library_loaded(&mut r);
    harnesses_loaded(&mut r);
    assert!(chip_text(&r, HARNESS_CHIP).contains("OpenCode"));
    click(&mut r.rt, HARNESS_CHIP);
    r.frames();
    assert!(r.has(&format!("{HARNESS_CHIP}-opencode2")) && !r.has(&format!("{HARNESS_CHIP}-claude-code")));
    click(&mut r.rt, &format!("{HARNESS_CHIP}-opencode2"));
    r.frames();
    assert!(chip_text(&r, HARNESS_CHIP).contains("OpenCode 2"));

    let body = send(&mut r, "POST", "/api/automations");
    assert_eq!(body["harnessType"], "opencode2");
}
