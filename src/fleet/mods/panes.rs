//! A mod's panes as tabs of the right panel (component 22, piece F): ports `lib/mods/panes.ts`, `lib/mods/pane-renders.ts` and
//! `composables/use-session-mod-panes.ts`. Opening, updating and closing a pane against `Canvases`, drawing it by asking its
//! mod (the newest ask per pane wins), and the events that open, close and redraw them. The tab's body is the next piece's
//! (G); here a pane's tree is held and drawn as a tree, nothing more.
use super::points::{ModPaneView, ModTreeView, Views, mod_pane_owner, mod_pane_view_id};
use super::renders::{Answer, parse_answer};
use super::resolve::Resolved;
use super::types::{Action, Author, Site, Surface};
use crate::fleet::canvases::{ModPaneRef, mod_pane_canvas_id};
use crate::fleet::events::{Event, Kind};
use crate::fleet::http;
use crate::fleet::state::Topic;
use crate::{State, Tx};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// The panes' views (`modPanes`), and which ask for each is the newest.
#[derive(Default)]
pub struct ModPanes {
    pub views: Views<ModPaneView>,
    newest: HashMap<String, u64>,
    /// Draws on their way.
    asking: usize,
}

impl ModPanes {
    /// No draw is on its way.
    pub fn idle(&self) -> bool {
        self.asking == 0
    }

    pub fn get(&self, session: &str, mod_name: &str, pane: &str) -> Option<Arc<ModPaneView>> {
        self.views.get(&mod_pane_view_id(session, mod_name, pane))
    }

    /// Whether the view is a draft's (`isDraftModPane`).
    pub fn is_draft(&self, session: &str, mod_name: &str, pane: &str) -> bool {
        self.get(session, mod_name, pane).is_some_and(|v| super::points::is_draft_view(&v.view))
    }
}

/// The element key a pane's tree is drawn under, which also names its controls' state.
pub fn pane_key(session: &str, mod_name: &str, pane: &str) -> String {
    format!("mod-pane:{}", mod_pane_view_id(session, mod_name, pane))
}

/// A pane with no tree yet, credited to its mod.
pub fn empty_view(session: &str, mod_name: &str, pane: &str, title: &str) -> ModPaneView {
    ModPaneView {
        session_id: session.into(),
        pane_id: pane.into(),
        title: title.into(),
        mod_id: mod_name.into(),
        view: ModTreeView::new(Value::Null, vec![Author { name: mod_name.into(), draft: false }]),
    }
}

fn text<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

impl State {
    fn set_pane_view(&mut self, view: ModPaneView) {
        let (session, mod_name, pane) = (view.session_id.clone(), view.mod_id.clone(), view.pane_id.clone());
        let owner = mod_pane_owner(&session, &mod_name, &pane);
        self.mod_panes.views.remove_by_owner(&owner);
        self.mod_panes.views.contribute(&owner, vec![view]);
        // The controls of its tree know it: a new Input starts, a changed `value` replaces its text.
        let key = pane_key(&session, &mod_name, &pane);
        match self.mod_panes.get(&session, &mod_name, &pane) {
            Some(v) => match self.mod_resolver.resolve(Some(&v), Site::Pane) {
                Resolved::Draws { tree, .. } => self.mods_ui.sync(&key, &tree),
                Resolved::NothingDrawn => self.mods_ui.clear(&key),
            },
            None => self.mods_ui.clear(&key),
        }
    }

    /// `openModPane`: the pane's view replaces the tab's, and the tab comes forward (one tab per pane per mod, no new one when it
    /// is open).
    pub fn open_mod_pane(&mut self, view: ModPaneView) {
        let (session, pane, title) =
            (view.session_id.clone(), ModPaneRef { mod_name: view.mod_id.clone(), pane_id: view.pane_id.clone() }, view.title.clone());
        self.set_pane_view(view);
        self.canvases.open_mod_pane(&session, pane, &title);
        self.sync_review();
    }

    /// `updateModPane`: replaces an open pane's view and its tab's title without opening it or moving the active tab. A pane whose
    /// tab is closed (or never was open) is left alone: its view went with the tab and must not come back.
    pub fn update_mod_pane(&mut self, view: ModPaneView) {
        let pane = ModPaneRef { mod_name: view.mod_id.clone(), pane_id: view.pane_id.clone() };
        let id = mod_pane_canvas_id(&pane.mod_name, &pane.pane_id);
        if !self.canvases.session(&view.session_id).canvases.iter().any(|c| c.id == id) {
            return;
        }
        let (session, title) = (view.session_id.clone(), view.title.clone());
        self.set_pane_view(view);
        self.canvases.update_mod_pane(&session, &pane, &title);
    }

    /// `closeModPane`: the tab and its view go.
    pub fn close_mod_pane(&mut self, session: &str, mod_name: &str, pane: &str) {
        self.canvases.close(session, &mod_pane_canvas_id(mod_name, pane));
        self.drop_pane_view(session, mod_name, pane);
        self.sync_review();
    }

    /// The view of a pane whose tab closed, wherever it was closed from.
    pub fn drop_pane_view(&mut self, session: &str, mod_name: &str, pane: &str) {
        self.mod_panes.views.remove_by_owner(&mod_pane_owner(session, mod_name, pane));
        self.mods_ui.clear(&pane_key(session, mod_name, pane));
    }

    /// `drawModPane`: asks the mod to draw the pane (`site: Pane`, `requestId` the pane id) and updates the open tab. The newest
    /// ask wins, so a slow answer can't replace a newer one; no answer leaves the pane as it was; an answer with no mods is
    /// the pane's mod's.
    pub fn draw_mod_pane(&mut self, session: &str, mod_name: &str, pane: &str, title: &str) {
        let key = mod_pane_view_id(session, mod_name, pane);
        let mine = self.mod_panes.newest.get(&key).copied().unwrap_or(0) + 1;
        self.mod_panes.newest.insert(key.clone(), mine);
        let Some(link) = self.link_for(session) else { return };
        self.mod_panes.asking += 1;
        let body = json!({ "site": "Pane", "requestId": pane, "mod": mod_name, "props": { "title": title } });
        let path = super::renders_state::session_path(session, "render");
        let (id, mod_id, pane_id, title) = (session.to_string(), mod_name.to_string(), pane.to_string(), title.to_string());
        link.off_thread(
            move |base| -> Option<Answer> {
                let (status, reply) = http::text_with("POST", base, &path, Some(&body)).ok()?;
                if !(200..300).contains(&status) {
                    return None;
                }
                parse_answer(&serde_json::from_str(&reply).ok()?)
            },
            move |s: &mut State, _tx: &mut Tx, answer: Option<Answer>| {
                s.mod_panes.asking = s.mod_panes.asking.saturating_sub(1);
                let Some(answer) = answer.filter(|_| s.mod_panes.newest.get(&key) == Some(&mine)) else { return };
                let mods = if answer.mods.is_empty() { vec![Author { name: mod_id.clone(), draft: false }] } else { answer.mods };
                s.update_mod_pane(ModPaneView { session_id: id, pane_id, title, mod_id, view: ModTreeView::new(answer.tree, mods) });
            },
        );
    }

    /// `openAndDrawModPane`: opens (or brings forward) the pane's tab, then draws it.
    pub fn open_and_draw_mod_pane(&mut self, session: &str, mod_name: &str, pane: &str, title: &str) {
        self.open_mod_pane(empty_view(session, mod_name, pane, title));
        self.draw_mod_pane(session, mod_name, pane, title);
    }

    /// Draws the session's open panes again; only the named mod's when one is.
    fn redraw_mod_panes(&mut self, session: &str, only: Option<&str>) {
        let open: Vec<(String, String, String)> = self
            .canvases
            .session(session)
            .canvases
            .iter()
            .filter_map(|c| c.mod_pane.as_ref().map(|p| (p.mod_name.clone(), p.pane_id.clone(), c.title())))
            .filter(|(m, ..)| only.is_none_or(|o| o == m))
            .collect();
        for (mod_name, pane, title) in open {
            self.draw_mod_pane(session, &mod_name, &pane, &title);
        }
    }

    /// `useSessionModPanes`: `mod.pane` opens a tab (and the right panel, if it was closed) or closes it; `mod.ui` draws the
    /// open session's panes again (that mod's, or all with no mod), on its topic and on `sessions` (for it, or with no session);
    /// `mods.changed` draws them all, since a draw that landed while the host reloaded came back empty and nothing else asks again.
    pub(crate) fn on_mod_panes_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(kind, payload) = event else { return };
        match (kind, topic) {
            (Kind::ModPane, Topic::Session(id)) => {
                let (Some(mod_name), Some(pane)) = (text(payload, "mod"), text(payload, "id")) else { return };
                if payload.get("open").and_then(Value::as_bool) == Some(true) {
                    let title = text(payload, "title").unwrap_or(pane);
                    self.open_and_draw_mod_pane(id, mod_name, pane, title);
                    self.set_content_open(true);
                } else {
                    self.close_mod_pane(id, mod_name, pane);
                }
            }
            (Kind::ModUi, Topic::Session(id)) => self.redraw_mod_panes(id, text(payload, "mod")),
            (Kind::ModUi, Topic::Sessions) => {
                if let Some(open) = self.open_id().filter(|o| text(payload, "sessionId").is_none_or(|s| s == o)) {
                    self.redraw_mod_panes(&open, text(payload, "mod"));
                }
            }
            (Kind::ModsChanged, Topic::Sessions) => {
                if let Some(open) = self.open_id() {
                    self.redraw_mod_panes(&open, None);
                }
            }
            _ => {}
        }
    }

    /// The panel is open on a session: its mods' hooks and declared panes are read once (`watchModSession` in `CanvasHost`'s setup).
    /// True when that asked for something.
    pub fn watch_open_mod_session(&mut self) -> bool {
        let Some(session) = self.open_id().filter(|id| self.content_open && !self.mod_renders.knows(id)) else { return false };
        self.watch_mod_session(&session);
        true
    }

    /// The pane a handle belongs to: the view whose tree holds it.
    pub(crate) fn mod_pane_of_handle(&self, handle: &str) -> Option<Arc<ModPaneView>> {
        self.mod_panes.views.items().into_iter().find(|v| super::rows::holds(&v.view.tree, handle))
    }

    /// A used control of a pane goes to its mod; when the server can't deliver it, the pane draws again so it shows the current
    /// tree (`modActionSender`'s redraw).
    pub fn mod_pane_action(&mut self, view: &ModPaneView, action: Action) {
        let Some(link) = self.link_for(&view.session_id) else { return };
        let (session, mod_name, pane, title) = (view.session_id.clone(), view.mod_id.clone(), view.pane_id.clone(), view.title.clone());
        let sent = session.clone();
        link.off_thread(
            move |base| super::actions::send(base, &sent, &action, Surface::Desktop),
            move |s: &mut State, _tx: &mut Tx, handled: bool| {
                if !handled {
                    s.draw_mod_pane(&session, &mod_name, &pane, &title);
                }
            },
        );
    }
}
