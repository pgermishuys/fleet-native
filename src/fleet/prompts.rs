//! A prompt the person sent, shown at once and confirmed when the server says it has it
//! (`composables/use-send-prompt.ts`, `lib/merge-messages.ts`). No Rune and no I/O: the clock is passed in.
//!
//! The life of one: `add` (pending, with a `prompt-<uuid>` correlation id) → `confirm` from the send's
//! receipt or the `user.prompt.committed` event (whichever comes first; the second is a no-op) → gone when
//! the conversation holds the message (`reconcile`). Not confirmed in 15 s it becomes `NeedsRetry`.
//! A prompt the server refuses is `remove`d and its text goes back to the composer (`State::on_prompt_refused`).
use super::model::{Msg, PartData};
use crate::view::conversation::{Message, Part, Role};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};

/// `PROMPT_CONFIRMATION_TIMEOUT_MS`, in seconds of the app's clock.
pub const CONFIRMATION_TIMEOUT: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptStatus {
    Pending,
    Confirmed,
    NeedsRetry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SentPrompt {
    /// The id the message will carry: ours until the server's replaces it.
    pub id: String,
    pub correlation_id: String,
    pub event_id: Option<i64>,
    pub server_message_id: Option<String>,
    pub status: PromptStatus,
    pub body: String,
    pub created_at: i64,
    /// When it was sent, on the app's clock: the 15 s start here.
    pub sent_at: f64,
    /// The pictures that went with it, drawn under its words until the server's message replaces it (A9.7).
    pub images: Vec<SentImage>,
    /// The sessions its `@` names, as (token, session id): what Fleet turns into chips in the first bubble (A8.12).
    pub session_references: Vec<(String, String)>,
}

/// A picture sent with a prompt: base64, as the request carried it.
#[derive(Clone, Debug, PartialEq)]
pub struct SentImage {
    pub mime: String,
    pub filename: Option<String>,
    pub data: std::sync::Arc<str>,
}

impl SentImage {
    /// `data:{mime};base64,{data}`, what the bubble's picture is read from.
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime, self.data)
    }
}

/// What the conversation draws for a prompt the stream hasn't echoed yet: its words, then its pictures (A9.7).
pub fn optimistic_view(prompt: &SentPrompt) -> Message {
    let mut m = Message::user(format!("optimistic-{}", prompt.correlation_id), prompt.body.clone());
    m.needs_retry = prompt.status == PromptStatus::NeedsRetry;
    for (n, image) in prompt.images.iter().enumerate() {
        m.parts.push(Part::File {
            id: format!("{}-img-{n}", m.id),
            mime: image.mime.clone(),
            filename: image.filename.clone().or_else(|| Some("image.png".into())),
            url: image.data_url(),
        });
    }
    m
}

/// What a confirmation says (`ConfirmSentPromptOptions`).
#[derive(Clone, Debug, Default)]
pub struct Confirm {
    pub correlation_id: Option<String>,
    pub event_id: Option<i64>,
    pub server_message_id: Option<String>,
}

/// The sent prompts of every session, in the order they were sent.
#[derive(Default)]
pub struct Prompts {
    by_session: HashMap<String, Vec<SentPrompt>>,
    /// How many assistant messages with something to draw each session had when last looked at
    /// (`lastRenderableAssistantCount`), so only a new reply clears the prompts.
    assistants: HashMap<String, usize>,
    /// The user messages a session already had while no prompt of it was in flight (`baselineUserMessageIds`): the match by text
    /// must not take one of them for the delivery of a prompt sent since.
    baseline: HashMap<String, HashSet<String>>,
}

/// `crypto.randomUUID()` without a hyphen, from what std has: random enough to tell prompts apart.
pub(crate) fn uuid() -> String {
    let hash = |salt: u8| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u8(salt);
        h.finish()
    };
    format!("{:016x}{:016x}", hash(0), hash(1))
}

/// `tidyPrompt` (`lib/prompt-text.ts`): what a prompt is sent as. Blank lines before the first line and spaces after the last
/// are dropped; the first line's indent is not, so a prompt that starts with an indented code block keeps it.
pub fn tidy_prompt(text: &str) -> String {
    let mut rest = text;
    while let Some(n) = rest.find('\n') {
        if !rest[..n].trim_end_matches('\r').chars().all(|c| c == ' ' || c == '\t') {
            break;
        }
        rest = &rest[n + 1..];
    }
    rest.trim_end().to_string()
}

/// `draftAfterRefusal` (`lib/refused-draft.ts`): the draft after a refused send. What was sent comes back in front, a blank
/// line, then whatever was typed while it was pending; nothing typed since (or the same text again) leaves the refused text.
pub fn draft_after_refusal(refused: &str, current: &str) -> String {
    if current.trim().is_empty() || current == refused {
        return refused.to_string();
    }
    // Fleet never asks with nothing refused; here a caller might, and a blank line in front would be noise.
    if refused.trim().is_empty() {
        return current.to_string();
    }
    format!("{}\n\n{}", refused.trim_end(), current)
}

/// A correlation id for a prompt that has no text to show as pending (pictures only).
pub fn new_correlation_id() -> String {
    format!("prompt-{}", uuid())
}

impl Prompts {
    /// Shows a message as sent. `None` for text that is only space (nothing is sent).
    pub fn add(&mut self, session: &str, body: &str, created_at: i64, now: f64) -> Option<String> {
        self.add_with(session, body, created_at, now, Vec::new(), Vec::new())
    }

    /// The same, with the pictures and the session references the message went with.
    pub fn add_with(
        &mut self,
        session: &str,
        body: &str,
        created_at: i64,
        now: f64,
        images: Vec<SentImage>,
        session_references: Vec<(String, String)>,
    ) -> Option<String> {
        let body = tidy_prompt(body);
        if body.is_empty() {
            return None;
        }
        let correlation_id = format!("prompt-{}", uuid());
        self.by_session.entry(session.to_string()).or_default().push(SentPrompt {
            id: format!("user-{}", uuid()),
            correlation_id: correlation_id.clone(),
            event_id: None,
            server_message_id: None,
            status: PromptStatus::Pending,
            body: body.clone(),
            created_at,
            sent_at: now,
            images,
            session_references,
        });
        Some(correlation_id)
    }

    /// `showSentPrompt`: a prompt the server has already taken, under the id it stored it with, until the conversation holds
    /// it. A side question goes to a session the app only learns of from the answer.
    pub fn show_sent(&mut self, session: &str, id: &str, correlation_id: &str, body: &str, created_at: i64, now: f64) {
        let body = tidy_prompt(body);
        if body.is_empty() {
            return;
        }
        self.by_session.entry(session.to_string()).or_default().push(SentPrompt {
            id: id.to_string(),
            correlation_id: correlation_id.to_string(),
            event_id: None,
            server_message_id: Some(id.to_string()),
            status: PromptStatus::Confirmed,
            body: body.clone(),
            created_at,
            sent_at: now,
            images: Vec::new(),
            session_references: Vec::new(),
        });
    }

    pub fn of(&self, session: &str) -> &[SentPrompt] {
        self.by_session.get(session).map_or(&[], Vec::as_slice)
    }

    /// `hasPendingPrompts`.
    pub fn has_pending(&self, session: &str) -> bool {
        self.of(session).iter().any(|p| p.status == PromptStatus::Pending)
    }

    /// `confirmSentPrompt`: found by correlation id, else by the server's message id. Confirming twice
    /// changes nothing more; the server's ids are adopted.
    pub fn confirm(&mut self, session: &str, with: &Confirm) {
        let Some(prompts) = self.by_session.get_mut(session) else { return };
        let found = prompts.iter_mut().find(|p| {
            with.correlation_id.as_deref() == Some(p.correlation_id.as_str())
                || with.server_message_id.as_deref().is_some_and(|id| id == p.id)
        });
        let Some(prompt) = found else { return };
        if let Some(c) = &with.correlation_id {
            prompt.correlation_id = c.clone();
        }
        prompt.status = PromptStatus::Confirmed;
        if with.event_id.is_some() {
            prompt.event_id = with.event_id;
        }
        if let Some(id) = &with.server_message_id {
            prompt.server_message_id = Some(id.clone());
            prompt.id = id.clone();
        }
    }

    /// `removeSentPromptByCorrelationId`; the prompt, if it was there.
    pub fn remove(&mut self, session: &str, correlation_id: &str) -> Option<SentPrompt> {
        let prompts = self.by_session.get_mut(session)?;
        let at = prompts.iter().position(|p| p.correlation_id == correlation_id)?;
        let gone = prompts.remove(at);
        if prompts.is_empty() {
            self.by_session.remove(session);
        }
        Some(gone)
    }

    /// `clearSentPrompts`.
    pub fn clear(&mut self, session: &str) {
        self.by_session.remove(session);
    }

    /// Pending prompts older than 15 s become `NeedsRetry` (the timer of `schedulePromptConfirmationTimeout`,
    /// run on the app's clock). Whether anything changed.
    pub fn expire(&mut self, now: f64) -> bool {
        let mut changed = false;
        for p in self.by_session.values_mut().flatten() {
            if p.status == PromptStatus::Pending && now - p.sent_at >= CONFIRMATION_TIMEOUT {
                p.status = PromptStatus::NeedsRetry;
                changed = true;
            }
        }
        changed
    }

    /// The conversation changed (`ActivityStream.vue`'s watch of `sessionMessages`): drop the prompts it
    /// now holds, and all of them once a new reply has something to draw and every message of the person's has text.
    pub fn on_messages(&mut self, session: &str, messages: &[Msg]) {
        self.reconcile(session, messages);
        let renderable = messages.iter().filter(|m| has_renderable_assistant_content(m)).count();
        let seen = self.assistants.entry(session.to_string()).or_insert(0);
        if self.by_session.get(session).is_none_or(Vec::is_empty) {
            *seen = renderable;
            return;
        }
        if renderable > *seen {
            *seen = renderable;
            if messages.iter().filter(|m| m.role == Role::User).all(has_text) {
                self.by_session.remove(session);
            }
        }
    }

    /// `reconcileSentPrompts`: by id, else by body, counting each delivered message once.
    pub fn reconcile(&mut self, session: &str, messages: &[Msg]) {
        let Some(prompts) = self.by_session.get_mut(session) else {
            let seen = messages.iter().filter(|m| m.role == Role::User).map(|m| m.id.clone()).collect();
            self.baseline.insert(session.to_string(), seen);
            return;
        };
        let baseline = self.baseline.get(session);
        // The id match looks at every message; the text match only at those that weren't there before the send.
        let all_users = || messages.iter().filter(|m| m.role == Role::User);
        let users = || all_users().filter(|m| baseline.is_none_or(|b| !b.contains(&m.id)));
        let before = prompts.len();
        prompts.retain(|p| !all_users().any(|m| m.id == p.id));
        if prompts.len() == before {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for m in users() {
                let text = body_of(m);
                if !text.is_empty() {
                    *counts.entry(text).or_insert(0) += 1;
                }
            }
            // A message with no text yet (its `message.updated` came before its part) is not delivered.
            let mut unmatched = users().filter(|m| has_text(m)).count() as i64;
            prompts.retain(|p| {
                if let Some(n) = counts.get_mut(&p.body).filter(|n| **n > 0) {
                    *n -= 1;
                    unmatched -= 1;
                    return false;
                }
                if unmatched > 0 {
                    unmatched -= 1;
                    return false;
                }
                true
            });
        }
        if prompts.is_empty() {
            self.by_session.remove(session);
        }
    }
}

fn has_text(m: &Msg) -> bool {
    m.parts.iter().any(|p| matches!(p, PartData::Text { text, .. } if !text.trim().is_empty()))
}

/// The message's text parts joined as Fleet joins them, trimmed.
fn body_of(m: &Msg) -> String {
    let texts: Vec<&str> =
        m.parts.iter().filter_map(|p| if let PartData::Text { text, .. } = p { Some(text.as_str()) } else { None }).collect();
    tidy_prompt(&texts.join("\n\n"))
}

/// `hasRenderableAssistantContent`.
fn has_renderable_assistant_content(m: &Msg) -> bool {
    m.role == Role::Assistant
        && m.parts.iter().any(|p| match p {
            PartData::Tool { .. } => true,
            PartData::File { filename, url, .. } => filename.as_deref().is_some_and(|f| !f.trim().is_empty()) || !url.is_empty(),
            PartData::Text { text, .. } => !text.trim().is_empty(),
            PartData::Reasoning { text, summary, .. } => !summary.as_deref().unwrap_or(text).trim().is_empty(),
            PartData::Compaction { .. } => false,
        })
}

/// `mergeMessagesByTimestamp`: the server's order is the truth, so delivered messages come first and
/// the sent ones after, each in their own order, whatever their timestamps say.
pub fn merge_by_timestamp<T>(mut delivered: Vec<T>, optimistic: Vec<T>) -> Vec<T> {
    delivered.extend(optimistic);
    delivered
}
