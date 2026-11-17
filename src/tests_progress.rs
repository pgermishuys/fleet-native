//! The Progress tab drawn (component 10, piece G): `ProgressCanvas.test.ts` under its names, read from the scenes' invented
//! plan and todos, and the rules the tab adds (the tab introduced once, the rail's ring, a fold held per session).
use crate::mock::progress::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_machines_page::{exists, words};
use rune::Runtime;

fn shown(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_progress_tab(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn shows_the_plans_title_file_count_and_one_flow_segment_per_group() {
    let rt = shown(Case::Plan);
    assert_eq!(words(&rt, "progress-eyebrow"), "PLAN");
    assert_eq!(words(&rt, "progress-title"), "Page the order list");
    assert!(words(&rt, "progress-source").starts_with(".weave/plans/paging.md · tracked since "));
    assert_eq!(words(&rt, "progress-count"), "2/5");
    assert!(exists(&rt, "progress-flow-0") && exists(&rt, "progress-flow-1") && !exists(&rt, "progress-flow-2"));
    assert_eq!(words(&rt, "progress-now"), "Client · next up: step 3");
    assert!(exists(&rt, "progress-now-1") && !exists(&rt, "progress-now-0"), "the ring is on the current group");
}

#[test]
fn folds_finished_groups_and_opens_one_on_click() {
    let mut rt = shown(Case::Plan);
    assert!(!exists(&rt, "progress-steps-0"), "a complete group starts folded");
    assert!(exists(&rt, "progress-steps-1"), "an unfinished one starts open");
    let head = words(&rt, "progress-group-head-0");
    assert!(head.starts_with("Server") && head.contains("2/2 · "), "{head}");
    click(&mut rt, "progress-group-head-0");
    rt.frame(FRAME);
    assert!(exists(&rt, "progress-steps-0"));
    click(&mut rt, "progress-group-head-1");
    rt.frame(FRAME);
    assert!(!exists(&rt, "progress-steps-1"));
}

#[test]
fn keeps_a_fold_for_its_own_session() {
    let mut rt = shown(Case::Plan);
    click(&mut rt, "progress-group-head-0");
    rt.update(|s, _| {
        let me = s.canvas_session();
        assert!(s.progress.group_open(&me, "0:Server", false));
        assert!(!s.progress.group_open("another-session", "0:Server", false));
    });
}

#[test]
fn marks_the_current_step_and_shows_the_live_todo_list_under_it() {
    let rt = shown(Case::Plan);
    assert!(words(&rt, "progress-step-3").contains("next"));
    assert_eq!(words(&rt, "progress-todos-label"), "TODOS · 1 OF 4");
    for i in 0..4 {
        assert!(exists(&rt, &format!("progress-todo-{i}")));
    }
    assert!(!exists(&rt, "progress-step-4") || !words(&rt, "progress-step-4").contains("TODOS"));
}

#[test]
fn renders_backticked_text_in_a_step_title_as_code() {
    let rt = shown(Case::Plan);
    let all = rt.elements();
    let title = all.iter().find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("Read nextCursor in the list"))).expect("the title");
    assert!(!title.text.clone().unwrap_or_default().contains('`'));
    assert!(title.text.clone().unwrap_or_default().ends_with("1/3"), "the sub-checklist count follows the title");
}

#[test]
fn shows_a_flat_plan_without_group_headings() {
    let rt = shown(Case::Flat);
    assert!(!exists(&rt, "progress-group-head-0"));
    assert!((0..5).all(|i| exists(&rt, &format!("progress-flow-{i}"))));
    assert_eq!(words(&rt, "progress-now"), "next up: step 3");
}

#[test]
fn says_every_step_is_ticked_and_shows_no_todo_list_when_none_is_left() {
    let rt = shown(Case::Done);
    assert_eq!(words(&rt, "progress-now"), "Every step is ticked");
    assert!(!exists(&rt, "progress-todos"));
}

#[test]
fn shows_just_the_todo_list_when_theres_no_plan() {
    let rt = shown(Case::Todos);
    assert_eq!(words(&rt, "progress-eyebrow"), "TODOS");
    assert_eq!(words(&rt, "progress-title"), "The agent's todo list");
    assert_eq!(words(&rt, "progress-count"), "1/3");
    assert!(exists(&rt, "progress-todos") && !exists(&rt, "progress-flow"));
    assert_eq!(words(&rt, "progress-todo-note"), "This session hasn't written a markdown checklist, so there's no plan to show.");
}

#[test]
fn explains_where_progress_comes_from_when_theres_nothing_yet() {
    let rt = shown(Case::Empty);
    assert!(words(&rt, "progress-empty").starts_with("Nothing to track yet"));
    assert!(words(&rt, "progress-empty").contains(".weave/plans/"));
}

#[test]
fn counts_the_todo_form_by_its_list_not_by_the_total() {
    let mut rt = shown(Case::Todos);
    rt.update(|s, _| {
        let id = s.canvas_session();
        let mut d = s.progress.of(&id).cloned().expect("a detail");
        d.summary.total = 0;
        d.summary.done = 0;
        s.progress.set(&id, Some(d));
    });
    rt.frame(FRAME);
    assert_eq!(words(&rt, "progress-count"), "0/0");
    assert!(exists(&rt, "progress-todos"));
}

#[test]
fn the_progress_tab_is_introduced_once_when_there_is_something_to_count() {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects[0].sessions[0].row.progress =
            Some(crate::fleet::row_chips::Progress { kind: "todos".into(), done: 1, total: 4, current: None });
        s.keep_progress_tab();
        assert!(s.open_canvases().canvases.iter().any(|c| c.id == "progress"));
        assert_ne!(s.active_canvas(), "progress", "introduced without focus");
        s.edit_canvases(|c, id| c.close(id, "progress"));
        s.keep_progress_tab();
        assert!(!s.open_canvases().canvases.iter().any(|c| c.id == "progress"), "closed for good");
    });
}

#[test]
fn counts_the_collapsed_rails_ring_from_the_sessions_progress() {
    let mut rt = app();
    rt.update(|s, _| {
        s.projects[0].sessions[0].row.progress =
            Some(crate::fleet::row_chips::Progress { kind: "plan".into(), done: 2, total: 9, current: None });
        s.content_open = false;
    });
    rt.frame(FRAME);
    assert_eq!(words(&rt, "content-rail-pending"), "7");
    assert!(exists(&rt, "content-rail-ring"));
}
