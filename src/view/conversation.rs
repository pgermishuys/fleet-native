//! The conversation: the messages of the open session, scrolled and pinned to the end. Ports
//! `components/session/MessageList.vue`, `ToolCard.vue` (summary row) and `WorkingIndicator.vue`.
//!
//! This file also holds the message type it draws. It follows weave-fleet's `AccumulatedMessage`
//! (`client/src/lib/client-types.ts`) so the reducer in `src/fleet/` can fill it later: a message
//! has a role and ordered parts, and every part has an id.
use super::parts::quad;
use super::prose;
use super::toolrow;
use crate::State;
use crate::conv::tools::{ToolRowEntry, ToolStatus, group_reads};
use crate::icons::*;
use crate::mock::{Session, Status};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, Cycle, El, GrowthSpec, ScrollSpec, col, el, icon, memo, row, text};
use rune_kit::markdown::MarkdownStyle;
use std::borrow::Cow;

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
    /// A sent prompt the server hasn't confirmed in time: a line under it says so (`optimistic-retry`).
    pub needs_retry: bool,
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
        /// The tool's name as the harness sent it (`bash`, `Read`, `mcp__fleet__fleet_page_show`): the registry
        /// (`conv::tools`) says what it is called and drawn with.
        tool: String,
        call_id: String,
        /// The call's title: what it ran or touched (`getToolLabel`), or Fleet's answer for its own tools.
        detail: String,
        status: ToolStatus,
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

/// What a finished tool call has to show on the right of its row. Fleet keeps the whole state as JSON; the
/// reducer boils it down to one of these. Whether the call is running or failed is its `status`.
#[derive(Clone, Copy, PartialEq)]
pub enum ToolState {
    /// Lines added and removed by an edit.
    Diff(u32, u32),
    /// How long a shell command took, in seconds.
    Took(f32),
    Plain,
}

impl Message {
    pub fn new(id: impl Into<String>, role: Role, parts: Vec<Part>) -> Self {
        Message { id: id.into(), role, parts, needs_retry: false }
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
        Part::Tool { call_id: format!("call-{id}"), id, tool: tool.into(), detail: detail.into(), status: ToolStatus::Completed, state }
    }

    /// A call that is still running.
    pub fn running_tool(id: impl Into<String>, tool: &str, detail: &str) -> Self {
        match Part::tool(id, tool, detail, ToolState::Plain) {
            Part::Tool { id, tool, call_id, detail, state, .. } => {
                Part::Tool { id, tool, call_id, detail, status: ToolStatus::Running, state }
            }
            other => other,
        }
    }
}

/// What a row is worth when it hasn't been laid out yet; the real height replaces it once the row
/// is on screen.
const ESTIMATED_ROW: f32 = 150.0;

impl Part {
    /// A number that moves whenever the part draws differently. Text only ever grows while it
    /// streams, so its length says so; hashing it on every frame is the cost `memo` avoids.
    pub fn weight(&self) -> usize {
        match self {
            Part::Text { text, .. } | Part::Reasoning { text, .. } => text.len(),
            Part::Tool { state, status, detail, .. } => {
                let shown = match *state {
                    ToolState::Diff(a, r) => 2 + a as usize + r as usize,
                    ToolState::Took(secs) => 3 + secs.to_bits() as usize,
                    ToolState::Plain => 4,
                };
                shown + (*status as usize) * 7 + detail.len()
            }
            _ => 5,
        }
    }
}

impl Message {
    /// Changes whenever something the row draws changes, in a few integer reads.
    pub fn version(&self) -> (usize, usize) {
        (self.parts.len(), self.parts.iter().map(Part::weight).sum())
    }
}

/// The scrolling column of messages and, while a turn runs, the "Working" line. Only the rows on
/// screen are built (`virtual_rows`), and a row whose message hasn't changed is reused (`memo`).
pub fn stream(t: &Theme, session: &Session, partial: bool, older: Option<Older>) -> El {
    let theme = *t;
    let md = prose::style(t);
    let working = session.status == Status::Working;
    let count = session.messages.len() + usize::from(working);
    let rows = col().key("timeline").w_full().max_w(COLUMN).growth(GrowthSpec::default()).virtual_rows(
        count,
        ESTIMATED_ROW,
        |s: &State, i| match s.session().messages.get(i) {
            Some(m) => Cow::Borrowed(m.id.as_str()),
            None => Cow::Borrowed("working"),
        },
        move |s: &State, i| {
            let session = s.session();
            let Some(msg) = session.messages.get(i) else { return working_row(s, &theme) };
            let after_tool = i.checked_sub(1).map(|p| matches!(session.messages[p].parts.last(), Some(Part::Tool { .. })));
            let streaming = i + 1 == session.messages.len() && session.status == Status::Working && msg.role == Role::Assistant;
            let opened = open_mask(msg, &s.open_reads);
            memo(msg.id.clone(), (msg.version(), theme.dark, after_tool, streaming, msg.needs_retry, opened), || {
                message_row(&theme, &md, msg, after_tool, streaming, &s.open_reads)
            })
        },
    );
    col().key("stream").w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: true }).child(
        col()
            .w_full()
            .pt(24.0)
            .px(32.0)
            .pb(12.0)
            .items_center()
            .children(partial.then(|| partial_banner(&theme)))
            .children(older.map(|o| older_row(&theme, o)))
            .child(rows),
    )
}

/// `.partial-snapshot-banner` (`ActivityStream.vue`): the harness was unavailable and this is cached data.
/// Fleet's ⚠️ is an emoji; the drawing has no colour emoji font, so the mark is the text glyph.
fn partial_banner(t: &Theme) -> El {
    let font = sans(13.0, 400.0, 1.4);
    row()
        .key("partial-banner")
        .w_full()
        .max_w(COLUMN)
        .mb(16.0)
        .px(16.0)
        .py(12.0)
        .gap(8.0)
        .center_y()
        .fill(t.idle.alpha(0.10))
        .border(1.0, t.idle.alpha(0.30))
        .border_layout()
        .radius(RADIUS_CARD)
        .child(text("⚠").font(sans(14.0, 400.0, 1.0)).color(t.idle_text))
        .child(text("Connection to live session is temporarily unavailable. Showing cached data.").font(font).color(t.idle_text))
}

/// What the top of the stream offers when the conversation has older messages (`hasMore`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Older {
    /// "Load older messages".
    Ask,
    /// "Loading older messages…" with a spinner.
    Loading,
    /// "Couldn't load older messages · Try again".
    Failed,
}

/// `.load-older-indicator` and `.load-older-button` (`ActivityStream.vue`): 13px muted, centred, 8px above and below.
/// Fleet's spinner turns; Rune has no looping rotation, so this one fades slowly, which is what Fleet itself does
/// under reduced motion.
fn older_row(t: &Theme, older: Older) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    let font = sans(13.0, 400.0, 1.3);
    let line = row().w_full().max_w(COLUMN).py(8.0).gap(8.0).center_y().justify_center();
    match older {
        Older::Loading => line
            .key("load-older-loading")
            .child(icon(&LOADER_CIRCLE, 14.0).color(t.accent_text).cycle(Cycle {
                period: 1.2,
                phase: 0.0,
                stops: &FADE,
                still: 1.0,
                fps: 30.0,
            }))
            .child(text("Loading older messages\u{2026}").font(font).color(t.muted)),
        Older::Ask | Older::Failed => {
            let label = if older == Older::Failed { "Couldn't load older messages \u{b7} Try again" } else { "Load older messages" };
            line.key("load-older")
                .opacity(0.7)
                .hover_group()
                .cursor(Cursor::Pointer)
                .label(label)
                .on_press(|s: &mut State| s.load_older())
                .child(text(label).font(font).color(t.muted).group_color(t.text))
        }
    }
}

/// The ids of the "Read N files" groups a message draws, in order.
fn read_groups(msg: &Message) -> Vec<String> {
    let mut ids = Vec::new();
    for run in tool_runs(&msg.parts) {
        for entry in group_reads(run) {
            if let ToolRowEntry::Reads { id, .. } = entry {
                ids.push(id);
            }
        }
    }
    ids
}

/// Which of a message's read groups are open, one bit each: part of the row's memo key.
fn open_mask(msg: &Message, open: &std::collections::HashSet<String>) -> u64 {
    if open.is_empty() {
        return 0;
    }
    read_groups(msg).iter().take(64).enumerate().filter(|(_, id)| open.contains(*id)).fold(0, |mask, (i, _)| mask | 1 << i)
}

/// The runs of consecutive tool calls among a message's parts.
fn tool_runs(parts: &[Part]) -> Vec<Vec<&Part>> {
    let mut runs: Vec<Vec<&Part>> = Vec::new();
    let mut last_was_tool = false;
    for part in parts {
        let is_tool = matches!(part, Part::Tool { .. });
        match (is_tool, last_was_tool, runs.last_mut()) {
            (true, true, Some(run)) => run.push(part),
            (true, _, _) => runs.push(vec![part]),
            _ => {}
        }
        last_was_tool = is_tool;
    }
    runs
}

/// One message: its parts, spaced as Fleet spaces them. `after_tool` is how the message above
/// ended (`None` for the first), so a run of tool rows reads as one block across messages.
fn message_row(
    t: &Theme,
    md: &MarkdownStyle,
    msg: &Message,
    after_tool: Option<bool>,
    streaming: bool,
    open_reads: &std::collections::HashSet<String>,
) -> El {
    let mut column = col().w_full();
    let mut after_tool = after_tool;
    // 20px after a message or text, none between tool rows, 12px before the text that follows a run of them.
    let above = |after: Option<bool>, is_tool: bool| match (after, is_tool) {
        (None, _) | (Some(true), true) => 0.0,
        (Some(true), false) => 12.0,
        (Some(false), _) => 20.0,
    };
    let mut k = 0;
    while k < msg.parts.len() {
        if matches!(msg.parts[k], Part::Tool { .. }) {
            let end = msg.parts[k..].iter().position(|p| !matches!(p, Part::Tool { .. })).map_or(msg.parts.len(), |n| k + n);
            // Two or more reads in a row fold into one "Read N files" row (`groupReads`).
            for entry in group_reads(msg.parts[k..end].iter()) {
                let row = match entry {
                    ToolRowEntry::Tool(part) => toolrow::tool_entry(t, part),
                    ToolRowEntry::Reads { tools, id } => toolrow::reads_entry(t, &id, &tools, open_reads.contains(&id)),
                };
                column = column.child(el().w_full().mt(above(after_tool, true)).py(2.0).child(row));
                after_tool = Some(true);
            }
            k = end;
            continue;
        }
        let live = streaming && k + 1 == msg.parts.len();
        column = column.child(el().w_full().mt(above(after_tool, false)).py(2.0).child(part_view(t, md, msg.role, &msg.parts[k], live)));
        after_tool = Some(false);
        k += 1;
    }
    if msg.needs_retry {
        // `.optimistic-retry`: 12px in the idle colour, right-aligned under the user's bubble.
        column = column.child(row().w_full().justify_end().mt(4.0).mb(8.0).child(
            text("Not confirmed yet. Retry from the composer if this did not send.").font(sans(12.0, 400.0, 1.3)).color(t.idle).wrap(),
        ));
    }
    column
}

fn part_view(t: &Theme, md: &MarkdownStyle, role: Role, part: &Part, live: bool) -> El {
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
        (Part::Text { id, text: body }, Role::Assistant) => prose::prose(md, id, body, live),
        (Part::Tool { .. }, _) => toolrow::tool_entry(t, part),
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

/// "Working · 18s" under the last message (`WorkingIndicator.vue`), without Fleet's shimmer.
fn working_row(s: &State, t: &Theme) -> El {
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
