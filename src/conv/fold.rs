//! A finished turn folds into one line, "Worked for 34s · read 4 files · edited 2 · ran 2 commands", with the answer
//! after it. Ports `lib/turn-fold.ts`. No Rune in here: the fold row and the list draw from what this returns.
//!
//! A turn is one message of yours and every agent message until the next one. It folds unless it is the turn still
//! running. Folding hides the turn's thinking and its completed tool rows, nothing else: the agent's text is never
//! hidden, and failed, stopped and background rows, sub-agent rows, pages and screenshots stay (they are part of the
//! answer, or a failure would be folded away).
use super::message_list::{FileStat, FoldedTurn, ListMessage, ListTool, Outcome, TurnSummary};
use super::tools::{ToolStatus, get_tool, is_read_call, is_shell_row, is_write_call};
use crate::view::conversation::Role;
use std::collections::{HashMap, HashSet};

/// Fleet's tools that hand the user something to open (a page, a running app, a browser tab, a walkthrough).
const OPENS_SOMETHING: [&str; 4] = ["fleet_page_show", "fleet_app_start", "fleet_browser_open", "fleet_walkthrough_show"];

/// The call produced something the user opens: its Show button has to stay on screen, so it stays out of the fold and is
/// not counted as "other".
fn is_page_show(tool: &ListTool) -> bool {
    tool.page || tool.canvas || OPENS_SOMETHING.contains(&get_tool(&tool.kind).name.as_str())
}

/// Claude Code's ExitPlanMode hands over the plan: it reads nothing and changes no file.
fn is_plan_exit(tool: &ListTool) -> bool {
    get_tool(&tool.kind).name == "exitplanmode"
}

/// A call the fold keeps on screen.
fn stays_visible(tool: &ListTool) -> bool {
    tool.delegation || tool.screenshot || is_page_show(tool) || tool.status != ToolStatus::Completed
}

// ─── paths ──────────────────────────────────────────────────────────────────

fn is_windows_path(path: &str) -> bool {
    let b = path.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || path.starts_with("\\\\")
}

/// The path as the folder knows it: relative, with "/" between names, when it is inside the folder.
pub fn relative_path(path: &str, directory: Option<&str>) -> String {
    let Some(directory) = directory.filter(|d| !d.is_empty()) else { return path.to_string() };
    if !is_windows_path(directory) {
        let base = directory.trim_end_matches('/');
        return path.strip_prefix(&format!("{base}/")).unwrap_or(path).to_string();
    }
    // Windows: either slash, and a drive letter's case doesn't tell folders apart.
    let base = directory.replace('\\', "/").trim_end_matches('/').to_string();
    let slashed = path.replace('\\', "/");
    let prefix = format!("{base}/");
    match slashed.get(..prefix.len()) {
        Some(head) if head.to_lowercase() == prefix.to_lowercase() => slashed[prefix.len()..].to_string(),
        _ => path.to_string(),
    }
}

/// The path is absolute and lies outside the folder: the session's diff can't list it.
fn is_outside(path: &str, directory: Option<&str>) -> bool {
    directory.is_some_and(|d| !d.is_empty()) && (path.starts_with('/') || is_windows_path(path)) && relative_path(path, directory) == path
}

// ─── turns ──────────────────────────────────────────────────────────────────

fn has_content(m: &ListMessage) -> bool {
    !m.body.trim().is_empty()
        || !m.images.is_empty()
        || !m.tools.is_empty()
        || !m.question_parts.is_empty()
        || m.turn_error.is_some()
        || m.background
        || m.compaction
        || m.shell
}

struct TurnRange {
    /// Index of the turn's prompt.
    user: Option<usize>,
    /// Indices of the turn's agent messages.
    agent: Vec<usize>,
}

fn turn_ranges(messages: &[ListMessage]) -> Vec<TurnRange> {
    let mut ranges: Vec<TurnRange> = Vec::new();
    let mut current: Option<usize> = None;
    for (index, message) in messages.iter().enumerate() {
        match message.role {
            Role::User => {
                ranges.push(TurnRange { user: Some(index), agent: Vec::new() });
                current = Some(ranges.len() - 1);
            }
            Role::Assistant => {
                let at = *current.get_or_insert_with(|| {
                    ranges.push(TurnRange { user: None, agent: Vec::new() });
                    ranges.len() - 1
                });
                ranges[at].agent.push(index);
            }
            // A shell block of yours ends the turn it sits in.
            Role::Shell => current = None,
        }
    }
    ranges
}

/// The id of the turn each message belongs to (the turn's first message: its prompt, or its first agent message when it
/// has none), for the messages of turns that have agent messages. Folded turns are opened by this id; Find opens them
/// by it too.
pub fn turn_ids_of(messages: &[ListMessage]) -> HashMap<String, String> {
    let mut ids = HashMap::new();
    for range in turn_ranges(messages) {
        let Some(&first) = range.agent.first() else { continue };
        let turn_id = messages[range.user.unwrap_or(first)].id.clone();
        if let Some(user) = range.user {
            ids.insert(messages[user].id.clone(), turn_id.clone());
        }
        for index in range.agent {
            ids.insert(messages[index].id.clone(), turn_id.clone());
        }
    }
    ids
}

// ─── file stats ─────────────────────────────────────────────────────────────

/// The files' change as the session's diff counts it, by path as the session's folder knows it.
pub type FileStats = HashMap<String, FileStat>;

/// The session's files as one turn ended, kept by the server: `ended_at` in ms.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnStatsSnapshot {
    pub ended_at: i64,
    pub files: FileStats,
}

/// When a turn began: its prompt's time, or its first message's when it has no prompt. `from..to` are the turn's messages.
fn turn_start(messages: &[ListMessage], from: usize, to: usize) -> Option<i64> {
    if let Some(prompt) = from.checked_sub(1).and_then(|p| messages.get(p))
        && prompt.role == Role::User
        && let Some(at) = prompt.created_at
    {
        return Some(at);
    }
    messages[from..to].iter().find_map(|m| m.created_at)
}

/// The snapshot that says where the turn left the files: the last one the server took after the turn's prompt and before
/// the next. (A turn can idle more than once, and clocks differ a little, so the window is by prompt, not by message.)
fn snapshot_for(snapshots: &[TurnStatsSnapshot], start: Option<i64>, next_start: Option<i64>) -> Option<&TurnStatsSnapshot> {
    let start = start?;
    snapshots.iter().rfind(|s| s.ended_at >= start && next_start.is_none_or(|next| s.ended_at < next))
}

/// Gives every call that wrote a whole file the change it made: the file's change as the server saw it when the turn
/// ended (`snapshots`), or, for the latest turn before its snapshot arrives, the file's session diff, the same numbers
/// as the Changes panel. A call whose change is not known gets `None` and draws no counts: an earlier turn's call is
/// never recounted from today's diff, and a file outside the session's folder is not in that diff.
pub fn with_file_stats(
    messages: &[ListMessage],
    stats: Option<&FileStats>,
    directory: Option<&str>,
    snapshots: Option<&[TurnStatsSnapshot]>,
) -> Vec<ListMessage> {
    let users: Vec<usize> = (0..messages.len()).filter(|&i| messages[i].role == Role::User).collect();
    let last_user = users.last().map(|&u| u as isize).unwrap_or(-1);
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            if !message.tools.iter().any(|t| t.diff_from_content) {
                return message.clone();
            }
            let in_last = index as isize > last_user;
            let snapshot = snapshots.filter(|s| !s.is_empty()).and_then(|snapshots| {
                let start = users.iter().rfind(|&&u| u < index).map_or(0, |&u| u + 1);
                let next_user = users.iter().find(|&&u| u >= index).copied().unwrap_or(messages.len());
                let next_start = messages.get(next_user).and_then(|m| m.created_at);
                snapshot_for(snapshots, turn_start(messages, start, next_user), next_start)
            });
            let tools = message
                .tools
                .iter()
                .map(|tool| {
                    let Some(path) = tool.file_path.as_deref().filter(|_| tool.diff_from_content) else { return tool.clone() };
                    let mut found = None;
                    if !is_outside(path, directory) {
                        let relative = relative_path(path, directory);
                        // A file the diff doesn't list has no net change.
                        let unchanged = FileStat::new(0, 0);
                        if let Some(snapshot) = snapshot {
                            found = Some(snapshot.files.get(&relative).copied().unwrap_or(unchanged));
                        } else if in_last && let Some(stats) = stats {
                            found = Some(stats.get(&relative).copied().unwrap_or(unchanged));
                        }
                    }
                    ListTool { diff_stat: found, ..tool.clone() }
                })
                .collect();
            ListMessage { tools, ..message.clone() }
        })
        .collect()
}

// ─── folding ────────────────────────────────────────────────────────────────

pub struct FoldOptions<'a> {
    /// The session is working: its last turn is still running and stays open.
    pub running: bool,
    /// You watched the latest turn run: it stays as you saw it after it ends, and folds once you send the next prompt.
    /// Without it (a reload, a session opened after the turn) every finished turn folds.
    pub keep_last: bool,
    /// The session's folder, so a call that named an absolute path shows it as the folder knows it.
    pub directory: Option<&'a str>,
    /// Turn ids the user opened: their rows show again (the fold row stays).
    pub open: &'a HashSet<String>,
}

/// Whether you watched the latest turn run (`watchedLastTurn`): set when the session starts working, cleared when another
/// session is opened. It is `FoldOptions::keep_last`, so the turn you watched stays as you saw it until the next prompt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WatchedTurn(bool);

impl WatchedTurn {
    /// The session's working state changed (or was read again).
    pub fn on_working(&mut self, working: bool) {
        self.0 |= working;
    }
    pub fn on_session_change(&mut self) {
        self.0 = false;
    }
    pub fn watched(self) -> bool {
        self.0
    }
}

/// The fold row's click (`toggleTurn`): the turn opens, or closes again. The set belongs to the stream and is not reset
/// when another session opens; ids are message ids, which no two sessions share.
pub fn toggle_turn(open: &mut HashSet<String>, turn_id: &str) {
    if !open.remove(turn_id) {
        open.insert(turn_id.to_string());
    }
}

/// The messages as they draw once finished turns fold. A turn with nothing to fold (no calls, no thinking) is left as
/// it is.
pub fn fold_turns(messages: &[ListMessage], options: &FoldOptions) -> Vec<ListMessage> {
    let mut out: Vec<Option<ListMessage>> = messages.iter().cloned().map(Some).collect();
    let ranges = turn_ranges(messages);

    for (range_index, range) in ranges.iter().enumerate() {
        let Some(&first_agent) = range.agent.first() else { continue };
        if range_index == ranges.len() - 1 && (options.running || options.keep_last) {
            continue;
        }
        let turn_id = messages[range.user.unwrap_or(first_agent)].id.clone();

        let mut summary = TurnSummary { duration_ms: None, reads: 0, edits: 0, commands: 0, other: 0, failed: 0, outcome: Outcome::Done };
        let mut files: HashSet<String> = HashSet::new();
        let (mut has_thinking, mut foldable, mut ended, mut ended_at) = (false, 0, false, None::<i64>);

        for &index in &range.agent {
            let message = &messages[index];
            // The turn's own working time ends where it ended: a notice that arrives later (background work finishing)
            // or a retry after a failure doesn't stretch it.
            let finished = message.completed_at.or(message.created_at);
            // The first notice of finished background work is where the turn's own work stopped: what follows is the
            // agent picking that work up again.
            if message.background {
                ended = true;
            }
            if let (false, Some(finished)) = (ended, finished)
                && ended_at.is_none_or(|e| finished > e)
            {
                ended_at = Some(finished);
            }
            if message.turn_error.is_some() {
                ended = true;
                summary.outcome = Outcome::Failed;
            }
            has_thinking |= message.reasoning.iter().any(|r| r.has_text());

            for tool in &message.tools {
                if tool.delegation || is_page_show(tool) {
                    continue;
                }
                if tool.status != ToolStatus::Completed {
                    summary.failed += 1;
                    if tool.status == ToolStatus::Stopped && summary.outcome == Outcome::Done {
                        summary.outcome = Outcome::Stopped;
                    }
                    continue;
                }
                if is_plan_exit(tool) {
                    summary.other += 1;
                } else if is_read_call(&tool.kind) {
                    summary.reads += 1;
                } else if is_shell_row(&tool.kind) {
                    summary.commands += 1;
                } else if is_write_call(&tool.kind) {
                    if let Some(path) = &tool.file_path {
                        files.insert(relative_path(path, options.directory));
                    }
                } else {
                    summary.other += 1;
                }
                if !stays_visible(tool) {
                    foldable += 1;
                }
            }
        }
        summary.edits = files.len();
        let started = range.user.and_then(|u| messages[u].created_at);
        if let (Some(started), Some(ended_at)) = (started, ended_at)
            && ended_at > started
        {
            summary.duration_ms = Some(ended_at - started);
        }

        if foldable == 0 && !has_thinking {
            continue;
        }
        let turn = FoldedTurn { id: turn_id.clone(), summary };
        // The fold hides tool rows and thinking only: every block of the agent's text stays, in order.
        let collapsed = !options.open.contains(&turn_id);

        for (position, &index) in range.agent.iter().enumerate() {
            let mut view = messages[index].clone();
            if collapsed {
                view.tools.retain(stays_visible);
                view.reasoning.clear();
            }
            let is_host = position == 0;
            if collapsed && !has_content(&view) {
                // Nothing left of this message. The first one stays as the fold row's host; the rest go.
                if !is_host {
                    out[index] = None;
                    continue;
                }
                view.fold_only = true;
            }
            if is_host {
                view.fold = Some(turn.clone());
            }
            out[index] = Some(view);
        }
    }
    out.into_iter().flatten().collect()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "read 4 files · edited 2 · ran 2 commands": what the turn did, in the order of the fold line.
pub fn summary_parts(summary: &TurnSummary) -> Vec<String> {
    let mut parts = Vec::new();
    if summary.reads > 0 {
        parts.push(format!("read {}", plural(summary.reads, "file", "files")));
    }
    if summary.edits > 0 {
        parts.push(format!("edited {}", summary.edits));
    }
    if summary.commands > 0 {
        parts.push(format!("ran {}", plural(summary.commands, "command", "commands")));
    }
    if summary.other > 0 {
        parts.push(plural(summary.other, "other call", "other calls"));
    }
    if summary.failed > 0 {
        parts.push(format!("{} failed", summary.failed));
    }
    parts
}

/// `formatDuration(ms, { tenths: true })` (`lib/format/duration.ts`): "0.4 s" under ten seconds, "34 s", "2 min 05 s",
/// "1 h 05 min", "1 d 02 h". Empty when there is no duration.
pub fn format_duration_tenths(ms: Option<i64>) -> String {
    let Some(ms) = ms else { return String::new() };
    let span = ms.max(0);
    if span < 10_000 {
        return format!("{:.1} s", ((span / 100) as f64 / 10.0).max(0.1));
    }
    let seconds = span / 1000;
    if seconds < 60 {
        return format!("{seconds} s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        let s = seconds % 60;
        return if s == 0 { format!("{minutes} min") } else { format!("{minutes} min {s:02} s") };
    }
    let hours = minutes / 60;
    if hours < 24 {
        return if minutes % 60 == 0 { format!("{hours} h") } else { format!("{hours} h {:02} min", minutes % 60) };
    }
    let days = hours / 24;
    if hours % 24 == 0 { format!("{days} d") } else { format!("{days} d {:02} h", hours % 24) }
}

/// The fold row's first words (`TurnFoldRow` `worked`): "Worked for 34 s", "Worked briefly" under a second, "Worked" when
/// the turn has no duration.
pub fn worked_text(duration_ms: Option<i64>) -> String {
    let took = format_duration_tenths(duration_ms);
    if took.starts_with("0.") { "Worked briefly".into() } else { format!("Worked for {took}").trim_end_matches(" for ").to_string() }
}
