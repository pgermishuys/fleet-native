//! Invented trees for the mod views' scenes and tests (component 22). Nothing here is from a real mod or machine.
use crate::State;
use crate::fleet::canvases::{CanvasInstance, CanvasKind};
use crate::fleet::mods::points::ModPaneView;
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
            mod_pane: None,
        });
        tabs.active_id = format!("mod:{id}-demo@v1:{id}");
        self.canvases.replace(&session, tabs);
        self.set_mod_scene(Some(ModScene { tree, site: Site::Pane, slot: true }));
    }
}

/// What the pane scenes show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaneCase {
    /// The `+` menu open, with "From mods": a pane of a kept mod and a draft's pane.
    Menu,
    /// The draft's pane open: its tab carries the compact Draft mark, beside the kept mod's open one.
    Tab,
    /// Piece G: the kept `release-board`'s pane open, drawing its tree (the board of the sample).
    Board,
    /// A pane restored after a reload, before its mod has drawn it.
    Waiting,
    /// `release-board` turned off: its pane says so and offers Turn on.
    Off,
}

/// What `release-board`'s own pane draws (the sample's tree, with its invented items).
pub fn release_board() -> Value {
    let pill = |tone: &str, label: &str, title: &str| {
        serde_json::json!({ "type": "Box", "props": { "flexDirection": "row", "gap": 2, "alignItems": "center" }, "children": [
            { "type": "Pill", "props": { "tone": tone, "label": label } },
            { "type": "Text", "props": {}, "children": [title] } ] })
    };
    serde_json::json!({ "type": "Box", "props": { "flexDirection": "column", "gap": 3 }, "children": [
        { "type": "Text", "props": { "bold": true }, "children": ["Release 2.4"] },
        pill("good", "Done", "Cut the release branch"),
        pill("warn", "Running", "Smoke test on staging"),
        pill("neutral", "To do", "Write the release notes"),
        { "type": "Box", "props": { "flexDirection": "row" }, "children": [
            { "type": "Button", "props": { "key": "close", "label": "Close" }, "handles": { "onPress": "h1" } } ] } ] })
}

impl State {
    /// A session whose mods declare two panes (invented: `release-board` kept, `deploy-notes` a draft), the `+` menu open or the
    /// draft's pane open.
    pub fn show_mod_panes(&mut self, case: PaneCase, dark: bool) {
        use crate::fleet::mods::points::ModTreeView;
        use crate::fleet::mods::renders::DeclaredPane;
        use crate::fleet::mods::types::Author;
        self.show_chips(super::context::Case::Chips, dark);
        self.content_open = true;
        let session = self.canvas_session();
        self.canvases.replace(&session, Default::default());
        let pane = |m: &str, id: &str, title: &str, icon: &str, draft: bool| DeclaredPane {
            mod_name: m.into(),
            id: id.into(),
            title: title.into(),
            icon: Some(icon.into()),
            draft,
            off: matches!(case, PaneCase::Off) && m == "release-board",
        };
        self.mod_renders.set_declared_panes(
            &session,
            vec![
                pane("release-board", "board", "Release board", "gauge", false),
                pane("deploy-notes", "notes", "Deploy notes", "file", true),
            ],
        );
        let open_board = |s: &mut State| {
            s.sidebar_open = false;
            s.right.widened = true;
        };
        match case {
            PaneCase::Board => {
                open_board(self);
                self.open_mod_pane(ModPaneView {
                    session_id: session.clone(),
                    pane_id: "board".into(),
                    title: "Release board".into(),
                    mod_id: "release-board".into(),
                    view: ModTreeView::new(release_board(), vec![Author { name: "release-board".into(), draft: false }]),
                });
            }
            PaneCase::Waiting => {
                open_board(self);
                // The tab is there and its mod hasn't drawn it: no view.
                self.canvases.open_mod_pane(
                    &session,
                    crate::fleet::canvases::ModPaneRef { mod_name: "release-board".into(), pane_id: "board".into() },
                    "Release board",
                );
            }
            PaneCase::Off => {
                open_board(self);
                self.open_mod_pane(ModPaneView {
                    session_id: session.clone(),
                    pane_id: "board".into(),
                    title: "Release board".into(),
                    mod_id: "release-board".into(),
                    view: ModTreeView::new(release_board(), vec![Author { name: "release-board".into(), draft: false }]),
                });
            }
            PaneCase::Menu => self.ui.open = Some(crate::ui::Open::Menu("canvas")),
            PaneCase::Tab => {
                self.sidebar_open = false;
                self.right.widened = true;
                let view = |m: &str, id: &str, title: &str, draft: bool, tree: Value| crate::fleet::mods::points::ModPaneView {
                    session_id: session.clone(),
                    pane_id: id.into(),
                    title: title.into(),
                    mod_id: m.into(),
                    view: ModTreeView::new(tree, vec![Author { name: m.into(), draft }]),
                };
                let words = |w: &str| serde_json::json!({ "type": "Text", "props": {}, "children": [w] });
                self.open_mod_pane(view("release-board", "board", "Release board", false, words("Release 2.4 is ready to ship")));
                self.open_mod_pane(view("deploy-notes", "notes", "Deploy notes", true, elements()));
            }
        }
    }
}
