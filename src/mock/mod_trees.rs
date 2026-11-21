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

/// The tree of the `mods-controls` scene: every Button, Input and Select variant, Markdown and Code. Also a standin mod's pane
/// (`scripts/mod-standin/controls-demo`).
pub fn controls() -> Value {
    serde_json::from_str(include_str!("../../scripts/mod-standin/controls-demo/tree.json")).expect("the standin's tree is JSON")
}

/// What a Mod tab draws in a scene or a test: the tree, the site it is drawn for, and whether the site has a drawing of its own
/// for a `Fleet` node (and an invalid tree) to show.
pub struct ModScene {
    pub tree: Value,
    pub site: Site,
    pub slot: bool,
}

impl State {
    /// Draws `scene` in the Mod tab, and tells the controls what the tree now holds: new Inputs start, a changed `value` replaces
    /// the text, an Input that is gone sends what it still held (`fleet::mods::controls`).
    pub fn set_mod_scene(&mut self, scene: Option<ModScene>) {
        use crate::view::mods::tree::{Draw, to_draw};
        match scene.as_ref().map(|sc| to_draw(&sc.tree, sc.site)) {
            Some(Draw::Tree(tree)) => self.mods_ui.sync("mod-tree", &tree),
            _ => self.mods_ui.clear("mod-tree"),
        }
        self.mod_scene = scene;
    }

    /// A Mod tab drawing `tree()`, the right panel widened over the session list so the pane has Fleet's room.
    pub fn show_mod_elements(&mut self, dark: bool) {
        self.show_mod_pane(dark, "elements", "Elements", elements());
    }

    /// The same Mod tab with the controls' tree.
    pub fn show_mod_controls(&mut self, dark: bool) {
        self.show_mod_pane(dark, "controls", "Controls", controls());
    }

    fn show_mod_pane(&mut self, dark: bool, id: &str, title: &str, tree: Value) {
        self.show_chips(super::context::Case::Chips, dark);
        self.content_open = true;
        let session = self.canvas_session();
        self.canvases.replace(&session, Default::default());
        self.sidebar_open = false;
        self.right.widened = true;
        let mut tabs = self.open_canvases();
        tabs.canvases.push(CanvasInstance {
            id: format!("mod:{id}-demo@v1:{id}"),
            kind: CanvasKind::Mod,
            payload: None,
            title: Some(title.into()),
            file: None,
            server: None,
        });
        tabs.active_id = format!("mod:{id}-demo@v1:{id}");
        self.canvases.replace(&session, tabs);
        self.set_mod_scene(Some(ModScene { tree, site: Site::Pane, slot: true }));
    }
}
