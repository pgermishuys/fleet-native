//! One lane of the Board (`KanbanColumn.vue`), as work mode shows it: the "Inbox" and count pills over the name, the cards (or
//! "Inbox is empty." / "Drop a card here or add one below."), and the composer; in manage mode the actions row and the rename
//! field (`manage`). Dropping a card comes with piece O; the slots between cards are laid out now, keyed for it.
use super::{card, manage};
use crate::State;
use crate::fleet::board::{Card, Lane};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::text_menu::text_input;
use crate::view::session_row::RowClock;
use rune::{El, Key, KeyEvent, ScrollSpec, Tx, col, el, row, text};

/// The lane's width: `min-width` and `max-width` both 320.
pub const WIDTH: f32 = 320.0;

/// `.kanban-col__count` and `.kanban-col__inbox-pill`: 24 high at least, 10 px capitals.
fn pill(t: &Theme, key: String, words: &str, counting: bool) -> El {
    let (ink, fill, border) =
        if counting { (t.muted, rune::Color::TRANSPARENT, t.border) } else { (t.accent_text, t.accent_dim, rune::Color::TRANSPARENT) };
    row()
        .min_h(24.0)
        .px(8.0)
        .center()
        .radius(999.0)
        .border(1.0, border)
        .border_layout()
        .fill(fill)
        .child(text(words.to_uppercase()).key(key).font(sans(10.0, 700.0, 1.5).tracking(0.4)).color(ink))
}

/// The composer: "+ Add a card", or its form while open.
fn composer(s: &State, t: &Theme, lane: &Lane) -> El {
    let busy = s.board.is_mutating();
    let id = lane.id.clone();
    let Some(edit) = s.board.compose.composers.get(&lane.id) else {
        let open = button(t, Variant::Outline, Size::Sm, None, "+ Add a card")
            .key(format!("board-lane-{id}-add"))
            .w_full()
            .border(1.0, t.border)
            .border_layout()
            .border_dash(3.0, 3.0);
        let open = if busy { disabled(open) } else { open.on_press(move |s: &mut State| s.board_begin_card(&id)) };
        return col().w_full().child(open);
    };
    let (a, b, c) = (id.clone(), id.clone(), id.clone());
    let field = text_input(edit, sans(14.0, 400.0, 1.4))
        .key(format!("board-composer-{id}-input"))
        .label("Add a card title")
        .single_line()
        .placeholder("Add a card title", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(10.0)
        .py(9.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .focus_ring(1.0, 0.0, t.accent)
        .disabled(busy)
        .on_edit(move |s: &mut State| s.board.compose.composers.entry(a.clone()).or_default())
        .on_input_change(move |s: &mut State, _: &mut Tx| s.board_card_typed(&b))
        .on_submit({
            let id = id.clone();
            move |s: &mut State, _: &mut Tx| s.board_submit_card(&id)
        })
        .on_input_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Escape => {
                s.board_cancel_card(&c);
                true
            }
            _ => false,
        });
    let empty = edit.text.trim().is_empty();
    let add = button(t, Variant::Default, Size::Sm, None, "Add card").key(format!("board-lane-{id}-submit"));
    let add = if busy || empty {
        disabled(add)
    } else {
        let id = id.clone();
        add.on_press(move |s: &mut State| s.board_submit_card(&id))
    };
    let cancel = button(t, Variant::Outline, Size::Sm, None, "Cancel").key(format!("board-lane-{id}-cancel"));
    let cancel = if busy {
        disabled(cancel)
    } else {
        let id = id.clone();
        cancel.on_press(move |s: &mut State| s.board_cancel_card(&id))
    };
    col()
        .key(format!("board-lane-{id}-form"))
        .w_full()
        .gap(8.0)
        .child(field)
        .child(row().flex_wrap().center_y().gap(6.0).child(add).child(cancel))
}

/// `.kanban-col__drop-slot`: 8 high with 4 above and below, between cards and at both ends. Piece O fills them.
fn slot(lane: &Lane, index: usize) -> El {
    el().key(format!("board-slot-{}-{index}", lane.id)).w_full().h(8.0).my(4.0).radius(999.0)
}

/// `.kanban-col__cards`: the cards in a column that scrolls, at least 180 high.
fn cards_area(s: &State, t: &Theme, lane: &Lane, cards: &[Card], clock: &RowClock) -> El {
    let mut area = col()
        .key(format!("board-lane-{}-cards", lane.id))
        .w_full()
        .grow(1.0)
        .min_h(180.0)
        .shrink()
        .scroll(ScrollSpec { stick_bottom: false })
        .pr(4.0)
        .child(slot(lane, 0));
    for (i, c) in cards.iter().enumerate() {
        area = area.child(card::view(s, t, c, clock)).child(slot(lane, i + 1));
    }
    if cards.is_empty() {
        let words = if lane.is_inbox { "Inbox is empty." } else { "Drop a card here or add one below." };
        area = area.child(
            col()
                .key(format!("board-lane-{}-empty", lane.id))
                .w_full()
                .items_center()
                .px(14.0)
                .py(18.0)
                .radius(RADIUS_CARD)
                .border(1.0, t.border)
                .border_layout()
                .border_dash(3.0, 3.0)
                .fill(t.board_faint_fill())
                .child(text(words).font(sans(11.0, 400.0, 1.5)).color(t.muted).wrap().align(rune::Align::Center)),
        );
    }
    area
}

pub fn view(s: &State, t: &Theme, lane: &Lane, cards: &[Card], clock: &RowClock, ends: (bool, bool)) -> El {
    let count = format!("{} card{}", cards.len(), if cards.len() == 1 { "" } else { "s" });
    let mut eyebrow = row().flex_wrap().center_y().gap(6.0);
    if lane.is_inbox {
        eyebrow = eyebrow.child(pill(t, format!("board-lane-{}-inbox-pill", lane.id), "Inbox", false));
    }
    eyebrow = eyebrow.child(pill(t, format!("board-lane-{}-count", lane.id), &count, true));
    // Renaming swaps the title for a field; a rename form outlives nothing but manage mode (`Compose::leave_manage`).
    let title = match s.board.compose.lane_renames.get(&lane.id) {
        Some(edit) => manage::lane_rename_field(s, t, lane, edit),
        None => {
            text(lane.name.clone()).key(format!("board-lane-{}-title", lane.id)).font(sans(16.0, 700.0, 1.5)).color(t.text).wrap_anywhere()
        }
    };
    let mut header = col().w_full().gap(8.0).child(col().w_full().gap(8.0).child(eyebrow).child(title));
    if s.board.is_manage_mode() {
        header = header.child(manage::lane_actions(s, t, lane, ends.0, ends.1));
    }
    let surface = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0)
        .padding(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.board_lane_fill())
        .child(header)
        .child(cards_area(s, t, lane, cards, clock))
        .child(composer(s, t, lane));
    col()
        .key(format!("board-lane-{}", lane.id))
        .role("region")
        .label(format!("{} column", lane.name))
        .w(WIDTH)
        .h_full()
        .min_h(0.0)
        .child(surface)
}
