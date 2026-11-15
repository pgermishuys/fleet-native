//! Shell commands the user runs from the composer (`!git status`). The server gives each one a message of role `shell`
//! holding one tool part: its input is `{ command }`, its output what the command printed, and its metadata how it
//! ended (`exit`, `status`, `truncated`) when the harness says. Ports `lib/shell-commands.ts` and the collapsing and
//! state label of `ShellCommandBlock.vue`. No Rune in here.
use crate::fleet::model::{Msg, PartData};
use serde_json::Value;

/// Lines shown while collapsed.
pub const COLLAPSED_LINES: usize = 12;

/// How a command ended: still `Running`; `Done` (exit 0, or a harness that doesn't report exit codes); `Failed` (a
/// non-zero exit); `Timeout` or `Killed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellState {
    Running,
    Done,
    Failed,
    Timeout,
    Killed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShellCommand {
    pub id: String,
    pub command: String,
    pub output: String,
    pub state: ShellState,
    /// The exit code, when the harness reports one.
    pub exit: Option<i64>,
    /// The harness kept only part of the output.
    pub truncated: bool,
}

fn output_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        None | Some(Value::Null) => String::new(),
        Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// What a tool part's state shows as a shell command (`toShellCommandView`, from the part's id and raw state).
pub fn shell_command_of(id: &str, state: &Value) -> ShellCommand {
    let status = state.get("status").and_then(Value::as_str);
    let meta = state.get("metadata");
    let exit = meta.and_then(|m| m.get("exit")).and_then(Value::as_f64).map(|n| n as i64);
    let ended = meta.and_then(|m| m.get("status")).and_then(Value::as_str);
    let shell_state = if matches!(status, Some("pending" | "running")) {
        ShellState::Running
    } else if ended == Some("timeout") {
        ShellState::Timeout
    } else if ended == Some("killed") {
        ShellState::Killed
    } else if status == Some("error") || exit.is_some_and(|e| e != 0) {
        ShellState::Failed
    } else {
        ShellState::Done
    };
    // `state.output ?? (error when the call failed)`
    let printed = state.get("output").filter(|v| !v.is_null()).or_else(|| (status == Some("error")).then(|| state.get("error")).flatten());
    ShellCommand {
        id: id.to_string(),
        command: state.pointer("/input/command").and_then(Value::as_str).unwrap_or("").to_string(),
        output: output_text(printed),
        state: shell_state,
        exit,
        truncated: meta.and_then(|m| m.get("truncated")) == Some(&Value::Bool(true)),
    }
}

/// What a `shell` message shows, or `None` when it has no command part yet (the part arrives after the message).
pub fn to_shell_command(m: &Msg) -> Option<ShellCommand> {
    m.parts.iter().find_map(|p| if let PartData::Tool { id, state, .. } = p { Some(shell_command_of(id, state)) } else { None })
}

/// The right of the header: how the command ended; empty for a clean finish with no exit code to say.
pub fn state_label(c: &ShellCommand) -> String {
    match (c.state, c.exit) {
        (ShellState::Running, _) => "Running\u{2026}".into(),
        (ShellState::Failed, Some(code)) => format!("Failed \u{b7} exit {code}"),
        (ShellState::Failed, None) => "Failed".into(),
        (ShellState::Timeout, _) => "Timed out".into(),
        (ShellState::Killed, _) => "Stopped".into(),
        (ShellState::Done, Some(code)) => format!("exit {code}"),
        (ShellState::Done, None) => String::new(),
    }
}

/// The output's lines, trailing line ends off (none for no output).
pub fn output_lines(output: &str) -> Vec<&str> {
    let text = output.trim_end_matches('\n');
    if text.is_empty() { Vec::new() } else { text.split('\n').collect() }
}

/// Which lines are shown: the last 12 until the user asks for the rest.
pub fn shown_range(total: usize, expanded: bool) -> std::ops::Range<usize> {
    let hidden = if total > COLLAPSED_LINES && !expanded { total - COLLAPSED_LINES } else { 0 };
    hidden..total
}

/// Whether a failed, timed out or stopped command (the three that colour the block's border and state label).
pub fn went_wrong(state: ShellState) -> bool {
    matches!(state, ShellState::Failed | ShellState::Timeout | ShellState::Killed)
}
