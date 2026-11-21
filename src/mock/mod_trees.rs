//! Invented trees for the mod views' scenes and tests (component 22). Nothing here is from a real mod or machine.
use crate::State;
use crate::fleet::canvases::{CanvasInstance, CanvasKind};
use crate::fleet::mods::types::Site;
use serde_json::Value;

/// The tree of the `mods-elements` scene: every Box, Text, Pill and Icon variant. The same file is the standin mod's pane
/// (`scripts/mod-standin/elements-demo`), so the native app and Fleet's own client draw one tree.
pub fn elements() -> Value {
    serde_json::from_str(include_str!("../../scripts/mod-standin/elements-demo/tree.json")).expect("the standin's tree is JSON")
}

/// What a Mod tab draws in a scene or a test: the tree, the site it is drawn for, and whether the site has a drawing of its own
/// for a `Fleet` node (and an invalid tree) to show.
pub struct ModScene {
    pub tree: Value,
    pub site: Site,
    pub slot: bool,
}

impl State {
    /// A Mod tab drawing `tree()`, the right panel widened over the session list so the pane has Fleet's room.
    pub fn show_mod_elements(&mut self, dark: bool) {
        self.show_chips(super::context::Case::Chips, dark);
        self.content_open = true;
        let id = self.canvas_session();
        self.canvases.replace(&id, Default::default());
        self.sidebar_open = false;
        self.right.widened = true;
        let mut tabs = self.open_canvases();
        tabs.canvases.push(CanvasInstance {
            id: "mod:elements-demo@v1:elements".into(),
            kind: CanvasKind::Mod,
            payload: None,
            title: Some("Elements".into()),
            file: None,
            server: None,
        });
        tabs.active_id = "mod:elements-demo@v1:elements".into();
        self.canvases.replace(&id, tabs);
        self.mod_scene = Some(ModScene { tree: elements(), site: Site::Pane, slot: true });
    }
}
