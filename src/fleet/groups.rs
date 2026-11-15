//! How the list groups sessions: by project in the projects' own order, Ungrouped last, with the Pinned
//! group split off and the filter's matching. Pure data, no Rune, no I/O. Ports `lib/session-project-groups.ts`,
//! `lib/session-pins.ts` and `composables/use-projects.ts` (the load flags).
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// The Pinned group's id, for folding it like a project (`PINNED_GROUP_ID`).
pub const PINNED_GROUP_ID: &str = "pinned";
/// The id of the group for sessions with no project.
pub const UNGROUPED_ID: &str = "ungrouped";
/// The name of that group.
pub const UNGROUPED_NAME: &str = "Ungrouped";

/// `ProjectResponse.type`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ProjectKind {
    #[default]
    User,
    Scratch,
}

/// What a group needs to know about a project (`ProjectSummary`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub kind: ProjectKind,
    pub position: i64,
}

/// `GET /api/projects`: an array of `ProjectResponse`. A row with no id is dropped; one with no name is called "".
pub fn parse_projects(body: &Value) -> Vec<ProjectInfo> {
    body.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let text = |k: &str| p.get(k).and_then(Value::as_str);
            Some(ProjectInfo {
                id: text("id")?.to_string(),
                name: text("name").unwrap_or("").to_string(),
                kind: if text("type") == Some("scratch") { ProjectKind::Scratch } else { ProjectKind::User },
                position: p.get("position").and_then(Value::as_f64).map_or(0, |n| n as i64),
            })
        })
        .collect()
}

/// What a list row says about where it belongs: its project, its place among the pins and what it came from.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Listing {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub workspace_directory: String,
    pub pin_order: Option<f64>,
    pub forked_from: Option<String>,
    pub spawned_by: Option<String>,
    /// `spawnKind`: "fork" decides which parent wins when a session is both a fork and started by an agent.
    pub spawn_kind: Option<String>,
    pub parent: Option<String>,
    /// `lineageDetachedAt` is set: the user moved the session out of its parent.
    pub detached: bool,
    /// `workflowRunId`: the steps of one run move between projects together.
    pub workflow_run_id: Option<String>,
}

impl Listing {
    pub fn from_row(row: &Value) -> Listing {
        let text = |k: &str| row.get(k).and_then(Value::as_str).map(str::to_string);
        Listing {
            project_id: text("projectId"),
            project_name: text("projectName"),
            workspace_directory: text("workspaceDirectory").unwrap_or_default(),
            pin_order: row.get("pinOrder").and_then(Value::as_f64),
            forked_from: text("forkedFromSessionId"),
            spawned_by: text("spawnedBySessionId"),
            spawn_kind: text("spawnKind"),
            parent: text("parentSessionId"),
            detached: row.get("lineageDetachedAt").is_some_and(|v| !v.is_null()),
            workflow_run_id: text("workflowRunId"),
        }
    }
}

/// A row the list can group, pin and filter.
pub trait Listed {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    /// `sessionStatus`, the raw word (`idle`, `active`, `waiting_input`, `stopped`...).
    fn status_word(&self) -> &str;
    fn listing(&self) -> &Listing;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineageKind {
    Subagent,
    Fork,
    Started,
}

/// Where a session came from, whether or not the user has moved it out since (`lineageOriginOf`).
pub fn lineage_origin_of(l: &Listing) -> Option<(String, LineageKind)> {
    if let Some(parent) = &l.parent {
        return Some((parent.clone(), LineageKind::Subagent));
    }
    let fork = l.forked_from.clone().map(|id| (id, LineageKind::Fork));
    let started = l.spawned_by.clone().map(|id| (id, LineageKind::Started));
    if l.spawn_kind.as_deref() == Some("fork") { fork.or(started) } else { started.or(fork) }
}

/// Where a session came from, or `None` when the user started it, or moved it out of its parent (`lineageOf`).
pub fn lineage_of(l: &Listing) -> Option<(String, LineageKind)> {
    lineage_origin_of(l).filter(|(_, kind)| *kind == LineageKind::Subagent || !l.detached)
}

/// A group of the list, with the sessions that are in it (`ProjectTreeGroup`).
#[derive(Clone, PartialEq, Debug)]
pub struct ProjectTreeGroup<T> {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub is_ungrouped: bool,
    pub can_move_up: bool,
    pub can_move_down: bool,
    pub move_up_targets: Vec<ReorderTarget>,
    pub move_down_targets: Vec<ReorderTarget>,
    pub session_count: usize,
    pub sessions: Vec<T>,
}

/// Where a project goes when the order changes (`ProjectReorderTarget`); `position` counts from 1.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReorderTarget {
    pub project_id: String,
    pub position: usize,
}

fn project_named<'a>(projects: &'a [ProjectInfo], id: &str) -> Option<&'a ProjectInfo> {
    projects.iter().find(|p| p.id == id)
}

/// The name a session's group has (`projectDisplayName`).
pub fn project_display_name(l: &Listing, projects: &[ProjectInfo]) -> String {
    let Some(id) = l.project_id.as_deref().filter(|id| !id.is_empty()) else { return UNGROUPED_NAME.into() };
    if let Some(name) = l.project_name.as_deref().filter(|n| !n.trim().is_empty()) {
        return name.to_string();
    }
    project_named(projects, id).map_or(UNGROUPED_NAME.into(), |p| p.name.clone())
}

/// The group a new-session draft's session will land in: its project, or Scratch (where the server puts a session without
/// one), keyed the way `build_project_groups` keys it (`draftGroupKeyFor`).
pub fn draft_group_key_for(project_id: Option<&str>, projects: &[ProjectInfo]) -> String {
    if let Some(id) = project_id.filter(|id| projects.iter().any(|p| p.id == *id)) {
        return id.to_string();
    }
    projects.iter().find(|p| p.kind == ProjectKind::Scratch).map_or("Ungrouped".into(), |p| p.id.clone())
}

/// The id of the group a draft row shows in (`draftGroupIdFor`).
pub fn draft_group_id_for(key: Option<&str>, projects: &[ProjectInfo]) -> Option<String> {
    key.map(|k| if projects.iter().any(|p| p.id == k) { k.to_string() } else { UNGROUPED_ID.to_string() })
}

fn reorder_targets(order: &[&Draft]) -> Vec<ReorderTarget> {
    order
        .iter()
        .enumerate()
        .filter_map(|(i, g)| g.project_id.clone().map(|project_id| ReorderTarget { project_id, position: i + 1 }))
        .collect()
}

struct Draft {
    id: String,
    project_id: Option<String>,
    name: String,
    sort_position: i64,
    is_ungrouped: bool,
}

/// `localeCompare` for names: case-insensitive first, then the plain order so two names never tie.
fn compare_names(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// The sessions (the ones outside the Pinned group) in their projects. Every user project shows, even an empty one; Scratch
/// only while it has sessions, or while the draft (`draft_key`) will land in it (`buildProjectGroups`).
pub fn build_project_groups<T: Listed>(sessions: Vec<T>, projects: &[ProjectInfo], draft_key: Option<&str>) -> Vec<ProjectTreeGroup<T>> {
    // The groups in the order they were made, by key; `known` says which keys exist.
    let mut made: Vec<(String, Draft)> = Vec::new();
    let mut known: HashSet<String> = HashSet::new();
    let mut members: HashMap<String, Vec<T>> = HashMap::new();
    for p in projects.iter().filter(|p| p.kind != ProjectKind::Scratch) {
        let head = Draft {
            id: p.id.clone(),
            project_id: Some(p.id.clone()),
            name: p.name.clone(),
            sort_position: p.position,
            is_ungrouped: false,
        };
        known.insert(p.id.clone());
        made.push((p.id.clone(), head));
    }
    for session in sessions {
        let l = session.listing();
        let project = l.project_id.as_deref().and_then(|id| project_named(projects, id));
        let name = project_display_name(l, projects);
        let key = l.project_id.clone().unwrap_or_else(|| name.clone());
        if known.insert(key.clone()) {
            let head = Draft {
                id: l.project_id.clone().unwrap_or_else(|| UNGROUPED_ID.into()),
                project_id: l.project_id.clone(),
                sort_position: project.map_or(i64::MAX, |p| p.position),
                is_ungrouped: name == UNGROUPED_NAME,
                name,
            };
            made.push((key.clone(), head));
        }
        members.entry(key).or_default().push(session);
    }
    // The draft's group shows even before it has a session (Scratch, the first time).
    if let Some(key) = draft_key.filter(|k| !known.contains(*k)) {
        let project = project_named(projects, key);
        let head = Draft {
            id: project.map_or(UNGROUPED_ID.into(), |p| p.id.clone()),
            project_id: project.map(|p| p.id.clone()),
            name: project.map_or(UNGROUPED_NAME.into(), |p| p.name.clone()),
            sort_position: project.map_or(i64::MAX, |p| p.position),
            is_ungrouped: project.is_none(),
        };
        made.push((key.to_string(), head));
    }

    let mut sorted = made;
    // The comparator of the original, which puts Ungrouped last whichever side it is on.
    sorted.sort_by(|(_, l), (_, r)| {
        if l.is_ungrouped {
            Ordering::Greater
        } else if r.is_ungrouped {
            Ordering::Less
        } else {
            l.sort_position.cmp(&r.sort_position).then_with(|| compare_names(&l.name, &r.name))
        }
    });
    let ordered: Vec<&Draft> = sorted.iter().map(|(_, d)| d).filter(|d| !d.is_ungrouped).collect();
    let swapped = |a: usize, b: usize| {
        let mut next = ordered.clone();
        next.swap(a, b);
        reorder_targets(&next)
    };
    let moves: Vec<_> = sorted
        .iter()
        .map(|(_, head)| {
            let at = ordered.iter().position(|d| d.id == head.id);
            let up = at.filter(|&i| i > 0).map(|i| swapped(i, i - 1));
            let down = at.filter(|&i| i + 1 < ordered.len()).map(|i| swapped(i, i + 1));
            (up, down)
        })
        .collect();
    sorted
        .into_iter()
        .zip(moves)
        .map(|((key, head), (up, down))| {
            let (can_move_up, can_move_down) = (up.is_some(), down.is_some());
            let sessions = members.remove(&key).unwrap_or_default();
            ProjectTreeGroup {
                id: head.id,
                project_id: head.project_id,
                name: head.name,
                is_ungrouped: head.is_ungrouped,
                can_move_up,
                can_move_down,
                move_up_targets: up.unwrap_or_default(),
                move_down_targets: down.unwrap_or_default(),
                session_count: sessions.len(),
                sessions,
            }
        })
        .collect()
}

/// Whether `item` matches the filter's lower-case `query`: its title, id, project or status (`sessionMatchesQuery`).
pub fn session_matches_query<T: Listed>(item: &T, query: &str, projects: &[ProjectInfo]) -> bool {
    let searchable =
        [item.title(), item.id(), &project_display_name(item.listing(), projects), item.status_word()].join(" ").to_lowercase();
    searchable.contains(query)
}

/// Which of a group's sessions the filter leaves, by position: all of them when the project's own name matches, else the
/// matches; `None` when the group goes (no match and a name that doesn't match either).
pub fn matching_sessions<T>(name: &str, sessions: &[T], query: &str, matches: impl Fn(&T) -> bool) -> Option<Vec<usize>> {
    let by_name = name.to_lowercase().contains(query);
    let kept: Vec<usize> =
        if by_name { (0..sessions.len()).collect() } else { (0..sessions.len()).filter(|&i| matches(&sessions[i])).collect() };
    (!kept.is_empty() || by_name).then_some(kept)
}

/// The groups the filter leaves: a project whose name matches keeps all its sessions, the rest only the matches
/// (`filterProjectGroups`).
pub fn filter_project_groups<T>(groups: Vec<ProjectTreeGroup<T>>, query: &str, matches: impl Fn(&T) -> bool) -> Vec<ProjectTreeGroup<T>> {
    groups
        .into_iter()
        .filter_map(|mut group| {
            let kept: HashSet<usize> = matching_sessions(&group.name, &group.sessions, query, &matches)?.into_iter().collect();
            let mut at = 0;
            group.sessions.retain(|_| {
                at += 1;
                kept.contains(&(at - 1))
            });
            group.session_count = group.sessions.len();
            Some(group)
        })
        .collect()
}

/// A machine's Pinned group, shaped like a project (`pinnedProjectGroup`).
pub fn pinned_project_group<T>(sessions: Vec<T>) -> ProjectTreeGroup<T> {
    ProjectTreeGroup {
        id: PINNED_GROUP_ID.into(),
        project_id: None,
        name: "Pinned".into(),
        is_ungrouped: false,
        can_move_up: false,
        can_move_down: false,
        move_up_targets: vec![],
        move_down_targets: vec![],
        session_count: sessions.len(),
        sessions,
    }
}

pub fn is_pinned<T: Listed>(item: &T) -> bool {
    item.listing().pin_order.is_some()
}

fn pinned_in_order<'a, T: Listed + 'a>(items: impl IntoIterator<Item = &'a T>) -> Vec<&'a T> {
    let mut pinned: Vec<&T> = items.into_iter().filter(|i| is_pinned(*i)).collect();
    pinned.sort_by(|a, b| a.listing().pin_order.partial_cmp(&b.listing().pin_order).unwrap_or(Ordering::Equal));
    pinned
}

/// Splits the list into the Pinned group and the rest: each pinned session in its pinned order, then everything that came
/// from one (in the list's order, to nest under it). A pinned session that came from one that isn't pinned stands on its
/// own there (`splitPinned`).
pub fn split_pinned<T: Listed>(items: Vec<T>) -> (Vec<T>, Vec<T>) {
    let by_id: HashMap<&str, usize> = items.iter().enumerate().map(|(i, item)| (item.id(), i)).collect();
    let in_pinned = |start: usize| {
        let mut seen: HashSet<usize> = HashSet::new();
        let mut current = Some(start);
        while let Some(at) = current.filter(|at| !seen.contains(at)) {
            if is_pinned(&items[at]) {
                return true;
            }
            seen.insert(at);
            current = lineage_of(items[at].listing()).and_then(|(parent, _)| by_id.get(parent.as_str()).copied());
        }
        false
    };
    let flags: Vec<bool> = (0..items.len()).map(in_pinned).collect();
    let (mut pinned, mut rest) = (Vec::new(), Vec::new());
    for (item, flag) in items.into_iter().zip(flags) {
        if flag { pinned.push(item) } else { rest.push(item) }
    }
    // The pinned ones by their order, then what came from them in the list's order.
    let (mut own, from): (Vec<T>, Vec<T>) = pinned.into_iter().partition(is_pinned);
    own.sort_by(|a, b| a.listing().pin_order.partial_cmp(&b.listing().pin_order).unwrap_or(Ordering::Equal));
    own.extend(from);
    (own, rest)
}

/// The order a session gets when it's pinned just before `before`, or at the end when that's `None` or isn't pinned: what
/// the server answers too, so the list can move at once (`pinOrderFor`).
pub fn pin_order_for<T: Listed>(items: &[T], session: &str, before: Option<&str>) -> f64 {
    let others: Vec<&T> = pinned_in_order(items.iter().filter(|i| i.id() != session));
    let order = |t: &&T| t.listing().pin_order.unwrap_or(0.0);
    let at = before.and_then(|b| others.iter().position(|i| i.id() == b));
    match at {
        None => others.last().map_or(0.0, order) + 1.0,
        Some(at) => {
            let after = if at == 0 { order(&others[0]) - 1.0 } else { order(&others[at - 1]) };
            (after + order(&others[at])) / 2.0
        }
    }
}

/// Where a pinned session goes when it moves one place up (-1) or down (+1): the pinned session it then sits before, or
/// `Some(None)` for the end. `None` when it can't move that way (`pinnedNeighbourFor`).
pub fn pinned_neighbour_for<T: Listed>(items: &[T], session: &str, delta: i32) -> Option<Option<String>> {
    let ids: Vec<&str> = pinned_in_order(items).into_iter().map(|i| i.id()).collect();
    let index = ids.iter().position(|id| *id == session)? as i32;
    let target = index + delta;
    if target < 0 || target >= ids.len() as i32 {
        return None;
    }
    // Moving up sits before the one above; moving down sits before the one after the next (or at the end).
    Some(if delta < 0 { Some(ids[target as usize].to_string()) } else { ids.get(target as usize + 1).map(|id| id.to_string()) })
}

/// The load flags of the projects read (`useProjects`): the first fetch with nothing held is "loading", later ones are
/// "refreshing", a reply to an overtaken request is dropped, and the first failure sets `error`.
#[derive(Default, Debug)]
pub struct ProjectsLoad {
    pub projects: Vec<ProjectInfo>,
    pub is_loading: bool,
    pub is_refreshing: bool,
    pub error: Option<String>,
    enabled: bool,
    request: u64,
    loaded_once: bool,
}

impl ProjectsLoad {
    pub fn new(enabled: bool) -> Self {
        ProjectsLoad { enabled, ..Default::default() }
    }

    /// `enabled` changed. Returns whether a fetch should start now (it turned on).
    pub fn set_enabled(&mut self, enabled: bool) -> bool {
        let turned_on = enabled && !self.enabled;
        self.enabled = enabled;
        if !enabled {
            self.is_loading = false;
            self.is_refreshing = false;
        }
        turned_on
    }

    /// A fetch starts: the id its reply must come back with, or `None` while disabled.
    pub fn begin(&mut self) -> Option<u64> {
        if !self.enabled {
            self.is_loading = false;
            self.is_refreshing = false;
            return None;
        }
        self.request += 1;
        if !self.loaded_once && self.projects.is_empty() {
            self.is_loading = true;
        } else {
            self.is_refreshing = true;
        }
        Some(self.request)
    }

    /// A reply. One for a request that a newer one has overtaken is dropped.
    pub fn finish(&mut self, request: u64, result: Result<Vec<ProjectInfo>, String>) {
        if request != self.request {
            return;
        }
        match result {
            Ok(projects) => {
                self.projects = projects;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
        self.loaded_once = true;
        self.is_loading = false;
        self.is_refreshing = false;
    }
}
