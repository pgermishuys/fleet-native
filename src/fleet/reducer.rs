//! Fleet's reducer, ported as a plain function: `client/src/lib/domain-event-reducer.ts` and
//! `event-state.ts`. A snapshot makes a `StreamState`; each event changes it. No Rune, no I/O.
//!
//! The event type itself is `events.rs`. Done: messages and their parts, text deltas, turn start/end/failure, session idle,
//! delegations and the status derived from them (`delegations.rs`), running work (`work.rs`), context
//! usage (`context.rs`) and the `command` and `mod` marks on a prompt. A committed prompt is applied as a
//! message update; confirming the optimistic prompt it answers is `State::on_stream_events`'s, because the registry
//! (`prompts.rs`) isn't part of the stream.
use super::context::to_context_usage;
use super::delegations::{self, Delegation};
use super::model::{Explicit, ModKind, ModMark, Msg, PartData, SlashCommand, StreamState, Tokens};
use super::work::{self, apply_item, to_item};
use crate::view::conversation::Role;
use serde_json::{Map, Value};

pub use super::events::{Event, Kind, Lifecycle};

/// `createSessionStreamState`: the snapshot's messages become `message.created` events on an
/// empty state.
pub fn create_state(snapshot: &Value) -> StreamState {
    let explicit_status = to_explicit(snapshot.get("activityStatus").and_then(Value::as_str).unwrap_or(""));
    let delegations: Vec<Delegation> =
        snapshot.get("delegations").and_then(Value::as_array).into_iter().flatten().filter_map(Delegation::from_snapshot).collect();
    let mut state = StreamState {
        messages: Vec::new(),
        session_status: delegations::derive_snapshot_status(explicit_status, &delegations),
        delegations,
        running_work: work::to_items(snapshot.get("runningWork")),
        context: snapshot.get("context").and_then(to_context_usage),
        explicit_status,
        last_event_id: snapshot.get("lastEventId").or_else(|| snapshot.get("lastSequenceNumber")).and_then(Value::as_i64),
        is_partial: snapshot.get("isPartial").and_then(Value::as_bool).unwrap_or(false),
    };
    for message in snapshot.get("messages").and_then(Value::as_array).into_iter().flatten() {
        if let Some(info) = message.get("info") {
            let life = Lifecycle { info: info.clone(), parts: message.get("parts").and_then(Value::as_array).cloned() };
            apply(&mut state, &Event::MessageCreated(life));
        }
    }
    state
}

/// `applyDomainEvent`, changing the state in place.
pub fn apply(state: &mut StreamState, event: &Event) {
    match event {
        Event::MessageCreated(life) | Event::MessageUpdated(life) | Event::PromptCommitted { life, .. } => {
            apply_lifecycle(&mut state.messages, life)
        }
        Event::PartUpdated(part) => apply_part_update(&mut state.messages, part),
        Event::TextDelta { session_id, message_id, part_id, delta, offset } => {
            apply_text_delta(&mut state.messages, message_id, part_id, session_id, delta, *offset)
        }
        Event::TurnStarted => set_explicit(state, Explicit::Busy),
        Event::TurnEnded | Event::SessionIdled => set_explicit(state, Explicit::Idle),
        Event::Delegation { kind, event } => {
            if *kind == Kind::DelegationCreated {
                delegations::apply_created(&mut state.delegations, event);
            } else {
                delegations::upsert(&mut state.delegations, event);
            }
            rederive(state);
        }
        // Only a sub-agent's status is read here; the session's own comes from its turn events.
        Event::ActivityStatus { session_id, activity_status } => {
            if delegations::set_child_activity(&mut state.delegations, session_id, activity_status) {
                rederive(state);
            }
        }
        Event::Work { item, .. } => {
            if let Some(item) = to_item(item) {
                apply_item(&mut state.running_work, item);
            }
        }
        Event::Context(usage) => {
            if let Some(usage) = to_context_usage(usage) {
                state.context = Some(usage);
            }
        }
        Event::Cursor(id) => state.last_event_id = Some(*id),
        // Read, and handed to whoever owns them (`State::subscribe`); nothing for the stream to change.
        Event::Other(..) | Event::Unknown { .. } => {}
        Event::TurnFailed { session_id, message_id, error } => {
            // The turn is over and the harness has used up its retries: stop looking busy now,
            // not when the idle that follows arrives.
            set_explicit(state, Explicit::Idle);
            apply_turn_failure(&mut state.messages, session_id, message_id.as_deref(), error);
        }
    }
}

fn to_explicit(activity: &str) -> Explicit {
    match activity {
        "busy" | "working" => Explicit::Busy,
        "retry" => Explicit::Retry,
        _ => Explicit::Idle,
    }
}

fn rederive(state: &mut StreamState) {
    state.session_status = delegations::derive_status(state.explicit_status, &state.delegations);
}

fn set_explicit(state: &mut StreamState, explicit: Explicit) {
    state.explicit_status = explicit;
    rederive(state);
}

/// `withActivityStatus`: a kept stream that is opened again takes the session list's current status,
/// which the `sessions` topic kept current while it was away.
pub fn with_activity_status(state: &mut StreamState, activity: &str) {
    set_explicit(state, to_explicit(activity));
}

fn to_role(role: &Value) -> Role {
    match role.as_str() {
        Some("user") => Role::User,
        Some("shell") => Role::Shell,
        _ => Role::Assistant,
    }
}

// ─── message order ──────────────────────────────────────────────────────────

/// Orders two message ids. They sort as text, except ids that share a stem and end in a counter
/// (`msg_<stem>_6`, `msg_<stem>_17`): a fork names its copies that way.
pub fn compare_message_ids(a: &str, b: &str) -> std::cmp::Ordering {
    fn counted(id: &str) -> Option<(&str, u64)> {
        let (stem, n) = id.rsplit_once('_')?;
        Some((stem, n.parse().ok().filter(|_| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))?))
    }
    match (counted(a), counted(b)) {
        (Some((s1, n1)), Some((s2, n2))) if s1 == s2 => n1.cmp(&n2),
        _ => a.cmp(b),
    }
}

/// OpenCode 2 names a schedule's prompt `msg_sch_<schedule>_<run>`: the part after `msg_` isn't a
/// time, so such a message goes by when it was created.
fn compare_messages(a: &Msg, b: &Msg) -> std::cmp::Ordering {
    if (a.id.starts_with("msg_sch_") || b.id.starts_with("msg_sch_"))
        && let (Some(x), Some(y)) = (a.created_at, b.created_at)
        && x != y
    {
        return x.cmp(&y);
    }
    compare_message_ids(&a.id, &b.id)
}

/// Insert by message id, binary search. Ids without the `msg_` prefix go to the end.
fn insert_sorted(messages: &mut Vec<Msg>, new: Msg) {
    if !new.id.starts_with("msg_") {
        messages.push(new);
        return;
    }
    let (mut left, mut right) = (0, messages.len());
    while left < right {
        let mid = (left + right) / 2;
        if !messages[mid].id.starts_with("msg_") {
            right = mid;
        } else if compare_messages(&messages[mid], &new).is_lt() {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    messages.insert(left, new);
}

// ─── messages ───────────────────────────────────────────────────────────────

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn ensure_message(messages: &mut Vec<Msg>, info: &Value) {
    let id = str_of(info, "id").unwrap_or_default();
    if messages.iter().any(|m| m.id == id) {
        return;
    }
    let mut msg = Msg::new(id, str_of(info, "sessionID").unwrap_or_default(), to_role(info.get("role").unwrap_or(&Value::Null)));
    msg.created_at = info.pointer("/time/created").and_then(Value::as_i64);
    msg.agent = str_of(info, "agent");
    msg.model_id = str_of(info, "modelID");
    msg.parent_id = str_of(info, "parentID");
    msg.steered = info.get("steered") == Some(&Value::Bool(true));
    msg.compaction_summary = info.get("compactionSummary") == Some(&Value::Bool(true));
    if msg.role == Role::User {
        msg.command = read_command(info);
        msg.mod_mark = read_mod(info);
    }
    insert_sorted(messages, msg);
}

/// `readCommand`: the slash command on a message info. Empty arguments are dropped.
fn read_command(info: &Value) -> Option<SlashCommand> {
    let command = info.get("command").filter(|c| c.is_object())?;
    let name = command.get("name")?.as_str().filter(|n| !n.is_empty())?.to_string();
    let arguments = command.get("arguments").and_then(Value::as_str).filter(|a| !a.is_empty()).map(str::to_string);
    Some(SlashCommand { name, arguments })
}

/// `readMod`: which mod sent or rewrote a prompt.
fn read_mod(info: &Value) -> Option<ModMark> {
    let mark = info.get("mod").filter(|m| m.is_object())?;
    let kind = match mark.get("kind")?.as_str()? {
        "sent" => ModKind::Sent,
        "rewritten" => ModKind::Rewritten,
        _ => return None,
    };
    Some(ModMark { kind, name: mark.get("name")?.as_str().filter(|n| !n.is_empty())?.to_string() })
}

fn is_turn_error(v: &Value) -> bool {
    v.get("message").and_then(Value::as_str).is_some_and(|m| !m.is_empty())
}

fn tokens_of(v: &Value) -> Option<Tokens> {
    v.is_object().then(|| Tokens {
        input: v.get("input").and_then(Value::as_i64).unwrap_or(0),
        output: v.get("output").and_then(Value::as_i64).unwrap_or(0),
        reasoning: v.get("reasoning").and_then(Value::as_i64).unwrap_or(0),
    })
}

/// `applyMessageLifecycle`: make the message if it's new, then merge `info` and any parts into it.
fn apply_lifecycle(messages: &mut Vec<Msg>, life: &Lifecycle) {
    ensure_message(messages, &life.info);
    merge_update(messages, &life.info, life.parts.as_deref());
}

/// `mergeMessageUpdate`.
fn merge_update(messages: &mut [Msg], info: &Value, parts: Option<&[Value]>) {
    let id = str_of(info, "id").unwrap_or_default();
    let Some(msg) = messages.iter_mut().find(|m| m.id == id) else { return };
    if let Some(created) = info.pointer("/time/created").and_then(Value::as_i64) {
        msg.created_at = Some(created);
    }
    if let Some(done) = info.pointer("/time/completed").and_then(Value::as_i64)
        && msg.completed_at.is_none()
    {
        msg.completed_at = Some(done);
    }
    if let Some(model) = str_of(info, "modelID") {
        msg.model_id = Some(model);
    }
    // The server's totals are authoritative for a finished message; the larger value wins in case
    // a step-finish part was missed.
    if let Some(incoming) = info.get("tokens").and_then(tokens_of) {
        let have = msg.tokens.unwrap_or_default();
        msg.tokens = Some(Tokens {
            input: have.input.max(incoming.input),
            output: have.output.max(incoming.output),
            reasoning: have.reasoning.max(incoming.reasoning),
        });
    }
    let cost = info.get("cost").and_then(Value::as_f64).unwrap_or(0.0);
    if cost > msg.cost.unwrap_or(0.0) {
        msg.cost = Some(cost);
    }
    if let Some(error) = info.get("turnError").filter(|e| is_turn_error(e)) {
        msg.turn_error = Some(error.clone());
    }
    if let Some(finish) = str_of(info, "finish") {
        msg.finish = Some(finish);
    }
    if let Some(role) = info.get("role") {
        msg.role = to_role(role);
    }
    if info.get("steered") == Some(&Value::Bool(true)) {
        msg.steered = true;
    }
    if info.get("compactionSummary") == Some(&Value::Bool(true)) {
        msg.compaction_summary = true;
    }
    // A prompt's marks can arrive on a later update; an agent message never has them.
    if msg.role == Role::User {
        msg.command = read_command(info).or(msg.command.take());
        msg.mod_mark = read_mod(info).or(msg.mod_mark.take());
    }
    if let Some(parts) = parts {
        let snapshot: Vec<PartData> = parts.iter().enumerate().filter_map(|(i, p)| committed_part(&id, p, i)).collect();
        msg.parts = merge_snapshot_parts(std::mem::take(&mut msg.parts), snapshot);
    }
}

fn committed_part(message_id: &str, part: &Value, index: usize) -> Option<PartData> {
    let id = |kind: &str| str_of(part, "id").unwrap_or_else(|| format!("{message_id}-{kind}-{index}"));
    let s = |k: &str| str_of(part, k);
    Some(match part.get("type")?.as_str()? {
        "compaction" => to_compaction(part, &format!("{message_id}-compaction-{index}")),
        "text" => PartData::Text { id: id("text"), text: s("text").unwrap_or_default() },
        "file" => PartData::File {
            id: id("file"),
            mime: s("mime").unwrap_or_default(),
            filename: s("filename"),
            url: s("url").unwrap_or_default(),
        },
        "reasoning" => PartData::Reasoning { id: id("reasoning"), text: s("text").unwrap_or_default(), summary: s("summary") },
        "tool" => PartData::Tool {
            id: id("tool"),
            tool: s("tool").unwrap_or_default(),
            call_id: s("callID").unwrap_or_default(),
            state: part.get("state").cloned().unwrap_or(Value::Null),
        },
        _ => return None,
    })
}

/// `mergeCommittedSnapshotParts`: the snapshot's parts win by id, a longer streamed text is kept,
/// and parts only known live are kept. The result is grouped by kind, as Fleet does it.
fn merge_snapshot_parts(existing: Vec<PartData>, snapshot: Vec<PartData>) -> Vec<PartData> {
    // In Fleet's order: compactions, text, files, reasoning, tools.
    let kinds: [fn(&PartData) -> bool; 5] = [
        |p| matches!(p, PartData::Compaction { .. }),
        |p| matches!(p, PartData::Text { .. }),
        |p| matches!(p, PartData::File { .. }),
        |p| matches!(p, PartData::Reasoning { .. }),
        |p| matches!(p, PartData::Tool { .. }),
    ];
    let mut out = Vec::new();
    for (n, is) in kinds.into_iter().enumerate() {
        let have: Vec<&PartData> = existing.iter().filter(|p| is(p)).collect();
        for part in snapshot.iter().filter(|p| is(p)) {
            // A text the stream has gone further on keeps the stream's version.
            let longer = have.iter().find(|h| h.id() == part.id()).filter(|h| match (h, part) {
                (PartData::Text { text: a, .. }, PartData::Text { text: b, .. }) => a.len() > b.len(),
                _ => false,
            });
            out.push((*longer.unwrap_or(&part)).clone());
        }
        // Parts only known live stay (not text: the snapshot's list of those is whole).
        if n != 1 {
            out.extend(have.iter().filter(|h| !snapshot.iter().any(|p| p.id() == h.id())).map(|h| (*h).clone()));
        }
    }
    out
}

fn to_compaction(part: &Value, fallback_id: &str) -> PartData {
    let number = |k: &str| part.get(k).and_then(Value::as_f64).filter(|n| n.is_finite() && *n >= 0.0).map(|n| n as u32);
    let text = |k: &str| part.get(k).and_then(Value::as_str).filter(|t| !t.trim().is_empty()).map(str::to_string);
    PartData::Compaction {
        id: str_of(part, "id").unwrap_or_else(|| fallback_id.to_string()),
        trigger: text("trigger"),
        tokens_before: number("tokensBefore"),
        tokens_after: number("tokensAfter"),
        summary: text("summary"),
    }
}

// ─── parts ──────────────────────────────────────────────────────────────────

/// `applyPartUpdate`. A part can arrive before its message, which then starts out as the agent's.
fn apply_part_update(messages: &mut Vec<Msg>, part: &Value) {
    let message_id = str_of(part, "messageID").unwrap_or_default();
    if !messages.iter().any(|m| m.id == message_id) {
        insert_sorted(messages, Msg::new(message_id.clone(), str_of(part, "sessionID").unwrap_or_default(), Role::Assistant));
    }
    let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) else { return };
    let id = str_of(part, "id").unwrap_or_default();
    let s = |k: &str| str_of(part, k);
    let put = |parts: &mut Vec<PartData>, at: Option<usize>, new: PartData| match at {
        Some(i) => parts[i] = new,
        None => parts.push(new),
    };
    match part.get("type").and_then(Value::as_str) {
        // A text or reasoning part with this id is replaced, whichever kind it was.
        Some("text") => {
            let at = msg.parts.iter().position(|p| matches!(p, PartData::Text { .. } | PartData::Reasoning { .. }) && p.id() == id);
            put(&mut msg.parts, at, PartData::Text { id, text: s("text").unwrap_or_default() });
        }
        Some("reasoning") => {
            let at = msg.parts.iter().position(|p| matches!(p, PartData::Text { .. } | PartData::Reasoning { .. }) && p.id() == id);
            put(&mut msg.parts, at, PartData::Reasoning { id, text: s("text").unwrap_or_default(), summary: s("summary") });
        }
        Some("tool") => {
            let call_id = s("callID").unwrap_or_default();
            // The call id says it's the same row even when a harness names the part twice.
            let at = msg.parts.iter().position(|p| {
                matches!(p, PartData::Tool { call_id: c, .. } if !call_id.is_empty() && *c == call_id)
                    || (matches!(p, PartData::Tool { .. }) && p.id() == id)
            });
            let incoming = part.get("state").cloned().unwrap_or(Value::Null);
            let (part_id, state) = match at.map(|i| &msg.parts[i]) {
                Some(PartData::Tool { id: kept, state: old, .. }) => {
                    let mut merged: Map<String, Value> = old.as_object().cloned().unwrap_or_default();
                    merged.extend(incoming.as_object().cloned().unwrap_or_default());
                    (kept.clone(), Value::Object(merged))
                }
                _ => (id, incoming),
            };
            put(&mut msg.parts, at, PartData::Tool { id: part_id, tool: s("tool").unwrap_or_default(), call_id, state });
        }
        Some("file") => {
            let at = msg.parts.iter().position(|p| p.id() == id);
            put(
                &mut msg.parts,
                at,
                PartData::File { id, mime: s("mime").unwrap_or_default(), filename: s("filename"), url: s("url").unwrap_or_default() },
            );
        }
        Some("compaction") => {
            let new = to_compaction(part, &id);
            let at = msg.parts.iter().position(|p| p.id() == new.id());
            put(&mut msg.parts, at, new);
        }
        Some("step-finish") => {
            msg.cost = Some(msg.cost.unwrap_or(0.0) + part.get("cost").and_then(Value::as_f64).unwrap_or(0.0));
            let (have, add) = (msg.tokens.unwrap_or_default(), tokens_of(part.get("tokens").unwrap_or(&Value::Null)).unwrap_or_default());
            msg.tokens =
                Some(Tokens { input: have.input + add.input, output: have.output + add.output, reasoning: have.reasoning + add.reasoning });
        }
        _ => {}
    }
}

/// `applyTextDelta`. With its `offset` (where it starts in the part's text) only the text the part
/// doesn't have yet is added: a snapshot taken mid-reply already has some of it.
fn apply_text_delta(messages: &mut Vec<Msg>, message_id: &str, part_id: &str, session_id: &str, delta: &str, offset: Option<i64>) {
    let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) else {
        let mut msg = Msg::new(message_id, session_id, Role::Assistant);
        msg.parts.push(PartData::Text { id: part_id.to_string(), text: delta.to_string() });
        insert_sorted(messages, msg);
        return;
    };
    let at = msg.parts.iter().position(|p| matches!(p, PartData::Text { .. } | PartData::Reasoning { .. }) && p.id() == part_id);
    match at.map(|i| &mut msg.parts[i]) {
        Some(PartData::Text { text, .. } | PartData::Reasoning { text, .. }) => {
            // Offsets count UTF-16 units on the wire (the server is .NET); the text here is UTF-8.
            let have = text.encode_utf16().count() as i64;
            let known = offset.map_or(0, |o| (have - o).max(0)) as usize;
            let units: Vec<u16> = delta.encode_utf16().collect();
            if known >= units.len() {
                return;
            }
            text.push_str(&String::from_utf16_lossy(&units[known..]));
        }
        _ => msg.parts.push(PartData::Text { id: part_id.to_string(), text: delta.to_string() }),
    }
}

/// `applyTurnFailure`: the failure goes on the message it belongs to. When the turn died before
/// producing one, it gets a message of its own, so a failed turn never shows nothing.
fn apply_turn_failure(messages: &mut Vec<Msg>, session_id: &str, message_id: Option<&str>, error: &Value) {
    let named = message_id.and_then(|id| messages.iter().position(|m| m.id == id));
    let candidate = named.or_else(|| messages.iter().rposition(|m| m.role == Role::Assistant));
    // A reply before the latest prompt answered an earlier turn; pinning the failure there would
    // hide it above the prompt it doesn't belong to.
    let last_user = messages.iter().rposition(|m| m.role == Role::User);
    let target = candidate.filter(|c| last_user.is_none_or(|u| *c > u));
    match target {
        Some(i) => messages[i].turn_error = Some(error.clone()),
        None => {
            let mut msg = Msg::new(format!("turn-failed-{session_id}-{}", messages.len()), session_id, Role::Assistant);
            msg.turn_error = Some(error.clone());
            messages.push(msg);
        }
    }
}
