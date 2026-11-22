//! `components/mods/ModComposerBand.vue` (component 22, piece K): the band a mod draws above a session's composer, styled as Fleet's
//! own strips there. No contribution, an invalid tree or a `null` tree draws nothing at all, not even a wrapper.
use super::draft_mark::draft_mark;
use super::tree::{Look, tree_keyed};
use crate::State;
use crate::fleet::mods::sites::BAND_KEY;
use crate::fleet::mods::types::Site;
use crate::theme::{RADIUS_CARD, Theme};
use rune::{El, row};

/// The band's key; its tree is `mod-composer-band-tree`.
pub const KEY: &str = "mod-composer-band";

/// `.mod-composer-band--desktop`: `6px 12px` inside a 1 px border, `--card-bg`, 12 px text, at most 760 wide and centred, 6 below.
pub fn band(s: &State, t: &Theme) -> Option<El> {
    let (tree, draft) = s.composer_band()?;
    let inner =
        tree_keyed(s, t, BAND_KEY, &tree, Site::ComposerBand, Look::of(Site::ComposerBand), false, None).grow(1.0).shrink().min_w(0.0);
    let mut band = row()
        .key(KEY)
        .w_full()
        .max_w(super::super::conversation::COLUMN)
        .mb(6.0)
        .px(12.0)
        .py(6.0)
        .center_y()
        .justify_between()
        .flex_wrap()
        .gaps(6.0, 8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .clip_rounded(RADIUS_CARD)
        .child(inner);
    if draft {
        band = band.child(draft_mark(t, false));
    }
    Some(band)
}
