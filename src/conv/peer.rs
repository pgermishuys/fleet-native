//! Messages between sessions (`lib/session-messages.ts`): one session's agent tells another something with `fleet_message`,
//! and Fleet delivers it as a prompt wrapped in a tag that names the sender. The tag is in the text itself, so it survives
//! a reload. A sender on another machine is named with that machine; an update (the sender asked to hear back) has its own
//! tag so it can't pass for something the session's agent wrote.
use super::output::plain_terminal_text;

/// Who sent it: the session, and the machine when it isn't the one the message arrived at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerSender {
    pub session_id: String,
    pub title: String,
    pub machine_id: Option<String>,
    pub machine_name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerOutcome {
    Finished,
    Failed,
}

impl PeerOutcome {
    pub fn word(self) -> &'static str {
        match self {
            PeerOutcome::Finished => "finished",
            PeerOutcome::Failed => "failed",
        }
    }
}

/// What a peer's message came to: the sender, how its turn ended (an update only) and the words.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub sender: PeerSender,
    pub outcome: Option<PeerOutcome>,
    pub text: String,
}

/// Fleet HTML-encodes the id and title in the tag.
fn decode(value: &str) -> String {
    value.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// `name="value"` pairs of a tag's head, in order; values hold no quote (they are encoded).
fn attributes(head: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = head.trim_start();
    while let Some(eq) = rest.find("=\"") {
        let name = rest[..eq].trim();
        let after = &rest[eq + 2..];
        let Some(end) = after.find('"') else { break };
        out.push((name, &after[..end]));
        rest = after[end + 1..].trim_start();
    }
    out
}

/// The text between a tag's `>` and its closing tag, with the one newline after and before it dropped.
fn inner<'a>(body: &'a str, tag: &str) -> Option<(&'a str, &'a str)> {
    let open = format!("<{tag} ");
    let close = format!("</{tag}>");
    let rest = body.strip_prefix(open.as_str())?;
    let body_end = rest.trim_end().strip_suffix(close.as_str())?;
    let head_end = body_end.find('>')?;
    let (head, text) = (&body_end[..head_end], &body_end[head_end + 1..]);
    let text = text.strip_prefix('\n').unwrap_or(text);
    Some((head, text.strip_suffix('\n').unwrap_or(text)))
}

/// The sender and the text of a message another session sent, or `None` for anything else (`parsePeerMessage`).
pub fn parse_peer_message(body: &str) -> Option<Peer> {
    let (head, text) = inner(body, "fleet-session-message")?;
    let attrs = attributes(head);
    let get = |name: &str| attrs.iter().find(|(n, _)| *n == name).map(|(_, v)| decode(v));
    // The tag is `from`, `title`, then optionally `machine` and `machine-name`, in that order (the regex's shape).
    let names: Vec<&str> = attrs.iter().map(|(n, _)| *n).collect();
    if names != ["from", "title"] && names != ["from", "title", "machine", "machine-name"] {
        return None;
    }
    Some(Peer {
        sender: PeerSender {
            session_id: get("from")?,
            title: plain_terminal_text(&get("title")?),
            machine_id: get("machine"),
            machine_name: get("machine-name"),
        },
        outcome: None,
        text: plain_terminal_text(text),
    })
}

/// The session, how its turn ended and what it said, for an update Fleet sent (`parsePeerUpdate`).
pub fn parse_peer_update(body: &str) -> Option<Peer> {
    let (head, text) = inner(body, "fleet-session-update")?;
    let attrs = attributes(head);
    let get = |name: &str| attrs.iter().find(|(n, _)| *n == name).map(|(_, v)| decode(v));
    let names: Vec<&str> = attrs.iter().map(|(n, _)| *n).collect();
    let ok = names == ["session", "title", "outcome"] || names == ["session", "title", "outcome", "machine", "machine-name"];
    if !ok {
        return None;
    }
    let outcome = match get("outcome")?.as_str() {
        "finished" => PeerOutcome::Finished,
        "failed" => PeerOutcome::Failed,
        _ => return None,
    };
    Some(Peer {
        sender: PeerSender {
            session_id: get("session")?,
            title: plain_terminal_text(&get("title")?),
            machine_id: get("machine"),
            machine_name: get("machine-name"),
        },
        outcome: Some(outcome),
        text: plain_terminal_text(text),
    })
}
