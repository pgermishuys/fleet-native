//! `components/mods/ModStatusChip.vue` (component 22, piece K): a mod's chip in the status bar for the session on screen. Draws
//! nothing, not even a wrapper, unless a mod's valid tree is there.
use super::draft_mark::draft_mark;
use super::tree::{Look, tree_keyed};
use crate::State;
use crate::fleet::mods::sites::CHIP_KEY;
use crate::fleet::mods::types::Site;
use crate::theme::Theme;
use rune::{El, row};

/// The chip's key; its tree is `mod-status-chip-tree`.
pub const KEY: &str = "mod-status-chip";

/// `.mod-status-chip`: inline, gap 6, at most 260 wide, clipped.
pub fn chip(s: &State, t: &Theme) -> Option<El> {
    let (tree, draft) = s.status_chip()?;
    let mut chip = row().key(KEY).center_y().gap(6.0).min_w(0.0).max_w(260.0).clip_rounded(0.0).child(tree_keyed(
        s,
        t,
        CHIP_KEY,
        &tree,
        Site::StatusChip,
        Look::of(Site::StatusChip),
        false,
        None,
    ));
    if draft {
        chip = chip.child(draft_mark(t, false));
    }
    Some(chip)
}
