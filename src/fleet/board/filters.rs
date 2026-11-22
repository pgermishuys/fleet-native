//! The Board's filters and the counts they feed (`stores/board.ts` 323-334, 379-488): project, status and agents, Group by
//! and Sort. They change only the side panels' counts and the Summary tab, never the lanes (FB3). Every card counts,
//! archived ones too, as one of the side panels' "sessions" (`cards::as_session`).
use super::cards::{CardSession, SortBy, Status, as_session};
use super::{Board, Card, Lane, order};
use crate::fleet::work::parse_iso_ms;
use std::collections::BTreeMap;

/// `BoardGroupBy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupBy {
    #[default]
    Status,
    Project,
    Agent,
}

impl GroupBy {
    pub const ALL: [GroupBy; 3] = [GroupBy::Status, GroupBy::Project, GroupBy::Agent];

    /// `groupByLabels`.
    pub fn label(self) -> &'static str {
        match self {
            GroupBy::Status => "Status",
            GroupBy::Project => "Project",
            GroupBy::Agent => "Agent",
        }
    }
}

/// What "All projects" is kept as.
pub const ALL_PROJECTS: &str = "all";

/// The left panel's choices. Kept while the app runs, not saved (Fleet's store is in memory too).
#[derive(Clone, Debug)]
pub struct Filters {
    /// "all", or the board's name.
    pub project: String,
    /// One switch per status, in `Status::ALL`'s order; all on.
    pub statuses: [bool; 5],
    /// One switch per agent that has a card; a new agent comes in on, one with no card left goes.
    pub agents: BTreeMap<String, bool>,
    pub group_by: GroupBy,
    pub sort_by: SortBy,
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            project: ALL_PROJECTS.into(),
            statuses: [true; 5],
            agents: BTreeMap::new(),
            group_by: GroupBy::Status,
            sort_by: SortBy::Newest,
        }
    }
}

fn slot(status: Status) -> usize {
    Status::ALL.iter().position(|s| *s == status).unwrap_or(0)
}

impl Filters {
    pub fn status_on(&self, status: Status) -> bool {
        self.statuses[slot(status)]
    }

    /// `setStatusFilter`.
    pub fn set_status(&mut self, status: Status, on: bool) {
        self.statuses[slot(status)] = on;
    }

    /// `agentFilters[agent] ?? true`.
    pub fn agent_on(&self, agent: &str) -> bool {
        self.agents.get(agent).copied().unwrap_or(true)
    }

    /// `setAgentFilter`.
    pub fn set_agent(&mut self, agent: &str, on: bool) {
        self.agents.insert(agent.to_string(), on);
    }

    /// The two watchers: agents with no card leave, new ones come in on; a picked project that is gone goes back to "all".
    pub fn follow(&mut self, agents: &[String], projects: &[String]) {
        self.agents.retain(|a, _| agents.contains(a));
        for agent in agents {
            self.agents.entry(agent.clone()).or_insert(true);
        }
        if self.project != ALL_PROJECTS && !projects.contains(&self.project) {
            self.project = ALL_PROJECTS.into();
        }
    }
}

/// `sessions`: every card in the store's order (`sortBoardCards`), as a session of the board.
pub fn sessions(board: Option<&Board>, lanes: &[Lane], cards: &[Card]) -> Vec<CardSession> {
    order::sorted_cards(cards).iter().map(|c| as_session(c, board, lanes.iter().find(|l| l.id == c.lane_id))).collect()
}

/// `availableProjects`: the board's name, or none.
pub fn available_projects(board: Option<&Board>) -> Vec<String> {
    board.map(|b| vec![b.name.clone()]).unwrap_or_default()
}

/// `availableAgents`: each card's agent once, sorted.
pub fn available_agents(sessions: &[CardSession]) -> Vec<String> {
    let mut agents: Vec<String> = sessions.iter().map(|s| s.agent.clone()).collect();
    agents.sort_by(|a, b| locale_cmp(a, b));
    agents.dedup();
    agents
}

/// `localeCompare` without ICU: case folded first, then as written, so the order is stable. It differs from a browser's only
/// for accents and punctuation.
pub fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// `filteredSessions`.
pub fn filtered(sessions: &[CardSession], filters: &Filters) -> Vec<CardSession> {
    sessions
        .iter()
        .filter(|s| filters.project == ALL_PROJECTS || s.project_name == filters.project)
        .filter(|s| filters.status_on(s.status) && filters.agent_on(&s.agent))
        .cloned()
        .collect()
}

/// `sortedSessions`: the filtered ones, newest or oldest first by when each card was made, or by title.
pub fn sorted(sessions: &[CardSession], filters: &Filters) -> Vec<CardSession> {
    let mut out = filtered(sessions, filters);
    let made = |s: &CardSession| parse_iso_ms(&s.created_at).unwrap_or(0);
    match filters.sort_by {
        SortBy::Newest => out.sort_by_key(|s| std::cmp::Reverse(made(s))),
        SortBy::Oldest => out.sort_by_key(made),
        SortBy::Title => out.sort_by(|a, b| locale_cmp(&a.title, &b.title)),
    }
    out
}

/// One group of `groupedSessions`: its key (the status's value, the project or the agent), label and sessions.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub key: String,
    pub label: String,
    pub sessions: Vec<CardSession>,
}

/// `groupedSessions`: the sorted sessions by status (in the options' order), project or agent (by label).
pub fn grouped(sessions: &[CardSession], filters: &Filters) -> Vec<Group> {
    let mut groups: Vec<(Option<Status>, Group)> = Vec::new();
    for session in sorted(sessions, filters) {
        let (key, label, status) = match filters.group_by {
            GroupBy::Status => (session.status.value().to_string(), session.status.label().to_string(), Some(session.status)),
            GroupBy::Project => (session.project_name.clone(), session.project_name.clone(), None),
            GroupBy::Agent => (session.agent.clone(), session.agent.clone(), None),
        };
        match groups.iter_mut().find(|(_, g)| g.key == key) {
            Some((_, group)) => group.sessions.push(session),
            None => groups.push((status, Group { key, label, sessions: vec![session] })),
        }
    }
    match filters.group_by {
        GroupBy::Status => groups.sort_by_key(|(status, _)| status.map_or(0, slot)),
        _ => groups.sort_by(|(_, a), (_, b)| locale_cmp(&a.label, &b.label)),
    }
    groups.into_iter().map(|(_, g)| g).collect()
}

/// `quickStats`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuickStats {
    pub total: usize,
    pub visible: usize,
    pub active: usize,
    pub completed: usize,
    pub projects: usize,
}

pub fn quick_stats(sessions: &[CardSession], filters: &Filters) -> QuickStats {
    let visible = filtered(sessions, filters);
    let mut projects: Vec<&str> = visible.iter().map(|s| s.project_name.as_str()).collect();
    projects.sort_unstable();
    projects.dedup();
    QuickStats {
        total: sessions.len(),
        visible: visible.len(),
        active: visible.iter().filter(|s| s.status == Status::Active).count(),
        completed: visible.iter().filter(|s| s.status == Status::Completed).count(),
        projects: projects.len(),
    }
}

/// `filterSummary`: "<visible> visible • grouped by <status|project|agent>".
pub fn filter_summary(sessions: &[CardSession], filters: &Filters) -> String {
    format!("{} visible • grouped by {}", quick_stats(sessions, filters).visible, filters.group_by.label().to_lowercase())
}
