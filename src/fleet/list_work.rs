//! What runs in every session, for the list's sub-agent rows (`useRunningWorkAcrossSessions`): read from `GET /api/work/running`
//! and kept current by the `work.*` events on the `sessions` topic. Only running items are kept; an ended one leaves.
use super::events::Event;
use super::work::{RunningWorkItem, is_running, sort, to_items};
use crate::State;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// One session's running work (`RunningWorkGroup`).
#[derive(Clone, Debug, PartialEq)]
pub struct WorkGroup {
    pub session_id: String,
    pub items: Vec<RunningWorkItem>,
}

/// A group with what the popover's header says about its session (`orderedGroups`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedGroup {
    pub group: WorkGroup,
    pub title: String,
    /// `this session`, else the workspace's name or the project's; none when the list knows neither.
    pub place: Option<String>,
    pub instance_id: Option<String>,
}

#[derive(Default)]
pub struct ListWork {
    items: HashMap<String, RunningWorkItem>,
    /// Bumped by each read, so a slow answer can't overwrite a newer one (`acrossLoadId`).
    load: u64,
}

impl ListWork {
    /// `applyToAcross`: a running item is kept (replacing its older self), anything else leaves.
    pub fn apply(&mut self, item: RunningWorkItem) {
        if is_running(&item) {
            self.items.insert(item.id.clone(), item);
        } else {
            self.items.remove(&item.id);
        }
    }

    pub fn running(&self) -> impl Iterator<Item = &RunningWorkItem> {
        self.items.values()
    }

    pub fn get(&self, id: &str) -> Option<&RunningWorkItem> {
        self.items.get(id)
    }

    /// `running`: everything running, in every session, oldest start first, then by id.
    pub fn running_sorted(&self) -> Vec<RunningWorkItem> {
        let mut items: Vec<RunningWorkItem> = self.items.values().cloned().collect();
        sort(&mut items);
        items
    }

    /// `groups`: the same by session, in the order each session's oldest item started.
    pub fn groups(&self) -> Vec<WorkGroup> {
        let mut groups: Vec<WorkGroup> = Vec::new();
        for item in self.running_sorted() {
            match groups.iter_mut().find(|g| g.session_id == item.session_id) {
                Some(group) => group.items.push(item),
                None => groups.push(WorkGroup { session_id: item.session_id.clone(), items: vec![item] }),
            }
        }
        groups
    }

    /// `sessionCount`: how many sessions have work running.
    pub fn session_count(&self) -> usize {
        self.items.values().map(|i| i.session_id.as_str()).collect::<HashSet<_>>().len()
    }

    /// The counter's words: `5 running in 2 sessions`, `1 running in 1 session`.
    pub fn label(&self) -> String {
        let sessions = self.session_count();
        let place = if sessions == 1 { "1 session".to_string() } else { format!("{sessions} sessions") };
        format!("{} running in {place}", self.items.len())
    }

    /// Whether anything runs: a list with nothing running needs no read to stay right.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl State {
    /// `orderedGroups`: the groups with the open session's first (the rest keep their order), each with its session's title
    /// and place from the list. A session the list doesn't hold is "Untitled session" with no place.
    pub fn placed_work_groups(&self) -> Vec<PlacedGroup> {
        let active = self.open_id();
        let mut groups = self.list_work.groups();
        groups.sort_by_key(|g| Some(&g.session_id) != active.as_ref());
        groups
            .into_iter()
            .map(|group| {
                let session = self.row_of(&group.session_id).map(|(p, i)| &self.projects[p].sessions[i]);
                let listing = session.map(|s| &s.listing);
                let place = if Some(&group.session_id) == active.as_ref() {
                    Some("this session".to_string())
                } else {
                    listing.and_then(|l| l.workspace_display_name.clone().or_else(|| l.project_name.clone()))
                };
                let title = session.map(|s| s.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Untitled session".into());
                PlacedGroup { title, place, instance_id: listing.and_then(|l| l.instance_id.clone()), group }
            })
            .collect()
    }

    /// Reads what runs in every session. Only the newest read counts. Not asked of a list with no server.
    pub fn load_list_work(&mut self) {
        self.list_work.load += 1;
        let load = self.list_work.load;
        let Some(link) = &self.link else { return };
        link.off_thread(
            |base| super::http::get(base, "/api/work/running"),
            move |s, _, result| {
                if let Ok(body) = result {
                    s.on_list_work_loaded(load, &body);
                }
            },
        );
    }

    /// The number of the newest read asked for.
    pub fn list_work_load(&self) -> u64 {
        self.list_work.load
    }

    /// An answer of the read: replaces the list, unless a newer read has been asked since.
    pub fn on_list_work_loaded(&mut self, load: u64, body: &Value) {
        if self.list_work.load != load {
            return;
        }
        self.list_work.items.clear();
        for item in to_items(Some(body)).into_iter().filter(is_running) {
            self.list_work.items.insert(item.id.clone(), item);
        }
    }

    /// `work.started`, `.updated` and `.ended` on any topic.
    pub fn on_list_work_event(&mut self, event: &Event) {
        if let Event::Work { item, .. } = event
            && let Some(item) = super::work::to_item(item)
        {
            self.list_work.apply(item);
        }
    }
}
