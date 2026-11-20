//! A session's side conversation (`/btw`) as plain Rust: what Fleet's `use-side-conversation.ts` keeps per session, and
//! `lib/side-conversation.ts`'s `messagesAfter`. No Rune and no I/O; the clock is passed in. The requests are in
//! `side_http`, the panel is drawn by `view::side`.
use super::reducer::compare_message_ids;
use serde_json::Value;

/// How long Undo is offered after a discard (`SIDE_DISCARD_UNDO_MS`). The server keeps the fork a little longer.
pub const SIDE_DISCARD_UNDO_MS: u64 = 8_000;

/// Which composer draft: the session's, or its side conversation's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draft {
    Main,
    Side,
}

/// `SideConversationResponse`: a hidden session of its own, shown after `boundary_message_id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub session_id: String,
    pub instance_id: String,
    pub title: String,
    pub boundary_message_id: Option<String>,
    pub created_at: String,
    pub minimized: bool,
    pub seen_answer_id: Option<String>,
}

impl Side {
    pub fn from_json(v: &Value) -> Option<Side> {
        let text = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
        Some(Side {
            session_id: text("sessionId").filter(|id| !id.is_empty())?,
            instance_id: text("instanceId").unwrap_or_default(),
            title: text("title").unwrap_or_default(),
            boundary_message_id: text("boundaryMessageId"),
            created_at: text("createdAt").unwrap_or_default(),
            minimized: v.get("minimized").and_then(Value::as_bool).unwrap_or(false),
            seen_answer_id: text("seenAnswerId"),
        })
    }
}

/// What a side conversation's stream reports: whether it is working, and its newest answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub working: bool,
    pub latest_answer: Option<String>,
    pub latest_answer_id: Option<String>,
}

/// One session's state (`SideConversationState`).
#[derive(Clone, Debug)]
pub struct SideState {
    pub side: Option<Side>,
    pub loaded: bool,
    /// The load is on its way.
    pub loading: bool,
    /// A question is on its way; before the first one's answer the side conversation is being forked.
    pub asking: bool,
    /// The question that started it, shown while the fork is made.
    pub starting: Option<String>,
    pub error: Option<String>,
    /// Its conversation has reported since the app started (it stays running while folded).
    pub reported: bool,
    pub working: bool,
    pub latest_answer: Option<String>,
    pub latest_answer_id: Option<String>,
    /// An answer that landed on this run while it was folded: its dot pulses. One found after a restart doesn't.
    pub pulse_answer_id: Option<String>,
    /// The drafts that are not in the composer: the side's while it is folded, the session's while it is open.
    pub drafts: [String; 2],
    /// A side conversation discarded moments ago, which Undo can still bring back, and until when (the app's clock).
    pub discarded: Option<Side>,
    pub undo_ms: u64,
    pub undo_until: Option<f64>,
}

impl Default for SideState {
    fn default() -> SideState {
        SideState {
            side: None,
            loaded: false,
            loading: false,
            asking: false,
            starting: None,
            error: None,
            reported: false,
            working: false,
            latest_answer: None,
            latest_answer_id: None,
            pulse_answer_id: None,
            drafts: [String::new(), String::new()],
            discarded: None,
            undo_ms: SIDE_DISCARD_UNDO_MS,
            undo_until: None,
        }
    }
}

impl SideState {
    pub fn minimized(&self) -> bool {
        self.side.as_ref().is_some_and(|s| s.minimized)
    }

    /// The panel is open: a side conversation that isn't minimized, or the first question making one (`isOpen`).
    pub fn is_open(&self) -> bool {
        (self.side.is_some() && !self.minimized()) || self.starting.is_some()
    }

    /// It is folded and its newest finished answer is newer than the newest one seen with it open (`unread`).
    pub fn unread(&self) -> bool {
        self.minimized()
            && self.reported
            && !self.working
            && self.latest_answer_id.is_some()
            && self.latest_answer_id != self.side.as_ref().and_then(|s| s.seen_answer_id.clone())
    }

    /// The unread dot pulses for an answer that landed on this run, not for one found after a restart (`pulse`).
    pub fn pulse(&self) -> bool {
        self.unread() && self.pulse_answer_id == self.latest_answer_id
    }

    pub fn draft(&self, which: Draft) -> &str {
        &self.drafts[which as usize]
    }

    pub fn set_draft(&mut self, which: Draft, text: &str) {
        self.drafts[which as usize] = text.to_string();
    }

    /// The side session whose draft is kept: the live one, or the one just discarded.
    pub fn draft_side_id(&self) -> Option<&str> {
        self.side.as_ref().or(self.discarded.as_ref()).map(|s| s.session_id.as_str())
    }

    /// Records `answer` as seen here. Whether it changed, so the caller tells the server (`markSeen`).
    pub fn mark_seen(&mut self, answer: &str) -> bool {
        match &mut self.side {
            Some(side) if side.seen_answer_id.as_deref() != Some(answer) => {
                side.seen_answer_id = Some(answer.to_string());
                true
            }
            _ => false,
        }
    }

    /// Its conversation's report (`reportProgress`). An answer that finishes while it is open is seen; one while it is
    /// folded is news. Returns the answer to mark seen on the server, if any.
    pub fn report_progress(&mut self, p: Progress) -> Option<String> {
        let first = !self.reported;
        let was_working = self.working;
        let previous = self.latest_answer_id.clone();
        self.reported = true;
        self.working = p.working;
        self.latest_answer = p.latest_answer;
        self.latest_answer_id = p.latest_answer_id;
        let id = self.latest_answer_id.clone().filter(|_| !p.working)?;
        if !self.minimized() {
            return self.mark_seen(&id).then_some(id);
        }
        if !first && (was_working || Some(&id) != previous.as_ref()) {
            self.pulse_answer_id = Some(id);
        }
        None
    }

    /// Shows Undo for a discarded side conversation for `ms`, from `now` (`offerUndo`).
    pub fn offer_undo(&mut self, discarded: Side, ms: u64, now: f64) {
        self.discarded = Some(discarded);
        self.undo_ms = ms;
        self.undo_until = Some(now + ms as f64 / 1000.0);
    }

    /// Ends the offer without a timer: Undo used, refused, or replaced. The discarded one, if there was one.
    pub fn end_undo(&mut self) -> Option<Side> {
        self.undo_until = None;
        self.discarded.take()
    }

    /// The window has passed: the offer ends, and the discarded one's draft goes with it.
    pub fn expire(&mut self, now: f64) -> Option<Side> {
        if self.undo_until.is_some_and(|t| now >= t) {
            self.drafts[Draft::Side as usize].clear();
            return self.end_undo();
        }
        None
    }

    /// Seconds of Undo left, rounded up (the toast counts down in whole seconds).
    pub fn undo_seconds_left(&self, now: f64) -> Option<u64> {
        self.undo_until.map(|t| (t - now).max(0.0).ceil() as u64)
    }
}

/// The messages of a side conversation: those after `boundary`, the newest one it copied from its session
/// (`messagesAfter`). A boundary that is not among them is an id that sorts after what was copied, or one older than
/// what is loaded: the messages that sort after it. All of them when there is no boundary, or when none or every one does.
pub fn messages_after<'a, T>(messages: &'a [T], boundary: Option<&str>, id: impl Fn(&T) -> &str) -> Vec<&'a T> {
    let Some(boundary) = boundary.filter(|b| !b.is_empty()) else { return messages.iter().collect() };
    if let Some(at) = messages.iter().position(|m| id(m) == boundary) {
        return messages[at + 1..].iter().collect();
    }
    let after: Vec<&T> = messages.iter().filter(|m| compare_message_ids(id(m), boundary).is_gt()).collect();
    if after.len() == messages.len() { messages.iter().collect() } else { after }
}

impl Side {
    /// This side with the fields `v` names replaced (`{...discarded, ...answer}`).
    pub fn with(&self, v: &Value) -> Side {
        let text = |key: &str, was: &str| v.get(key).and_then(Value::as_str).map_or_else(|| was.to_string(), str::to_string);
        let option = |key: &str, was: &Option<String>| match v.get(key) {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Null) => None,
            _ => was.clone(),
        };
        Side {
            session_id: text("sessionId", &self.session_id),
            instance_id: text("instanceId", &self.instance_id),
            title: text("title", &self.title),
            boundary_message_id: option("boundaryMessageId", &self.boundary_message_id),
            created_at: text("createdAt", &self.created_at),
            minimized: v.get("minimized").and_then(Value::as_bool).unwrap_or(self.minimized),
            seen_answer_id: option("seenAnswerId", &self.seen_answer_id),
        }
    }
}

/// `side-<32 hex>`: the id a side question goes with, which the server echoes.
pub fn new_correlation_id() -> String {
    format!("side-{}", super::prompts::uuid())
}
