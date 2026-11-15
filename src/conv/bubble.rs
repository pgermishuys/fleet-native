//! What a message's frame decides before anything is drawn (`lib/activity-messages.ts`, `MessageBubble.vue`,
//! `ActivityStream.vue`): who a message sits close to, whether it shows its time and model, and what its marks say.
//! No Rune in here.
use super::references::format_slash_command;
use crate::fleet::model::{ModKind, SlashCommand};

/// Where a message sits among consecutive messages of one sender (`ClusterPosition`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cluster {
    Single,
    First,
    Middle,
    Last,
}

pub fn cluster_position(grouped_with_previous: bool, grouped_with_next: bool) -> Cluster {
    match (grouped_with_previous, grouped_with_next) {
        (true, true) => Cluster::Middle,
        (true, false) => Cluster::Last,
        (false, true) => Cluster::First,
        (false, false) => Cluster::Single,
    }
}

/// The pill with the time (and the model) hangs 20px below its message, but messages of one cluster sit 6px apart: on
/// a cluster's first and middle messages it would land on the next message's first line. Your own messages keep it,
/// because theirs hangs on the left (`showMeta`).
pub fn show_meta(user: bool, cluster: Cluster) -> bool {
    user || matches!(cluster, Cluster::Single | Cluster::Last)
}

/// A command's prompt is behind "Show prompt" when the body is more than the command typed: the harness expanded a
/// template. Fleet's own message while the harness works, or a harness that expands the command itself, is the command.
pub fn has_expanded_prompt(body: &str, command: &SlashCommand) -> bool {
    body.trim() != format_slash_command(&command.name, command.arguments.as_deref())
}

/// The mod mark's words and its tooltip (rows 1.55, 2.15).
pub fn mod_mark_text(kind: ModKind, name: &str) -> (String, &'static str) {
    match kind {
        ModKind::Sent => (
            format!("Sent by {name}"),
            if name.starts_with("Fleet") { "Fleet sent this message, not you." } else { "A mod sent this message, not you." },
        ),
        ModKind::Rewritten => (format!("Rewritten by {name}"), "A mod reworded what you typed before it was sent."),
    }
}

pub const STEERED_TEXT: &str = "Sent mid-turn";
pub const STEERED_TIP: &str =
    "You sent this while the agent was working. It read it at its next step, without waiting for the turn to end.";

/// Text worth running through Markdown; anything else is drawn as the plain text it is, which sizes a short bubble to
/// its words instead of the column.
pub fn looks_like_markdown(body: &str) -> bool {
    body.contains("```")
        || body.contains('`')
        || body.contains("**")
        || body.contains("](")
        || body.lines().any(|l| {
            let l = l.trim_start();
            l.starts_with("- ")
                || l.starts_with("* ")
                || l.starts_with("> ")
                || l.starts_with('#')
                || l.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
}
