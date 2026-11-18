//! The server's canvases (browser and page tabs) as tabs of the right panel (component 10, piece C; rows 2.14 to 2.20 and 2.25
//! of `docs/parity/10-content-panel.md`). Ports `composables/use-server-canvases.ts`. The tab model is `canvases.rs`; page
//! choices (`pages.rs`) are fed from the same load and events.
//!
//! The list loads when a session opens and on every reconnect (canvas events aren't replayed), events that arrive while a load
//! is out are applied again on top of it, and a load overtaken by a newer one is dropped.
use super::canvases::{CanvasEvent, ServerCanvas, read_server_canvas, server_tab_id};
use super::events::{Event, Kind};
use super::http;
use super::pages::{PageCanvas, read_page_canvas, read_page_choice};
use crate::State;
use serde_json::Value;

/// The load in flight, and the events that came while it was out.
#[derive(Debug, Default)]
pub struct Loads {
    /// The newest load asked for; an answer carrying an older number is dropped (`loadId`).
    id: u64,
    pending: Option<Pending>,
}

#[derive(Debug)]
struct Pending {
    session: String,
    buffered: Vec<CanvasEvent>,
}

impl State {
    /// No load is out (for a test that waits for one to land).
    #[cfg(test)]
    pub(crate) fn server_loads_idle(&self) -> bool {
        self.server_loads.pending.is_none()
    }

    /// A canvas event, applied to the session's tabs and page choices, and kept for replay when a load is out.
    fn apply_canvas(&mut self, session: &str, event: CanvasEvent) {
        match &event {
            CanvasEvent::Updated(canvas) => {
                if let Some(page) = page_of(canvas) {
                    self.pages.updated(session, page);
                }
            }
            CanvasEvent::Closed(id) => self.pages.closed(session, id),
            CanvasEvent::Focused(_) => {}
        }
        self.canvases.apply_canvas_event(session, event);
        self.sync_review();
    }

    /// `canvas.updated`, `canvas.focused` and `canvas.closed` on a session's topic.
    pub fn on_server_canvas_event(&mut self, event: &Event) {
        let Event::Other(kind, payload) = event else { return };
        let Some(session) = payload.get("sessionId").and_then(Value::as_str) else { return };
        let canvas_id = || payload.get("canvasId").and_then(Value::as_str).map(str::to_string);
        let event = match kind {
            Kind::CanvasUpdated => read_server_canvas(payload).map(CanvasEvent::Updated),
            Kind::CanvasFocused => canvas_id().map(CanvasEvent::Focused),
            Kind::CanvasClosed => canvas_id().map(CanvasEvent::Closed),
            _ => None,
        };
        let Some(event) = event else { return };
        if let Some(pending) = self.server_loads.pending.as_mut().filter(|p| p.session == session) {
            pending.buffered.push(event.clone());
        }
        self.apply_canvas(session, event);
    }

    /// Loads the session's server canvases when it opens and after a reconnect. A failed load is not shown; the next open or
    /// reconnect loads again.
    pub fn load_server_canvases(&mut self, session: &str) {
        self.load_server_canvases_then(session, None);
    }

    /// `load_server_canvases`, then the tab `activate` names comes forward (a tab a browser tab opened from a link).
    pub fn load_server_canvases_then(&mut self, session: &str, activate: Option<String>) {
        let Some(link) = self.link_for(session) else { return };
        let generation = self.pages.begin_load(session);
        self.server_loads.id += 1;
        let mine = self.server_loads.id;
        self.server_loads.pending = Some(Pending { session: session.to_string(), buffered: Vec::new() });
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::canvases(base, &asked),
            move |s, _, result| {
                if s.server_loads.id != mine {
                    return;
                }
                let pending = s.server_loads.pending.take();
                let Ok((status, body)) = result else { return };
                if !(200..300).contains(&status) {
                    return;
                }
                let items = body.as_array().map(Vec::as_slice).unwrap_or_default();
                let list: Vec<ServerCanvas> = items.iter().filter_map(read_server_canvas).collect();
                s.pages.loaded(&id, generation, items.iter().filter_map(read_page_canvas).collect());
                s.canvases.set_server_canvases(&id, list);
                for event in pending.map(|p| p.buffered).unwrap_or_default() {
                    s.canvases.apply_canvas_event(&id, event);
                }
                if let Some(tab) = &activate {
                    s.canvases.activate(&id, tab);
                }
                s.sync_review();
            },
        );
    }

    /// The tab goes at once, then `DELETE`; if the server refuses, the list loads again so the tab comes back
    /// (`closeServerCanvas`).
    pub fn close_server_canvas(&mut self, canvas: &str) {
        let session = self.canvas_session();
        self.apply_canvas(&session, CanvasEvent::Closed(canvas.to_string()));
        let Some(link) = self.link_for(&session) else { return };
        let (id, asked) = (canvas.to_string(), session.clone());
        link.off_thread(
            move |base| http::canvas_close(base, &asked, &id),
            move |s, _, result| {
                if !matches!(result, Ok((status, _)) if (200..300).contains(&status)) {
                    s.load_server_canvases(&session);
                }
            },
        );
    }

    /// Brings a canvas forward: at once when its tab is open here, and through the server, which reopens a closed one and tells
    /// every client (`canvas.updated`, `canvas.focused`). A failure is only logged (`focusServerCanvas`).
    pub fn focus_server_canvas(&mut self, canvas: &str) {
        let session = self.canvas_session();
        self.canvases.activate(&session, &server_tab_id(canvas));
        self.sync_review();
        let Some(link) = self.link_for(&session) else { return };
        let id = canvas.to_string();
        link.off_thread(
            move |base| http::canvas_focus(base, &session, &id).map(|(status, _)| status),
            move |_, _, result| match result {
                Ok(status) if (200..300).contains(&status) => {}
                Ok(status) => eprintln!("Failed to focus a canvas: HTTP {status}"),
                Err(error) => eprintln!("Failed to focus a canvas: {error}"),
            },
        );
    }

    /// The tool card's Show: the canvas comes forward and the panel opens (`handleShowCanvas`).
    pub fn show_server_canvas(&mut self, canvas: &str) {
        self.focus_server_canvas(canvas);
        self.set_content_open(true);
    }

    /// "Open page" on the page-pick card: the page's tab is the one the panel shows.
    pub fn open_page(&mut self, canvas: &str) {
        self.page_target = Some(canvas.to_string());
        self.show_server_canvas(canvas);
    }
}

fn page_of(canvas: &ServerCanvas) -> Option<PageCanvas> {
    (canvas.kind == super::canvases::CanvasKind::Page).then(|| PageCanvas {
        id: canvas.id.clone(),
        title: canvas.title.clone(),
        version: canvas.version,
        choice: canvas.state.get("choice").and_then(read_page_choice),
    })
}
