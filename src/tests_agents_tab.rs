//! The Agents tab drawn (component 10, piece L): `AgentsCanvas.test.ts` under its names, and the rules read from
//! `AgentsCanvas.vue`, `AgentsCanvasRow.vue` and `AgentDetail.vue`.
use crate::mock::agents_tab::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_machines_page::{exists, words};
use rune::Runtime;
use serde_json::json;

fn shown(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_agents_tab(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn settle(rt: &mut Runtime<crate::State>) {
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn says_what_it_will_show_when_there_is_nothing_yet() {
    let rt = shown(Case::Empty);
    // The scene's session is a fork, so the Parent row shows; with none it is the empty state.
    assert!(!exists(&rt, "agents-running") && !exists(&rt, "agents-earlier"));
    let mut rt = rt;
    rt.update(|s, _| {
        s.projects[0].sessions[0].listing = Default::default();
    });
    settle(&mut rt);
    assert!(exists(&rt, "agents-empty") && exists(&rt, "agents-head"));
    assert_eq!(words(&rt, "agents-empty"), "Subagents this session's agent starts, and sessions it forks or starts, show here");
    assert!(!exists(&rt, "agents-summary"));
}

#[test]
fn shows_the_parent_what_runs_now_the_sessions_it_started_and_earlier_agents_folded() {
    let mut rt = shown(Case::Main);
    assert_eq!(words(&rt, "agents-summary"), "2 running · 1 waiting for you");
    assert!(exists(&rt, "agents-parent") && exists(&rt, "agents-running") && exists(&rt, "agents-started"));
    assert!(exists(&rt, "agents-earlier-toggle") && !exists(&rt, "agents-row-work:a1"), "Earlier agents starts closed");
    assert_eq!(words(&rt, "agents-earlier-toggle"), "Earlier agents · 1");
    click(&mut rt, "agents-earlier-toggle");
    assert!(exists(&rt, "agents-row-work:a1"));
    assert!(words(&rt, "agents-state-work:a1").contains("min"), "{}", words(&rt, "agents-state-work:a1"));
    click(&mut rt, "agents-earlier-toggle");
    assert!(!exists(&rt, "agents-row-work:a1"));
}

#[test]
fn opens_the_first_running_agents_detail_with_open_session_and_stop() {
    let rt = shown(Case::Main);
    assert!(exists(&rt, "agents-detail"), "the first running row with work opens without asking");
    assert_eq!(words(&rt, "agents-asked"), "Migrate the cache");
    assert!(exists(&rt, "agents-open") && exists(&rt, "agents-stop"));
}

#[test]
fn selecting_another_row_shows_its_detail_selecting_it_again_closes_it() {
    let mut rt = shown(Case::Main);
    click(&mut rt, "agents-row-work:a91d");
    assert!(!exists(&rt, "agents-detail"), "pressing the shown row closes it, and the automatic one doesn't come back");
    click(&mut rt, "agents-row-work:a91d");
    assert!(exists(&rt, "agents-detail"));
    let fork =
        rt.state.agents_lineage(&rt.state.canvas_session()).running.iter().find(|r| r.work.is_none()).map(|r| r.key.clone()).unwrap();
    click(&mut rt, &format!("agents-row-{fork}"));
    assert!(exists(&rt, "agents-detail") && !exists(&rt, "agents-stop"), "a fork's detail has Open session and no Stop");
    assert!(exists(&rt, "agents-open"));
}

#[test]
fn stop_moves_the_row_to_earlier_agents() {
    let mut rt = shown(Case::Main);
    let session = rt.state.canvas_session();
    rt.update(|s, _| s.stop_work("a91d"));
    let ended = json!({
        "id": "a91d", "sessionId": session, "workId": "w-a91d", "kind": "subagent", "title": "general", "label": "Migrate the cache",
        "status": "cancelled", "endedReason": "cancelled", "startedAt": "2026-10-04T10:00:00Z", "endedAt": "2026-10-04T10:00:40Z",
    });
    rt.update(|s, _| s.on_work_stopped("a91d", Ok((200, ended))));
    settle(&mut rt);
    assert_eq!(words(&rt, "agents-earlier-toggle"), "Earlier agents · 2");
    assert_eq!(rt.state.canvases.agents_running, 2);
}

#[test]
fn a_refused_stop_says_why_under_the_row() {
    let rt = shown(Case::Refused);
    assert_eq!(words(&rt, "agents-stop-error"), "The session isn't running.");
}

#[test]
fn the_inside_turn_note_shows_with_neither_open_nor_stop() {
    let rt = shown(Case::Inside);
    assert_eq!(words(&rt, "agents-inside"), "Runs inside the tool call. Interrupt the turn to stop it.");
    assert!(!exists(&rt, "agents-open") && !exists(&rt, "agents-stop"));
}

#[test]
fn a_lost_agent_says_lost() {
    let mut rt = shown(Case::Earlier);
    settle(&mut rt);
    assert_eq!(words(&rt, "agents-state-work:e3"), "lost");
    assert_eq!(words(&rt, "agents-state-work:e2"), "failed");
    assert_eq!(words(&rt, "agents-state-work:e4"), "stopped");
    assert_eq!(words(&rt, "agents-state-work:e5"), "done");
    assert_eq!(words(&rt, "agents-ended"), "2 files read");
}

#[test]
fn the_tab_is_added_once_something_runs_and_counts_what_runs_or_waits() {
    let rt = shown(Case::Main);
    assert_eq!(rt.state.canvases.agents_running, 3);
    assert!(rt.state.open_canvases().introduced.contains(&crate::fleet::canvases::CanvasKind::Agents));
    assert!(matches!(rt.state.tab_badge("agents"), Some(crate::fleet::canvas_host::Badge::Count(n, _)) if n == "3"));
}

#[test]
fn pressing_the_parent_opens_it() {
    let mut rt = shown(Case::Main);
    let before = rt.state.open_id();
    let key = rt.state.agents_parent(&rt.state.canvas_session()).map(|p| p.row.key).unwrap();
    let parent = key.trim_start_matches("parent:").to_string();
    click(&mut rt, &format!("agents-row-{key}"));
    assert_ne!(rt.state.open_at(), (0, 0), "{before:?}");
    assert_eq!(rt.state.session().id, parent);
}

// Row 5.12: a press on a parent the list doesn't hold still opens it, by its id (`/sessions/{parentId}`, no instance).
#[test]
fn pressing_a_parent_the_list_does_not_hold_opens_it_by_its_id() {
    let mut rt = shown(Case::Main);
    rt.update(|s, tx| {
        let target = crate::fleet::agents::OpenTarget::Own { session: "parent-the-list-lacks".into(), instance_id: None };
        s.open_agent(&target, "Parent session", tx);
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.route, crate::fleet::store::Route::Session("parent-the-list-lacks".into()));
    assert!(rt.state.opening.holds(&crate::fleet::store::Route::Session("parent-the-list-lacks".into())), "it is read by its id");
}
