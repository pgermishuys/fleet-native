//! The queue above the composer: what was sent while the agent worked, in the order it goes. Ports
//! `QueuedMessages.vue` (docs/parity/05-composer.md, piece I, rows 12.8 to 12.10).
use super::conversation::COLUMN;
use crate::State;
use crate::fleet::queue::Item;
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, sans};
use rune::{Cursor, El, col, el, icon, row, text};

/// The list, or nothing when the queue is empty.
pub fn list(s: &State, t: &Theme) -> Option<El> {
    let items = s.queued();
    if items.is_empty() {
        return None;
    }
    let mut list = col().key("queued-messages").role("list").label("Queued messages").w_full().max_w(COLUMN).gap(2.0).mb(6.0);
    for (index, item) in items.iter().enumerate() {
        list = list.child(entry(t, index, item, s.can_send_queued_now(item)));
    }
    Some(list)
}

/// `Next` for the first, `Queued` for the rest, with the mod that queued it.
pub fn label(index: usize, item: &Item) -> String {
    let word = if index == 0 { "Next" } else { "Queued" };
    match &item.from_mod {
        Some(m) => format!("{word} · from {m}"),
        None => word.to_string(),
    }
}

fn entry(t: &Theme, index: usize, item: &Item, can_send_now: bool) -> El {
    let mut r = row()
        .key(format!("queued-{index}"))
        .role("listitem")
        .w_full()
        .min_w(0.0)
        .center_y()
        .gap(8.0)
        .pl(12.0)
        .pr(4.0)
        .py(3.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .fill(t.card_bg)
        .child(text(label(index, item).to_uppercase()).font(sans(11.0, 600.0, 1.2).tracking(0.44)).color(t.muted))
        .child(
            row()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .help(item.text.clone())
                .child(text(item.text.clone()).font(sans(12.0, 400.0, 1.2)).color(t.text).truncate().shrink().min_w(0.0)),
        );
    if can_send_now {
        r = r.child(
            row()
                .key(format!("queued-send-now-{index}"))
                .role("button")
                .label("Send now")
                .help("Send now: the agent reads it at its next step, without waiting for the turn to end")
                .h(26.0)
                .px(8.0)
                .gap(6.0)
                .center_y()
                .radius(8.0)
                .border(1.0, rune::Color::TRANSPARENT)
                .hover(t.accent.alpha(0.1))
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| s.send_queued_now(index))
                .child(icon(&CORNER_DOWN_RIGHT, 12.25).color(t.text))
                .child(text("Send now").font(sans(12.0, 500.0, 1.2)).color(t.text)),
        );
    }
    r.child(
        el().key(format!("queued-remove-{index}"))
            .role("button")
            .label(format!("Remove queued message: {}", item.text))
            .help("Remove from the queue")
            .size(24.5, 24.5)
            .center()
            .radius(8.0)
            .hover(t.wash(6.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut State| s.remove_queued(index))
            .child(icon(&X, 12.25).color(t.muted).group_color(t.text)),
    )
}
