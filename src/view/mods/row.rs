//! A mod's trees on a tool row (component 22, piece E; `ToolCard.vue`'s `.tool-header__mod` and its `ToolResult` body): the
//! line's slot with its cap, the body's tree with Fleet's own body for a `Fleet` node, and the part of the row's memo key that says
//! "this row's mod drawing changed" (and nothing about any other row).
use super::tree::{Look, tree_keyed};
use crate::State;
use crate::fleet::mods::points::ToolRowSite;
use crate::fleet::mods::rows::tree_key;
use crate::fleet::mods::types::Site;
use crate::theme::Theme;
use crate::view::conversation::{Message, Part};
use rune::{El, row};
use serde_json::Value;

/// The header's padding either side (`.tool-header` 8 px): the shares below are of what is left.
const HEADER_PAD: f32 = 16.0;
/// `12ch` of the title's mono 12 px (a digit is 0.6 em).
const TWELVE_CH: f32 = 12.0 * 7.2;

/// `.tool-header__mod { max-width: 60% }`: Rune has no percentage, so it is a share of the header as last laid out (`State::mod_line_w`).
fn cap(s: &State) -> Option<f32> {
    (s.mod_line_w > HEADER_PAD).then_some((s.mod_line_w - HEADER_PAD) * 0.6)
}

/// `min-width: min(30%, 12ch)`: what a title keeps when a mod draws on the line.
pub fn title_floor(s: &State) -> f32 {
    if s.mod_line_w > HEADER_PAD { ((s.mod_line_w - HEADER_PAD) * 0.3).min(TWELVE_CH) } else { TWELVE_CH }
}

/// `.tool-header__mod`: the tree, right-aligned and wrapping inside its cap. A press anywhere in it stops here (`@click.stop`), so
/// the row under it doesn't open or close; a control's own press is taken first.
pub fn line_slot(s: &State, t: &Theme, id: &str, call: &str, tree: &Value) -> El {
    let key = tree_key(ToolRowSite::ToolUse, &s.mod_row_session(), call);
    let slot = row()
        .key(format!("tool-{id}-mod"))
        .center_y()
        .justify_end()
        .min_w(0.0)
        .shrink()
        .on_press(|_: &mut State| {})
        .child(tree_keyed(s, t, &key, tree, Site::ToolUse, Look::of(Site::ToolUse), true, None));
    match cap(s) {
        Some(c) => slot.max_w(c),
        None => slot,
    }
}

/// The body's tree in place of Fleet's own, which a `Fleet` node draws where it stands.
pub fn body_tree(s: &State, t: &Theme, call: &str, tree: &Value, fleet: El) -> El {
    let key = tree_key(ToolRowSite::ToolResult, &s.mod_row_session(), call);
    tree_keyed(s, t, &key, tree, Site::ToolResult, Look::of(Site::ToolResult), false, Some(fleet))
}

/// What a message's rows draw from their mods, for the row's memo: nothing moves it while no mod has drawn on the session.
pub fn key(s: &State, msg: &Message) -> u64 {
    use std::hash::{Hash, Hasher};
    if s.mod_renders.views.is_empty() {
        return 0;
    }
    let stamps: Vec<_> =
        msg.parts.iter().filter_map(|p| if let Part::Tool { call_id, .. } = p { Some(s.mod_row_stamp(call_id)) } else { None }).collect();
    // A message none of whose calls has a view draws as it always did, whatever other rows have.
    if stamps.iter().all(|st| *st == (0, 0, 0, 0)) {
        return 0;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    stamps.hash(&mut hash);
    hash.finish()
}
