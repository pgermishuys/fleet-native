//! The conversation: the messages of the open session, scrolled and pinned to the end. Ports
//! `components/session/MessageList.vue`, `ToolCard.vue` (summary row) and `WorkingIndicator.vue`.
//!
//! This file also holds the message type it draws. It follows weave-fleet's `AccumulatedMessage`
//! (`client/src/lib/client-types.ts`) so the reducer in `src/fleet/` can fill it later: a message
//! has a role and ordered parts, and every part has an id.
use super::parts::{pulse, quad};
use crate::State;
use crate::icons::*;
use crate::mock::{Session, Status};
use crate::theme::{Theme, mono, sans};
use rune::{Cursor, El, Icon, ScrollSpec, col, el, icon, row, text};

/// The conversation's and the composer's column (`max-width: 760px`).
pub const COLUMN: f32 = 760.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    User,
    Assistant,
    /// A command the user ran from the composer.
    Shell,
}

pub struct Message {
    pub id: String,
    pub role: Role,
    pub parts: Vec<Part>,
}

pub enum Part {
    /// Markdown.
    Text {
        id: String,
        text: String,
    },
    Reasoning {
        id: String,
        text: String,
        summary: Option<String>,
    },
    Tool {
        id: String,
        tool: String,
        call_id: String,
        detail: String,
        state: ToolState,
    },
    File {
        id: String,
        mime: String,
        filename: Option<String>,
        url: String,
    },
    /// Where the harness compacted the conversation.
    Compaction {
        id: String,
        tokens_before: Option<u32>,
        tokens_after: Option<u32>,
    },
}

/// How a tool call went, as its summary row shows it. Fleet keeps the whole state as JSON; the
/// reducer boils it down to one of these.
#[derive(Clone, Copy, PartialEq)]
pub enum ToolState {
    Running,
    /// Lines added and removed by an edit.
    Diff(u32, u32),
    /// How long a shell command took, in seconds.
    Took(f32),
    Plain,
}

impl Message {
    pub fn new(id: impl Into<String>, role: Role, parts: Vec<Part>) -> Self {
        Message { id: id.into(), role, parts }
    }

    pub fn user(id: impl Into<String>, body: impl Into<String>) -> Self {
        let id = id.into();
        let part = Part::text(format!("{id}-t"), body);
        Message::new(id, Role::User, vec![part])
    }

    pub fn assistant(id: impl Into<String>, parts: Vec<Part>) -> Self {
        Message::new(id, Role::Assistant, parts)
    }
}

impl Part {
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Part::Text { id: id.into(), text: text.into() }
    }

    pub fn tool(id: impl Into<String>, tool: &str, detail: &str, state: ToolState) -> Self {
        let id = id.into();
        Part::Tool { call_id: format!("call-{id}"), id, tool: tool.into(), detail: detail.into(), state }
    }
}

/// The scrolling column of messages and, while a turn runs, the "Working" line.
pub fn stream(s: &State, t: &Theme, session: &Session) -> El {
    let mut column = col().w_full().max_w(COLUMN);
    // Fleet's margins: 20px after a message or text, none between tool rows, 12px before the text
    // that follows a run of them.
    let mut after_tool = None;
    for msg in &session.messages {
        for part in &msg.parts {
            let is_tool = matches!(part, Part::Tool { .. });
            let above = match (after_tool, is_tool) {
                (None, _) => 0.0,
                (Some(true), true) => 0.0,
                (Some(true), false) => 12.0,
                (Some(false), _) => 20.0,
            };
            column = column.child(el().w_full().mt(above).py(2.0).child(part_view(t, msg.role, part)));
            after_tool = Some(is_tool);
        }
    }
    if session.status == Status::Working {
        column = column.child(working(s, t));
    }
    col()
        .key("stream")
        .w_full()
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: true })
        .child(col().w_full().pt(24.0).px(32.0).pb(12.0).items_center().child(column))
}

fn part_view(t: &Theme, role: Role, part: &Part) -> El {
    match (part, role) {
        // Right-aligned with the small corner at bottom right. The empty 18% in front keeps the
        // bubble to Fleet's `max-width: 82%` of the column at any width.
        (Part::Text { text: body, .. }, Role::User | Role::Shell) => row().w_full().justify_end().child(el().w_frac(0.18)).child(
            text(body.clone())
                .font(sans(15.0, 400.0, 1.55))
                .color(t.text)
                .wrap()
                .selectable(true)
                .shrink()
                .px(14.0)
                .py(10.0)
                .corners([14.0, 14.0, 4.0, 14.0])
                .border(1.0, t.border)
                .fill(t.wash(5.0)),
        ),
        (Part::Text { text: body, .. }, Role::Assistant) => {
            text(body.clone()).font(sans(15.0, 400.0, 1.65)).color(t.text).wrap().selectable(true).w_full()
        }
        (Part::Tool { tool, detail, state, .. }, _) => tool_row(t, tool, detail, *state),
        // The remaining parts get their real look in the parts lane; for now they say what they are.
        (Part::Reasoning { text: body, summary, .. }, _) => {
            let shown = summary.as_deref().unwrap_or(body);
            text(shown.to_string()).font(sans(13.0, 400.0, 1.5)).color(t.muted).wrap().w_full()
        }
        (Part::File { filename, mime, .. }, _) => {
            text(filename.clone().unwrap_or_else(|| mime.clone())).font(mono(12.0, 400.0, 1.3)).color(t.muted).truncate().w_full()
        }
        (Part::Compaction { .. }, _) => text("Conversation compacted").font(sans(12.0, 400.0, 1.3)).color(t.muted).w_full(),
    }
}

/// One tool call (`ToolCard.vue`'s summary row): icon, name, what it ran, and how it went.
fn tool_row(t: &Theme, name: &str, detail: &str, state: ToolState) -> El {
    let glyph: &'static Icon = match name {
        "Bash" => &TERMINAL,
        "Edit" => &PENCIL,
        _ => &FILE_TEXT,
    };
    // Reads are quieter than commands and edits.
    let (weight, ink) = if name == "Read" { (400.0, t.muted) } else { (500.0, t.text) };
    let count = mono(12.0, 400.0, 1.3);
    let line = row()
        .w_full()
        .min_h(30.0)
        .center_y()
        .gap(9.0)
        .px(8.0)
        .radius(6.0)
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .on_press(|_: &mut State| {})
        .child(icon(glyph, 14.0).color(t.muted.alpha(0.75)))
        .child(text(name.to_string()).font(sans(13.0, weight, 1.3)).color(ink))
        .child(text(detail.to_string()).font(mono(12.0, 400.0, 1.3)).color(t.muted).truncate().grow(1.0));
    match state {
        ToolState::Running => line.child(pulse(t.running)),
        ToolState::Diff(added, removed) => line
            .child(text(format!("+{added}")).font(count).color(t.running))
            .child(text(format!("\u{2212}{removed}")).font(count).color(t.error)),
        ToolState::Took(secs) => line.child(text(format!("{secs:.1}s")).font(count).color(t.muted)),
        ToolState::Plain => line,
    }
}

/// "Working · 18s" under the last message (`WorkingIndicator.vue`), without Fleet's shimmer.
fn working(s: &State, t: &Theme) -> El {
    let seconds = (s.clock - s.working_since).max(0.0) as u64;
    let time = if seconds < 60 { format!("{seconds}s") } else { format!("{}m", seconds / 60) };
    let font = sans(13.0, 400.0, 1.3).features("'tnum' 1");
    row()
        .w_full()
        .h(24.0)
        .my(4.0)
        .center_y()
        .gap(10.0)
        .child(quad(t.text))
        .child(row().gap(5.0).child(text("Working").font(font).color(t.muted)).child(text(format!("· {time}")).font(font).color(t.muted)))
}
