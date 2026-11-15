//! `lib/__tests__/shell-commands.test.ts` (the tests of the composer's `!` draft belong to component 5, the two "arriving
//! live" ones to the reducer) and the parts of `ShellCommandBlock.test.ts` that are not about drawing.
use crate::conv::shell::{ShellCommand, ShellState, output_lines, shell_command_of, shown_range, state_label, to_shell_command};
use crate::fleet::model::{Msg, PartData};
use crate::fleet::reducer::to_role;
use crate::view::conversation::Role;
use serde_json::{Value, json};

fn shell_message(state: Value) -> Msg {
    let mut m = Msg::new("msg_1", "session-1", Role::Shell);
    m.parts.push(PartData::Tool { id: "msg_1-tool-sh_1".into(), tool: "bash".into(), call_id: "sh_1".into(), state });
    m
}

#[test]
fn keeps_the_users_and_the_shells_roles_and_puts_everything_else_on_the_agents_side() {
    assert_eq!(to_role(&json!("user")), Role::User);
    assert_eq!(to_role(&json!("shell")), Role::Shell);
    assert_eq!(to_role(&json!("assistant")), Role::Assistant);
    assert_eq!(to_role(&json!("system")), Role::Assistant);
    assert_eq!(to_role(&Value::Null), Role::Assistant);
}

#[test]
fn shows_a_command_still_running() {
    let view = to_shell_command(&shell_message(json!({"status": "running", "input": {"command": "npm test"}})));
    assert_eq!(
        view,
        Some(ShellCommand {
            id: "msg_1-tool-sh_1".into(),
            command: "npm test".into(),
            output: String::new(),
            state: ShellState::Running,
            exit: None,
            truncated: false
        })
    );
}

#[test]
fn reads_how_a_command_ended_from_its_metadata() {
    let ended = |state: Value| to_shell_command(&shell_message(state)).unwrap();
    let ok = ended(
        json!({"status": "completed", "input": {"command": "echo hello"}, "output": "hello\n", "metadata": {"exit": 0, "status": "exited", "truncated": false}}),
    );
    assert_eq!((ok.state, ok.exit, ok.output.as_str()), (ShellState::Done, Some(0), "hello\n"));
    let bad = ended(
        json!({"status": "completed", "input": {"command": "ls /nope; exit 3"}, "output": "ls: cannot access '/nope'\n", "metadata": {"exit": 3, "status": "exited"}}),
    );
    assert_eq!((bad.state, bad.exit), (ShellState::Failed, Some(3)));
    let slow =
        ended(json!({"status": "completed", "input": {"command": "sleep 999"}, "metadata": {"status": "timeout", "truncated": true}}));
    assert_eq!((slow.state, slow.truncated), (ShellState::Timeout, true));
    let stopped = ended(json!({"status": "completed", "input": {"command": "sleep 999"}, "metadata": {"status": "killed"}}));
    assert_eq!(stopped.state, ShellState::Killed);
    // A harness that failed the call itself: its error is what the command printed.
    let broke = ended(json!({"status": "error", "input": {"command": "x"}, "error": "no shell"}));
    assert_eq!((broke.state, broke.output.as_str()), (ShellState::Failed, "no shell"));
}

#[test]
fn calls_a_command_done_when_the_harness_reports_no_exit_code() {
    // OpenCode (1.x) keeps the output of a user's command but not its exit code.
    let view = to_shell_command(&shell_message(json!({
        "status": "completed",
        "input": {"command": "ls /nope; exit 3"},
        "output": "ls: cannot access '/nope'\n",
        "metadata": {"output": "ls: cannot access '/nope'\n"}
    })))
    .unwrap();
    assert_eq!((view.state, view.exit), (ShellState::Done, None));
}

#[test]
fn waits_for_the_commands_part() {
    assert_eq!(to_shell_command(&Msg::new("msg_1", "session-1", Role::Shell)), None);
}

#[test]
fn says_how_a_command_ended() {
    let label = |state: Value| state_label(&shell_command_of("p", &state));
    assert_eq!(label(json!({"status": "running"})), "Running\u{2026}");
    assert_eq!(label(json!({"status": "completed", "metadata": {"exit": 3}})), "Failed \u{b7} exit 3");
    assert_eq!(label(json!({"status": "error"})), "Failed");
    assert_eq!(label(json!({"status": "completed", "metadata": {"status": "timeout"}})), "Timed out");
    assert_eq!(label(json!({"status": "completed", "metadata": {"status": "killed"}})), "Stopped");
    assert_eq!(label(json!({"status": "completed", "metadata": {"exit": 0}})), "exit 0");
    assert_eq!(label(json!({"status": "completed"})), "");
}

#[test]
fn collapses_long_output_to_its_last_lines_until_asked_for_the_rest() {
    let output: String = (1..=40).map(|i| format!("line {i}\n")).collect();
    let lines = output_lines(&output);
    assert_eq!(lines.len(), 40);
    let shown = shown_range(lines.len(), false);
    assert_eq!((lines[shown.start], lines[shown.end - 1], shown.len()), ("line 29", "line 40", 12));
    assert_eq!(shown_range(40, true), 0..40);
    assert_eq!(shown_range(12, false), 0..12);
    assert!(output_lines("\n\n").is_empty());
}
