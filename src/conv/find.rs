//! Find in this conversation: what the bar searches, how it steps, and who owns Ctrl/Cmd+F. Ports `lib/conversation-find.ts`
//! and the state machine of `composables/use-conversation-find.ts`. The search reads the conversation's message data, not
//! what is drawn, so it covers turns that are folded and rows that aren't built. No Rune in here but the field's text; the
//! bar is `view::find` and what the state does with it (paging, keys, the jump) is `fleet::find`.
use rune::{KeyEvent, TextEdit};
use std::ops::Range;

/// What the bar searches. Thinking text is never searched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Messages,
    /// "+ Tool output": the output of the calls too, after the message's own text.
    Tools,
}

/// One thing the search can land in: a message, with the text it shows and the output its calls hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindUnit {
    pub id: String,
    /// The turn the message belongs to when that turn can be folded; a hit in it opens the turn.
    pub turn_id: Option<String>,
    pub text: String,
    pub tool_text: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Text,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindHit {
    pub unit_id: String,
    pub turn_id: Option<String>,
    /// Which of the message's matches this is, in the order they're drawn (text first, then tool output).
    pub ordinal: usize,
    pub kind: HitKind,
}

/// Typing shorter than this finds nothing: one letter matches almost everything.
pub const FIND_MIN_CHARS: usize = 1;

/// How many times `needle` is in `haystack`, ignoring case, without overlapping. Counted on the lowered strings, as Fleet's
/// is, so a letter whose lowercase is longer doesn't move the count.
pub fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() || haystack.is_empty() {
        return 0;
    }
    let (lower, target) = (haystack.to_lowercase(), needle.to_lowercase());
    lower.matches(target.as_str()).count()
}

/// Every match in the conversation, oldest first.
pub fn find_hits(units: &[FindUnit], query: &str, scope: Scope) -> Vec<FindHit> {
    if query.trim().chars().count() < FIND_MIN_CHARS {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for unit in units {
        let mut ordinal = 0;
        let mut add = |n: usize, kind: HitKind| {
            for _ in 0..n {
                hits.push(FindHit { unit_id: unit.id.clone(), turn_id: unit.turn_id.clone(), ordinal, kind });
                ordinal += 1;
            }
        };
        add(count_occurrences(&unit.text, query), HitKind::Text);
        if scope == Scope::Tools {
            for output in &unit.tool_text {
                add(count_occurrences(output, query), HitKind::Tool);
            }
        }
    }
    hits
}

/// Where `needle` is in `text`, as byte ranges of `text`: ignoring case, without overlapping. What the marks draw over text
/// that isn't markdown (tool output, a user's plain words).
pub fn plain_ranges(text: &str, needle: &str) -> Vec<Range<usize>> {
    if needle.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let target = needle.to_lowercase();
    // The lowered text, and for each of its bytes the letter it came from.
    let mut lower = String::with_capacity(text.len());
    let mut from: Vec<(usize, usize)> = Vec::with_capacity(text.len());
    for (i, ch) in text.char_indices() {
        for lc in ch.to_lowercase() {
            let before = lower.len();
            lower.push(lc);
            from.extend((before..lower.len()).map(|_| (i, i + ch.len_utf8())));
        }
    }
    lower.match_indices(target.as_str()).map(|(at, m)| from[at].0..from[at + m.len() - 1].1).collect()
}

/// What one drawn block of a message marks: every match of the query, and which of them is the current one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks {
    pub query: String,
    pub current: Option<usize>,
}

/// Which block holds the `ordinal`-th match of a message and which of that block's matches it is. `blocks` are the matches
/// each drawn block holds, text first and then tool output, as the hits number them. An ordinal past the last mark lands on
/// the last (Fleet's `ranges[min(ordinal, len - 1)]`): the count reads the source, the marks the drawn text.
pub fn locate(blocks: &[usize], ordinal: usize) -> Option<(usize, usize)> {
    let total: usize = blocks.iter().sum();
    let mut left = ordinal.min(total.checked_sub(1)?);
    for (block, &n) in blocks.iter().enumerate() {
        if left < n {
            return Some((block, left));
        }
        left -= n;
    }
    None
}

// ─── focus ──────────────────────────────────────────────────────────────────

/// Where focus sits, as far as Ctrl/Cmd+F cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Nothing in particular (the page itself).
    Nothing,
    /// Something inside the conversation.
    Conversation,
    /// The session's text area. It has no Ctrl/Cmd+F of its own, so it counts as part of the conversation.
    Composer,
    /// The bar's own field.
    BarField,
    /// Another text field, a select, an editor or the terminal: they keep the key's own meaning.
    Field,
    /// A dialog, menu or the palette.
    Dialog,
    /// Somewhere else in the window (the session list, the content panel).
    Elsewhere,
}

/// A field, an editor, the terminal or a dialog keeps Ctrl/Cmd+F for itself (`focusKeepsFindKey`).
pub fn focus_keeps_find_key(focus: Focus) -> bool {
    matches!(focus, Focus::Field | Focus::Dialog)
}

/// Ctrl+F (Cmd+F on a Mac) with nothing else held (`isFindShortcut`). Ctrl+Shift+F is Find in files.
pub fn is_find_shortcut(ev: &KeyEvent, apple: bool) -> bool {
    let m = ev.mods;
    ev.is_command('f') && !m.shift && !m.alt && if apple { m.cmd && !m.ctrl } else { m.ctrl && !m.logo }
}

/// What a press of Ctrl/Cmd+F does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Not ours: the key goes on.
    Ignore,
    Open,
    /// The bar is open and its field has focus: select what it holds, as a browser's find does.
    SelectField,
}

/// `onKeydown` of `use-conversation-find.ts`. `enabled` is false for a side conversation; `visible` is false while another
/// view owns the window.
pub fn press(open: bool, focus: Focus, enabled: bool, visible: bool) -> Press {
    if !enabled || !visible {
        return Press::Ignore;
    }
    if open && focus == Focus::BarField {
        return Press::SelectField;
    }
    if focus_keeps_find_key(focus) {
        return Press::Ignore;
    }
    match focus {
        Focus::Nothing | Focus::Conversation | Focus::Composer => Press::Open,
        _ => Press::Ignore,
    }
}

/// What a key of the window is, by its element key: the one place that knows the element keys of the places a key can sit.
pub fn focus_of_key(key: Option<&str>) -> Focus {
    const CONVERSATION: [&str; 22] = [
        "agent-",
        "attached-",
        "background-",
        "bubble-",
        "compaction-",
        "copy-",
        "diff-",
        "interrupted-",
        "jump-",
        "load-older",
        "message-",
        "meta-",
        "msg-",
        "partial-",
        "reads-",
        "reasoning-",
        "shell-",
        "shot-",
        "step",
        "tool-",
        "turn-",
        "work",
    ];
    match key {
        None => Focus::Nothing,
        Some("composer") => Focus::Composer,
        Some(FIELD) => Focus::BarField,
        Some("filter" | "palette-input") => Focus::Field,
        Some(k) if k.ends_with("-filter") || k.starts_with("g-") => Focus::Field,
        Some(k) if k == "stream" || CONVERSATION.iter().any(|p| k.starts_with(p)) => Focus::Conversation,
        Some(_) => Focus::Elsewhere,
    }
}

/// The bar's field.
pub const FIELD: &str = "find-field";

// ─── the bar's state ────────────────────────────────────────────────────────

/// What the status line of the bar's second row says about older history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paging {
    /// Pages are loading for the search: "searching older…" and Stop.
    Searching,
    /// The last page failed: "couldn't load older messages · Try again".
    Failed,
    Quiet,
}

/// What the hits were worked out from: the query and scope, and a number that moves when a message does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub query: String,
    pub scope: Scope,
    pub stamp: (usize, usize),
}

#[derive(Default)]
pub struct Find {
    pub open: bool,
    /// Kept when the bar closes, so reopening shows the old query already selected.
    pub query: TextEdit,
    pub scope: Scope,
    /// The index of the current match, from 0.
    pub current: usize,
    /// Stop was pressed: older history stays unloaded until the bar is closed and opened again.
    pub paging_stopped: bool,
    pub hits: Vec<FindHit>,
    pub seen: Option<Seen>,
    /// A message to bring into view on the next frame, once its turn is drawn (`jump`'s wait).
    pub jump: Option<String>,
    /// When to bring the current mark to the middle: once the jump's own scroll has built the message's row.
    pub settle: Option<f64>,
    /// The session the bar was opened on: it closes when another opens (Fleet keys the stream by session).
    pub at: (usize, usize),
}

impl Find {
    pub fn has_query(&self) -> bool {
        !self.query.text.trim().is_empty()
    }

    /// `count`: empty with no query, "No matches", else "3 of 12".
    pub fn count_label(&self) -> String {
        if !self.has_query() {
            return String::new();
        }
        match self.hits.len() {
            0 => "No matches".into(),
            n => format!("{} of {n}", self.current + 1),
        }
    }

    pub fn current_hit(&self) -> Option<&FindHit> {
        self.hits.get(self.current)
    }

    /// `olderFailed` and `searchingOlder`.
    pub fn older(&self, has_more: bool, load_failed: bool) -> Paging {
        match (self.open && !self.paging_stopped && has_more, load_failed) {
            (false, _) => Paging::Quiet,
            (true, true) => Paging::Failed,
            (true, false) => Paging::Searching,
        }
    }

    /// `open()`: paging starts again; the hits are worked out by the next sync.
    pub fn open(&mut self) {
        self.open = true;
        self.paging_stopped = false;
        self.seen = None;
    }

    /// `close()`. The query, the scope and the turns it unfolded stay as they were.
    pub fn close(&mut self) {
        self.open = false;
        self.hits.clear();
        self.seen = None;
        self.jump = None;
        self.settle = None;
    }

    /// `step`: the next or previous match, wrapping both ways. Nothing at no matches.
    pub fn step(&mut self, direction: i32) -> Option<&FindHit> {
        let total = self.hits.len() as i64;
        if total == 0 {
            return None;
        }
        self.current = (self.current as i64 + i64::from(direction) + total).rem_euclid(total) as usize;
        self.current_hit()
    }

    /// The hits changed: `current` stays unless it ran off the end.
    pub fn set_hits(&mut self, hits: Vec<FindHit>) {
        self.hits = hits;
        if self.current >= self.hits.len() {
            self.current = 0;
        }
    }
}
