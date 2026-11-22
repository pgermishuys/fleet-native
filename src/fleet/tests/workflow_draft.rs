//! Component 18, piece K: `lib/__tests__/workflow-draft.test.ts` under its names, then what Rust adds (the words, JSON both
//! ways). The fixture is Fleet's `reviewDraft`: Plan → Approve → Implement → Review, Review's changes going back.
use crate::fleet::workflows::Choice;
use crate::fleet::workflows::Step;
use crate::fleet::workflows::StepKind;
use crate::fleet::workflows::draft::*;
use serde_json::json;

fn agent_step(id: &str) -> Step {
    Step {
        id: id.into(),
        title: id.into(),
        kind: StepKind::Agent,
        agent: Some("build".into()),
        model: Some("standard".into()),
        outcomes: vec!["done".into()],
        prompt: Some("The request: {{request}}\n".into()),
        ..Step::default()
    }
}

fn choice(label: &str, to: &str, note: bool) -> Choice {
    Choice { label: label.into(), to: to.into(), note }
}

fn review_draft() -> WorkflowDraft {
    let you = Step {
        id: "ok-plan".into(),
        title: "Approve the plan".into(),
        kind: StepKind::You,
        agent: None,
        model: None,
        outcomes: vec![],
        prompt: None,
        ask: Some("Build it this way?".into()),
        choices: vec![choice("Approve", "implement", false), choice("Send back", "plan", true)],
        ..agent_step("ok-plan")
    };
    WorkflowDraft {
        name: "Build it our way".into(),
        description: None,
        placeholder: None,
        steps: vec![
            Step {
                title: "Plan".into(),
                model: Some("strong".into()),
                outcomes: vec!["ready".into()],
                writes: vec![".weave/plans/{{slug}}.md".into()],
                ..agent_step("plan")
            },
            you,
            Step {
                title: "Implement".into(),
                prompt: Some("Build {{steps.plan.files}}.\nThe request: {{request}}\n".into()),
                ..agent_step("implement")
            },
            Step {
                title: "Review".into(),
                model: Some("strong".into()),
                skill: Some("fleet-code-review".into()),
                outcomes: vec!["pass".into(), "changes".into()],
                routes: vec![("changes".into(), "implement".into())],
                max_loops: Some(2),
                prompt: Some("Review against {{ steps.plan.files }} and {{steps.plan.summary}}.\n".into()),
                ..agent_step("review")
            },
        ],
    }
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn renames_a_step_id_and_every_reference_follows_routes_choices_and_step_variables() {
    let renamed = rename_step_id(&review_draft(), "plan", "outline");
    assert_eq!(renamed.steps.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["outline", "ok-plan", "implement", "review"]);
    assert_eq!(renamed.steps[1].choices.iter().map(|c| c.to.as_str()).collect::<Vec<_>>(), ["implement", "outline"]);
    assert_eq!(renamed.steps[2].prompt.as_deref(), Some("Build {{steps.outline.files}}.\nThe request: {{request}}\n"));
    assert_eq!(renamed.steps[3].prompt.as_deref(), Some("Review against {{ steps.outline.files }} and {{steps.outline.summary}}.\n"));

    let implement = rename_step_id(&review_draft(), "implement", "build");
    assert_eq!(implement.steps[3].routes, vec![("changes".to_string(), "build".to_string())]);
    assert_eq!(implement.steps[1].choices[0].to, "build");
}

#[test]
fn leaves_a_step_id_that_only_starts_the_same_alone() {
    let mut draft = review_draft();
    draft.steps[2].prompt = Some("{{steps.plan-b.files}} and {{steps.plan.files}}".into());
    assert_eq!(
        rename_step_id(&draft, "plan", "outline").steps[2].prompt.as_deref(),
        Some("{{steps.plan-b.files}} and {{steps.outline.files}}")
    );
}

#[test]
fn knows_a_loop_back_to_an_earlier_step_or_itself_not_forward_or_to_the_end() {
    let draft = review_draft();
    assert!(is_loop(&draft, 3, Some("implement")));
    assert!(is_loop(&draft, 3, Some("review")));
    assert!(!is_loop(&draft, 0, Some("implement")));
    assert!(!is_loop(&draft, 3, Some("end")));
    assert_eq!(loop_outcomes(&draft, 3), ["changes"]);
}

#[test]
fn renames_and_removes_outcomes_with_where_they_lead() {
    let review = review_draft().steps[3].clone();
    let renamed = rename_outcome(&review, "changes", "fix");
    assert_eq!(renamed.outcomes, Some(strs(&["pass", "fix"])));
    assert_eq!(renamed.routes, Some(vec![("fix".to_string(), "implement".to_string())]));
    let removed = remove_outcome(&review, "changes");
    assert_eq!(removed.outcomes, Some(strs(&["pass"])));
    assert_eq!(removed.routes, Some(vec![]));
}

#[test]
fn drops_a_max_once_the_step_has_no_loop_and_keeps_it_empty_for_a_new_loop() {
    let draft = review_draft();
    let no_loop = drop_unused_max(&update_step(&draft, 3, &StepPatch { routes: Some(vec![]), ..StepPatch::default() }), 3);
    assert_eq!(no_loop.steps[3].max_loops, None);
    assert_eq!(drop_unused_max(&draft, 3).steps[3].max_loops, Some(2));
}

#[test]
fn offers_the_runs_variables_and_the_earlier_steps_ones() {
    let variables = variables_for(&review_draft(), 3);
    assert!(variables.contains(&"request".to_string()));
    assert!(variables.contains(&"steps.plan.files".to_string()));
    assert!(variables.contains(&"steps.implement.summary".to_string()));
    assert!(!variables.contains(&"steps.implement.files".to_string()));
    assert!(!variables.contains(&"steps.review.summary".to_string()));
}

#[test]
fn adds_steps_with_ids_no_other_step_has() {
    let draft = review_draft();
    assert_eq!(unique_id(&draft, "plan"), "plan-2");
    let agent = new_agent_step(&draft);
    assert_eq!((agent.id.as_str(), agent.title.as_str(), agent.model.as_deref()), ("step", "New step", Some("standard")));
    assert_eq!(agent.outcomes, ["done"]);
    assert!(agent.prompt.unwrap().contains("{{request}}"));
    assert_eq!(new_you_step(&draft, Some("review")).choices, vec![choice("Carry on", "review", false), choice("Stop here", "end", false)]);
}

#[test]
fn names_targets_for_people() {
    assert_eq!(target_title(&review_draft(), Some("implement")), "Implement");
    assert_eq!(target_title(&review_draft(), Some("end")), "End the run");
    assert_eq!(target_title(&review_draft(), None), "Next step");
}

#[test]
fn makes_the_same_slug_as_the_server() {
    assert_eq!(slug_of("Tidy up a flaky test"), "tidy-up-a-flaky-test");
    assert_eq!(slug_of("Build a feature, our way"), "build-a-feature-our-way");
    assert_eq!(slug_of("  --Weird!! Name__ "), "weird-name");
    assert_eq!(slug_of("!!!"), "workflow");
}

// ─── What Rust adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_slug_is_cut_at_sixty_without_a_trailing_dash() {
    let long = format!("{} tail", "a".repeat(59));
    assert_eq!(slug_of(&long), "a".repeat(59));
}

#[test]
fn every_edit_returns_a_new_draft_and_leaves_the_old_one() {
    let draft = review_draft();
    let inserted = insert_step(&draft, 1, new_agent_step(&draft));
    assert_eq!((draft.steps.len(), inserted.steps.len(), inserted.steps[1].id.as_str()), (4, 5, "step"));
    assert_eq!(remove_step(&inserted, 1), draft);
    assert!(!same_draft(Some(&draft), Some(&inserted)));
    assert_eq!(new_outcome(&draft.steps[2]), "outcome-2");
    assert_eq!(new_outcome(&draft.steps[3]), "done");
    let routed = set_route(&draft.steps[3], "pass", Some("end"));
    assert_eq!(routed.routes.unwrap().len(), 2);
    assert_eq!(set_route(&draft.steps[3], "changes", None).routes, Some(vec![]));
}

#[test]
fn the_banner_and_the_cost_say_what_fleet_says() {
    let mut drafted = DraftedWorkflow { session_title: Some("Fix the login bug".into()), asks: 1, ..DraftedWorkflow::default() };
    assert_eq!(drafted_from(&drafted), "Drafted from Fix the login bug. Review it before saving.");
    assert_eq!(draft_cost(&drafted), "Asked the model once.");
    drafted.tokens = Some(DraftTokens { total: 3412, from_cache: 3100 });
    assert_eq!(draft_cost(&drafted), "Asked the model once · 3,412 tokens, 3,100 from the cache.");
    drafted.tokens = Some(DraftTokens { total: 900, from_cache: 0 });
    drafted.asks = 2;
    drafted.session_title = None;
    assert_eq!(draft_cost(&drafted), "Asked the model twice: its first answer had errors · 900 tokens.");
    assert_eq!(drafted_from(&drafted), "Drafted from your description. Review it before saving.");
}

#[test]
fn a_draft_goes_to_the_server_as_json_and_comes_back_the_same() {
    let draft = review_draft();
    let json = draft.to_json();
    assert_eq!(json["steps"][3]["routes"], json!({"changes": "implement"}));
    assert_eq!(json["steps"][1]["kind"], "you");
    assert_eq!(WorkflowDraft::from_json(&json), draft);
    let check = WorkflowCheck::from_json(&json!({
        "text": "name: x\n", "errors": [{"line": 4, "message": "bad", "step": 1}], "draft": null,
        "comments": [{"line": 2, "text": "# hi"}]
    }));
    assert_eq!((check.draft.is_none(), check.errors[0].step, check.comments[0].line), (true, Some(1), 2));
    let file = WorkflowFile::from_json(
        &json!({"workflowId": "repo:x", "file": ".weave/workflows/x.yaml", "hash": "h1", "check": {"text": "", "draft": json}}),
    );
    assert_eq!((file.hash.as_str(), file.check.draft), ("h1", Some(draft)));
    let drafted = DraftedWorkflow::from_json(
        &json!({"repository": "/r", "repositoryName": "r", "sessionTitle": null, "asks": 2, "tokens": {"total": 5, "fromCache": 1}, "check": {}}),
    );
    assert_eq!((drafted.asks, drafted.tokens), (2, Some(DraftTokens { total: 5, from_cache: 1 })));
}
