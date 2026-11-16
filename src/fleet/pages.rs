//! The page canvases of each session, as far as the page-pick card needs them (component 4, piece F; rows 6.1, 6.4 and 6.5 of
//! `docs/parity/04-questions-and-permissions.md`). Component 14 owns pages: when its canvas store exists, `Pages` is replaced
//! by a read of it and `waiting_page` is the one function the card calls. Ports `lib/page-choice.ts` (`readPageChoice`,
//! `isChoiceLive`), the page half of `lib/server-canvas.ts` and `composables/use-waiting-page.ts`.
//!
//! They load when a session opens and after a reconnect (`GET /api/sessions/{id}/canvases`); `canvas.updated` carries a whole
//! canvas and `canvas.closed` removes one. (`session.pages` is only a count for the session row, so it plays no part here.)
use super::events::{Event, Kind};
use super::http;
use crate::State;
use serde_json::Value;
use std::collections::HashMap;

/// The pick a page waits for, as the canvas's state holds it (`PageChoice`).
#[derive(Clone, Debug, PartialEq)]
pub struct PageChoice {
    pub call_id: String,
    pub status: ChoiceStatus,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceStatus {
    Waiting,
    Answered,
    NotAnswered,
}

/// The choice in a page canvas's state, or `None` for anything malformed (`readPageChoice`).
pub fn read_page_choice(value: &Value) -> Option<PageChoice> {
    let record = value.as_object()?;
    let call_id = record.get("callId").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    record.get("at").and_then(Value::as_str)?;
    let status = match record.get("status").and_then(Value::as_str)? {
        "waiting" => ChoiceStatus::Waiting,
        "answered" => ChoiceStatus::Answered,
        "not-answered" => ChoiceStatus::NotAnswered,
        _ => return None,
    };
    let reason = record.get("reason").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    Some(PageChoice { call_id: call_id.to_string(), status, reason })
}

/// A waiting choice is live while the session is in a turn; a Fleet restart leaves "waiting" behind with the session idle
/// (`isChoiceLive`).
pub fn is_choice_live(choice: Option<&PageChoice>, activity: Option<&str>) -> bool {
    choice.is_some_and(|c| c.status == ChoiceStatus::Waiting) && activity.is_some_and(|a| !a.is_empty() && a != "idle")
}

/// One page canvas: its id, the tab's title, and its choice. `version` orders an event against a load.
#[derive(Clone, Debug, PartialEq)]
pub struct PageCanvas {
    pub id: String,
    pub title: String,
    pub version: i64,
    pub choice: Option<PageChoice>,
}

/// `toServerCanvasInstance` for a page; any other kind (a browser canvas is component 14's) is `None`.
pub fn read_page_canvas(item: &Value) -> Option<PageCanvas> {
    if item.get("kind").and_then(Value::as_str) != Some("page") {
        return None;
    }
    let id = item.get("canvasId").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    Some(PageCanvas {
        id: id.to_string(),
        title: item.get("title").and_then(Value::as_str).unwrap_or_default().to_string(),
        version: item.get("version").and_then(Value::as_i64).unwrap_or(0),
        choice: item.get("state").and_then(|s| s.get("choice")).and_then(read_page_choice),
    })
}

/// Each session's page canvases, oldest first, and the number of the newest load or close per session: a load asked for before
/// a close arrived is older than it.
#[derive(Debug, Default)]
pub struct Pages {
    by_session: HashMap<String, Vec<PageCanvas>>,
    generation: HashMap<String, u64>,
}

impl Pages {
    pub fn of(&self, session: &str) -> &[PageCanvas] {
        self.by_session.get(session).map_or(&[], Vec::as_slice)
    }

    pub fn begin_load(&mut self, session: &str) -> u64 {
        let n = self.generation.entry(session.to_string()).or_default();
        *n += 1;
        *n
    }

    /// A load's answer (`setServerCanvases`): the list replaces the held one, but a canvas an event made newer stays as the
    /// event left it. Dropped when a close came since it was asked for.
    pub fn loaded(&mut self, session: &str, generation: u64, list: Vec<PageCanvas>) {
        if self.generation.get(session) != Some(&generation) {
            return;
        }
        let held = self.by_session.entry(session.to_string()).or_default();
        let next = list
            .into_iter()
            .map(|loaded| match held.iter().find(|h| h.id == loaded.id) {
                Some(kept) if kept.version > loaded.version => kept.clone(),
                _ => loaded,
            })
            .collect();
        *held = next;
    }

    /// `canvas.updated`: added or redrawn in place; never back to an older version.
    pub fn updated(&mut self, session: &str, canvas: PageCanvas) {
        let held = self.by_session.entry(session.to_string()).or_default();
        match held.iter_mut().find(|h| h.id == canvas.id) {
            Some(existing) if existing.version > canvas.version => {}
            Some(existing) => *existing = canvas,
            None => held.push(canvas),
        }
    }

    /// `canvas.closed`.
    pub fn closed(&mut self, session: &str, canvas: &str) {
        *self.generation.entry(session.to_string()).or_default() += 1;
        if let Some(held) = self.by_session.get_mut(session) {
            held.retain(|c| c.id != canvas);
        }
    }

    /// For a scene: the session has exactly these.
    pub fn set(&mut self, session: &str, canvases: Vec<PageCanvas>) {
        self.by_session.insert(session.to_string(), canvases);
    }
}

impl State {
    /// What the open session is doing, in the words of `activityStatus`.
    fn open_activity(&self) -> &'static str {
        match self.session().status {
            crate::mock::Status::Working => "busy",
            crate::mock::Status::NeedsInput => "waiting_input",
            crate::mock::Status::Idle => "idle",
        }
    }

    /// The page the agent waits on for a pick, while the open session is stopped on you (`useWaitingPage`).
    pub fn waiting_page(&self) -> Option<&PageCanvas> {
        let id = self.draft_owner()?;
        if self.open_activity() != "waiting_input" {
            return None;
        }
        self.pages
            .of(&id)
            .iter()
            .find(|c| c.choice.as_ref().is_some_and(|choice| choice.status == ChoiceStatus::Waiting))
            .filter(|c| is_choice_live(c.choice.as_ref(), Some(self.open_activity())))
    }

    /// "Open page" (`handleShowCanvas`): the page's tab is the one the content panel shows. The tab itself is component 14's;
    /// until it exists the target is only kept.
    pub fn open_page(&mut self, canvas: &str) {
        self.page_target = Some(canvas.to_string());
        self.set_content_open(true);
    }

    /// `canvas.updated` and `canvas.closed` on a session's topic.
    pub fn on_pages_event(&mut self, event: &Event) {
        let Event::Other(kind, payload) = event else { return };
        let Some(session) = payload.get("sessionId").and_then(Value::as_str) else { return };
        match kind {
            Kind::CanvasUpdated => {
                if let Some(canvas) = read_page_canvas(payload) {
                    self.pages.updated(session, canvas);
                }
            }
            Kind::CanvasClosed => {
                if let Some(id) = payload.get("canvasId").and_then(Value::as_str) {
                    self.pages.closed(session, id);
                }
            }
            _ => {}
        }
    }

    /// Loads the session's page canvases when it opens and after a reconnect. A failed load is not shown.
    pub fn load_pages(&mut self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        let generation = self.pages.begin_load(session);
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::canvases(base, &asked),
            move |s, _, result| {
                if let Ok((status, body)) = result
                    && (200..300).contains(&status)
                {
                    let list = body.as_array().map(|a| a.iter().filter_map(read_page_canvas).collect()).unwrap_or_default();
                    s.pages.loaded(&id, generation, list);
                }
            },
        );
    }
}
