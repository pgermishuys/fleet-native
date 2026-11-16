//! `CheckIcon.vue`: one check's state: success, failure, pending (turning), skipped or neutral.
use crate::icons::{CIRCLE_CHECK, CIRCLE_DASHED, CIRCLE_X, LOADER_CIRCLE, MINUS};
use crate::theme::Theme;
use rune::{El, icon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
    Success,
    Failure,
    Pending,
    Skipped,
    Neutral,
}

pub fn check_icon(t: &Theme, kind: CheckKind, size: f32) -> El {
    match kind {
        CheckKind::Success => icon(&CIRCLE_CHECK, size).color(t.check_pass()),
        CheckKind::Failure => icon(&CIRCLE_X, size).color(t.check_fail()),
        // Rune has no turning animation yet (docs/RUNE.md): the fade every other spinner here uses.
        CheckKind::Pending => icon(&LOADER_CIRCLE, size).color(t.check_pending()).cycle(crate::view::ring_popover::spinner()),
        CheckKind::Skipped => icon(&CIRCLE_DASHED, size).color(t.muted),
        CheckKind::Neutral => icon(&MINUS, size).color(t.muted),
    }
}
