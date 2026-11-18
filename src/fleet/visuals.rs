//! The conversation's visuals and what the app does with them (`ActivityStream.vue`'s "Visual canvases" watch, and the tool
//! card's Expand): every one is offered in the + menu's "From this conversation"; one that arrives while the conversation is
//! open also opens as a tab, once per call. Fleet's watch runs in the stream; here `keep_visuals` runs every frame.
use super::canvases::VisualPayload;
use crate::State;
use crate::conv::visual::{is_live, visual_refs};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct Visuals {
    /// When each session's conversation was first seen open, in milliseconds (`mountedAt`).
    mounted: HashMap<String, i64>,
    /// Calls whose visual has been given its one chance to open (`autoOpenedToolIds`).
    seen: HashSet<(String, String)>,
    /// What was last told to the canvas model, by session: the call, and the length of its content, so a frame with nothing new
    /// does no work.
    told: HashMap<String, Vec<(String, usize)>>,
}

impl Visuals {
    /// When the session's conversation was first seen open here.
    pub fn mounted_at(&self, session: &str) -> Option<i64> {
        self.mounted.get(session).copied()
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl State {
    /// From `poll_rest`, every frame.
    pub fn keep_visuals(&mut self) {
        self.watch_visuals(now_ms());
    }

    /// The watch, with the clock given so a test can set it.
    pub fn watch_visuals(&mut self, now: i64) {
        let open = self.canvas_session();
        if self.open_id().is_some_and(|id| self.session().id != id) {
            return;
        }
        let mounted = *self.visuals.mounted.entry(open.clone()).or_insert(now);
        let keys: Vec<(String, usize)> =
            visual_refs(&self.session().messages).map(|(id, _, v)| (id.to_string(), v.content.len())).collect();
        if self.visuals.told.get(&open) == Some(&keys) {
            return;
        }
        let all: Vec<(String, Option<i64>, VisualPayload)> =
            visual_refs(&self.session().messages).map(|(id, at, v)| (id.to_string(), at, v.clone())).collect();
        self.visuals.told.insert(open.clone(), keys);
        self.canvases.set_known_visuals(&open, all.iter().map(|(_, _, v)| v.clone()).collect());
        for (tool, created_at, payload) in all {
            if self.visuals.seen.insert((open.clone(), tool)) && is_live(created_at, mounted) {
                self.canvases.open_visual(&open, payload, true);
            }
        }
        self.sync_review();
    }

    /// The tool card's Expand: the visual as a tab, in front, and the panel shows it (`handleExpandVisual`).
    pub fn expand_visual(&mut self, payload: VisualPayload) {
        self.edit_canvases(|c, id| c.open_visual(id, payload, true));
        self.set_content_open(true);
    }
}
