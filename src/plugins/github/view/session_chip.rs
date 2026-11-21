//! `GitHubSessionChip.vue`: the Fleet session working on a pull request or issue, with its live status; a click opens the first.
use crate::fleet::row_status::Tone;
use crate::icons::MESSAGE_SQUARE;
use crate::plugins::github::sessions::{ItemSession, open_session};
use crate::theme::{Theme, sans};
use crate::view::session_row::glyph_el;
use crate::{State, Tx};
use rune::{Cursor, El, icon, row, text};

/// The chip's title: "In Fleet: {names}".
pub fn chip_title(sessions: &[ItemSession]) -> String {
    format!("In Fleet: {}", sessions.iter().map(|s| s.title.as_str()).collect::<Vec<_>>().join(", "))
}

/// `None` when no session is on the item.
pub fn session_chip(t: &Theme, key: &str, sessions: &[ItemSession]) -> Option<El> {
    let first = sessions.first()?;
    // Waiting for input is its own tone (`data-tone="attention"`); working has the accent; the rest is quiet.
    let (fill, ink) = if first.waiting {
        (t.status_waiting().alpha(0.14), t.status_waiting_text)
    } else if first.tone == Tone::Working {
        (t.accent_dim, t.text)
    } else {
        (t.wash(6.0), t.muted)
    };
    let glyph = if first.live { glyph_el(t, first.glyph) } else { None }.unwrap_or_else(|| icon(&MESSAGE_SQUARE, 12.0).color(ink));
    let mut chip = row()
        .key(key.to_string())
        .h(22.0)
        .px(8.0)
        .gap(6.0)
        .center_y()
        .radius(6.0)
        .fill(fill)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .role("button")
        .label(chip_title(sessions))
        .help(chip_title(sessions))
        .child(glyph)
        .child(text(first.words.clone()).key(format!("{key}-words")).font(sans(11.5, 400.0, 1.2)).color(ink));
    if sessions.len() > 1 {
        chip = chip.child(
            text(format!("+{}", sessions.len() - 1))
                .key(format!("{key}-more"))
                .font(sans(11.5, 400.0, 1.2).features("'tnum' 1"))
                .color(t.muted),
        );
    }
    let first = first.clone();
    Some(chip.on_press_tx(move |s: &mut State, tx: &mut Tx| open_session(s, &first, tx)))
}
