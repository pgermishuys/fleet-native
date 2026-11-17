//! Invented tabs for the panel's scenes and tests (component 10, piece B): the states the strip can be in that a scratch Fleet
//! can't make without a GitHub or a page canvas (piece C).
use super::context_tab::Case as Links;
use crate::State;
use crate::fleet::canvases::{CanvasInstance, CanvasKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Changes, Files, Context with a count, Progress `3/7`, Agents `1`, and a page tab whose title is cut at 200.
    Tabs,
    /// The same, with a linked pull request whose checks fail: Context carries the dot.
    Attention,
    /// The `+` menu open, Context already open.
    Menu,
    /// Widen on, the list folded to leave room.
    Widened,
    /// The panel as it opens on a session: Changes and Files.
    Fresh,
}

impl State {
    pub fn show_tabs(&mut self, case: Case, dark: bool) {
        self.show_context_tab_scene(if case == Case::Attention { Links::CardFailing } else { Links::Links }, dark);
        let progress = self.projects[0].sessions[0].row.progress.get_or_insert_with(|| crate::fleet::row_chips::Progress {
            kind: "todos".into(),
            done: 3,
            total: 7,
            current: Some("Write the migration".into()),
        });
        progress.done = 3;
        progress.total = 7;
        let id = self.canvas_session();
        match case {
            Case::Fresh | Case::Widened => {
                self.canvases.replace(&id, Default::default());
                self.sync_review();
                self.sidebar_open = case != Case::Widened;
                self.right.widened = case == Case::Widened;
            }
            Case::Tabs | Case::Attention => {
                // Widened with the list folded, so every tab fits and the badges show.
                self.sidebar_open = false;
                self.right.widened = true;
                // One agent running, as the work the Agents tab counts.
                let session = self.canvas_session();
                let running = serde_json::json!([{
                    "id": "scene-agent", "sessionId": session, "workId": "w1", "kind": "subagent", "title": "general",
                    "label": "Migrate the cache", "status": "running", "background": true, "startedAt": "2026-10-04T10:00:00Z",
                }]);
                self.agents_history.insert(session, crate::fleet::work::to_items(Some(&running)));
                self.canvases.agents_running = 1;
                self.edit_canvases(|c, id| {
                    c.open(id, CanvasKind::Progress);
                    c.open(id, CanvasKind::Agents);
                    c.open(id, CanvasKind::Context);
                });
                let mut tabs = self.open_canvases();
                tabs.canvases.push(CanvasInstance {
                    id: "canvas:docs".into(),
                    kind: CanvasKind::Page,
                    payload: None,
                    title: Some("Release notes for the image cache, second draft".into()),
                    file: None,
                    server: None,
                });
                self.canvases.replace(&id, tabs);
            }
            Case::Menu => {
                self.edit_canvases(|c, id| c.open(id, CanvasKind::Context));
                self.ui.open = Some(crate::ui::Open::Menu("canvas"));
            }
        }
    }
}
