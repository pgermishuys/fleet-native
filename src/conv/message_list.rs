//! The conversation's messages as the stream lists them, before anything is drawn: who wrote each, the text it shows,
//! which messages are dropped, how a run from one sender clusters, and where a prompt that was sent but not echoed yet
//! goes. Ports `lib/activity-messages.ts`, `lib/side-conversation.ts::messagesAfter`, `lib/merge-messages.ts` and the
//! pure half of `composables/use-activity-messages.ts` (`ActivityMessage`). No Rune in here.
//!
//! Not ported on purpose: the WeakMap caches that keep a message's view the same object from one update to the next
//! (the native stream memoises rows instead), and `message-diagnostics.ts`, a localStorage log nobody sees.
use super::background::{Finished, background_status, delegation_of, parse_background_notice};
use super::gist::Thinking;
use super::tools::{Category, ToolRowItem, ToolStatus, get_tool};
use crate::composer::choice::ModelOption;
use crate::composer::choice::model_display_name;
use crate::fleet::delegations::Delegation;
use crate::fleet::model::{ModMark, Msg, PartData, SlashCommand};
use crate::fleet::prompts::{PromptStatus, SentPrompt, merge_by_timestamp};
use crate::fleet::reducer::compare_message_ids;
use crate::view::conversation::Role;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Where a message sits in a run from one sender (`ClusterPosition`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterPosition {
    Single,
    First,
    Middle,
    Last,
}

/// The change a file got, as the session's diff counts it (`FileStats` entries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStat {
    pub additions: u32,
    pub deletions: u32,
    /// What the session diff says was done to the file, when it says (`status`): the Turns tab calls a file new only on `Added`.
    pub status: Option<FileStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

impl FileStat {
    pub fn new(additions: u32, deletions: u32) -> Self {
        FileStat { additions, deletions, status: None }
    }
}

/// A page a call showed in the conversation (`ToolCardPage`).
#[derive(Clone, Debug, PartialEq)]
pub struct ToolPage {
    /// Fleet's copy of the page, as an address on the server (`pageAddress`).
    pub path: String,
    pub id: String,
    /// The file the agent showed (`input.path`): a page shown again from it replaces the earlier show.
    pub source: String,
}

/// `toolPage`: the page the call showed, from `metadata.page {id, entry}`; none without both.
pub fn tool_page(state: &Value) -> Option<ToolPage> {
    let page = metadata(state, "page")?;
    let (id, entry) = (text_at(page, "id")?, text_at(page, "entry")?);
    let source = state.pointer("/input/path").and_then(Value::as_str).unwrap_or_default().to_string();
    let path =
        crate::fleet::web_page::page_address(&crate::fleet::web_page::ShownPage { page_id: id.clone(), entry, ..Default::default() });
    Some(ToolPage { path, id, source })
}

/// `withoutRedrawnPages`: a page shown again from the same file in one turn (a turn starts at each message of the user's) is
/// drawn once, at its last show. The calls stay; earlier turns keep theirs. Returns the calls whose page is no longer drawn.
pub fn without_redrawn_pages(messages: &mut [ListMessage]) -> HashSet<String> {
    let mut redrawn = HashSet::new();
    let mut last: HashMap<String, String> = HashMap::new();
    for message in messages.iter_mut() {
        if message.role == Role::User {
            last.clear();
        }
        for tool in &message.tools {
            let Some(page) = tool.page.as_ref().filter(|p| !p.source.is_empty()) else { continue };
            if let Some(earlier) = last.insert(page.source.clone(), tool.id.clone()) {
                redrawn.insert(earlier);
            }
        }
    }
    for tool in messages.iter_mut().flat_map(|m| m.tools.iter_mut()) {
        if redrawn.contains(&tool.id) {
            tool.page = None;
        }
    }
    redrawn
}

/// A call as the list holds it (`ToolCardItem`): only what folding and grouping read. The drawn card comes from the
/// message's `Part::Tool` with the same id.
#[derive(Clone, Debug, PartialEq)]
pub struct ListTool {
    pub id: String,
    /// The tool's name as the harness sent it.
    pub kind: String,
    pub status: ToolStatus,
    /// The file a write call touched (`withFileEdit`).
    pub file_path: Option<String>,
    /// The call sent only the file's new contents, so its own lines are all additions (`diffFromContent`).
    pub diff_from_content: bool,
    /// What the file's change really was: `None` is unknown, which draws no counts (`diffStat: null`).
    pub diff_stat: Option<FileStat>,
    /// The call started a sub-agent that has a session by now.
    pub delegation: bool,
    /// Fleet kept a screenshot of the call.
    pub screenshot: bool,
    /// The page the call showed under itself (`toolPage`).
    pub page: Option<ToolPage>,
    /// The call opened a canvas.
    pub canvas: bool,
}

impl ListTool {
    pub fn new(id: impl Into<String>, kind: impl Into<String>, status: ToolStatus) -> Self {
        ListTool {
            id: id.into(),
            kind: kind.into(),
            status,
            file_path: None,
            diff_from_content: false,
            diff_stat: None,
            delegation: false,
            screenshot: false,
            page: None,
            canvas: false,
        }
    }
}

impl ToolRowItem for ListTool {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> &str {
        &self.kind
    }
    fn status(&self) -> ToolStatus {
        self.status
    }
    fn delegated(&self) -> bool {
        self.delegation
    }
}

/// The failure that ended a turn (`TurnError`).
#[derive(Clone, Debug, PartialEq)]
pub struct TurnError {
    pub name: String,
    pub message: String,
    pub is_retryable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListReasoning {
    pub text: String,
    pub summary: Option<String>,
}

impl ListReasoning {
    /// It has something to say: text or a summary that isn't blank.
    pub fn has_text(&self) -> bool {
        !self.text.trim().is_empty() || self.summary.as_deref().is_some_and(|s| !s.trim().is_empty())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListImage {
    pub url: String,
    pub filename: String,
}

/// What folding a turn says of it (`FoldedTurn`, `TurnSummary`).
#[derive(Clone, Debug, PartialEq)]
pub struct FoldedTurn {
    /// The id of the turn's first message: what the open state is kept by.
    pub id: String,
    pub summary: TurnSummary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnSummary {
    pub duration_ms: Option<i64>,
    pub reads: usize,
    pub edits: usize,
    pub commands: usize,
    pub other: usize,
    /// Calls left visible because they failed or didn't finish.
    pub failed: usize,
    /// The fold line draws its tick only for `Done`.
    pub outcome: Outcome,
}

/// A message as the conversation draws it (`ActivityMessage`).
#[derive(Clone, Debug, PartialEq)]
pub struct ListMessage {
    pub id: String,
    pub author: String,
    pub model_name: Option<String>,
    pub sender_key: String,
    pub role: Role,
    pub created_at: Option<i64>,
    /// When the harness finished writing it; a turn's "Worked for" ends at the last one.
    pub completed_at: Option<i64>,
    pub body: String,
    pub images: Vec<ListImage>,
    pub tools: Vec<ListTool>,
    /// Ids of the question calls, kept apart from the tool rows.
    pub question_parts: Vec<String>,
    pub reasoning: Vec<ListReasoning>,
    pub optimistic: Option<PromptStatus>,
    pub cluster: ClusterPosition,
    pub show_identity: bool,
    pub turn_error: Option<TurnError>,
    /// This is the notice that work the agent moved into the background finished (`conv::background`).
    pub background: bool,
    pub steered: bool,
    pub shell: bool,
    pub command: Option<SlashCommand>,
    pub mod_mark: Option<ModMark>,
    pub compaction: bool,
    /// Set by folding: the turn's fold row draws above this message.
    pub fold: Option<FoldedTurn>,
    /// Set by folding: nothing is left of the message but the fold row.
    pub fold_only: bool,
}

impl ListMessage {
    pub fn new(id: impl Into<String>, role: Role) -> Self {
        ListMessage {
            id: id.into(),
            author: String::new(),
            model_name: None,
            sender_key: match role {
                Role::User => "user".into(),
                Role::Assistant => String::new(),
                Role::Shell => "shell".into(),
            },
            role,
            created_at: None,
            completed_at: None,
            body: String::new(),
            images: Vec::new(),
            tools: Vec::new(),
            question_parts: Vec::new(),
            reasoning: Vec::new(),
            optimistic: None,
            cluster: ClusterPosition::Single,
            show_identity: true,
            turn_error: None,
            background: false,
            steered: false,
            shell: false,
            command: None,
            mod_mark: None,
            compaction: false,
            fold: None,
            fold_only: false,
        }
    }
}

// ─── activity-messages.ts ───────────────────────────────────────────────────

/// The text a message shows: its text parts joined with a blank line; reasoning and images aren't in it, a file that
/// isn't an image is a link or its label (`renderMessageBody`).
pub fn render_message_body(parts: &[PartData]) -> String {
    let shown: Vec<String> = parts
        .iter()
        .filter_map(|part| match part {
            PartData::Text { text, .. } => Some(text.clone()),
            PartData::File { mime, filename, url, .. } if !mime.starts_with("image/") => {
                let label = filename.as_deref().map(str::trim).filter(|f| !f.is_empty()).unwrap_or("Attached file");
                Some(if url.is_empty() { label.to_string() } else { format!("[{label}]({url})") })
            }
            _ => None,
        })
        .collect();
    shown.join("\n\n")
}

/// `_` and `-` read as spaces, runs of spaces as one.
fn normalize_identity(value: &str) -> String {
    let spaced: String = value.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "code_reviewer" is "Code Reviewer"; a name that already has capitals is kept; nothing is "Assistant".
pub fn format_agent_display_name(author: &str) -> String {
    let name = normalize_identity(author);
    if name.is_empty() {
        return "Assistant".into();
    }
    if name.to_lowercase() != name {
        return name;
    }
    // A letter after the start, a space or "(" gets a capital.
    let mut out = String::with_capacity(name.len());
    let mut after_break = true;
    for c in name.chars() {
        if after_break {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        after_break = c.is_whitespace() || c == '(';
    }
    out
}

pub fn display_author(role: Role, agent: Option<&str>) -> String {
    if role == Role::User { "You".into() } else { format_agent_display_name(agent.unwrap_or("Assistant")) }
}

pub fn sender_key(role: Role, author: Option<&str>) -> String {
    if role == Role::User { "user".into() } else { normalize_identity(author.unwrap_or("Assistant")).to_lowercase() }
}

pub fn cluster_position(grouped_with_previous: bool, grouped_with_next: bool) -> ClusterPosition {
    match (grouped_with_previous, grouped_with_next) {
        (true, true) => ClusterPosition::Middle,
        (true, false) => ClusterPosition::Last,
        (false, true) => ClusterPosition::First,
        (false, false) => ClusterPosition::Single,
    }
}

/// The same role and the same sender key (`isSameSender`).
pub fn is_same_sender(previous: Option<&ListMessage>, next: Option<&ListMessage>) -> bool {
    matches!((previous, next), (Some(p), Some(n)) if p.role == n.role && p.sender_key == n.sender_key)
}

/// A message is kept when it is the user's (even empty) or has something to show (`hasVisibleMessageContent`). Thinking
/// alone is a message, so the fold line follows it while the model is still thinking, unless thinking is hidden.
pub fn has_visible_content(message: &ListMessage, thinking: Thinking) -> bool {
    !message.body.trim().is_empty()
        || (thinking != Thinking::Hidden && message.reasoning.iter().any(ListReasoning::has_text))
        || !message.images.is_empty()
        || !message.tools.is_empty()
        || !message.question_parts.is_empty()
        || message.shell
        || message.compaction
        || message.background
        // A turn can fail before it produces anything; the failure is the content.
        || message.turn_error.is_some()
}

// ─── side-conversation.ts ───────────────────────────────────────────────────

/// A side conversation's own messages: those after the boundary, the newest one it copied from its session. A boundary
/// that isn't among them is an id that sorts after what was copied, or one older than what's loaded: the messages that
/// sort after it. All of them with no boundary. The flag says whether anything was cut (`hasMore` is false unless
/// nothing was).
pub fn messages_after<'a>(messages: &'a [Msg], boundary: Option<&str>) -> (&'a [Msg], bool) {
    let Some(boundary) = boundary.filter(|b| !b.is_empty()) else { return (messages, false) };
    if let Some(at) = messages.iter().position(|m| m.id == boundary) {
        return (&messages[at + 1..], true);
    }
    let first_after = messages.iter().position(|m| compare_message_ids(&m.id, boundary).is_gt());
    // Ids sort with the messages, so those after the boundary are the tail; a hole in the middle would not be one.
    match first_after {
        Some(0) => (messages, false),
        Some(at) => (&messages[at..], true),
        None => (&messages[messages.len()..], true),
    }
}

// ─── use-activity-messages.ts ───────────────────────────────────────────────

/// What the views read besides the message itself.
pub struct ListContext<'a> {
    pub delegations: &'a [Delegation],
    /// How the background work in the conversation ended (`finishedBackgroundWork`).
    pub finished: &'a Finished,
    pub models: &'a [ModelOption],
    pub thinking: Thinking,
}

fn metadata<'a>(state: &'a Value, key: &str) -> Option<&'a Value> {
    state.pointer(&format!("/metadata/{key}")).filter(|v| v.is_object())
}

fn text_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn list_tool(id: &str, kind: &str, call_id: &str, state: &Value, context: &ListContext) -> ListTool {
    let status =
        background_status(state, context.finished).unwrap_or_else(|| ToolStatus::from_word(state.get("status").and_then(Value::as_str)));
    let mut tool = ListTool::new(id, kind, status);
    // A sub-agent call points at its session once there is one (`withDelegation`).
    tool.delegation = get_tool(kind).category == Category::Subagent && delegation_of(context.delegations, call_id, id).is_some();
    tool.screenshot = metadata(state, "screenshot").is_some_and(|s| {
        text_at(s, "sessionId").is_some()
            && text_at(s, "id").is_some()
            && s.get("width").and_then(Value::as_f64).is_some_and(|w| w > 0.0)
            && s.get("height").and_then(Value::as_f64).is_some_and(|h| h > 0.0)
    });
    tool.page = tool_page(state);
    tool.canvas = state.pointer("/metadata/canvasId").and_then(Value::as_str).is_some_and(|c| !c.is_empty());
    if get_tool(kind).file_write.is_some()
        && let Some(edit) = super::diff::file_edits_of(kind, state).into_iter().next()
    {
        tool.file_path = Some(edit.path);
        // Only when the harness sent no diff of its own (`withFileEdit`).
        tool.diff_from_content = edit.whole_file && super::diff::card_diff_lines(state).is_empty();
    }
    tool
}

/// A delivered message as the list holds it (`toActivityMessage`). A command the user ran is its own block.
pub fn list_message(m: &Msg, context: &ListContext) -> ListMessage {
    let mut out = ListMessage::new(m.id.clone(), m.role);
    out.created_at = m.created_at;
    out.completed_at = m.completed_at;
    if m.role == Role::Shell {
        out.author = "You".into();
        out.shell = true;
        return out;
    }
    out.author = display_author(m.role, m.agent.as_deref());
    out.sender_key = sender_key(m.role, m.agent.as_deref());
    out.model_name = Some(model_display_name(m.model_id.as_deref(), context.models)).filter(|n| !n.is_empty());
    out.body = render_message_body(&m.parts);
    out.background = parse_background_notice(&out.body).is_some();
    out.steered = m.role == Role::User && m.steered;
    out.command = if m.role == Role::User { m.command.clone() } else { None };
    out.mod_mark = if m.role == Role::User { m.mod_mark.clone() } else { None };
    out.turn_error = m.turn_error.as_ref().map(|e| TurnError {
        name: text_at(e, "name").unwrap_or_default(),
        message: text_at(e, "message").unwrap_or_default(),
        is_retryable: e.get("isRetryable") == Some(&Value::Bool(true)),
    });
    for part in &m.parts {
        match part {
            PartData::File { mime, filename, url, .. } if mime.starts_with("image/") => out.images.push(ListImage {
                url: url.clone(),
                filename: filename.as_deref().map(str::trim).filter(|f| !f.is_empty()).unwrap_or("image").to_string(),
            }),
            PartData::Tool { tool, id, call_id, state } => {
                if get_tool(tool).category == Category::Question {
                    out.question_parts.push(id.clone());
                } else {
                    out.tools.push(list_tool(id, tool, call_id, state, context));
                }
            }
            PartData::Reasoning { text, summary, .. } => out.reasoning.push(ListReasoning { text: text.clone(), summary: summary.clone() }),
            PartData::Compaction { .. } => out.compaction = true,
            _ => {}
        }
    }
    out
}

/// A prompt that was sent and not echoed yet (`optimisticMessages`).
pub fn optimistic_message(prompt: &SentPrompt) -> ListMessage {
    let mut out = ListMessage::new(format!("optimistic-{}", prompt.id), Role::User);
    out.author = "You".into();
    out.created_at = Some(prompt.created_at);
    out.body = prompt.body.clone();
    out.optimistic = Some(prompt.status);
    out
}

/// The thinking block the model is writing right now: the last part of the newest message, while the turn runs
/// (`liveReasoningPartId`).
pub fn live_reasoning_part(messages: &[Msg], working: bool) -> Option<&str> {
    let last = messages.last().filter(|m| working && m.role == Role::Assistant)?;
    match last.parts.last()? {
        PartData::Reasoning { id, .. } => Some(id),
        _ => None,
    }
}

/// What the stream delivered: the messages in its order, minus the ones with nothing to show. The compaction summaries
/// are already folded into their dividers (`fleet::view::messages`), so `messages` is what is left of them.
pub fn delivered_messages(messages: &[Msg], context: &ListContext) -> Vec<ListMessage> {
    let hidden = super::compaction::fold_compaction_summaries(messages).hidden;
    messages
        .iter()
        .filter(|m| !hidden.contains(&m.id))
        .map(|m| list_message(m, context))
        .filter(|m| m.role == Role::User || has_visible_content(m, context.thinking))
        .collect()
}

/// Every message to draw: the delivered ones, then the prompts the stream hasn't echoed (no sort by time), each with its
/// place in its run of one sender.
pub fn build_list(messages: &[Msg], sent: &[SentPrompt], context: &ListContext) -> Vec<ListMessage> {
    let delivered = delivered_messages(messages, context);
    let ids: HashSet<&str> = delivered.iter().map(|m| m.id.as_str()).collect();
    let pending: Vec<ListMessage> =
        sent.iter().map(optimistic_message).filter(|m| !ids.contains(m.id.trim_start_matches("optimistic-"))).collect();
    cluster(merge_by_timestamp(delivered, pending))
}

/// Where each message sits in its run of one sender, and whether it shows who wrote it (only the first of a run does).
pub fn cluster(mut messages: Vec<ListMessage>) -> Vec<ListMessage> {
    let links: Vec<(bool, bool)> = (0..messages.len())
        .map(|i| {
            let previous = i.checked_sub(1).and_then(|p| messages.get(p));
            (is_same_sender(previous, messages.get(i)), is_same_sender(messages.get(i), messages.get(i + 1)))
        })
        .collect();
    for (message, (with_previous, with_next)) in messages.iter_mut().zip(links) {
        message.show_identity = !with_previous;
        message.cluster = cluster_position(with_previous, with_next);
    }
    messages
}
