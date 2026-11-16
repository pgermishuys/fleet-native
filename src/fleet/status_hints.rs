//! What each keyboard hint in the status bar says and whether it can be pressed (`StatusBar.vue`, `commandHint` and
//! `interruptHint`). Plain data: the view draws it, the tests read it.
use crate::ui::commands::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub disabled: bool,
    /// The command's label ("Hide sidebar"), empty when the command is missing.
    pub label: String,
    /// The tooltip: "<label> (<shortcut>)", or the command's own reason while it is disabled.
    pub title: String,
}

/// `commandHint`: a command that isn't there leaves its button disabled with "Not available yet".
pub fn command_hint(command: Option<&Command>, shortcut: &str) -> Hint {
    let Some(command) = command else { return Hint { disabled: true, label: String::new(), title: "Not available yet".into() } };
    let title = if command.disabled {
        if command.description.is_empty() { command.label.clone() } else { command.description.clone() }
    } else {
        format!("{} ({shortcut})", command.label)
    };
    Hint { disabled: command.disabled, label: command.label.clone(), title }
}

/// `interruptHint`: lit only while a session is open and its turn, delegation or retry is under way. A session that waits on
/// a question can be interrupted from the composer, but Fleet's hint stays dim then, and so does this.
pub fn interrupt_hint(command: Option<&Command>, session_open: bool, working: bool) -> Hint {
    let label = command.map(|c| c.label.clone()).unwrap_or_default();
    let reason = if command.is_none_or(|c| c.disabled) || !session_open {
        Some("Open a session to interrupt it")
    } else if !working {
        Some("Nothing to interrupt: the session isn't working")
    } else {
        None
    };
    match reason {
        Some(title) => Hint { disabled: true, label, title: title.into() },
        None => Hint { disabled: false, label, title: "Interrupt the session (Esc)".into() },
    }
}

/// Row 4.6: the terminal hint is there only while this Fleet allows terminals. With no server (the invented data of the
/// scenes) it is drawn, so the bar is complete.
pub fn terminal_shown(config: Option<bool>, has_server: bool) -> bool {
    config.unwrap_or(!has_server)
}
