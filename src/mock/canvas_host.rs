//! Invented tabs for the panel's scenes and tests (component 10, piece B): the states the strip can be in that a scratch Fleet
//! can't make without a GitHub or a page canvas (piece C).
use super::context_tab::Case as Links;
use crate::State;
use crate::fleet::canvases::{CanvasInstance, CanvasKind, VisualPayload, VisualType};

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
    /// Twelve tabs in the panel's own width, one in the middle active, so both ends are faded.
    Overflow,
    /// The `+` menu with "From this conversation": three documents, one of them open.
    MenuVisuals,
    /// A page tab a moment into its pulse.
    Pulse,
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
            Case::Overflow => {
                self.canvases.replace(&id, Default::default());
                self.edit_canvases(|c, id| {
                    c.open(id, CanvasKind::Context);
                    c.open(id, CanvasKind::Progress);
                    c.open(id, CanvasKind::Turns);
                    c.open(id, CanvasKind::Agents);
                    for title in ["Release notes", "Cache design", "Retry flow", "Cost table", "Open questions", "Rollout"] {
                        c.open_visual(id, scene_visual(title, VisualType::Markdown), false);
                    }
                    c.activate(id, "visual:Retry flow");
                });
            }
            Case::MenuVisuals => {
                self.edit_canvases(|c, id| {
                    c.open(id, CanvasKind::Context);
                    c.open_visual(id, scene_visual("Cache design", VisualType::Markdown), false);
                    c.set_known_visuals(
                        id,
                        vec![
                            scene_visual("Release notes", VisualType::Markdown),
                            scene_visual("Cache design", VisualType::Markdown),
                            scene_visual("Retry flow", VisualType::Html),
                        ],
                    );
                });
                self.ui.open = Some(crate::ui::Open::Menu("canvas"));
            }
            Case::Pulse => {
                self.canvases.replace(&id, Default::default());
                self.sidebar_open = false;
                self.right.widened = true;
                self.edit_canvases(|c, id| {
                    c.open_visual(id, scene_visual("Retry flow", VisualType::Html), false);
                    c.open_visual(id, scene_visual("Cache design", VisualType::Markdown), false);
                    c.mark_updated("visual:Retry flow", 0.0);
                });
            }
            Case::Menu => {
                self.edit_canvases(|c, id| c.open(id, CanvasKind::Context));
                self.ui.open = Some(crate::ui::Open::Menu("canvas"));
            }
        }
    }
}

fn scene_visual(title: &str, kind: VisualType) -> VisualPayload {
    VisualPayload { kind, title: Some(title.into()), source_path: None, content: "# Notes\n".into() }
}
