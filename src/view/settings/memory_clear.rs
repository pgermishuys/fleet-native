//! "Clear all memory" at the foot of Settings → Memory (`MemorySection.vue`): the row, and in its place the question,
//! which is inline rather than a dialog. Clearing one list has no question; this does.
use super::memory_notes::link;
use crate::State;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::rem;
use rune::{El, Span, col, rich, row, text};

pub fn clear_all(s: &State, t: &Theme) -> El {
    let outer = row()
        .key("memory-clear-row")
        .w_full()
        .mt(17.5)
        .pt(14.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .flex_wrap()
        .center_y()
        .justify_between()
        .gap(10.5);
    if !s.settings.memory_ui.confirming_clear {
        let button = row()
            .key("memory-clear-all")
            .role("button")
            .label("Clear all memory")
            .focusable(true)
            .px(10.5)
            .py(3.5)
            .radius(5.0)
            .border(1.0, t.error.alpha(0.4))
            .border_layout()
            .focus_ring(2.0, 1.0, t.accent)
            .child(text("Clear all memory").font(sans(rem(0.75), 500.0, 1.33)).color(t.error_text))
            .on_press(|s: &mut State| s.memory_set_confirming_clear(true));
        return outer
            .child(text("Clear every note, in every repository and for this machine.").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted))
            .child(button);
    }
    let lead = "Delete every note?";
    let words = format!("{lead} Sessions already running keep what they last read. This can't be undone.");
    let busy = s.settings.memory.saving;
    let confirm = row()
        .key("memory-clear-all-confirm")
        .role("button")
        .label("Delete all notes")
        .focusable(true)
        .disabled(busy)
        .opacity(if busy { 0.6 } else { 1.0 })
        .px(10.5)
        .py(3.5)
        .radius(6.0)
        .fill(t.destructive)
        .focus_ring(2.0, 1.0, t.accent)
        .child(text("Delete all notes").font(sans(rem(0.75), 500.0, 1.33)).color(rune::Color::WHITE))
        .on_press(|s: &mut State| s.memory_clear_all());
    outer.child(
        col()
            .key("memory-clear-confirm")
            .role("alertdialog")
            .label("Clear all memory")
            .w_full()
            .gap(7.0)
            .px(10.5)
            .py(10.5)
            .radius(RADIUS_CARD)
            .border(1.0, t.error.alpha(0.4))
            .border_layout()
            .fill(t.error.alpha(0.05))
            .child(
                rich(words, vec![Span::new(0, lead.len()).color(t.text).weight(600.0)])
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted)
                    .wrap()
                    .w_full(),
            )
            .child(row().gap(7.0).center_y().child(confirm).child(link(
                t,
                "memory-clear-all-cancel".to_string(),
                None,
                "Cancel",
                "Cancel",
                false,
                |s: &mut State| s.memory_set_confirming_clear(false),
            ))),
    )
}
