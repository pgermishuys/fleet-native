//! Component 18, piece I: runs in Sessions and on the dashboard. Fleet's `WorkflowRunGroup.test.ts` (2) and the two
//! `WorkflowStartedBy` cases that draw the group, under their names; then what the native list keeps: a run the store doesn't
//! know groups by its sessions' `workflowRunId`, another machine's steps don't group, the head opens the session the run
//! waits in, dragging a step row moves the run, and the dashboard's Needs you says what a run waits for.
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::Run;
use crate::mock::machines::Case;
use crate::mock::workflow_runs::{BUMP, PAGER, bump_run, pager_run};
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::Runtime;

type Rt = Runtime<State>;

/// The list of the scene: rows 0 and 1 are the pager run's Fix and Plan, 2 and 3 the bump run's, 4 a plain session.
fn rig() -> Rt {
    let mut rt = app();
    rt.update(|s, _| s.show_workflow_runs(true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn words(rt: &Rt, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text)
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn upsert(rt: &mut Rt, run: Run) {
    rt.update(move |s, _| {
        s.workflows.of_mut(HOME_MACHINE_KEY).upsert(run);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// The pager run, one update newer, with the edit applied to its JSON-shaped fields.
fn newer(edit: impl FnOnce(&mut Run)) -> Run {
    let mut run = pager_run();
    run.updated_at = "2026-11-19T10:45:00Z".into();
    edit(&mut run);
    run
}

// ─── WorkflowRunGroup.test.ts ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn says_with_you_on_the_run_and_on_the_step_the_user_finishes_not_needs_you() {
    let rt = rig();
    assert_eq!(words(&rt, "workflow-run-status-run-pager").as_deref(), Some("With you"));
    assert_eq!(words(&rt, "session-title-0-0").as_deref(), Some("Fix"));
    assert_eq!(words(&rt, "session-step-note-0-0").as_deref(), Some("With you"));
    assert!(!has(&rt, "session-step-note-0-1"), "only the step the user finishes");
}

#[test]
fn says_nothing_extra_on_a_step_the_agent_finishes() {
    let mut rt = rig();
    upsert(&mut rt, newer(|r| r.with_you = None));
    assert_eq!(words(&rt, "workflow-run-status-run-pager").as_deref(), Some("Working"));
    assert!(!has(&rt, "session-step-note-0-0"));
    assert_eq!(words(&rt, "session-title-0-0").as_deref(), Some("Fix"));
}

// ─── WorkflowStartedBy.test.ts ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn started_by_shows_on_the_run_s_group_in_sessions_and_opens_the_automation() {
    let mut rt = rig();
    let link = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-started-by")).expect("the link");
    assert_eq!(link.label.as_deref(), Some("Started by Weekly dependency bump"));
    click(&mut rt, "workflow-started-by");
    assert_eq!(rt.state.rail, Rail::Automations);
    assert_eq!(rt.state.automations_nav.open_id(), Some("a-bump"));
}

#[test]
fn started_by_isn_t_shown_on_a_run_started_from_the_run_box() {
    let rt = rig();
    // Only the bump run has one: the pager run (from the Run box) draws a single link in the whole list.
    let links = rt.elements().into_iter().filter(|e| e.key.as_deref() == Some("workflow-started-by")).count();
    assert_eq!(links, 1);
    assert!(has(&rt, "workflow-run-group-run-pager"));
}

// ─── native ─────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_run_s_steps_group_under_one_row_in_the_order_they_ran() {
    let rt = rig();
    for run in [PAGER, BUMP] {
        assert!(has(&rt, &format!("workflow-run-group-{run}")), "{run}");
    }
    // The plan ran before the fix, though the list has the fix first.
    let plan = rt.find("session-0-1").expect("plan").1;
    let fix = rt.find("session-0-0").expect("fix").1;
    assert!(plan < fix, "Plan above Fix");
    // The group sits where its newest session was, above the second run and the plain session.
    let head = rt.find("workflow-run-head-run-pager").expect("head").1;
    let bump = rt.find("workflow-run-head-run-bump").expect("head").1;
    let plain = rt.find("session-0-4").expect("plain").1;
    assert!(head < plan && fix < bump && bump < plain);
    assert_eq!(words(&rt, "workflow-run-title-run-pager").as_deref(), Some("The pager skips the last page"));
    assert_eq!(words(&rt, "workflow-run-status-run-bump").as_deref(), Some("Needs you"));
}

#[test]
fn a_run_the_store_doesn_t_know_groups_by_its_sessions_under_the_title_before_the_dot() {
    let mut rt = rig();
    rt.update(|s, _| *s.workflows.of_mut(HOME_MACHINE_KEY) = Default::default());
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(words(&rt, "workflow-run-title-run-pager").as_deref(), Some("Pager fix"));
    assert!(!has(&rt, "workflow-run-status-run-pager"), "no run, no status");
    assert!(!has(&rt, "workflow-started-by"));
    // Steps are named from their titles; the order is the list's.
    assert_eq!(words(&rt, "session-title-0-0").as_deref(), Some("Fix"));
    assert_eq!(words(&rt, "session-title-0-1").as_deref(), Some("Plan"));
}

#[test]
fn another_machine_s_steps_don_t_group() {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_machines(Case::Several, true);
        let falcon = s.machines.others.get_mut("m-falcon").expect("falcon");
        for session in &mut falcon.sessions {
            session.listing.workflow_run_id = Some("run-far".into());
        }
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!rt.elements().into_iter().any(|e| e.key.is_some_and(|k| k.starts_with("workflow-run-group-"))));
}

#[test]
fn the_head_opens_the_session_the_run_waits_in_else_the_newest_step() {
    let mut rt = rig();
    click(&mut rt, "workflow-run-head-run-bump");
    assert_eq!(rt.state.open_at(), (0, 2), "the approval step, where the card is");
    click(&mut rt, "workflow-run-head-run-pager");
    assert_eq!(rt.state.open_at(), (0, 0), "no card: the newest step, the Fix");
}

#[test]
fn dragging_a_step_row_moves_the_run() {
    let mut rt = rig();
    rt.update(|s, _| s.projects.push(crate::mock::projects().remove(1)));
    rt.frame(FRAME);
    rt.frame(FRAME);
    let target = rt.state.projects[1].id.clone();
    let (x, y) = rt.find("session-0-0").expect("the fix row");
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x + 12.0, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let end = rt.find("project-1").expect("the other project");
    rt.pointer_move(end.0, end.1);
    rt.frame(FRAME);
    rt.pointer_up(end.0, end.1);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let moved: Vec<(String, Option<String>)> = rt
        .state
        .projects
        .iter()
        .flat_map(|p| &p.sessions)
        .filter(|x| x.listing.workflow_run_id.as_deref() == Some(PAGER))
        .map(|x| (x.id.clone(), x.listing.project_id.clone()))
        .collect();
    assert_eq!(moved.len(), 2);
    assert!(moved.iter().all(|(_, project)| project.as_deref() == Some(target.as_str())), "{moved:?}");
}

// ─── the dashboard ──────────────────────────────────────────────────────────────────────────────────────────────────

fn dashboard() -> Rt {
    let mut rt = app();
    rt.update(|s, _| s.show_dashboard_workflow_waiting(true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn a_run_waiting_on_the_user_counts_under_needs_you_and_says_what_it_waits_for() {
    let rt = dashboard();
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("dash-row-s-bump-approve")).expect("the waiting step's row");
    assert_eq!(row.label.as_deref(), Some("Bump the tile library \u{b7} Approve the plan, Build a feature: Approve the plan"));
    assert_eq!(words(&rt, "dash-row-s-bump-approve-status").as_deref(), Some("Approve the plan"));
    // The needs-you section is above the others: its row is above the working step's.
    let waiting = rt.find("dash-row-s-bump-approve").expect("row").1;
    let working = rt.find("dash-row-s-pager-fix").expect("row").1;
    assert!(waiting < working);
    assert_eq!(words(&rt, "home-status").as_deref(), Some("1 session needs you, 1 working."));
}

#[test]
fn a_run_that_does_not_wait_in_the_session_leaves_the_row_as_it_was() {
    let mut rt = dashboard();
    let mut moved = bump_run();
    moved.updated_at = "2026-11-19T11:00:00Z".into();
    moved.waiting.as_mut().expect("waiting").session_id = Some("s-bump-plan".into());
    upsert(&mut rt, moved);
    // The card is in the Plan step's session now: Approve is an ordinary quiet row.
    assert_ne!(words(&rt, "dash-row-s-bump-approve-status").as_deref(), Some("Approve the plan"));
    assert_eq!(words(&rt, "dash-row-s-bump-plan-status").as_deref(), Some("Approve the plan"));
}
