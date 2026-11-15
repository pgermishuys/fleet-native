//! `lib/__tests__/background-work.test.ts`, the sub-agent half of `AgentTaskRow.test.ts` (what the row says) and its
//! "sub-agent call details", under the same names; what the view draws is tested in `src/tests.rs`.
use crate::conv::background::*;
use crate::conv::tools::ToolStatus;
use crate::fleet::delegations::{Delegation, DelegationStatus};
use serde_json::{Value, json};

/// What OpenCode 2 (2.0.9) really sends: a background shell's result, and the notice when it finishes.
fn shell_result() -> Value {
    json!({
        "status": "running",
        "background": true,
        "input": {"command": "sleep 6; echo done", "description": "Slow job", "background": true},
        "output": "Command moved to the background (shell ID: sh_1).\nOutput is streaming to: /tmp/sh_1.out",
        "metadata": {"status": "running", "truncated": false, "shellID": "sh_1"},
    })
}

const SHELL_NOTICE: &str =
    "<shell id=\"sh_1\" state=\"completed\" command=\"sleep 6; echo done\">\ndone\n\nCommand exited with code 0.\n</shell>";
const SUBAGENT_NOTICE: &str =
    "<subagent sessionID=\"ses_child\" state=\"completed\" description=\"Slow helper\">\nThe child finished its slow task.\n</subagent>";

/// The status `toToolCardItem` gives a call: the background word when its work went there, else the call's own.
fn card_status(state: &Value, finished: &Finished) -> ToolStatus {
    background_status(state, finished).unwrap_or_else(|| ToolStatus::from_word(state.get("status").and_then(Value::as_str)))
}

#[test]
fn parse_background_notice_reads_a_finished_shell() {
    assert_eq!(
        parse_background_notice(SHELL_NOTICE),
        Some(BackgroundNotice {
            kind: BackgroundKind::Shell,
            id: "sh_1".into(),
            state: BackgroundState::Completed,
            label: "sleep 6; echo done".into(),
            text: "done\n\nCommand exited with code 0.".into(),
        })
    );
}

#[test]
fn parse_background_notice_reads_a_finished_subagent_and_how_it_ended() {
    let n = parse_background_notice(SUBAGENT_NOTICE).unwrap();
    assert_eq!(
        (n.kind, n.id.as_str(), n.state, n.label.as_str()),
        (BackgroundKind::Subagent, "ses_child", BackgroundState::Completed, "Slow helper")
    );
    assert_eq!(n.text, "The child finished its slow task.");
    let state =
        |word: &str| parse_background_notice(&SUBAGENT_NOTICE.replace("state=\"completed\"", &format!("state=\"{word}\""))).unwrap().state;
    assert_eq!(state("error"), BackgroundState::Error);
    assert_eq!(state("cancelled"), BackgroundState::Cancelled);
    // A state the notice doesn't know reads as completed.
    assert_eq!(state("paused"), BackgroundState::Completed);
}

#[test]
fn parse_background_notice_leaves_an_ordinary_prompt_alone() {
    assert_eq!(parse_background_notice("Please run the tests"), None);
    assert_eq!(parse_background_notice("<shell> what does this tag do?"), None);
    // No line for the text, or no closing tag: not a notice.
    assert_eq!(parse_background_notice("<shell id=\"a\" state=\"completed\" command=\"x\">\n</shell>"), None);
    assert_eq!(parse_background_notice("<shell id=\"a\" state=\"completed\" command=\"x\">\ntext"), None);
    // Trailing whitespace after the closing tag is fine.
    assert!(parse_background_notice(&format!("{SHELL_NOTICE}\n\n")).is_some());
}

#[test]
fn a_backgrounded_calls_card_says_background_while_the_work_runs_although_the_call_itself_succeeded() {
    assert_eq!(card_status(&shell_result(), &Finished::new()), ToolStatus::Background);
    assert!(shell_result()["output"].as_str().unwrap().contains("moved to the background"));
}

#[test]
fn a_backgrounded_calls_card_is_finished_by_the_notice_that_names_its_shell() {
    let finished = finished_background_work([SHELL_NOTICE]);
    assert_eq!(card_status(&shell_result(), &finished), ToolStatus::Completed);
    assert_eq!(card_status(&shell_result(), &finished_background_work([])), ToolStatus::Background);
}

#[test]
fn a_backgrounded_calls_card_is_finished_by_the_notice_that_names_its_child_session() {
    let call = json!({
        "status": "running", "background": true, "input": {"description": "Slow helper", "background": true},
        "metadata": {"sessionID": "ses_child", "status": "running", "truncated": false},
    });
    assert_eq!(card_status(&call, &Finished::new()), ToolStatus::Background);
    assert_eq!(card_status(&call, &finished_background_work([SUBAGENT_NOTICE])), ToolStatus::Completed);
    let failed = SUBAGENT_NOTICE.replace("state=\"completed\"", "state=\"error\"");
    assert_eq!(card_status(&call, &finished_background_work([failed.as_str()])), ToolStatus::Error);
    let stopped = SUBAGENT_NOTICE.replace("state=\"completed\"", "state=\"cancelled\"");
    assert_eq!(card_status(&call, &finished_background_work([stopped.as_str()])), ToolStatus::Stopped);
}

#[test]
fn an_ordinary_call_is_left_alone() {
    let done = json!({"status": "completed", "output": "hi", "metadata": {"status": "completed", "exit": 0}});
    assert_eq!(background_work_id(&done), None);
    assert_eq!(card_status(&done, &finished_background_work([SHELL_NOTICE])), ToolStatus::Completed);
}

#[test]
fn a_subagent_that_is_merely_running_is_left_alone() {
    // A foreground sub-agent reports its child session with the same "status": "running" metadata, so only the call
    // having returned tells them apart.
    let working = json!({"status": "running", "input": {}, "metadata": {"sessionID": "ses_child", "status": "running"}});
    assert_eq!(background_work_id(&working), None);
    assert_eq!(card_status(&working, &finished_background_work([SUBAGENT_NOTICE])), ToolStatus::Running);
}

#[test]
fn a_notice_with_work_fleet_stopped_says_stopped_and_drops_the_error_text() {
    let error =
        parse_background_notice("<shell id=\"sh_dev\" state=\"error\" command=\"sh dev.sh\">\nShell.NotFoundError\n</shell>").unwrap();
    let mut finished = Finished::new();
    finished.insert("sh_dev".into(), BackgroundState::Cancelled);
    let shown = with_end(error.clone(), &finished);
    assert_eq!((shown.state, shown.text.as_str()), (BackgroundState::Cancelled, ""));
    // No answer from the stream, or the same one: unchanged.
    assert_eq!(with_end(error.clone(), &Finished::new()), error);
    assert_eq!(BackgroundState::Cancelled.word(), "stopped");
}

fn delegation(status: DelegationStatus, background: bool, waiting: bool) -> Delegation {
    Delegation {
        delegation_id: "d1".into(),
        parent_tool_call_id: Some("call-1".into()),
        child_session_id: Some("child-1".into()),
        title: "Map every archive entry point".into(),
        status,
        created_at: None,
        child_activity_status: waiting.then(|| "waiting_input".to_string()),
        background,
    }
}

fn row(status: DelegationStatus, background: bool, waiting: bool) -> AgentRow {
    let call = AgentCall { kind: "explore".into(), task: "Map every archive entry point".into() };
    AgentRow::of(&call, &delegation(status, background, waiting)).unwrap()
}

#[test]
fn agent_task_row_names_the_agent_and_its_task() {
    let r = row(DelegationStatus::Running, false, false);
    assert_eq!((r.agent.as_str(), r.task.as_str(), r.child_session_id.as_str()), ("explore", "Map every archive entry point", "child-1"));
    // A call that says nothing of its task takes the delegation's title.
    let bare = AgentRow::of(&AgentCall::default(), &delegation(DelegationStatus::Running, false, false)).unwrap();
    assert_eq!((bare.agent.as_str(), bare.task.as_str()), ("", "Map every archive entry point"));
}

#[test]
fn agent_task_row_shows_a_child_as_its_status_word() {
    for (status, word, working) in [
        (DelegationStatus::Running, "Working", true),
        (DelegationStatus::Pending, "Starting", true),
        (DelegationStatus::Completed, "Done", false),
        (DelegationStatus::Error, "Failed", false),
        (DelegationStatus::Cancelled, "Stopped", false),
    ] {
        let r = row(status, false, false);
        assert_eq!((r.status_word(), r.working()), (word, working), "{status:?}");
    }
}

#[test]
fn agent_task_row_says_a_child_stopped_on_a_question_needs_input_and_opens_it_to_answer() {
    let r = row(DelegationStatus::Running, false, true);
    assert_eq!(r.status_word(), "Needs input");
    assert_eq!(r.tip(), "Open Map every archive entry point to answer its question");
    assert_eq!(row(DelegationStatus::Running, false, false).tip(), "Open Map every archive entry point");
}

#[test]
fn agent_task_row_says_a_child_in_the_background_is_running_there_not_working_for_the_session() {
    assert_eq!(row(DelegationStatus::Running, true, false).status_word(), "Running in the background");
}

#[test]
fn agent_task_row_says_a_child_in_the_background_that_asks_needs_input() {
    assert_eq!(row(DelegationStatus::Running, true, true).status_word(), "Needs input");
}

#[test]
fn agent_task_row_says_a_child_that_came_back_from_the_background_is_done() {
    assert_eq!(row(DelegationStatus::Completed, true, false).status_word(), "Done");
}

#[test]
fn agent_task_row_ignores_a_stale_question_once_the_child_has_finished() {
    // `is_waiting` already needs the child to be active; a delegation that says waiting after it finished isn't one.
    let r = row(DelegationStatus::Completed, false, true);
    assert_eq!((r.asks(), r.status_word()), (false, "Done"));
}

#[test]
fn a_delegation_without_a_session_is_not_a_row_and_one_matches_by_call_or_part_id() {
    let mut d = delegation(DelegationStatus::Running, false, false);
    assert!(delegation_of(std::slice::from_ref(&d), "call-1", "p").is_some());
    assert!(delegation_of(std::slice::from_ref(&d), "other", "call-1").is_some());
    assert!(delegation_of(std::slice::from_ref(&d), "other", "p").is_none());
    d.child_session_id = None;
    assert!(delegation_of(&[d], "call-1", "p").is_none());
}

#[test]
fn sub_agent_call_details_read_opencodes_task_input() {
    let state =
        json!({"status": "running", "input": {"subagent_type": "explore", "description": "Find the auth middleware", "prompt": "…"}});
    assert_eq!((subagent_kind(&state).as_str(), subagent_task(&state).as_str()), ("explore", "Find the auth middleware"));
}

#[test]
fn sub_agent_call_details_read_opencode_2s_subagent_input() {
    let state = json!({"status": "running", "input": {"agent": "general", "description": "Summarise the diff"}});
    assert_eq!((subagent_kind(&state).as_str(), subagent_task(&state).as_str()), ("general", "Summarise the diff"));
}

#[test]
fn sub_agent_call_details_are_empty_when_the_call_doesnt_say() {
    let state = json!({"status": "running", "input": {}});
    assert_eq!((subagent_kind(&state).as_str(), subagent_task(&state).as_str()), ("", ""));
}
