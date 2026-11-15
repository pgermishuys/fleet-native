//! The conversation: the messages of the open session, scrolled and pinned to the end. Ports
//! `components/session/MessageList.vue`, `ToolCard.vue` (summary row) and `WorkingIndicator.vue`.
//!
//! This file also holds the message type it draws. It follows weave-fleet's `AccumulatedMessage`
//! (`client/src/lib/client-types.ts`) so the reducer in `src/fleet/` can fill it later: a message
//! has a role and ordered parts, and every part has an id.
use super::compaction;
use super::failure;
use super::fold;
use super::interrupted;
use super::message::{self, copy_text};
use super::prose;
use super::reasoning;
use super::shell;
use super::toolrow;
use super::working;
use crate::State;
use crate::conv::bubble::{cluster_position, show_meta};
use crate::conv::card::ToolBody;
use crate::conv::compaction::CompactionView;
use crate::conv::fold_plan::{FoldPlan, Kept};
use crate::conv::shell::ShellCommand;
use crate::conv::tools::{ToolRowEntry, ToolStatus, group_reads};
use crate::fleet::model::{ModMark, SlashCommand};
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
    pub meta: Meta,
}

/// What a message's frame shows besides its parts (`MessageBubble.vue`): when it was made, by which model, the slash
/// command a prompt came from, who sent it for the user, and whether it was sent mid-turn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub created_at: Option<i64>,
    pub model: Option<String>,
    pub command: Option<SlashCommand>,
    pub mod_mark: Option<ModMark>,
    pub steered: bool,
    /// The failure that ended the turn, drawn as a card under the message (`turnError`).
    pub failure: Option<Failure>,
}

/// A turn's failure (`TurnError`): the harness's name for it, its message, and the limit that caused it, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub name: String,
    pub message: String,
    pub kind: Option<String>,
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
        /// What opens under the row: output, summary, preview.
        body: Box<ToolBody>,
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
        trigger: Option<String>,
        tokens_before: Option<u32>,
        tokens_after: Option<u32>,
        /// What the model goes on from: the divider's own, or the message a harness wrote it as.
        summary: Option<String>,
    },
    /// A command the user ran from the composer (a `shell` message's one tool part).
    Shell(ShellCommand),
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
        Message { id: id.into(), role, parts, needs_retry: false, meta: Meta::default() }
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
        Part::Tool {
            call_id: format!("call-{id}"),
            id,
            tool: tool.into(),
            detail: detail.into(),
            status: ToolStatus::Completed,
            state,
            body: Box::default(),
        }
    }

    /// A call that is still running.
    pub fn running_tool(id: impl Into<String>, tool: &str, detail: &str) -> Self {
        match Part::tool(id, tool, detail, ToolState::Plain) {
            Part::Tool { id, tool, call_id, detail, state, body, .. } => {
                Part::Tool { id, tool, call_id, detail, status: ToolStatus::Running, state, body }
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
            Part::Tool { state, status, detail, body, .. } => {
                let shown = match *state {
                    ToolState::Diff(a, r) => 2 + a as usize + r as usize,
                    ToolState::Took(secs) => 3 + secs.to_bits() as usize,
                    ToolState::Plain => 4,
                };
                shown + (*status as usize) * 7 + detail.len() + body.output.len() + body.summary.len()
            }
            Part::Compaction { summary, .. } => 5 + summary.as_ref().map_or(0, String::len),
            Part::Shell(c) => 6 + c.output.len() + (c.state as usize) * 3 + c.exit.map_or(0, |e| e as usize),
            Part::File { .. } => 5,
        }
    }
}

impl Message {
    /// The message's text parts, as the body Fleet copies and parses.
    pub fn text(&self) -> String {
        let texts: Vec<&str> =
            self.parts.iter().filter_map(|p| if let Part::Text { text, .. } = p { Some(text.as_str()) } else { None }).collect();
        texts.join("\n\n")
    }

    /// Changes whenever something the row draws changes, in a few integer reads.
    pub fn version(&self) -> (usize, usize) {
        let failure = self.meta.failure.as_ref();
        (
            self.parts.len() + usize::from(failure.is_some()),
            self.parts.iter().map(Part::weight).sum::<usize>() + failure.map_or(0, |f| f.message.len()),
        )
    }
}

/// The scrolling column of messages and, while a turn runs, the "Working" line. Only the rows on
/// screen are built (`virtual_rows`), and a row whose message hasn't changed is reused (`memo`).
pub fn stream(t: &Theme, session: &Session, plan: &FoldPlan, partial: bool, older: Option<Older>) -> El {
    let theme = *t;
    let md = Styles { prose: prose::style(t), reasoning: reasoning::style(t), summary: compaction::style(t) };
    // After the messages: the card a restart left, then the line that says what the turn is doing. Fleet's line shows
    // while the turn isn't idle, so a question that holds it shows "Needs input" there.
    let working = session.status != Status::Idle;
    let interrupted = session.row.interrupted.is_some();
    let shown = plan.count(session.messages.len());
    let count = shown + usize::from(interrupted) + usize::from(working);
    let rows = col().key("timeline").w_full().max_w(COLUMN).growth(GrowthSpec::default()).virtual_rows(
        count,
        ESTIMATED_ROW,
        |s: &State, i| {
            let session = s.session();
            match s.fold_plan.index(i, session.messages.len()).and_then(|m| session.messages.get(m)) {
                Some(m) => Cow::Borrowed(m.id.as_str()),
                None if session.row.interrupted.is_some() && i == s.fold_plan.count(session.messages.len()) => Cow::Borrowed("interrupted"),
                None => Cow::Borrowed("working"),
            }
        },
        move |s: &State, i| {
            let session = s.session();
            let plan = &s.fold_plan;
            let len = session.messages.len();
            let Some(at) = plan.index(i, len) else {
                return match session.row.interrupted.as_ref() {
                    Some(card) if i == plan.count(len) => col().w_full().child(interrupted::card(&theme, s, card)),
                    _ => working::line(&theme, s),
                };
            };
            let msg = &session.messages[at];
            let kept = plan.kept.get(&msg.id);
            // The message above, as drawn: folding may have taken the ones between.
            let above = i.checked_sub(1).and_then(|p| plan.index(p, len)).map(|p| &session.messages[p]);
            let above_kept = above.and_then(|m| plan.kept.get(&m.id));
            let after_tool =
                above.map(|m| drawn_parts(m, above_kept).last().is_some_and(|p| matches!(p, Part::Tool { .. } | Part::Shell(_))));
            let streaming = at + 1 == len && session.status == Status::Working && msg.role == Role::Assistant;
            let opened = open_mask(msg, s);
            let same_as_prev = at.checked_sub(1).is_some_and(|p| session.messages[p].role == msg.role);
            // The pill's relative time moves every 30s; "Copied" lasts a moment.
            let frame = (
                same_as_prev,
                session.messages.get(at + 1).is_some_and(|n| n.role == msg.role),
                msg.meta.created_at.map(|_| message::now_ms(s) / 30_000),
                s.copied.as_ref().is_some_and(|(m, until)| *m == msg.id && s.clock < *until),
                s.prompt_open.contains(&msg.id),
            );
            // A failure card also follows the newest message, the retry's countdown and its buttons.
            let failure = msg.meta.failure.as_ref().map(|_| s.failure_frame(msg, at + 1 == len));
            memo(
                msg.id.clone(),
                (
                    msg.version(),
                    theme.dark,
                    after_tool,
                    streaming,
                    msg.needs_retry,
                    opened,
                    s.thinking,
                    frame,
                    s.lists().models.len(),
                    fold_key(kept),
                    failure,
                ),
                || message_row(&theme, &md, msg, (kept, after_tool), streaming, s, (frame.0, frame.1)),
            )
        },
    );
    col().key("stream").w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: true }).child(
        col()
            .w_full()
            .pt(24.0)
            .px(32.0)
            .pb(30.0)
            .items_center()
            .children(partial.then(|| partial_banner(&theme)))
            .children(older.map(|o| older_row(&theme, o)))
            .child(rows),
    )
}

/// The Markdown looks a message draws with: its text, its reasoning, and a compaction's summary.
pub struct Styles {
    pub prose: MarkdownStyle,
    reasoning: MarkdownStyle,
    summary: MarkdownStyle,
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

/// Which of a message's read groups and tool cards the user opened, widened or showed all of the diff of: part of the row's memo key. Nothing
/// opened (the common case) costs two empty checks.
fn open_mask(msg: &Message, s: &State) -> u64 {
    use std::hash::{Hash, Hasher};
    if s.open_reads.is_empty()
        && s.tool_open.is_empty()
        && s.tool_wide.is_empty()
        && s.diff_all.is_empty()
        && s.diff_open.is_empty()
        && s.reasoning_open.is_empty()
        && s.summary_open.is_empty()
        && s.shell_wide.is_empty()
        && s.prompt_open.is_empty()
        && !s.inline_diffs
    {
        return 0;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for id in read_groups(msg) {
        s.open_reads.contains(&id).hash(&mut hash);
    }
    for part in &msg.parts {
        match part {
            Part::Reasoning { id, .. } => s.reasoning_open.contains(id).hash(&mut hash),
            Part::Compaction { id, .. } => s.summary_open.contains(id).hash(&mut hash),
            Part::Shell(c) => s.shell_wide.contains(&c.id).hash(&mut hash),
            _ => {}
        }
        if let Part::Tool { id, .. } = part {
            s.tool_open.get(id.as_str()).hash(&mut hash);
            s.tool_wide.contains(id.as_str()).hash(&mut hash);
            s.diff_all.contains(id.as_str()).hash(&mut hash);
            s.inline_diffs.hash(&mut hash);
            // The collapsed runs opened in this card's diff (`tool-<id>-diff#<lines>:<run>`).
            let prefix = format!("tool-{id}-diff#");
            let mut runs: Vec<&String> = s.diff_open.iter().filter(|k| k.starts_with(&prefix)).collect();
            runs.sort();
            runs.hash(&mut hash);
        }
    }
    hash.finish()
}

/// The parts of a message that draw once its turn folds: only the calls that stayed, and its thinking only if it stayed.
fn drawn_parts<'a>(msg: &'a Message, kept: Option<&Kept>) -> Vec<&'a Part> {
    msg.parts
        .iter()
        .filter(|p| match (p, kept) {
            (Part::Tool { id, .. }, Some(k)) => k.tools.contains(id),
            (Part::Reasoning { .. }, Some(k)) => k.reasoning,
            _ => true,
        })
        .collect()
}

/// A number that moves when folding changes how the message draws (the fold row, its open state, what stayed).
fn fold_key(kept: Option<&Kept>) -> u64 {
    use std::hash::{Hash, Hasher};
    let Some(kept) = kept else { return 0 };
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    kept.tools.len().hash(&mut hash);
    (kept.reasoning, kept.fold_only).hash(&mut hash);
    if let Some((turn, open)) = &kept.fold {
        let s = &turn.summary;
        (open, s.duration_ms, s.reads, s.edits, s.commands, s.other, s.failed, s.outcome as u8).hash(&mut hash);
    }
    hash.finish() | 1
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
    md: &Styles,
    msg: &Message,
    // What folding left of it, and how the message above ended.
    (kept, after_tool): (Option<&Kept>, Option<bool>),
    streaming: bool,
    s: &State,
    // Whether the message above and the one below are the same sender's.
    (grouped_with_prev, grouped_with_next): (bool, bool),
) -> El {
    let mut column = col().key(format!("msg-{}", msg.id)).w_full().hover_group();
    let mut after_tool = after_tool;
    // 20px after a message or text, none between tool rows, 12px before the text that follows a run of them.
    let above = |after: Option<bool>, is_tool: bool| match (after, is_tool) {
        (None, _) | (Some(true), true) => 0.0,
        (Some(true), false) => 12.0,
        // Consecutive messages of one sender sit 6px apart (`first`/`middle` of a cluster); the rest 20px.
        (Some(false), _) if grouped_with_prev => 6.0,
        (Some(false), _) => 20.0,
    };
    // The fold row comes first among the marks above a bubble (row 1.22).
    let mut lead = above(after_tool, false);
    if let Some((turn, open)) = kept.and_then(|k| k.fold.as_ref()) {
        column = column.child(el().w_full().mt(above(after_tool, true)).child(fold::fold_row(t, turn, *open)));
        after_tool = Some(true);
        lead = 0.0;
    }
    let fold_only = kept.is_some_and(|k| k.fold_only);
    let parts = drawn_parts(msg, kept);
    // The gap above a message goes above its marks, not between them and the bubble.
    let marks = message::marks(t, msg, lead);
    let mut lead_spent = marks.is_some();
    column = column.children(marks);
    let mut k = 0;
    let mut copy_pinned = false;
    while k < parts.len() {
        // Thinking that is hidden, or has nothing to say, takes no room either.
        if let Part::Reasoning { text, summary, .. } = parts[k]
            && !reasoning::is_drawn(s.thinking, text, summary.as_deref())
        {
            k += 1;
            continue;
        }
        if matches!(parts[k], Part::Tool { .. }) {
            let end = parts[k..].iter().position(|p| !matches!(p, Part::Tool { .. })).map_or(parts.len(), |n| k + n);
            // Two or more reads in a row fold into one "Read N files" row (`groupReads`).
            for entry in group_reads(parts[k..end].iter().copied()) {
                let row = match entry {
                    ToolRowEntry::Tool(part) => toolrow::tool_entry(t, s, part),
                    ToolRowEntry::Reads { tools, id } => toolrow::reads_entry(t, s, &id, &tools, s.open_reads.contains(&id)),
                };
                column = column.child(el().w_full().mt(above(after_tool, true)).py(2.0).child(row));
                after_tool = Some(true);
            }
            k = end;
            continue;
        }
        let live = streaming && k + 1 == parts.len();
        // Commands the user ran sit close together, as tool rows do (Fleet stacks them 7px apart).
        let is_shell = matches!(parts[k], Part::Shell(_));
        let gap = if is_shell && after_tool == Some(true) { 3.0 } else { above(after_tool, false) };
        let gap = if std::mem::take(&mut lead_spent) { 0.0 } else { gap };
        let mut part = el().w_full().mt(gap).py(2.0).child(part_view(t, s, md, msg, parts[k], live));
        // The copy button of an assistant message sits at the top right of its text, below the gap above it (a user's
        // is inside its bubble). It hangs on the first text part so the 20px gap isn't counted into its offset.
        if !copy_pinned
            && !fold_only
            && msg.role != Role::User
            && matches!(parts[k], Part::Text { .. })
            && !copy_text(msg).trim().is_empty()
        {
            part = part.child(message::copy_button(t, s, msg).pin(None, Some(0.0), Some(0.0), None));
            copy_pinned = true;
        }
        column = column.child(part);
        after_tool = Some(is_shell);
        k += 1;
    }
    let user = msg.role == Role::User;
    let cluster = cluster_position(grouped_with_prev, grouped_with_next);
    // A message with no text part (tools only) still copies: the button goes on the column itself.
    if !user && !fold_only && !copy_pinned && !copy_text(msg).trim().is_empty() {
        column = column.child(message::copy_button(t, s, msg).pin(None, Some(0.0), Some(0.0), None));
    }
    // A turn of only tool rows has no bubble, so no pill either.
    if !user && !fold_only && show_meta(false, cluster) && !copy_text(msg).trim().is_empty() {
        column = column.children(message::meta_pill(t, s, msg));
    }
    if let Some(f) = &msg.meta.failure {
        let latest = s.session().messages.last().is_some_and(|m| m.id == msg.id);
        let around =
            failure::Around { latest, has_prompt: failure::last_prompt(&s.session().messages).is_some(), can_send: !s.composer_disabled() };
        column = column.child(failure::card(t, s, msg, f, &around));
    }
    if msg.needs_retry {
        // `.optimistic-retry`: 12px in the idle colour, right-aligned under the user's bubble.
        column = column.child(row().w_full().justify_end().mt(4.0).mb(8.0).child(
            text("Not confirmed yet. Retry from the composer if this did not send.").font(sans(12.0, 400.0, 1.3)).color(t.idle).wrap(),
        ));
    }
    column
}

fn part_view(t: &Theme, s: &State, md: &Styles, msg: &Message, part: &Part, live: bool) -> El {
    match (part, msg.role) {
        (Part::Text { id, text: body }, Role::User) => message::user_bubble(t, s, md, msg, id, body),
        (Part::Text { id, text: body }, Role::Shell) => message::user_bubble(t, s, md, msg, id, body),
        (Part::Text { id, text: body }, Role::Assistant) => prose::prose(&md.prose, id, body, live),
        (Part::Tool { .. }, _) => toolrow::tool_entry(t, s, part),
        (Part::Reasoning { id, text: body, summary }, _) => {
            reasoning::reasoning_row(t, s, &md.reasoning, id, body, summary.as_deref(), live)
        }
        (Part::Shell(command), _) => shell::shell_block(t, s, command),
        (Part::File { filename, mime, .. }, _) => {
            text(filename.clone().unwrap_or_else(|| mime.clone())).font(mono(12.0, 400.0, 1.3)).color(t.muted).truncate().w_full()
        }
        (Part::Compaction { id, trigger, tokens_before, tokens_after, summary }, _) => {
            let view = CompactionView {
                trigger: trigger.clone(),
                tokens_before: *tokens_before,
                tokens_after: *tokens_after,
                summary: summary.clone(),
            };
            compaction::divider(t, s, &md.summary, id, &view)
        }
    }
}
