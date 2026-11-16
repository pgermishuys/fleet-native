//! `GitHubItemIcon.vue`: a pull request's or issue's icon in its state's colour. An issue is open, or closed (done, in the
//! merged colour, as GitHub shows a completed issue).
use crate::fleet::pr_state::PrState;
use crate::icons::{CIRCLE_CHECK, CIRCLE_DOT, GIT_MERGE, GIT_PULL_REQUEST, GIT_PULL_REQUEST_CLOSED, GIT_PULL_REQUEST_DRAFT};
use crate::theme::Theme;
use rune::{El, icon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Pull,
    Issue,
}

pub(super) fn glyph(kind: ItemKind, state: PrState) -> &'static rune::Icon {
    match (kind, state) {
        (ItemKind::Issue, PrState::Closed | PrState::Merged) => &CIRCLE_CHECK,
        (ItemKind::Issue, _) => &CIRCLE_DOT,
        (_, PrState::Merged) => &GIT_MERGE,
        (_, PrState::Closed) => &GIT_PULL_REQUEST_CLOSED,
        (_, PrState::Draft) => &GIT_PULL_REQUEST_DRAFT,
        _ => &GIT_PULL_REQUEST,
    }
}

/// An issue that is closed takes the merged colour.
pub fn tone(kind: ItemKind, state: PrState) -> PrState {
    if kind == ItemKind::Issue && state == PrState::Closed { PrState::Merged } else { state }
}

pub fn item_icon(t: &Theme, kind: ItemKind, state: PrState, size: f32) -> El {
    icon(glyph(kind, state), size).color(t.pr_tone(tone(kind, state)))
}
