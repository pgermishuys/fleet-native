//! `.input-history` of `Composer.vue`: the last messages sent in the session, above the composer's frame (docs/parity/05-composer.md,
//! piece M, rows 8.2 to 8.4). Oldest at the top, newest next to the frame. The keys are `State::history_key`; a click takes
//! the entry into the text area without taking focus from it.
use crate::State;
use crate::theme::{Theme, sans};
use rune::{Color, Cursor, El, Shadow, col, row, text};

const MAX_H: f32 = 260.0;

/// The list, `width` wide (the frame's), to anchor on the frame's top edge; `None` while it is closed.
pub fn list(s: &State, t: &Theme, width: f32) -> Option<El> {
    let (entries, selected) = s.history_view()?;
    // Fleet draws the oldest first; its index counts from the newest.
    let mut rows = col().key("input-history-rows").w_full().items_stretch();
    for (index, entry) in entries.iter().enumerate().rev() {
        let mut item = row()
            .key(format!("history-row{index}"))
            .role("option")
            .label(entry.clone())
            .w_full()
            .px(16.0)
            .py(8.0)
            .hover(t.accent.alpha(0.12))
            .cursor(Cursor::Pointer)
            .focusable(false)
            .on_press_tx(move |s: &mut State, tx| s.history_pick(index, tx))
            .on_hover(move |s: &mut State| s.history_hover(index))
            .child(text(first_line(entry)).font(sans(12.0, 400.0, 1.4)).color(t.text).truncate().grow(1.0).min_w(0.0));
        if index == selected {
            item = item.fill(t.accent.alpha(0.18));
        }
        rows = rows.child(item);
    }
    Some(
        col()
            .key("input-history")
            .role("listbox")
            .label("Message history")
            .w(width)
            .max_h(MAX_H)
            .items_stretch()
            .fill(t.card_bg)
            .corners([14.0, 14.0, 0.0, 0.0])
            .clip_corners([14.0, 14.0, 0.0, 0.0])
            .border_sides([1.0, 1.0, 1.0, 0.0], t.border)
            .border_layout()
            .box_shadow(Shadow { x: 0.0, y: -4.0, blur: 16.0, spread: 0.0, color: Color::BLACK.alpha(0.25), inset: false })
            .child(rows.scroll(rune::ScrollSpec { stick_bottom: true }).key("history-scroll").w_full().shrink().min_h(0.0).max_h(MAX_H)),
    )
}

/// A button shows one line: a multi-line entry is cut at its first line break (`white-space: nowrap` folds it into one).
fn first_line(entry: &str) -> String {
    entry.split_whitespace().collect::<Vec<_>>().join(" ")
}
