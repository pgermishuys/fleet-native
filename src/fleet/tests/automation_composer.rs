//! The composer's state and request (`src/fleet/automations/composer.rs`), ahead of its view (piece I). Each case is one of
//! the request bodies Fleet's `AutomationsPage.test.ts` and `AutomationWorkflowTarget.test.ts` check, named after its case,
//! or one of the rules of `AutomationComposer.vue` and `AutomationDetailPanel.vue` the checklist names (7.10, 7.13-7.18,
//! 9.3, 9.4, 11.2, 12.2, 12.7). Fleet's clock: Tuesday 15 September 2026, 10:00, the offset fixed to 0.
use super::super::automations::composer::*;
use super::super::automations::model::Automation;
use super::super::automations::nav::Nav;
use super::super::automations::schedule::When;
use super::super::automations::words::plan_sentence;
use crate::composer::choice::{ModelReference, key_for_model};
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::workflows::{Step, Workflow};
use crate::mock::automations::{NOW_MS, REPOSITORY};
use serde_json::{Value, json};

const ZONE: &str = "Europe/London";

fn utc(_: i64) -> i64 {
    0
}

fn ctx(automation: Option<&Automation>) -> Context<'_> {
    Context::new(automation, ZONE, NOW_MS, &utc)
}

fn repo() -> Folder {
    Folder::Repository(REPOSITORY.into())
}

/// A new automation as Fleet's test starts one: the last folder used is the repository, then the sentence is typed.
fn start_new(text: &str) -> ComposerState {
    let mut s = ComposerState::fresh();
    s.apply_initial_folder(false, true, Some(repo()), false);
    s.set_text(text);
    s
}

fn body(s: &ComposerState, c: &Context) -> Value {
    s.request(c).expect("a request").to_json()
}

/// `weekly` of Fleet's test: every Monday at 9 in a worktree of the repository.
fn weekly() -> Automation {
    Automation {
        id: "a1".into(),
        name: "Weekly kettle digest".into(),
        prompt: "Summarise the open PRs".into(),
        trigger_type: "schedule".into(),
        trigger_config: "0 9 * * 1".into(),
        max_concurrent_runs: 1,
        max_runs_per_hour: 10,
        timeout_minutes: 30,
        is_enabled: true,
        workspace_id: Some(REPOSITORY.into()),
        target_type: Some("new_session".into()),
        time_zone: Some(ZONE.into()),
        isolation: Some("worktree".into()),
        ..Automation::default()
    }
}

/// `bump` of the workflow test: a weekly workflow automation with its Design step on.
fn bump() -> Automation {
    Automation {
        id: "a-wf".into(),
        name: "Weekly dependency bump".into(),
        prompt: "Bump the client's dependencies".into(),
        target_type: Some("workflow".into()),
        workflow_id: Some("builtin:build-a-feature".into()),
        workflow_steps: Some(vec!["design".into()]),
        ..weekly()
    }
}

fn library() -> Vec<Workflow> {
    let step = |id: &str, optional: bool| Step { id: id.into(), title: id.into(), optional, ..Step::default() };
    vec![
        Workflow {
            id: "builtin:build-a-feature".into(),
            built_in: true,
            name: "Build a feature".into(),
            steps: vec![step("design", true), step("build", false), step("verify", true)],
            ..Workflow::default()
        },
        Workflow { id: "repo:broken".into(), name: "Broken".into(), errors: vec!["line 3: steps is empty.".into()], ..Workflow::default() },
    ]
}

#[test]
fn reads_when_it_runs_out_of_the_sentence_and_creates_it_on_in_a_new_worktree_each_run() {
    let s = start_new("Every Monday at 9am, summarise the open PRs");
    let c = ctx(None);
    assert_eq!(&s.text[s.highlight().unwrap()], "Every Monday at 9am");
    assert_eq!(s.title(), "Summarise the open PRs", "the draft row");
    let plan = plan_sentence(&s.plan(&c));
    assert!(plan.contains("a new worktree of /work/kettle"), "{plan}");
    assert!(s.can_submit(&c, false));
    assert!(!s.can_submit(&c, true), "not while saving");
    // The whole body, so nothing is sent that Fleet leaves out (a session target has no workflow fields).
    assert_eq!(
        body(&s, &c),
        json!({
            "name": "Summarise the open PRs",
            "prompt": "Summarise the open PRs",
            "triggerType": "schedule",
            "triggerConfig": "0 9 * * 1",
            "maxConcurrentRuns": 1,
            "maxRunsPerHour": 10,
            "timeoutMinutes": 30,
            "workspaceId": REPOSITORY,
            "model": null,
            "agent": null,
            "harnessType": null,
            "targetType": "new_session",
            "targetTags": [],
            "timeZone": ZONE,
            "isolation": "worktree",
            "baseBranch": null,
            "targetMachineId": null,
        })
    );
}

#[test]
fn stores_once_on_a_monday_as_that_mondays_date_and_leaves_the_words_to_fleet() {
    let s = start_new("Create an automation that runs once on a monday to check whether the release notes cover everything.");
    let b = body(&s, &ctx(None));
    assert_eq!(b["prompt"], "Check whether the release notes cover everything.");
    assert_eq!(b["triggerType"], "once");
    assert_eq!(b["triggerConfig"], "2026-09-21T09:00");
}

#[test]
fn asks_when_it_should_run_and_wont_create_one_without_it() {
    let c = ctx(None);
    let s = start_new("Summarise the open PRs");
    assert_eq!(s.when(), None);
    assert!(s.request(&c).is_none());
    assert!(!s.can_submit(&c, false));
    assert!(s.show_send(&c), "a new automation always shows Send, disabled");
    // A schedule with nothing to do is no request either (7.16).
    let s = start_new("Every Monday at 9am");
    assert!(s.when().is_some());
    assert!(s.request(&c).is_none());
}

#[test]
fn asks_whether_on_friday_means_every_friday_and_can_make_it_just_once() {
    let c = ctx(None);
    let mut s = start_new("Tidy the changelog on Friday.");
    assert!(matches!(s.when(), Some(When::Weekly { ambiguous: true, .. })));
    s.set_force_once(true);
    let b = body(&s, &c);
    assert_eq!((b["triggerType"].as_str(), b["triggerConfig"].as_str()), (Some("once"), Some("2026-09-18T09:00")));

    // The same phrase elsewhere in a changed sentence is the same question, still answered.
    s.set_text("Tidy the changelog and the readme on Friday.");
    assert!(s.force_once, "the phrase didn't change");

    // "Just once" answered that sentence; a new one that says "every" repeats, and coming back asks again (7.10).
    s.set_text("Every Monday at 9am, summarise the open PRs");
    assert!(!s.force_once);
    assert!(matches!(s.when(), Some(When::Weekly { ambiguous: false, .. })));
    s.set_text("Tidy the changelog on Friday.");
    assert!(!s.force_once);
    assert!(matches!(s.when(), Some(When::Weekly { ambiguous: true, .. })));

    // Choosing in the When menu takes over, and clears the answer too.
    s.set_force_once(true);
    s.set_when(Some(When::Hours { n: 2 }));
    assert!(!s.force_once);
    assert_eq!(s.highlight(), None, "the words no longer decide");
    assert_eq!(body(&s, &c)["triggerConfig"], "0 */2 * * *");
}

#[test]
fn offers_save_only_once_something_changed_and_keeps_the_schedule_it_had() {
    // A prompt as someone typed it, lower case and all, isn't a change.
    let saved = Automation { prompt: "summarise the open PRs:".into(), ..weekly() };
    let c = ctx(Some(&saved));
    let mut s = ComposerState::state_for(&saved, &[]);
    assert!(!s.is_dirty(&c));
    assert!(!s.show_send(&c));
    assert!(!s.can_submit(&c, false));

    s.set_text("Summarise the open PRs and the drafts");
    assert!(s.is_dirty(&c) && s.show_send(&c) && s.can_submit(&c, false));
    let b = body(&s, &c);
    assert_eq!(b["prompt"], "Summarise the open PRs and the drafts");
    assert_eq!((b["triggerType"].as_str(), b["triggerConfig"].as_str()), (Some("schedule"), Some("0 9 * * 1")));
    assert_eq!(b["isolation"], "worktree");
    assert_eq!(b["name"], "Weekly kettle digest", "the saved name");

    // Back to what it was is no change.
    s.set_text("summarise the open PRs:");
    assert!(!s.is_dirty(&c));
}

#[test]
fn renames_an_automation_saving_the_typed_name() {
    let saved = weekly();
    let c = ctx(Some(&saved));
    let mut s = ComposerState::state_for(&saved, &[]);
    s.name = "Monday kettle digest".into();
    assert!(s.is_dirty(&c));
    assert_eq!(body(&s, &c)["name"], "Monday kettle digest");
}

#[test]
fn warns_that_saving_moves_an_older_utc_schedule_to_your_time_zone() {
    let saved = Automation { time_zone: None, ..weekly() };
    let plan = plan_sentence(&ComposerState::state_for(&saved, &[]).plan(&ctx(Some(&saved))));
    assert!(plan.contains("Saving moves it from UTC time to Europe/London."), "{plan}");
}

#[test]
fn runs_a_new_automation_on_the_agent_and_model_picked_for_it_on_that_harness() {
    let mut s = start_new("Every Monday at 9am, summarise the open PRs");
    s.agent = "tapestry".into();
    s.model = key_for_model(&ModelReference::new("github-copilot", "claude-haiku-4.5"));
    let c = Context { default_harness: Some("opencode"), ..ctx(None) };
    let b = body(&s, &c);
    assert_eq!(
        (&b["agent"], &b["model"], &b["harnessType"]),
        (&json!("tapestry"), &json!("github-copilot/claude-haiku-4.5"), &json!("opencode"))
    );
}

#[test]
fn leaves_the_agent_model_and_harness_empty_when_both_are_default() {
    let s = start_new("Every Monday at 9am, summarise the open PRs");
    let c = Context { default_harness: Some("opencode"), ..ctx(None) };
    let b = body(&s, &c);
    assert_eq!((&b["agent"], &b["model"], &b["harnessType"]), (&Value::Null, &Value::Null, &Value::Null));
}

#[test]
fn shows_an_automations_agent_and_model_and_offers_save_once_one_changes() {
    let saved = Automation {
        agent: Some("tapestry".into()),
        model: Some("github-copilot/claude-haiku-4.5".into()),
        harness_type: Some("opencode".into()),
        ..weekly()
    };
    let c = Context { default_harness: Some("claude"), ..ctx(Some(&saved)) };
    let mut s = ComposerState::state_for(&saved, &[]);
    assert_eq!(s.agent, "tapestry");
    assert_eq!(s.model, key_for_model(&ModelReference::new("github-copilot", "claude-haiku-4.5")));
    assert!(!s.is_dirty(&c));

    s.model.clear();
    assert!(s.is_dirty(&c));
    let b = body(&s, &c);
    assert_eq!((&b["agent"], &b["model"], &b["harnessType"]), (&json!("tapestry"), &Value::Null, &json!("opencode")));
}

#[test]
fn opens_an_automation_as_it_is_saved() {
    // 7.13: everything the composer shows, from the automation.
    let saved = Automation {
        trigger_type: "schedule".into(),
        trigger_config: "15 4 1-7 * 2".into(),
        isolation: Some("existing".into()),
        workspace_id: Some("/work/notes".into()),
        max_concurrent_runs: 0,
        target_machine_id: Some("m-2".into()),
        ..weekly()
    };
    let s = ComposerState::state_for(&saved, &[]);
    assert_eq!(s.text, "Summarise the open PRs");
    assert_eq!(s.manual_when, Some(When::Cron { expr: "15 4 1-7 * 2".into() }), "a cron it has no words for is Custom");
    assert_eq!(s.folder, Some(Folder::Directory("/work/notes".into())), "not a repository the list knows");
    assert_eq!(s.workspace, Workspace::Current);
    assert!(s.has_chosen_folder);
    assert_eq!(s.name, "Weekly kettle digest");
    assert!(!s.skip, "maxConcurrentRuns 0 doesn't skip");
    assert_eq!(s.machine_id.as_deref(), Some("m-2"));
    assert!(!s.is_dirty(&ctx(Some(&saved))));

    // A worktree is always a repository; a folder the list knows is one too.
    let based = Automation { base_branch: Some("origin/main".into()), ..weekly() };
    let b = ComposerState::state_for(&based, &[]);
    assert_eq!((b.folder.clone(), b.base_branch.as_deref()), (Some(repo()), Some("origin/main")));
    assert!(!b.is_dirty(&ctx(Some(&based))));
    let listed = ComposerState::state_for(&saved, &["/work/notes".into()]);
    assert_eq!(listed.folder, Some(Folder::Repository("/work/notes".into())));
    assert_eq!(ComposerState::state_for(&weekly(), &[]).manual_when, Some(When::Weekly { days: vec![1], t: (9, 0), ambiguous: false }));
}

#[test]
fn keeps_an_older_folderless_automation_in_the_first_workspace_root() {
    // Made before folders were kept: no isolation, no folder. Its request keeps isolation null until a folder is picked.
    let older = Automation { isolation: None, workspace_id: None, target_type: None, ..weekly() };
    let c = ctx(Some(&older));
    let mut s = ComposerState::state_for(&older, &[]);
    assert_eq!(s.folder, Some(Folder::None));
    assert_eq!(s.target_type, "new_session");
    assert!(!s.is_dirty(&c), "an older target type of none reads as new_session");
    let b = body(&s, &c);
    assert_eq!((&b["isolation"], &b["workspaceId"]), (&Value::Null, &Value::Null));

    s.set_folder(repo(), true, true);
    assert_eq!(body(&s, &c)["isolation"], "existing", "editing keeps the folder as it is");
    assert!(s.is_dirty(&c));

    // An older one with a folder ran in it as it is: "existing" is no change.
    let with_folder = Automation { isolation: None, workspace_id: Some("/work/notes".into()), ..weekly() };
    let s = ComposerState::state_for(&with_folder, &[]);
    assert!(!s.is_dirty(&ctx(Some(&with_folder))));
}

#[test]
fn a_folder_read_before_the_repositories_arrived_becomes_a_repository() {
    // 7.14, for both the draft and the edit state.
    let mut nav = Nav::default();
    nav.draft.folder = Some(Folder::Directory(REPOSITORY.into()));
    nav.edit.folder = Some(Folder::Directory("/work/notes".into()));
    nav.repositories_changed(&[REPOSITORY.into()]);
    assert_eq!(nav.draft.folder, Some(repo()));
    assert_eq!(nav.edit.folder, Some(Folder::Directory("/work/notes".into())), "not one the list has");
    // The draft row reads the sentence, and a reset draft is a fresh one.
    nav.draft.set_text("Every Monday at 9am, summarise the open PRs");
    assert_eq!(nav.draft_title(), "Summarise the open PRs");
    nav.reset_draft();
    assert_eq!((nav.draft.clone(), nav.draft_title()), (ComposerState::fresh(), "New automation".to_string()));
}

#[test]
fn a_new_automation_starts_in_the_folder_new_session_would() {
    // 9.3: once the repositories are ready, nothing chosen, never a plain folder in cloud mode, never when editing.
    let mut s = ComposerState::fresh();
    s.apply_initial_folder(false, false, Some(repo()), false);
    assert_eq!(s.folder, None, "the repositories haven't loaded");
    s.apply_initial_folder(true, true, Some(repo()), false);
    assert_eq!(s.folder, None, "editing never changes the folder by itself");
    s.apply_initial_folder(false, true, Some(Folder::Directory("/work/notes".into())), true);
    assert_eq!(s.folder, None, "a plain folder in cloud mode");
    s.apply_initial_folder(false, true, Some(repo()), false);
    assert_eq!(s.folder, Some(repo()));
    assert!(!s.has_chosen_folder, "a default, not a choice");
    assert_eq!(s.workspace, Workspace::New);

    let mut chosen = ComposerState::fresh();
    chosen.set_folder(Folder::None, true, false);
    chosen.apply_initial_folder(false, true, Some(repo()), false);
    assert_eq!(chosen.folder, Some(Folder::None), "the person chose no folder");
}

#[test]
fn another_folder_clears_the_base_and_a_new_one_gets_a_new_worktree() {
    // 9.4.
    let mut s = start_new("Every Monday at 9am, summarise the open PRs");
    s.base_branch = Some("origin/develop".into());
    s.workspace = Workspace::Current;
    s.set_folder(repo(), true, false);
    assert_eq!(s.base_branch.as_deref(), Some("origin/develop"), "the same place keeps the base");
    assert_eq!(s.workspace, Workspace::Current, "and the workspace");
    assert!(s.has_chosen_folder);

    s.set_folder(Folder::Repository("/work/teapot".into()), true, false);
    assert_eq!(s.base_branch, None);
    assert_eq!(s.workspace, Workspace::New);

    let saved = Automation { isolation: Some("existing".into()), ..weekly() };
    let mut e = ComposerState::state_for(&saved, &[REPOSITORY.into()]);
    e.set_folder(Folder::Repository("/work/teapot".into()), true, true);
    assert_eq!(e.workspace, Workspace::Current, "editing keeps how it runs");
    assert_eq!(body(&e, &ctx(Some(&saved)))["isolation"], "existing");

    // The base is sent only for a worktree.
    s.base_branch = Some("origin/main".into());
    assert_eq!(body(&s, &ctx(None))["baseBranch"], "origin/main");
    s.workspace = Workspace::Current;
    assert_eq!(body(&s, &ctx(None))["baseBranch"], Value::Null);
}

#[test]
fn picking_a_machine_clears_what_was_picked_on_this_one() {
    // 11.2.
    let mut s = start_new("Every Monday at 9am, summarise the open PRs");
    s.base_branch = Some("origin/main".into());
    s.agent = "tapestry".into();
    s.model = key_for_model(&ModelReference::new("acme", "m1"));
    s.harness_type = Some("opencode".into());
    s.workflow_id = Some("builtin:build-a-feature".into());
    s.workflow_steps = vec!["design".into()];
    s.target_type = "most_recent".into();
    s.pick_machine("m-2", "live");
    assert_eq!(s.machine_id.as_deref(), Some("m-2"));
    assert_eq!((s.folder.clone(), s.has_chosen_folder, s.base_branch.clone()), (None, false, None));
    assert_eq!(s.workspace, Workspace::New);
    assert!(s.agent.is_empty() && s.model.is_empty() && s.harness_type.is_none());
    assert!(s.workflow_id.is_none() && s.workflow_steps.is_empty());
    assert_eq!(s.target_type, "new_session", "an older target picks a session on this machine");
    assert_eq!(s.text, "Every Monday at 9am, summarise the open PRs", "what it does stays");

    // Picking the same machine again changes nothing; this machine is None.
    s.set_folder(repo(), true, false);
    s.pick_machine("m-2", "live");
    assert_eq!(s.folder, Some(repo()));
    s.pick_machine("live", "live");
    assert_eq!((s.machine_id.clone(), s.folder.clone()), (None, None));
    s.set_folder(repo(), true, false);
    assert_eq!(body(&s, &ctx(None))["targetMachineId"], Value::Null);
    s.pick_machine("m-3", "live");
    s.set_folder(repo(), true, false);
    assert_eq!(body(&s, &ctx(None))["targetMachineId"], "m-3");
}

#[test]
fn switching_kind_clears_agent_model_and_harness() {
    // 12.2.
    let mut s = start_new("Every Monday at 9am, summarise the open PRs");
    s.agent = "tapestry".into();
    s.harness_type = Some("opencode".into());
    s.workspace = Workspace::Current;
    s.set_target_kind(TargetKind::Workflow);
    assert!(s.is_workflow());
    assert!(s.agent.is_empty() && s.harness_type.is_none());
    assert_eq!(s.workspace, Workspace::New, "a workflow gets a new worktree");
    assert_eq!(s.folder, Some(repo()));

    s.harness_type = Some("opencode".into());
    s.set_target_kind(TargetKind::Workflow);
    assert_eq!(s.harness_type.as_deref(), Some("opencode"), "the same kind changes nothing");
    s.set_target_kind(TargetKind::Session);
    assert_eq!(s.target_type, "new_session");
    assert!(s.harness_type.is_none());

    let mut plain = start_new("Every Monday at 9am, summarise the open PRs");
    plain.set_folder(Folder::Directory("/work/notes".into()), true, false);
    plain.set_target_kind(TargetKind::Workflow);
    assert_eq!(plain.folder, None, "a plain folder is dropped");
    let mut none = start_new("Every Monday at 9am, summarise the open PRs");
    none.set_folder(Folder::None, true, false);
    none.set_target_kind(TargetKind::Workflow);
    assert_eq!(none.folder, Some(Folder::None), "no folder stays, for the plan to ask for a repository");
}

#[test]
fn switches_the_composer_to_a_workflow_and_sends_the_message_as_the_request() {
    let lib = library();
    let harnesses = ["opencode".to_string()];
    let c = Context { library: Some(&lib), default_harness: Some("opencode"), workflow_harnesses: &harnesses, ..ctx(None) };
    let mut s = start_new("Every Monday at 9am, bump the client's dependencies");
    s.set_target_kind(TargetKind::Workflow);
    assert!(s.request(&c).is_none(), "no workflow picked yet");
    s.set_workflow("builtin:build-a-feature");
    s.toggle_workflow_step("design");
    let plan = plan_sentence(&s.plan(&c));
    assert!(plan.contains("Each run: Build a feature in a new worktree of /work/kettle, with your message as the request."), "{plan}");
    let b = body(&s, &c);
    let pick = |k: &str| b[k].clone();
    assert_eq!(pick("prompt"), "Bump the client's dependencies");
    assert_eq!((pick("triggerType"), pick("triggerConfig")), (json!("schedule"), json!("0 9 * * 1")));
    assert_eq!(
        (pick("targetType"), pick("workflowId"), pick("workflowSteps")),
        (json!("workflow"), json!("builtin:build-a-feature"), json!(["design"]))
    );
    assert_eq!((pick("workspaceId"), pick("isolation")), (json!(REPOSITORY), json!("worktree")));
    assert_eq!((pick("model"), pick("agent"), pick("harnessType")), (Value::Null, Value::Null, Value::Null));
    assert_eq!(pick("maxConcurrentRuns"), 1);

    // A workflow needs a repository.
    s.set_folder(Folder::None, true, false);
    assert!(s.request(&c).is_none());
}

#[test]
fn a_workflows_request_pins_a_harness_only_when_the_default_cant_run_it() {
    // 12.6, 12.7.
    let lib = library();
    let mut s = start_new("Every Monday at 9am, bump the client's dependencies");
    s.set_target_kind(TargetKind::Workflow);
    s.set_workflow("builtin:build-a-feature");
    let steppers = ["opencode".to_string(), "codex".to_string()];
    let can = Context { library: Some(&lib), default_harness: Some("codex"), workflow_harnesses: &steppers, ..ctx(None) };
    assert_eq!(body(&s, &can)["harnessType"], Value::Null);
    let cant = Context { default_harness: Some("claude"), ..can };
    assert_eq!(body(&s, &cant)["harnessType"], "opencode", "the first that runs workflow steps");
    assert_eq!(s.workflow_harness(&cant).as_deref(), Some("opencode"));
    let unknown = Context { workflow_harnesses: &[], ..cant };
    assert_eq!(body(&s, &unknown)["harnessType"], Value::Null, "nothing pinned before the harnesses load");
    s.harness_type = Some("codex".into());
    assert_eq!(body(&s, &cant)["harnessType"], "codex", "a pick is sent");
}

#[test]
fn a_workflows_steps_are_kept_until_the_library_loads() {
    // 12.7: a saved step the Library hasn't loaded yet stays; one the workflow no longer has goes. At least one run at a time.
    let saved = Automation { workflow_steps: Some(vec!["design".into(), "gone".into()]), max_concurrent_runs: 0, ..bump() };
    let s = ComposerState::state_for(&saved, &[]);
    let loading = ctx(Some(&saved));
    let b = body(&s, &loading);
    assert_eq!(b["workflowSteps"], json!(["design", "gone"]));
    assert_eq!(b["maxConcurrentRuns"], 1);
    let lib = library();
    let loaded = Context { library: Some(&lib), ..loading };
    assert_eq!(body(&s, &loaded)["workflowSteps"], json!(["design"]));
}

#[test]
fn opens_a_saved_workflow_automation_with_its_choices_and_offers_save_only_once_one_changes() {
    let saved = bump();
    let lib = library();
    let c = Context { library: Some(&lib), ..ctx(Some(&saved)) };
    let mut s = ComposerState::state_for(&saved, &[]);
    assert!(s.is_workflow());
    assert_eq!(s.workflow_id.as_deref(), Some("builtin:build-a-feature"));
    assert!(!s.is_dirty(&c));
    s.toggle_workflow_step("verify");
    assert!(s.is_dirty(&c));
    let b = body(&s, &c);
    assert_eq!(
        (&b["targetType"], &b["workflowSteps"], &b["harnessType"]),
        (&json!("workflow"), &json!(["design", "verify"]), &Value::Null)
    );
    // The steps are a set: the same ones in another order are no change.
    let reordered = Automation { workflow_steps: Some(vec!["verify".into(), "design".into()]), ..bump() };
    assert!(!s.is_dirty(&Context { automation: Some(&reordered), ..c }));
    // Another workflow clears the steps.
    s.set_workflow("repo:other");
    assert!(s.workflow_steps.is_empty());
}

#[test]
fn the_draft_row_takes_its_title_from_the_text() {
    let mut s = ComposerState::fresh();
    assert_eq!(s.title(), "New automation");
    s.set_text("Every Monday at 9am");
    assert_eq!(s.title(), "New automation", "only a schedule");
    s.set_text("Every weekday at 8, check the kettle's open issues and sort them by age");
    assert_eq!(s.title(), s.name_placeholder());
    assert!(!s.title().is_empty() && s.title() != "New automation");
    s.name = "  Morning triage ".into();
    assert_eq!(s.title(), "Morning triage");
}

#[test]
fn a_new_automation_is_kept_with_its_limits_and_skips_by_default() {
    // 7.15: per-hour 10 and timeout 30 for a new one; an existing one keeps its own, and skipping keeps its own limit.
    let s = start_new("Every Monday at 9am, summarise the open PRs");
    assert!(s.skip);
    let saved =
        Automation { max_runs_per_hour: 3, timeout_minutes: 90, max_concurrent_runs: 4, target_tags: vec!["ops".into()], ..weekly() };
    let c = ctx(Some(&saved));
    let mut e = ComposerState::state_for(&saved, &[]);
    let b = body(&e, &c);
    assert_eq!(
        (&b["maxRunsPerHour"], &b["timeoutMinutes"], &b["maxConcurrentRuns"], &b["targetTags"]),
        (&json!(3), &json!(90), &json!(4), &json!(["ops"]))
    );
    e.skip = false;
    assert_eq!(body(&e, &c)["maxConcurrentRuns"], 0);
    assert!(e.is_dirty(&c));
}

#[test]
fn the_zone_is_the_machines_or_utc() {
    // 7.18: whatever the machine says, never empty; the request carries it.
    let zone = composer_time_zone();
    assert!(!zone.is_empty());
    let s = start_new("Every Monday at 9am, summarise the open PRs");
    assert_eq!(body(&s, &Context::new(None, &zone, NOW_MS, &utc))["timeZone"], json!(zone));
}
