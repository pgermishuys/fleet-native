//! Lineage in the session list (`lib/session-lineage.ts`, `ProjectGroup.vue`): which session nests under which, how deep, the
//! lines drawn beside the rows, and what a running sub-agent adds. Pure: no Rune and no I/O. Rows are positions in the slice a
//! function is given.
use super::groups::{LineageKind, Listed, lineage_of};
use super::work::{RunningWorkItem, WorkKind, is_running};
use std::collections::{HashMap, HashSet};

/// How many levels the list indents: a deeper row keeps this level's indent and line (`LINEAGE_MAX_DEPTH`).
pub const MAX_DEPTH: usize = 3;

/// `lineageKindLabel`: what a child's row calls how it came.
pub fn kind_label(kind: LineageKind) -> &'static str {
    match kind {
        LineageKind::Subagent => "subagent",
        LineageKind::Fork => "fork",
        LineageKind::Started => "started",
    }
}

/// Where a level's line hangs from the row's left edge (`lineX`).
pub fn line_x(level: usize) -> f32 {
    13.0 + (level.clamp(1, MAX_DEPTH) as f32 - 1.0) * 16.0
}

/// Where a child row's content starts (`--lineage-indent`): 25, 41 and 57 px.
pub fn indent(depth: usize) -> f32 {
    line_x(depth) + 12.0
}

/// Forks and started sessions under the session they came from (`NestedLineage`). Indexes are positions in the list given.
#[derive(Debug, Default, PartialEq)]
pub struct Nested {
    /// The sessions that show at the top level, in the order they came.
    pub roots: Vec<usize>,
    /// Each session's children, in the order they came.
    pub children: HashMap<usize, Vec<(usize, LineageKind)>>,
}

/// `nestLineage`: a child nests under its parent when the parent is in the same list; otherwise (archived, filtered out, in
/// another project) it stays at the top, as does every session in a loop of parents.
pub fn nest_lineage<T: Listed>(items: &[T]) -> Nested {
    let at: HashMap<&str, usize> = items.iter().enumerate().map(|(n, i)| (i.id(), n)).collect();
    let parent_in = |n: usize| lineage_of(items[n].listing()).and_then(|(p, kind)| at.get(p.as_str()).map(|&pn| (pn, kind)));
    let in_loop = |n: usize| {
        let mut seen = HashSet::new();
        let mut link = parent_in(n);
        while let Some((parent, _)) = link {
            if parent == n {
                return true;
            }
            if !seen.insert(parent) {
                return false;
            }
            link = parent_in(parent);
        }
        false
    };
    let mut nested = Nested::default();
    for n in 0..items.len() {
        match parent_in(n) {
            Some((parent, kind)) if !in_loop(n) => nested.children.entry(parent).or_default().push((n, kind)),
            _ => nested.roots.push(n),
        }
    }
    nested
}

/// A session that came from another, how, and how far in it sits (1 for a child of the top-level session).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Descendant {
    pub index: usize,
    pub kind: LineageKind,
    pub depth: usize,
}

/// `lineageDescendants`: depth first, each with its depth.
pub fn descendants(root: usize, nested: &Nested) -> Vec<Descendant> {
    fn walk(n: usize, depth: usize, nested: &Nested, out: &mut Vec<Descendant>) {
        for &(child, kind) in nested.children.get(&n).into_iter().flatten() {
            out.push(Descendant { index: child, kind, depth });
            walk(child, depth + 1, nested, out);
        }
    }
    let mut out = Vec::new();
    walk(root, 1, nested, &mut out);
    out
}

/// The lines beside one row (`LineageConnector`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connector {
    /// The level the row is drawn at: its depth, at most `MAX_DEPTH`.
    pub depth: usize,
    /// No later row hangs off the same line: its line stops at the row's middle.
    pub last: bool,
    /// The shallower levels whose line keeps running past this row, because that ancestor has later children.
    pub through: Vec<usize>,
}

/// `layoutLineage`: the lines for rows in tree order, each given as its depth.
pub fn layout_lineage(depths: &[usize]) -> Vec<Connector> {
    let levels: Vec<usize> = depths.iter().map(|d| (*d).clamp(1, MAX_DEPTH)).collect();
    // Whether a line at `level` goes on after row `index`: the next row at that level or shallower is at that level.
    let continues = |index: usize, level: usize| levels[index + 1..].iter().find(|&&other| other <= level) == Some(&level);
    levels
        .iter()
        .enumerate()
        .map(|(index, &depth)| Connector {
            depth,
            last: !continues(index, depth),
            through: (1..depth).filter(|&level| continues(index, level)).collect(),
        })
        .collect()
}

/// Where a row stands (`sessionAgentState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentState {
    Running,
    Waiting,
    Idle,
    Failed,
}

/// `sessionAgentState`: from `sessionStatus` and `activityStatus`.
pub fn agent_state(lifecycle: &str, activity: &str) -> AgentState {
    if lifecycle == "waiting_input" || activity == "waiting_input" {
        AgentState::Waiting
    } else if lifecycle == "active" {
        AgentState::Running
    } else if lifecycle == "error" {
        AgentState::Failed
    } else {
        AgentState::Idle
    }
}

/// `runningSubagentsBySession`: sub-agents that have a session of their own and still run, by the session that started them.
pub fn running_subagents<'a>(items: impl IntoIterator<Item = &'a RunningWorkItem>) -> HashMap<String, Vec<RunningWorkItem>> {
    let mut by_session: HashMap<String, Vec<RunningWorkItem>> = HashMap::new();
    for item in items {
        if item.kind == WorkKind::Subagent && item.child_session_id.is_some() && is_running(item) {
            by_session.entry(item.session_id.clone()).or_default().push(item.clone());
        }
    }
    by_session
}

/// A row under a top-level session (`ChildRow`).
#[derive(Clone, Debug, PartialEq)]
pub enum ChildRow {
    Subagent { work: Box<RunningWorkItem>, depth: usize },
    Session { index: usize, kind: LineageKind, depth: usize },
}

impl ChildRow {
    pub fn depth(&self) -> usize {
        match self {
            ChildRow::Subagent { depth, .. } | ChildRow::Session { depth, .. } => *depth,
        }
    }
}

/// `subagentRowsOf`: a session's running sub-agents, each followed by the sub-agents it runs itself (they run in its own
/// session), one level under it. `seen` guards against a session that is its own sub-agent.
fn subagent_rows(id: &str, depth: usize, subagents: &HashMap<String, Vec<RunningWorkItem>>, seen: &mut HashSet<String>) -> Vec<ChildRow> {
    let mut rows = Vec::new();
    for work in subagents.get(id).into_iter().flatten() {
        rows.push(ChildRow::Subagent { work: Box::new(work.clone()), depth });
        if let Some(child) = work.child_session_id.as_deref()
            && seen.insert(child.to_string())
        {
            rows.extend(subagent_rows(child, depth + 1, subagents, seen));
        }
    }
    rows
}

/// `childRowsOf`: what shows under a top-level session: its running sub-agents first, then each session it forked or started,
/// in tree order, each followed by that one's own sub-agents.
pub fn child_rows<T: Listed>(
    items: &[T],
    root: usize,
    nested: &Nested,
    subagents: &HashMap<String, Vec<RunningWorkItem>>,
) -> Vec<ChildRow> {
    let mut seen = HashSet::from([items[root].id().to_string()]);
    let mut rows = subagent_rows(items[root].id(), 1, subagents, &mut seen);
    for d in descendants(root, nested) {
        rows.push(ChildRow::Session { index: d.index, kind: d.kind, depth: d.depth });
        let mut seen = HashSet::from([items[d.index].id().to_string()]);
        rows.extend(subagent_rows(items[d.index].id(), d.depth + 1, subagents, &mut seen));
    }
    rows
}
