//! Which rows the live machine's list shows, and in which group (`SessionsPanel.vue`: `pinnedSplit`, `filteredPinnedSessions`,
//! `filteredProjectGroups`). Pure reading of `State`, shared by the view that draws them and by the selection, whose Shift range
//! and pruning need the rows in the order they are on screen. Rows are positions `(project, session)` in `State::projects`.
use super::groups::{Listed, Listing, matching_sessions, session_matches_query, split_pinned};
use crate::State;
use crate::mock::Session;

/// A session of the list with where it is, for the grouping functions.
struct At<'a>(usize, usize, &'a Session);

impl Listed for At<'_> {
    fn id(&self) -> &str {
        self.2.id()
    }
    fn title(&self) -> &str {
        self.2.title()
    }
    fn status_word(&self) -> &str {
        self.2.status_word()
    }
    fn listing(&self) -> &Listing {
        self.2.listing()
    }
}

/// One project's rows after the pinned ones left and the filter was applied.
pub struct Group {
    pub p: usize,
    pub rows: Vec<usize>,
}

pub struct HomeLayout {
    /// The Pinned group's rows: each pinned session in its pinned order, then what came from one.
    pub pinned: Vec<(usize, usize)>,
    /// The project groups that show (a project whose name matches the filter keeps all its rows).
    pub groups: Vec<Group>,
}

impl HomeLayout {
    /// What the live machine's heading counts: Pinned and every group's sessions, after the filter.
    pub fn shown(&self) -> usize {
        self.pinned.len() + self.groups.iter().map(|g| g.rows.len()).sum::<usize>()
    }
}

/// The id of each row on screen, top to bottom: Pinned, then each project's rows, for groups that are open and only while the
/// live machine's tree is (`SP` `watch(... selection.setVisibleOrder)`, row 1.50).
pub fn visible_ids(s: &State, layout: &HomeLayout) -> Vec<String> {
    let id_of = |p: usize, i: usize| s.projects[p].sessions[i].id.clone();
    let mut ids = Vec::new();
    if s.home_tree_open() {
        if s.home_pinned_open() {
            ids.extend(layout.pinned.iter().map(|&(p, i)| id_of(p, i)));
        }
        for g in layout.groups.iter().filter(|g| s.projects[g.p].open) {
            ids.extend(g.rows.iter().map(|&i| id_of(g.p, i)));
        }
    }
    ids
}

impl State {
    pub fn home_layout(&self) -> HomeLayout {
        let needle = self.filter.text.trim().to_lowercase();
        let infos = &self.project_load.projects;
        let all: Vec<At> = self
            .projects
            .iter()
            .enumerate()
            .flat_map(|(p, project)| project.sessions.iter().enumerate().map(move |(i, session)| At(p, i, session)))
            .filter(|at| !self.hides(at.2))
            .collect();
        let (pinned, rest) = split_pinned(all);
        // The Pinned group has no project name to match: each session is tested alone.
        let pinned: Vec<(usize, usize)> =
            pinned.into_iter().filter(|at| needle.is_empty() || session_matches_query(at, &needle, infos)).map(|at| (at.0, at.1)).collect();
        let rest: std::collections::HashSet<(usize, usize)> = rest.iter().map(|at| (at.0, at.1)).collect();
        let mut groups = Vec::new();
        for (p, project) in self.projects.iter().enumerate() {
            let matches = |session: &Session| session_matches_query(session, &needle, infos);
            let Some(kept) = matching_sessions(&project.name, &project.sessions, &needle, matches) else { continue };
            let rows: Vec<usize> = kept.into_iter().filter(|&i| rest.contains(&(p, i))).collect();
            groups.push(Group { p, rows });
        }
        HomeLayout { pinned, groups }
    }

    /// Whether the live machine's tree shows (a machine heading above it may fold it).
    pub fn home_tree_open(&self) -> bool {
        !self.machines.has_machines() || self.machine_is_open(super::settings::HOME_MACHINE_KEY)
    }

    /// The Pinned group's fold: kept under `project:home:pinned`, as Fleet's is.
    pub fn home_pinned_open(&self) -> bool {
        self.machine_project_is_open(super::settings::HOME_MACHINE_KEY, super::groups::PINNED_GROUP_ID)
    }

    pub fn toggle_home_pinned(&mut self) {
        self.toggle_machine_project(super::settings::HOME_MACHINE_KEY, super::groups::PINNED_GROUP_ID);
    }
}
