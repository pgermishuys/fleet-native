//! Invented band, chip and safe-mode state for the scenes and tests of component 22, piece K. Fleet can't show a band or a chip
//! live (they come from a mod's own hooks), so these come from here. Nothing is from a real mod or machine.
use crate::State;
use crate::fleet::mods::points::{ComposerBandView, ModTreeView, StatusChipView};
use crate::fleet::mods::types::Author;
use crate::fleet::settings::mods_kept::Switch;
use serde_json::{Value, json};

/// What a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// The Mods switch on and safe mode on: the banner over the app.
    SafeMode,
    /// A draft mod's band above the composer.
    Band,
    /// A draft mod's chip at the end of the status bar.
    Chip,
    /// The palette searched for "mods", safe mode off: "Start without mods".
    Palette,
    /// The same with safe mode on: "Turn mods back on".
    PaletteSafe,
}

/// A line of passed, failed and skipped counts (the invented `test-chips` mod).
pub fn band_tree() -> Value {
    json!({ "type": "Box", "props": { "flexDirection": "row", "gap": 2, "alignItems": "center" }, "children": [
        { "type": "Pill", "props": { "tone": "good", "label": "42 passed" } },
        { "type": "Pill", "props": { "tone": "bad", "label": "1 failed" } },
        { "type": "Text", "props": { "dimColor": true }, "children": ["paginate.test.ts is the one that failed"] },
        { "type": "Button", "props": { "key": "rerun", "label": "Run again", "icon": "play" }, "handles": { "onPress": "h1" } } ] })
}

pub fn chip_tree() -> Value {
    json!({ "type": "Box", "props": { "flexDirection": "row", "gap": 2, "alignItems": "center" }, "children": [
        { "type": "Icon", "props": { "name": "git-branch", "color": "muted" } },
        { "type": "Text", "props": { "dimColor": true }, "children": ["feature/paging"] } ] })
}

impl State {
    pub fn show_mod_sites(&mut self, case: Case, dark: bool) {
        self.dark = Some(dark);
        let session = self.mod_row_session();
        let author = vec![Author { name: "test-chips".into(), draft: true }];
        self.mod_sites.switch = Some(Switch { on: true, safe_mode: matches!(case, Case::SafeMode | Case::PaletteSafe) });
        match case {
            Case::Band => self
                .mod_sites
                .bands
                .contribute("scene", vec![ComposerBandView { session_id: session, view: ModTreeView::new(band_tree(), author) }]),
            Case::Chip => self
                .mod_sites
                .chips
                .contribute("scene", vec![StatusChipView { session_id: session, view: ModTreeView::new(chip_tree(), author) }]),
            Case::Palette | Case::PaletteSafe => {
                self.ui.open = Some(crate::ui::Open::Palette);
                self.ui.palette.query = rune::TextEdit::new("mods");
                0
            }
            Case::SafeMode => 0,
        };
    }
}
