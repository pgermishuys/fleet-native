//! The Board's manage mode (`KanbanColumn.vue` and `KanbanBoard.vue`): a lane's actions row and rename field, the lane creator,
//! "+ Add another lane", the header's "Rename board" and "Add lane" and the board's rename form. Drawn only in manage mode;
//! the state they keep is `fleet::board::manage`.
use crate::State;
use crate::fleet::board::Lane;
use crate::fleet::board::manage::Way;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::text_menu::text_input;
use rune::{El, Key, KeyEvent, TextEdit, Tx, col, el, row};

/// An input's padding (y, x) and the fill behind it.
pub(super) struct Look {
    pub pad: (f32, f32),
    pub fill: rune::Color,
}

/// One of Fleet's plain text inputs: single line, a 1 px border, `--radius-btn`, `padding` (y, x) round it, `fill` behind.
pub(super) fn field(t: &Theme, edit: &TextEdit, key: String, label: &str, placeholder: &str, look: Look, busy: bool) -> El {
    let Look { pad, fill } = look;
    text_input(edit, sans(14.0, 400.0, 1.4))
        .key(key)
        .label(label.to_string())
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .px(pad.1)
        .py(pad.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(fill)
        .focus_ring(1.0, 0.0, t.accent)
        .disabled(busy)
}

/// A button that does nothing while a change runs.
fn press(b: El, off: bool, go: impl Fn(&mut State) + 'static) -> El {
    if off { disabled(b) } else { b.on_press(go) }
}

/// `.kanban-col__lane-actions`: ← → Rename/Save, Make inbox/Inbox and Delete, 6 apart, wrapping.
pub fn lane_actions(s: &State, t: &Theme, lane: &Lane, left: bool, right: bool) -> El {
    let busy = s.board.is_mutating();
    let id = &lane.id;
    let renaming = s.board.compose.lane_renames.contains_key(id);
    let ghost = |key: String, words: &str| button(t, Variant::Ghost, Size::Sm, None, words).key(key);
    let mv = |way: Way, key: &str, words: &str, ok: bool| {
        let lane = id.clone();
        press(
            ghost(format!("board-lane-{id}-{key}"), words).label(if way == Way::Left { "Move lane left" } else { "Move lane right" }),
            busy || !ok,
            move |s| s.board_move_lane(&lane, way),
        )
    };
    let (a, b, c) = (id.clone(), id.clone(), id.clone());
    let rename = if renaming {
        press(ghost(format!("board-lane-{id}-save"), "Save"), busy, move |s| s.board_submit_lane_rename(&a))
    } else {
        press(ghost(format!("board-lane-{id}-rename"), "Rename"), busy, move |s| s.board_begin_lane_rename(&a))
    };
    let inbox = press(
        ghost(format!("board-lane-{id}-inbox"), if lane.is_inbox { "Inbox" } else { "Make inbox" }),
        busy || lane.is_inbox,
        move |s| s.board_make_inbox(&b),
    );
    let delete = button(t, Variant::Destructive, Size::Sm, None, "Delete").key(format!("board-lane-{id}-delete"));
    let delete =
        if busy { disabled(delete) } else { delete.on_press_tx(move |s: &mut State, tx: &mut Tx| s.board_delete_lane_ask(&c, tx)) };
    row()
        .key(format!("board-lane-{id}-actions"))
        .flex_wrap()
        .center_y()
        .gap(6.0)
        .child(mv(Way::Left, "left", "←", left))
        .child(mv(Way::Right, "right", "→", right))
        .child(rename)
        .child(inbox)
        .child(delete)
}

/// `.kanban-col__rename-input`: the lane's name as a field (120 at most, no placeholder, Esc cancels, Enter saves).
pub fn lane_rename_field(s: &State, t: &Theme, lane: &Lane, edit: &TextEdit) -> El {
    let id = lane.id.clone();
    let (a, b, c, d) = (id.clone(), id.clone(), id.clone(), id.clone());
    field(
        t,
        edit,
        format!("board-lane-{id}-rename-input"),
        "Lane name",
        "",
        Look { pad: (9.0, 10.0), fill: t.card_bg },
        s.board.is_mutating(),
    )
    .w_full()
    .on_edit(move |s: &mut State| s.board.compose.lane_renames.entry(a.clone()).or_default())
    .on_input_change(move |s: &mut State, _: &mut Tx| s.board_lane_rename_typed(&b))
    .on_submit(move |s: &mut State, _: &mut Tx| s.board_submit_lane_rename(&c))
    .on_input_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
        Key::Escape => {
            s.board_cancel_lane_rename(&d);
            true
        }
        _ => false,
    })
}

/// `.kanban-lane-creator`: padding 12 24 0, one card holding the input (240 wide at least, growing) and Save lane / Cancel.
/// Esc does nothing here, as in Fleet (FB12).
pub fn lane_creator(s: &State, t: &Theme) -> Option<El> {
    if !s.board.is_manage_mode() {
        return None;
    }
    let edit = s.board.compose.creator.as_ref()?;
    let busy = s.board.is_mutating();
    let input = field(
        t,
        edit,
        "board-creator-input".into(),
        "Add a lane name",
        "Add a lane name",
        Look { pad: (10.0, 12.0), fill: t.board_faint_fill() },
        busy,
    )
    .min_w(240.0)
    .grow(1.0)
    .shrink()
    .on_edit(|s: &mut State| s.board.compose.creator.get_or_insert_with(Default::default))
    .on_input_change(|s: &mut State, _: &mut Tx| s.board_creator_typed())
    .on_submit(|s: &mut State, _: &mut Tx| s.board_submit_creator());
    let save = button(t, Variant::Default, Size::Sm, None, "Save lane").key("board-creator-save");
    let save = press(save, busy || edit.text.trim().is_empty(), |s| s.board_submit_creator());
    let cancel =
        press(button(t, Variant::Outline, Size::Sm, None, "Cancel").key("board-creator-cancel"), busy, |s| s.board_cancel_creator());
    Some(
        row().key("board-lane-creator").role("region").label("Create lane").w_full().px(24.0).pt(12.0).child(
            row()
                .w_full()
                .flex_wrap()
                .center_y()
                .gap(10.0)
                .padding(12.0)
                .radius(RADIUS_CARD)
                .border(1.0, t.border)
                .border_layout()
                .fill(t.card_bg)
                .child(input)
                .child(row().flex_wrap().center_y().gap(8.0).child(save).child(cancel)),
        ),
    )
}

/// `.kanban-board__adder`: "+ Add another lane" after the last lane, 220 wide at least and as tall as the lanes.
pub fn adder(s: &State, t: &Theme) -> Option<El> {
    if !s.board.is_manage_mode() {
        return None;
    }
    let b = button(t, Variant::Outline, Size::Sm, None, "+ Add another lane")
        .key("board-adder-button")
        .w_full()
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0);
    Some(
        el().key("board-adder")
            .min_w(220.0)
            .h_full()
            .items_start()
            .child(col().w_full().child(press(b, s.board.is_mutating(), |s| s.board_open_creator()))),
    )
}

/// The header's own buttons in manage mode: "Rename board" (with a board, not while renaming) and "Add lane".
pub fn header_buttons(s: &State, t: &Theme) -> Vec<El> {
    let b = &s.board;
    if !b.is_manage_mode() {
        return Vec::new();
    }
    let first_load = b.is_loading && !b.is_loaded;
    let mut out = Vec::new();
    if b.has_board() && b.compose.board_rename.is_none() {
        let rename = button(t, Variant::Outline, Size::Sm, None, "Rename board").key("board-rename-board");
        out.push(press(rename, first_load || b.is_mutating(), |s| s.board_begin_board_rename()));
    }
    let add = button(t, Variant::Default, Size::Sm, None, "Add lane").key("board-add-lane");
    out.push(press(add, first_load, |s| s.board_open_creator()));
    out
}

/// "Create first lane", the empty state's action in manage mode.
pub fn create_first_lane(s: &State, t: &Theme) -> Option<El> {
    s.board.is_manage_mode().then(|| {
        button(t, Variant::Default, Size::Sm, None, "Create first lane")
            .key("board-empty-create")
            .on_press(|s: &mut State| s.board_open_creator())
    })
}

/// `.kanban-header__rename-form`, in the header's `heading` slot: the field (360 wide at most) over Save board and Cancel.
pub fn board_rename_form(s: &State, t: &Theme, edit: &TextEdit) -> El {
    let busy = s.board.is_mutating();
    let input = field(
        t,
        edit,
        "board-rename-input".into(),
        "Rename board",
        "Rename board",
        Look { pad: (10.0, 12.0), fill: t.board_faint_fill() },
        busy,
    )
    .w_full()
    .max_w(360.0)
    .on_edit(|s: &mut State| s.board.compose.board_rename.get_or_insert_with(Default::default))
    .on_input_change(|s: &mut State, _: &mut Tx| s.board_name_typed())
    .on_submit(|s: &mut State, _: &mut Tx| s.board_submit_board_rename())
    .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
        Key::Escape => {
            s.board_cancel_board_rename();
            true
        }
        _ => false,
    });
    let save = button(t, Variant::Default, Size::Sm, None, "Save board").key("board-rename-save");
    let save = press(save, busy || edit.text.trim().is_empty(), |s| s.board_submit_board_rename());
    let cancel =
        press(button(t, Variant::Outline, Size::Sm, None, "Cancel").key("board-rename-cancel"), busy, |s| s.board_cancel_board_rename());
    col().key("board-rename-form").gap(10.0).child(input).child(row().flex_wrap().gap(8.0).child(save).child(cancel))
}
