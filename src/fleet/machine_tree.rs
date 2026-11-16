//! The tree of one other machine's sessions, as `MachineSessionsGroup.vue` builds it: Pinned first while it has sessions, then
//! the machine's projects in its own order, each session's forks and started sessions under it, and the filter's matches.
//! Pure: no Rune and no I/O. Rows are positions in the machine's session list, so a view reads the session from there.
use super::groups::{
    LineageKind, Listed, ProjectInfo, build_project_groups, filter_project_groups, lineage_of, pinned_project_group, session_matches_query,
    split_pinned,
};
use crate::mock::Session;
use std::collections::HashMap;

/// One row of a group: the session (its position in the machine's list), how far in it sits, and how it came from its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub index: usize,
    /// 0 for a top-level session; a fork or started session is one in from the one it came from.
    pub depth: usize,
    pub kind: Option<LineageKind>,
}

/// A project's group (or Pinned) with the rows it draws.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeGroup {
    /// The project's id, `pinned` for the Pinned group, `ungrouped` for sessions with no project.
    pub id: String,
    pub name: String,
    pub pinned: bool,
    /// Every session in the group, children too (`sessionCount`).
    pub count: usize,
    pub rows: Vec<TreeRow>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    pub groups: Vec<TreeGroup>,
    /// What the heading counts: Pinned and every project's sessions, after the filter.
    pub count: usize,
}

/// A session of the list, for the grouping functions.
struct Item<'a> {
    index: usize,
    session: &'a Session,
}

impl Listed for Item<'_> {
    fn id(&self) -> &str {
        self.session.id()
    }
    fn title(&self) -> &str {
        self.session.title()
    }
    fn status_word(&self) -> &str {
        self.session.status_word()
    }
    fn listing(&self) -> &super::groups::Listing {
        self.session.listing()
    }
}

/// The rows of one group: top-level sessions in the group's order, each followed by what came from it, depth first. A session
/// whose parent isn't in the group stands on its own there.
fn rows_of(items: &[Item]) -> Vec<TreeRow> {
    let at: HashMap<&str, usize> = items.iter().enumerate().map(|(n, i)| (i.session.id.as_str(), n)).collect();
    let parent_of = |n: usize| lineage_of(&items[n].session.listing).and_then(|(p, kind)| at.get(p.as_str()).map(|&pn| (pn, kind)));
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut top = Vec::new();
    for n in 0..items.len() {
        match parent_of(n) {
            Some((parent, _)) if parent != n => children.entry(parent).or_default().push(n),
            _ => top.push(n),
        }
    }
    let mut rows = Vec::new();
    let mut seen = vec![false; items.len()];
    fn walk(
        n: usize,
        depth: usize,
        kind: Option<LineageKind>,
        items: &[Item],
        children: &HashMap<usize, Vec<usize>>,
        seen: &mut [bool],
        rows: &mut Vec<TreeRow>,
    ) {
        if std::mem::replace(&mut seen[n], true) {
            return;
        }
        rows.push(TreeRow { index: items[n].index, depth, kind });
        for &child in children.get(&n).into_iter().flatten() {
            let kind = lineage_of(&items[child].session.listing).map(|(_, k)| k);
            walk(child, depth + 1, kind, items, children, seen, rows);
        }
    }
    for n in top {
        walk(n, 0, None, items, &children, &mut seen, &mut rows);
    }
    rows
}

fn group_of(id: String, name: String, pinned: bool, items: Vec<Item>) -> TreeGroup {
    TreeGroup { id, name, pinned, count: items.len(), rows: rows_of(&items) }
}

/// The tree for `sessions` with the filter `query` (lower case, trimmed; empty for none). A machine's sessions are the last
/// list it returned, so archived and sub-agent rows aren't in it (the read drops them).
pub fn build(sessions: &[Session], projects: &[ProjectInfo], query: &str) -> Tree {
    build_hiding(sessions, projects, query, |_| false)
}

/// `build` without the sessions `hidden` names: the ones an archive is holding back (`pendingIds`). Rows keep their place in
/// `sessions`, so what hides leaves the others' indexes alone.
pub fn build_hiding(sessions: &[Session], projects: &[ProjectInfo], query: &str, hidden: impl Fn(&Session) -> bool) -> Tree {
    let items: Vec<Item> =
        sessions.iter().enumerate().filter(|(_, session)| !hidden(session)).map(|(index, session)| Item { index, session }).collect();
    let matches = |i: &Item| session_matches_query(i, query, projects);
    let (pinned, rest) = split_pinned(items);
    let pinned: Vec<Item> = if query.is_empty() { pinned } else { pinned.into_iter().filter(&matches).collect() };
    let mut groups = Vec::new();
    if !pinned.is_empty() {
        let g = pinned_project_group(pinned);
        groups.push(group_of(g.id, g.name, true, g.sessions));
    }
    let tree = build_project_groups(rest, projects, None);
    let tree = if query.is_empty() { tree } else { filter_project_groups(tree, query, matches) };
    for g in tree {
        groups.push(group_of(g.id, g.name, false, g.sessions));
    }
    let count = groups.iter().map(|g| g.count).sum();
    Tree { groups, count }
}
