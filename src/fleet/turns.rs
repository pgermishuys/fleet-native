//! Turns: the rounds of a session, derived from its messages (weave-fleet's `client/src/lib/turns.ts`, and the words
//! `TurnsCanvas.vue` writes around them). Plain logic, no Rune.
//!
//! A turn here is a *round*: one prompt, plus everything the agent did until the next prompt. Not the harness's
//! `turn.started`/`turn.ended`, and not the conversation's fold turns (`conv::fold::turn_ids_of` skips steered prompts;
//! every user message starts a round here). What a turn reports is the edits it made, read off its own tool calls.
use super::delegations::{Delegation, DelegationStatus};
use super::model::{Msg, PartData};
use super::work::parse_iso_ms;
use crate::composer::choice::ModelOption;
use crate::composer::format::{format_count, format_money, format_tokens};
use crate::conv::diff::{DiffLine, FileEdit, file_edits_of};
use crate::conv::fold::{FileStats, relative_path};
use crate::conv::message_list::FileStatus;
use crate::conv::references::format_slash_command;
use crate::conv::tools::{get_tool, is_shell_row, parse_tool_input};
use crate::conv::working::format_duration_exact;
use crate::view::conversation::Role;
use std::collections::{HashMap, HashSet, VecDeque};

/// A file a turn wrote, with the lines that turn wrote to it.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnFile {
    pub path: String,
    /// The file's own name, for the strong half of the row.
    pub name: String,
    /// Its directory, dimmed after the name.
    pub dir: String,
    pub additions: u32,
    pub deletions: u32,
    /// The turn created the file (as far as its tool calls, and the session diff for a whole-file write, can tell).
    pub created: bool,
    /// The lines this turn wrote, in call order; empty when the harness sent no diff to derive them from.
    pub diff: Vec<DiffLine>,
    /// Written whole by a call that sent only the new contents, so its counts come from the session diff.
    pub whole_file: bool,
}

/// A sub-agent started during the turn.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnDelegation {
    pub id: String,
    pub title: String,
    pub status: DelegationStatus,
    pub child_session_id: Option<String>,
}

/// A shell command the turn ran, for the "Also ran" line.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnCommand {
    pub label: String,
    /// `Some(true)` finished, `Some(false)` failed, `None` still running or unknown.
    pub ok: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionTurn {
    /// The id of the round's first message: also what "Show this turn in the chat" scrolls to.
    pub id: String,
    /// 1 for the oldest *loaded* round. Older rounds shift it.
    pub number: usize,
    /// First line of the prompt, for the row.
    pub prompt: String,
    /// The whole prompt, for the row's tooltip.
    pub prompt_full: String,
    /// False for a round whose prompt is older than the loaded history.
    pub has_prompt: bool,
    /// The last model that answered in the round.
    pub model_id: Option<String>,
    /// The round ran on more than one model; the row says so.
    pub model_changed: bool,
    pub agent: Option<String>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost: f64,
    pub files: Vec<TurnFile>,
    pub additions: u32,
    pub deletions: u32,
    /// Every tool call in the round, including reads and failed calls.
    pub tool_count: usize,
    /// Calls that wrote a file.
    pub edit_count: usize,
    pub commands: Vec<TurnCommand>,
    pub delegations: Vec<TurnDelegation>,
    /// The round ran tools but wrote nothing: it still gets a row, and says so.
    pub read_only: bool,
}

/// What `derive_turns` reads besides the messages.
#[derive(Clone, Copy, Debug, Default)]
pub struct TurnOptions<'a> {
    /// The session diff's counts per file, for files a call wrote whole: a call that sent only the new contents can't
    /// say what changed, so its file takes the diff's counts (0 while they're not known, which draws none).
    pub file_stats: Option<&'a FileStats>,
    /// The session's folder, which the paths in `file_stats` are relative to.
    pub directory: Option<&'a str>,
}

// ─── The per-call cache ─────────────────────────────────────────────────────────────────────────────────────────────

/// Fleet keeps at most this many calls' edits.
pub const MAX_CACHED_CALLS: usize = 4000;

/// Parsing a call's diff is the expensive part and a completed call never changes, so the result is kept per call
/// (`partId|callId|tool`). Bounded: a long session is thousands of calls. The oldest entry goes first.
#[derive(Debug, Default)]
pub struct EditCache {
    map: HashMap<String, Vec<FileEdit>>,
    order: VecDeque<String>,
}

impl EditCache {
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// `fileEditsOf` for a tool part, from the cache when the call was read before. Only a completed write call is kept:
    /// one that failed or hasn't finished wrote nothing yet and may still change.
    pub fn edits_of(&mut self, part_id: &str, call_id: &str, tool: &str, state: &serde_json::Value) -> Vec<FileEdit> {
        let known = get_tool(tool);
        if known.file_write.is_none() || !is_completed(state) {
            return Vec::new();
        }
        let key = format!("{part_id}|{call_id}|{}", known.name);
        if let Some(hit) = self.map.get(&key) {
            return hit.clone();
        }
        let edits = file_edits_of(tool, state);
        if self.map.len() >= MAX_CACHED_CALLS
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.map.insert(key, edits.clone());
        edits
    }
}

fn status_word(state: &serde_json::Value) -> Option<String> {
    state.get("status").and_then(|s| s.as_str()).map(str::to_lowercase)
}

fn is_completed(state: &serde_json::Value) -> bool {
    status_word(state).as_deref() == Some("completed")
}

// ─── Rounds ─────────────────────────────────────────────────────────────────────────────────────────────────────────

/// Split messages (oldest first) into rounds: a user message and everything until the next one. The first message
/// starts a round even when it isn't a prompt (its prompt is older than the loaded history).
fn to_rounds(messages: &[Msg]) -> Vec<(Vec<&Msg>, bool)> {
    let mut rounds: Vec<(Vec<&Msg>, bool)> = Vec::new();
    for message in messages {
        match rounds.last_mut() {
            Some(round) if message.role != Role::User => round.0.push(message),
            _ => rounds.push((vec![message], message.role == Role::User)),
        }
    }
    rounds
}

fn prompt_of(message: &Msg) -> String {
    // A slash command reads as you sent it, not as the prompt the harness expanded it into.
    if let Some(command) = &message.command {
        return format_slash_command(&command.name, command.arguments.as_deref());
    }
    let texts: Vec<&str> = message
        .parts
        .iter()
        .filter_map(|p| match p {
            PartData::Text { text, .. } if !text.trim().is_empty() => Some(text.as_str()),
            _ => None,
        })
        .collect();
    texts.join("\n").trim().to_string()
}

/// The first non-blank line, trimmed, and cut to `max - 1` characters plus "…" when longer than `max`.
fn first_line(text: &str, max: usize) -> String {
    let line = text.split('\n').find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() > max { format!("{}…", line.chars().take(max - 1).collect::<String>()) } else { line.to_string() }
}

/// Newest first, matching the sidebar (`deriveTurns`).
pub fn derive_turns(messages: &[Msg], delegations: &[Delegation], options: TurnOptions) -> Vec<SessionTurn> {
    derive_turns_cached(messages, delegations, options, &mut EditCache::default())
}

/// `derive_turns` with the caller's cache, so a long session parses each call once across derivations.
pub fn derive_turns_cached(messages: &[Msg], delegations: &[Delegation], options: TurnOptions, cache: &mut EditCache) -> Vec<SessionTurn> {
    let rounds = to_rounds(messages);
    let mut turns: Vec<SessionTurn> = Vec::with_capacity(rounds.len());

    // Files an earlier turn wrote: a later whole-file write to one of them rewrites it, it doesn't create it.
    let mut written: HashSet<String> = HashSet::new();
    for (index, (round, has_prompt)) in rounds.iter().enumerate() {
        let turn = to_turn(round, *has_prompt, index + 1, options, &written, cache);
        written.extend(turn.files.iter().map(|f| f.path.clone()));
        turns.push(turn);
    }

    attach_delegations(&mut turns, &rounds, delegations);
    turns.reverse();
    turns
}

fn to_turn(
    round: &[&Msg],
    has_prompt: bool,
    number: usize,
    options: TurnOptions,
    written: &HashSet<String>,
    cache: &mut EditCache,
) -> SessionTurn {
    let first = round[0];
    let prompt_text = if has_prompt { prompt_of(first) } else { String::new() };

    let mut models: Vec<&str> = Vec::new();
    let (mut agent, mut cost, mut tokens_input, mut tokens_output) = (None::<String>, 0.0, 0, 0);
    let mut ended_at: Option<i64> = None;
    let mut files: Vec<TurnFile> = Vec::new();
    let mut commands: Vec<TurnCommand> = Vec::new();
    let (mut tool_count, mut edit_count) = (0, 0);

    for message in round {
        if let Some(finished) = message.completed_at.or(message.created_at)
            && ended_at.is_none_or(|e| finished > e)
        {
            ended_at = Some(finished);
        }
        if message.role != Role::Assistant {
            continue;
        }
        if let Some(model) = message.model_id.as_deref().filter(|m| !m.is_empty())
            && models.last() != Some(&model)
        {
            models.push(model);
        }
        if agent.is_none() {
            agent = message.agent.clone().filter(|a| !a.is_empty());
        }
        cost += message.cost.unwrap_or(0.0);
        tokens_input += message.tokens.map_or(0, |t| t.input);
        tokens_output += message.tokens.map_or(0, |t| t.output);

        for part in &message.parts {
            let PartData::Tool { id, tool, call_id, state } = part else { continue };
            tool_count += 1;

            if is_shell_row(tool) && commands.len() < 4 {
                let input = parse_tool_input(state.get("input").unwrap_or(&serde_json::Value::Null));
                // `stringField(input, "command", "description")`: the first of the two that has text.
                if let Some(label) = input.command.or(input.description) {
                    let ok = match status_word(state).as_deref() {
                        Some("completed") => Some(true),
                        Some("error") => Some(false),
                        _ => None,
                    };
                    commands.push(TurnCommand { label: first_line(&label, 48), ok });
                }
            }

            let edits = cache.edits_of(id, call_id, tool, state);
            if edits.is_empty() {
                continue;
            }
            edit_count += 1;
            for edit in edits {
                merge_file_edit(&mut files, edit);
            }
        }
    }

    for file in &mut files {
        if !file.whole_file {
            continue;
        }
        let known = options.file_stats.and_then(|stats| stats.get(&relative_path(&file.path, options.directory)));
        // A Write is a create or a rewrite; only the session diff says which. Unknown isn't "new".
        file.additions = known.map_or(0, |k| k.additions);
        file.deletions = known.map_or(0, |k| k.deletions);
        file.created = known.is_some_and(|k| k.status == Some(FileStatus::Added)) && !written.contains(&file.path);
    }
    let additions = files.iter().map(|f| f.additions).sum();
    let deletions = files.iter().map(|f| f.deletions).sum();
    let started_at = first.created_at;
    let duration_ms = match (started_at, ended_at) {
        (Some(start), Some(end)) if end > start => Some(end - start),
        _ => None,
    };

    SessionTurn {
        id: first.id.clone(),
        number,
        prompt: if has_prompt { first_line(&prompt_text, 160) } else { String::new() },
        prompt_full: prompt_text,
        has_prompt,
        model_id: models.last().map(|m| m.to_string()),
        model_changed: models.iter().collect::<HashSet<_>>().len() > 1,
        agent,
        started_at,
        ended_at,
        duration_ms,
        tokens_input,
        tokens_output,
        cost,
        read_only: files.is_empty(),
        files,
        additions,
        deletions,
        tool_count,
        edit_count,
        commands,
        delegations: Vec::new(),
    }
}

fn merge_file_edit(files: &mut Vec<TurnFile>, edit: FileEdit) {
    if let Some(existing) = files.iter_mut().find(|f| f.path == edit.path) {
        existing.additions += edit.additions;
        existing.deletions += edit.deletions;
        // A file this turn created and then edited again is still new this turn.
        existing.created |= edit.created;
        existing.diff.extend(edit.diff);
        existing.whole_file |= edit.whole_file;
        return;
    }
    let (dir, name) = match edit.path.rfind('/') {
        Some(slash) => (edit.path[..slash].to_string(), edit.path[slash + 1..].to_string()),
        None => (String::new(), edit.path.clone()),
    };
    files.push(TurnFile {
        path: edit.path,
        name,
        dir,
        additions: edit.additions,
        deletions: edit.deletions,
        created: edit.created,
        diff: edit.diff,
        whole_file: edit.whole_file,
    });
}

/// A sub-agent belongs to the round that started it: by the tool call it came from, or failing that by when it was
/// created; with neither it belongs nowhere.
fn attach_delegations(turns: &mut [SessionTurn], rounds: &[(Vec<&Msg>, bool)], delegations: &[Delegation]) {
    if delegations.is_empty() {
        return;
    }
    let mut round_by_call: HashMap<&str, usize> = HashMap::new();
    for (index, (round, _)) in rounds.iter().enumerate() {
        for message in round {
            for part in &message.parts {
                if let PartData::Tool { call_id, .. } = part
                    && !call_id.is_empty()
                {
                    round_by_call.insert(call_id, index);
                }
            }
        }
    }
    for delegation in delegations {
        let by_call = delegation.parent_tool_call_id.as_deref().and_then(|id| round_by_call.get(id).copied());
        let index = by_call.or_else(|| {
            let at = delegation.created_at.as_deref().and_then(parse_iso_ms)?;
            turns.iter().rposition(|t| t.started_at.is_some_and(|s| s <= at))
        });
        let Some(index) = index else { continue };
        turns[index].delegations.push(TurnDelegation {
            id: delegation.delegation_id.clone(),
            title: delegation.title.clone(),
            status: delegation.status,
            child_session_id: delegation.child_session_id.clone(),
        });
    }
}

// ─── Totals and display ─────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnTotals {
    pub turns: usize,
    pub additions: u32,
    pub deletions: u32,
}

/// Over every turn, not just the ones the filter shows (`turnTotals`).
pub fn turn_totals(turns: &[SessionTurn]) -> TurnTotals {
    TurnTotals {
        turns: turns.len(),
        additions: turns.iter().map(|t| t.additions).sum(),
        deletions: turns.iter().map(|t| t.deletions).sum(),
    }
}

/// The friendly name for a model id, from the session's catalog (`modelDisplayName`): "Unknown model" for none; ids
/// reach us both bare ("claude-opus-5") and qualified ("anthropic/claude-opus-5").
pub fn model_display_name(model_id: Option<&str>, models: &[ModelOption]) -> String {
    let Some(id) = model_id.filter(|m| !m.is_empty()) else { return "Unknown model".into() };
    if let Some(exact) = models.iter().find(|m| m.id == id) {
        return exact.name.clone();
    }
    let bare = id.rsplit('/').next().unwrap_or(id);
    match models.iter().find(|m| m.id == bare || m.id.ends_with(&format!("/{bare}"))) {
        Some(loose) => loose.name.clone(),
        None => bare.to_string(),
    }
}

// ─── The words ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// The row's prompt line: the prompt's first line, else what stands in for it.
pub fn prompt_line(turn: &SessionTurn) -> &str {
    if !turn.prompt.is_empty() {
        &turn.prompt
    } else if turn.has_prompt {
        "(no text in this prompt)"
    } else {
        "Before the loaded history"
    }
}

/// How long the turn took, at least a second: "1 s", "3 min 20 s"; none when the duration is unknown.
pub fn took(turn: &SessionTurn) -> Option<String> {
    turn.duration_ms.map(|ms| format_duration_exact(ms.max(1000)))
}

pub fn file_summary(turn: &SessionTurn) -> String {
    if turn.read_only {
        "read only".into()
    } else if turn.files.len() == 1 {
        "1 file".into()
    } else {
        format!("{} files", turn.files.len())
    }
}

/// The tail of the row's sub-line: "· 3 min 20 s · 2 files".
pub fn facts(turn: &SessionTurn) -> String {
    [took(turn), Some(file_summary(turn))].into_iter().flatten().map(|p| format!("· {p}")).collect::<Vec<_>>().join(" ")
}

/// "3 turns", "1 turn" (the summary bar).
pub fn turns_text(count: usize) -> String {
    format!("{count} turn{}", if count == 1 { "" } else { "s" })
}

/// "+1,234" when there are additions, else none.
pub fn additions_text(additions: u32) -> Option<String> {
    (additions > 0).then(|| format!("+{}", format_count(Some(additions as f64))))
}

/// "−12" (U+2212) when there are deletions, else none.
pub fn deletions_text(deletions: u32) -> Option<String> {
    (deletions > 0).then(|| format!("\u{2212}{}", format_count(Some(deletions as f64))))
}

/// The open turn's meta line as (label, value) pairs, in order: Model, Agent, Took, Tokens, Est. cost.
pub fn meta_items(turn: &SessionTurn, models: &[ModelOption]) -> Vec<(&'static str, String)> {
    let mut items = vec![("Model", model_display_name(turn.model_id.as_deref(), models))];
    if let Some(agent) = &turn.agent {
        items.push(("Agent", agent.clone()));
    }
    if let Some(took) = took(turn) {
        items.push(("Took", took));
    }
    if turn.tokens_input > 0 || turn.tokens_output > 0 {
        let (i, o) = (format_tokens(Some(turn.tokens_input as f64)), format_tokens(Some(turn.tokens_output as f64)));
        items.push(("Tokens", format!("{i} in \u{b7} {o} out")));
    }
    if turn.cost > 0.0 {
        items.push(("Est. cost", format_money(Some(turn.cost))));
    }
    items
}

/// What the open turn says when it wrote nothing. Fleet's own text has a space before the final period (Vue keeps the
/// newline before `</template>`), so this does too.
pub fn nothing_written(turn: &SessionTurn) -> String {
    if turn.tool_count == 0 {
        return "Nothing was written this turn.".into();
    }
    let calls = format!("{} tool call{}", turn.tool_count, if turn.tool_count == 1 { "" } else { "s" });
    format!("Nothing was written this turn \u{2014} {calls}, all reads .")
}

/// The word a command chip ends with; none while it runs.
pub fn command_outcome(command: &TurnCommand) -> Option<&'static str> {
    command.ok.map(|ok| if ok { "passed" } else { "failed" })
}
