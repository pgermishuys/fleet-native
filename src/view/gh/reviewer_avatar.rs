//! `ReviewerAvatar.vue`: a GitHub avatar, ringed in its review state's colour (approved, changes requested, still to
//! answer). The picture itself is not fetched yet: the initial shows, as Fleet's does while the picture is missing.
use crate::fleet::pr_state::{ReviewerState, reviewer_words};
use crate::theme::{Theme, sans};
use rune::{El, Shadow, row, text};

/// The ring's colour, `None` for an avatar with no review state.
fn ring(t: &Theme, state: Option<ReviewerState>) -> Option<rune::Color> {
    Some(match state? {
        ReviewerState::Approved => t.check_pass(),
        ReviewerState::ChangesRequested => t.idle,
        _ => t.wash(22.0),
    })
}

pub fn reviewer_avatar(t: &Theme, key: &str, login: &str, state: Option<ReviewerState>, size: f32) -> El {
    let title = match state {
        Some(state) => format!("{login}: {}", reviewer_words(state)),
        None => login.to_string(),
    };
    let initial = login.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    let mut avatar = row()
        .key(key.to_string())
        .size(size, size)
        .center()
        .radius(size)
        .fill(t.wash(12.0))
        .help(title)
        .child(text(initial).font(sans((size * 0.5).round(), 600.0, 1.0)).color(t.text));
    if let Some(ring) = ring(t, state) {
        // `0 0 0 2px var(--panel-bg), 0 0 0 3.5px <ring>`
        avatar = avatar.box_shadow(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 3.5, color: ring, inset: false }).box_shadow(Shadow {
            x: 0.0,
            y: 0.0,
            blur: 0.0,
            spread: 2.0,
            color: t.panel_bg,
            inset: false,
        });
    }
    avatar
}
