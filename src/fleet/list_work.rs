//! What runs in every session, for the list's sub-agent rows (`useRunningWorkAcrossSessions`): read from `GET /api/work/running`
//! and kept current by the `work.*` events on the `sessions` topic. Only running items are kept; an ended one leaves.
use super::events::Event;
use super::work::{RunningWorkItem, is_running, to_items};
use crate::State;
use std::collections::HashMap;

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

    /// Whether anything runs: a list with nothing running needs no read to stay right.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl State {
    /// Reads what runs in every session. Only the newest read counts. Not asked of a list with no server.
    pub fn load_list_work(&mut self) {
        self.list_work.load += 1;
        let load = self.list_work.load;
        let Some(link) = &self.link else { return };
        link.off_thread(
            |base| super::http::get(base, "/api/work/running"),
            move |s, _, result| {
                let Ok(body) = result else { return };
                if s.list_work.load != load {
                    return;
                }
                s.list_work.items.clear();
                for item in to_items(Some(&body)).into_iter().filter(is_running) {
                    s.list_work.items.insert(item.id.clone(), item);
                }
            },
        );
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
