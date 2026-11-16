//! A group's rows with each session's family under it (`ProjectGroup.vue`: `childRowsOf`, `opensByItself`, `childrenExpanded`,
//! `toggleChildren`). The groups give a flat list of rows; this puts forks, started sessions and running sub-agents under the
//! session they came from, in tree order, and leaves out what a folded parent hides. Rows are positions `(project, session)`
//! in `State::projects`.
use super::groups::LineageKind;
use super::home_layout::At;
use super::lineage::{AgentState, ChildRow, Connector, agent_state, child_rows, layout_lineage, nest_lineage, running_subagents};
use super::work::RunningWorkItem;
use crate::State;
use crate::mock::{Session, Status};

/// One row of a group, in the order it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// A top-level session. `caret` is `Some(open)` when something hangs under it.
    Root { at: (usize, usize), caret: Option<bool> },
    /// A fork or a started session, with the lines beside it.
    Child { at: (usize, usize), kind: LineageKind, line: Connector, parent: String },
    /// A running sub-agent: the work item, and the session that runs it.
    Subagent { work: Box<RunningWorkItem>, line: Connector },
}

impl Entry {
    /// The list row the entry is, if it is one.
    pub fn at(&self) -> Option<(usize, usize)> {
        match self {
            Entry::Root { at, .. } | Entry::Child { at, .. } => Some(*at),
            Entry::Subagent { .. } => None,
        }
    }
}

/// How a row stands for `opensByItself`: the server's words, or the invented sessions' glyph.
fn state_of(s: &Session) -> AgentState {
    if s.row.lifecycle.is_empty() && s.row.activity.is_empty() {
        return match s.status {
            Status::Working => AgentState::Running,
            Status::NeedsInput => AgentState::Waiting,
            Status::Idle => AgentState::Idle,
        };
    }
    agent_state(&s.row.lifecycle, &s.row.activity)
}

impl State {
    /// `family_plan` for the rows of one group, folded the way the user and the list want them.
    pub fn family_plan(&self, rows: &[(usize, usize)]) -> Vec<Entry> {
        let items: Vec<At> = rows.iter().map(|&(p, i)| At(p, i, &self.projects[p].sessions[i])).collect();
        let nested = nest_lineage(&items);
        let subs = running_subagents(self.list_work.running());
        let open_at = self.open_at();
        let mut plan = Vec::new();
        for &root in &nested.roots {
            let at = (items[root].0, items[root].1);
            let kids = child_rows(&items, root, &nested, &subs);
            if kids.is_empty() {
                plan.push(Entry::Root { at, caret: None });
                continue;
            }
            let id = items[root].2.id.as_str();
            let by_itself = open_at == at
                || kids.iter().any(|k| match k {
                    ChildRow::Subagent { .. } => true,
                    ChildRow::Session { index, .. } => {
                        open_at == (items[*index].0, items[*index].1)
                            || matches!(state_of(items[*index].2), AgentState::Running | AgentState::Waiting)
                    }
                });
            let expanded = self.family_open.get(id).copied().unwrap_or(by_itself);
            plan.push(Entry::Root { at, caret: Some(expanded) });
            if !expanded {
                continue;
            }
            let lines = layout_lineage(&kids.iter().map(ChildRow::depth).collect::<Vec<_>>());
            for (kid, line) in kids.into_iter().zip(lines) {
                plan.push(match kid {
                    ChildRow::Subagent { work, .. } => Entry::Subagent { work, line },
                    ChildRow::Session { index, kind, .. } => {
                        Entry::Child { at: (items[index].0, items[index].1), kind, line, parent: parent_of(&items, index) }
                    }
                });
            }
        }
        plan
    }

    /// The caret, or Right and Left on a row with children: shows or hides what it started (`toggleChildren`).
    pub fn set_family_open(&mut self, id: &str, open: bool) {
        self.family_open.insert(id.to_string(), open);
    }
}

/// The id of the session a child hangs from, for its tooltip and a test.
fn parent_of(items: &[At], index: usize) -> String {
    super::groups::lineage_of(&items[index].2.listing).map(|(p, _)| p).unwrap_or_default()
}

impl State {
    /// A running sub-agent's row: opens its own session under its parent (`SubagentSessionRow.open`). The server keeps these
    /// sessions out of the list, so one the list doesn't hold is kept as a session of this machine, as an opened one is.
    pub fn open_subagent_session(&mut self, parent: &str, child: &str, title: &str, tx: &mut rune::Tx) {
        if self.open_child(parent, child, tx) {
            return;
        }
        let session = Session {
            id: child.to_string(),
            title: title.to_string(),
            status: Status::Working,
            ago_secs: None,
            branch: String::new(),
            messages: Vec::new(),
            tokens: 0,
            row: super::rows::Row { lifecycle: "active".into(), activity: "busy".into(), ..Default::default() },
            listing: super::groups::Listing { parent: Some(parent.to_string()), ..Default::default() },
        };
        self.store.upsert_elsewhere(super::settings::HOME_MACHINE_KEY, session);
        self.go(super::store::Route::Session(child.to_string()));
        self.after_open(tx);
        self.work.opened_from = Some(parent.to_string());
    }
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

impl State {
    /// A family in the first project for a picture: the open session with a running sub-agent, a fork of it that has a fork of
    /// its own (a level deeper), and a session it started; `folded` shows the same family with its caret closed.
    pub fn show_lineage(&mut self, folded: bool) {
        let ids: Vec<String> = self.projects[0].sessions.iter().map(|s| s.id.clone()).collect();
        let set = |s: &mut Session, fork_of: Option<&str>, started_by: Option<&str>| {
            s.listing.forked_from = fork_of.map(str::to_string);
            s.listing.spawn_kind = Some(if fork_of.is_some() { "fork" } else { "agent" }.into());
            s.listing.spawned_by = started_by.map(str::to_string);
        };
        set(&mut self.projects[0].sessions[2], Some(&ids[0]), None);
        set(&mut self.projects[0].sessions[3], None, Some(&ids[0]));
        if ids.len() > 4 {
            set(&mut self.projects[0].sessions[4], Some(&ids[2]), None);
        }
        let item = super::work::to_item(&serde_json::json!({
            "id": "scene-work", "sessionId": ids[0], "kind": "subagent", "title": "general", "label": "Check the eviction edge cases",
            "status": "running", "childSessionId": "scene-sub-session",
        }));
        if let Some(item) = item {
            self.list_work.apply(item);
        }
        if folded {
            self.family_open.insert(ids[0].clone(), false);
        }
    }
}
