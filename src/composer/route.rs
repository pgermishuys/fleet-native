//! Where a draft goes (`routeDraft`) and the parses it uses: `parseSlashCommand` (`slash-command-utils.ts`),
//! `isShellDraft` / `shellDraftCommand` (`shell-commands.ts`), `parseSideQuestion` (`side-conversation.ts`).
use super::status::{Caps, ComposerStatus};

/// Where a draft goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `/btw`, or anything typed while the side conversation is open.
    Side {
        question: String,
    },
    Shell {
        command: String,
        queue: bool,
    },
    Command {
        command: String,
        args: String,
        queue: bool,
    },
    Prompt {
        queue: bool,
        steer: bool,
    },
    Empty,
}

/// What `routeDraft` needs besides the text.
#[derive(Debug, Clone, Copy)]
pub struct DraftContext {
    pub status: ComposerStatus,
    /// Send now: into the running turn, when the harness can take it there.
    pub steer: bool,
    pub caps: Caps,
    /// The side conversation is open: everything typed goes to it.
    pub side_open: bool,
}

/// `parseSlashCommand`: trim the start, it must start with `/`, the command runs to the first whitespace, the
/// rest is trimmed as arguments. A bare `/` is none.
pub fn parse_slash_command(text: &str) -> Option<(String, String)> {
    let without = text.trim_start().strip_prefix('/')?;
    let (command, args) = match without.find(char::is_whitespace) {
        Some(at) => (&without[..at], without[at..].trim_start_matches(char::is_whitespace).trim()),
        None => (without, ""),
    };
    // `args = withoutSlash.slice(spaceIndex + 1).trim()`: one whitespace char dropped, then trimmed; the same text.
    (!command.is_empty()).then(|| (command.to_string(), args.to_string()))
}

/// The composer's shell mode: a draft that starts with `!`.
pub fn is_shell_draft(text: &str) -> bool {
    text.starts_with('!')
}

/// The command a shell-mode draft runs: everything after the `!`, trimmed.
pub fn shell_draft_command(text: &str) -> String {
    if is_shell_draft(text) { text[1..].trim().to_string() } else { String::new() }
}

/// `parseSideQuestion`, `/^\s*\/btw(?:\s+([\s\S]*))?$/`: the text after `/btw`, trimmed (empty for a bare
/// `/btw`). `None` when the draft isn't a `/btw` command (`/btwx` isn't one).
pub fn parse_side_question(draft: &str) -> Option<String> {
    let rest = draft.trim_start().strip_prefix("/btw")?;
    if rest.is_empty() {
        return Some(String::new());
    }
    // The pattern needs whitespace between the command and the question.
    rest.starts_with(char::is_whitespace).then(|| rest.trim().to_string())
}

/// `routeDraft`: `/btw` (or anything while the side conversation is open) to the side; `!` to the shell, queued
/// while a turn runs; a slash command as a command, queued while a turn runs; anything else as a prompt, queued
/// while a turn runs unless it's steered in and the harness can take it.
pub fn route_draft(text: &str, cx: &DraftContext) -> Route {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Route::Empty;
    }
    let side = parse_side_question(trimmed);
    if side.is_some() || cx.side_open {
        return Route::Side { question: side.unwrap_or_else(|| trimmed.to_string()) };
    }
    let busy = cx.status == ComposerStatus::Busy;
    if cx.caps.supports_shell && is_shell_draft(text) {
        return Route::Shell { command: shell_draft_command(text), queue: busy };
    }
    if let Some((command, args)) = parse_slash_command(text) {
        return Route::Command { command, args, queue: busy };
    }
    let steer = cx.steer && busy && cx.caps.can_steer;
    Route::Prompt { queue: busy && !steer, steer }
}

/// 12.4: whether Enter or Ctrl/Cmd+Enter steers. A harness that steers by default steers on Enter and queues on
/// the modifier; the others queue on Enter and steer on the modifier. Idle, either key is a plain send.
pub fn steer_for_key(caps: &Caps, modifier: bool) -> bool {
    if caps.steers_by_default { !modifier } else { modifier }
}
