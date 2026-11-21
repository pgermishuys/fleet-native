//! Mission control as data (component 23, piece B; `lib/mission-control.ts`): every machine's sessions as tiles, grouped by
//! folder, machine or state, and what waits on the user. No Rune and no I/O: the feeds (piece C) fill `MissionMachine` in and
//! the page (pieces D and E) draws what this decides. `peek` and `requests` are the other two files of the same data.
use super::dashboard::{Bucket, is_top_level, session_bucket, updated_at};
use super::groups::Listed;
use crate::asks::permission::PermissionAsk;
use crate::asks::question::QuestionInfo;
use crate::asks::wording::ask_preview_wording;
use crate::mock::Session;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub mod feed;
pub mod peek;
pub mod read;
pub mod requests;
pub mod store;

/// A day: an idle session quiet for longer drops off the wall (`FINISHED_FOR_MS`).
pub const FINISHED_FOR_MS: i64 = 24 * 60 * 60_000;

/// What a machine's feed is doing (`FeedStatus`, `lib/machine-feed.ts`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FeedStatus {
    #[default]
    Connecting,
    Live,
    Polling,
    Unreachable,
}

/// What a machine says a session is doing this second (`GET /api/sessions/now`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionNow {
    pub text: String,
    pub kind: NowKind,
    pub at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NowKind {
    Tool,
    Thinking,
    Reply,
}

/// What a waiting session waits on (`InboxAsk`): a permission, or a question with how many more follow it.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Permission(Box<PermissionAsk>),
    Question { request_id: String, question: QuestionInfo, more: usize },
}

/// One machine as mission control knows it (`MissionMachine`).
#[derive(Default)]
pub struct MissionMachine {
    pub id: String,
    pub name: String,
    pub status: FeedStatus,
    /// When the machine last answered, in ms; none until its first answer.
    pub last_heard_at: Option<i64>,
    pub sessions: Vec<Session>,
    /// Asks by session id.
    pub asks: HashMap<String, Ask>,
    /// Now lines by session id; empty when the machine is too old to say.
    pub now: HashMap<String, SessionNow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileState {
    NeedsYou,
    Working,
    Idle,
    Error,
}

impl TileState {
    /// The state in words (`STATE_LABEL`).
    pub fn label(self) -> &'static str {
        match self {
            TileState::NeedsYou => "Needs you",
            TileState::Working => "Working",
            TileState::Idle => "Idle",
            TileState::Error => "Stopped with an error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    Folder,
    Machine,
    State,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    /// `<machine id>:<session id>`.
    pub key: String,
    pub machine_id: String,
    pub machine_name: String,
    pub session_id: String,
    pub title: String,
    pub folder: Option<String>,
    pub branch: Option<String>,
    pub state: TileState,
    pub harness: String,
    /// The harness's own name (`opencode2`), which says what it can do.
    pub harness_type: String,
    pub model: Option<String>,
    /// What it's doing: the question, the ask, the machine's now line, else the state in words.
    pub now: String,
    pub todos: Option<(u32, u32)>,
    pub helpers: u32,
    pub ask: Option<Ask>,
    pub updated_at: i64,
    /// The machine didn't answer last time: this is how it was then.
    pub stale: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileGroup {
    pub key: String,
    pub label: String,
    pub tiles: Vec<Tile>,
}

/// A harness type in words (`HARNESS_NAMES`); one it doesn't know is shown as it is.
pub fn harness_name(kind: &str) -> String {
    match kind {
        "opencode" => "OpenCode",
        "opencode2" => "OpenCode 2",
        "claude-code" => "Claude Code",
        "copilot" => "GitHub Copilot",
        "pi" => "Pi",
        other => other,
    }
    .to_string()
}

/// `sessionFolder`: the project's or workspace's name, else the last part of the path. (Fleet reads `sourceDirectory`
/// first; the list rows here keep only `workspaceDirectory`.)
pub fn session_folder(session: &Session) -> Option<String> {
    let l = &session.listing;
    let named = l.project_name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if let Some(name) = named.or_else(|| l.workspace_display_name.as_deref().map(str::trim).filter(|n| !n.is_empty())) {
        return Some(name.to_string());
    }
    let path = l.workspace_directory.trim_end_matches(['/', '\\']);
    path.rsplit(['/', '\\']).next().filter(|last| !last.is_empty()).map(str::to_string)
}

fn state_of(session: &Session) -> TileState {
    if session.status_word() == "error" {
        return TileState::Error;
    }
    match session_bucket(session, &|_| false) {
        Bucket::NeedsYou => TileState::NeedsYou,
        Bucket::Working => TileState::Working,
        Bucket::Recent => TileState::Idle,
    }
}

/// The line a tile shows for a session that waits on an ask.
fn ask_line(ask: &Ask) -> String {
    match ask {
        Ask::Question { question, .. } => format!("? {}", question.question),
        Ask::Permission(ask) => {
            let preview = ask_preview_wording(ask);
            match preview.detail.filter(|d| !d.is_empty()) {
                Some(detail) => format!("{} {detail}", preview.lead),
                None => preview.lead,
            }
        }
    }
}

fn now_line(state: TileState, ask: Option<&Ask>, line: Option<&SessionNow>) -> String {
    match (state, ask, line) {
        (TileState::NeedsYou, Some(ask), _) => ask_line(ask),
        (TileState::Working, _, Some(line)) => line.text.clone(),
        (TileState::Idle, _, Some(line)) if line.kind == NowKind::Reply => line.text.clone(),
        _ => state.label().to_string(),
    }
}

fn tile(machine: &MissionMachine, session: &Session) -> Tile {
    let state = state_of(session);
    let ask = machine.asks.get(&session.id);
    let title = session.title.trim();
    let harness_type = session.row.harness_type.clone().unwrap_or_default();
    Tile {
        key: format!("{}:{}", machine.id, session.id),
        machine_id: machine.id.clone(),
        machine_name: machine.name.clone(),
        session_id: session.id.clone(),
        title: if title.is_empty() { "Untitled session".into() } else { title.into() },
        folder: session_folder(session),
        branch: Some(session.branch.trim()).filter(|b| !b.is_empty()).map(str::to_string),
        state,
        harness: harness_name(&harness_type),
        harness_type,
        model: session.row.selected_model.as_ref().map(|m| m.model_id.clone()),
        now: now_line(state, ask, machine.now.get(&session.id)),
        todos: session.row.progress.as_ref().filter(|p| p.total > 0).map(|p| (p.done, p.total)),
        helpers: session.row.running_work,
        ask: ask.cloned(),
        updated_at: updated_at(session),
        stale: machine.status == FeedStatus::Unreachable,
    }
}

/// `buildTiles`: every machine's top-level sessions that wait on the user or work, and those that went quiet in the last
/// day, newest first. `archiving` holds the ids archived a moment ago (the archive queue's pending ones).
pub fn build_tiles(machines: &[MissionMachine], now: i64, quiet_for: i64, archiving: &HashSet<String>) -> Vec<Tile> {
    let mut tiles = Vec::new();
    for machine in machines {
        for session in &machine.sessions {
            if !is_top_level(session) || session.row.archived || archiving.contains(&session.id) {
                continue;
            }
            let entry = tile(machine, session);
            if entry.state == TileState::Idle && now - entry.updated_at > quiet_for {
                continue;
            }
            tiles.push(entry);
        }
    }
    // Stable, as the web's sort is: equal times keep the machines' order.
    tiles.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
    tiles
}

/// `needsYou`: what waits on the user (asking or stopped with an error), in the tiles' order.
pub fn needs_you(tiles: &[Tile]) -> Vec<&Tile> {
    tiles.iter().filter(|t| matches!(t.state, TileState::NeedsYou | TileState::Error)).collect()
}

/// `localeCompare` for labels: case and accents don't decide first, the letters do.
fn locale_compare(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| b.cmp(a))
}

/// `groupTiles`: by folder or machine (alphabetical), or by state (needs you first). Tiles keep their order inside a group.
pub fn group_tiles(tiles: &[Tile], by: Grouping) -> Vec<TileGroup> {
    let mut groups: Vec<TileGroup> = Vec::new();
    for entry in tiles {
        let (key, label) = match by {
            Grouping::Folder => (entry.folder.clone().unwrap_or_default(), entry.folder.clone().unwrap_or_else(|| "No folder".into())),
            Grouping::Machine => (entry.machine_id.clone(), entry.machine_name.clone()),
            Grouping::State => (state_key(entry.state).to_string(), entry.state.label().to_string()),
        };
        match groups.iter_mut().find(|g| g.key == key) {
            Some(group) => group.tiles.push(entry.clone()),
            None => groups.push(TileGroup { key, label, tiles: vec![entry.clone()] }),
        }
    }
    if by == Grouping::State {
        let order = |g: &TileGroup| ["needs-you", "error", "working", "idle"].iter().position(|k| *k == g.key);
        groups.sort_by_key(order);
    } else {
        groups.sort_by(|a, b| locale_compare(&a.label, &b.label));
    }
    groups
}

fn state_key(state: TileState) -> &'static str {
    match state {
        TileState::NeedsYou => "needs-you",
        TileState::Error => "error",
        TileState::Working => "working",
        TileState::Idle => "idle",
    }
}

/// What the line above the wall counts (`summarize`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub running: usize,
    pub helpers: u32,
    pub need_you: usize,
}

pub fn summarize(tiles: &[Tile]) -> Summary {
    Summary {
        running: tiles.iter().filter(|t| t.state == TileState::Working).count(),
        helpers: tiles.iter().map(|t| t.helpers).sum(),
        need_you: needs_you(tiles).len(),
    }
}
