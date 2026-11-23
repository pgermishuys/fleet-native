//! Reading a card: its status from its lane (`inferLegacyStatus`), the card as one of the side panels' "sessions"
//! (`toLegacySession`), the status and sort options (`stores/board.ts` 86-98, 225-308), and what `KanbanCard.vue` reads
//! from it (39-133): its source's name, its JSON metadata and GitHub's fields in it.
use super::{Board, Card, Lane};
use crate::theme::Theme;
use rune::Color;
use serde_json::{Map, Value};

/// `BoardStatus`, in the options' order (Active, Idle, Waiting, Complete, Error).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Active,
    Idle,
    Waiting,
    Completed,
    Error,
}

impl Status {
    /// `boardStatusOptions`' order.
    pub const ALL: [Status; 5] = [Status::Active, Status::Idle, Status::Waiting, Status::Completed, Status::Error];

    /// The option's label.
    pub fn label(self) -> &'static str {
        match self {
            Status::Active => "Active",
            Status::Idle => "Idle",
            Status::Waiting => "Waiting",
            Status::Completed => "Complete",
            Status::Error => "Error",
        }
    }

    /// The value Fleet keys it by (`waiting_input` for Waiting).
    pub fn value(self) -> &'static str {
        match self {
            Status::Active => "active",
            Status::Idle => "idle",
            Status::Waiting => "waiting_input",
            Status::Completed => "completed",
            Status::Error => "error",
        }
    }

    /// The option's colour: `--running`, `#71717a`, `#f59e0b`, `--complete`, `--error`.
    pub fn color(self, t: &Theme) -> Color {
        match self {
            Status::Active => t.running,
            Status::Idle => t.board_status_idle(),
            Status::Waiting => t.board_status_waiting(),
            Status::Completed => t.complete,
            Status::Error => t.error,
        }
    }
}

/// `inferLegacyStatus`: an archived card is complete; otherwise its lane's name says (an inbox lane, or one named like
/// inbox/todo/backlog/queued, is idle; review/approval/approve/qa waiting; done/complete/completed/shipped complete;
/// error/failed/blocked error); anything else is active. The words match anywhere in the lower-cased name.
pub fn status_of(card: &Card, lane: Option<&Lane>) -> Status {
    if card.is_archived() {
        return Status::Completed;
    }
    let name = lane.map(|l| l.name.to_lowercase()).unwrap_or_default();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if lane.is_some_and(|l| l.is_inbox) || has(&["inbox", "todo", "backlog", "queued"]) {
        Status::Idle
    } else if has(&["review", "approval", "approve", "qa"]) {
        Status::Waiting
    } else if has(&["done", "complete", "completed", "shipped"]) {
        Status::Completed
    } else if has(&["error", "failed", "blocked"]) {
        Status::Error
    } else {
        Status::Active
    }
}

/// `BoardSortBy`, with the options' labels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
    #[default]
    Newest,
    Oldest,
    Title,
}

impl SortBy {
    /// `boardSortOptions`' values.
    pub fn value(self) -> &'static str {
        match self {
            SortBy::Newest => "newest",
            SortBy::Oldest => "oldest",
            SortBy::Title => "title",
        }
    }

    pub const ALL: [SortBy; 3] = [SortBy::Newest, SortBy::Oldest, SortBy::Title];

    pub fn label(self) -> &'static str {
        match self {
            SortBy::Newest => "Newest first",
            SortBy::Oldest => "Oldest first",
            SortBy::Title => "Title (A–Z)",
        }
    }
}

/// A card as the side panels count it (`BoardSession`): only the fields something on `main` reads. Cards carry no tokens or
/// cost, so both are always 0 (FB5).
#[derive(Clone, Debug, PartialEq)]
pub struct CardSession {
    pub id: String,
    pub title: String,
    /// The board's name, else "Board".
    pub project_name: String,
    pub status: Status,
    /// The card's `sourceType`, else "manual".
    pub agent: String,
    /// The card's own ISO time.
    pub created_at: String,
    pub total_tokens: u64,
    pub cost: f64,
}

/// `toLegacySession`.
pub fn as_session(card: &Card, board: Option<&Board>, lane: Option<&Lane>) -> CardSession {
    CardSession {
        id: card.id.clone(),
        title: card.title.clone(),
        project_name: board.map_or_else(|| "Board".to_string(), |b| b.name.clone()),
        status: status_of(card, lane),
        agent: card.source_type.clone().unwrap_or_else(|| "manual".to_string()),
        created_at: card.created_at.clone(),
        total_tokens: 0,
        cost: 0.0,
    }
}

// ─── KanbanCard's reading ───────────────────────────────────────────────────

/// `formatSourceType`: "Manual" for none, "GitHub" for `github`, else the type with its first letter upper-cased (so a
/// real synced card, `github-issue`, says "Github-issue", FB1).
pub fn format_source_type(source_type: Option<&str>) -> String {
    match source_type {
        None => "Manual".to_string(),
        Some("github") => "GitHub".to_string(),
        Some(other) => {
            let mut chars = other.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        }
    }
}

/// The source pill: "Manual", or "<source> sync".
pub fn source_label(card: &Card) -> String {
    match &card.source_type {
        None => "Manual".to_string(),
        Some(_) => format!("{} sync", format_source_type(card.source_type.as_deref())),
    }
}

/// `isGitHubCard`: only `github`, which the server never writes (it writes `github-issue`, FB1; decision D2).
pub fn is_github(card: &Card) -> bool {
    card.source_type.as_deref() == Some("github")
}

/// `parseMetadataObject`: the metadata as a JSON object; anything else (blank, not JSON, a list, a string) is none.
pub fn parse_metadata(metadata: Option<&str>) -> Option<Map<String, Value>> {
    let text = metadata?.trim();
    if text.is_empty() {
        return None;
    }
    match serde_json::from_str::<Value>(text).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// `metadataText`: metadata that isn't a JSON object, shown as it is.
pub fn metadata_text(card: &Card) -> Option<String> {
    let metadata = card.metadata.as_deref()?;
    parse_metadata(Some(metadata)).is_none().then(|| metadata.to_string())
}

/// `sourceDetail`: the source key, else "No source key".
pub fn source_detail(card: &Card) -> String {
    card.source_key.clone().unwrap_or_else(|| "No source key".to_string())
}

/// `GitHubCardMetadata`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitHubMeta {
    pub number: Option<f64>,
    pub state: Option<String>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub html_url: Option<String>,
    pub updated_at: Option<String>,
    pub stale: bool,
}

fn meta_text(m: &Map<String, Value>, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// `githubMetadata`: GitHub's fields, only for a `github` card whose metadata is an object. Strings are trimmed and a blank
/// one is none; labels keep the non-blank strings as written; `stale` only when it is `true`.
pub fn github_meta(card: &Card) -> Option<GitHubMeta> {
    if !is_github(card) {
        return None;
    }
    let m = parse_metadata(card.metadata.as_deref())?;
    Some(GitHubMeta {
        number: m.get("number").and_then(Value::as_f64),
        state: meta_text(&m, "state"),
        labels: m
            .get("labels")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string).collect())
            .unwrap_or_default(),
        assignee: meta_text(&m, "assignee"),
        html_url: meta_text(&m, "html_url"),
        updated_at: meta_text(&m, "updated_at"),
        stale: m.get("stale") == Some(&Value::Bool(true)),
    })
}

/// `githubIssueLabel`: "Issue #<n>", else "GitHub issue".
pub fn github_issue_label(meta: Option<&GitHubMeta>) -> String {
    match meta.and_then(|m| m.number) {
        Some(n) => format!("Issue #{n}"),
        None => "GitHub issue".to_string(),
    }
}
