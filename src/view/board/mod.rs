//! The Board (component 23): `[board-controls][board]`. Ports `routes/board.tsx` (the off notice or the board) and the
//! context's layout, with `KanbanBoard.vue`'s page: the header and its states (`header`). The lanes and cards (`lane`, `card`)
//! and, in manage mode, what `manage` adds. Dragging, the controls (`BoardControlsPanel`) and the right panel come with later
//! pieces.
mod activity;
mod card;
mod header;
mod lane;
mod manage;
mod right_panel;
pub mod sources;
mod summary;

use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::ui::empty_state;
use crate::view::labs_frame;
use crate::view::session_row::RowClock;
use crate::view::workflows::sheet;
use rune::{El, col, el, row};

/// The body under the header: the wait on the first load, "No lanes yet" when there are none, else the lanes' row. A first load
/// that failed is neither (FB17): the banner over an empty row.
fn body(s: &State, t: &Theme) -> El {
    let b = &s.board;
    if b.is_loading && !b.is_loaded {
        return header::loading(t);
    }
    if b.is_loaded && b.lanes.is_empty() {
        // Fleet's `EmptyState` is `flex-1` but starts at the top, right under the tiles.
        return col().key("board-empty").w_full().grow(1.0).min_h(0.0).items_center().child(empty_state::full(
            t,
            "board-empty-state",
            &crate::icons::LAYOUT_GRID,
            "No lanes yet",
            "Click Edit Board to unlock lane management, then add your first board column.",
            manage::create_first_lane(s, t),
        ));
    }
    let clock = RowClock::now(s);
    let all = b.lanes_with_cards();
    let last = all.len().saturating_sub(1);
    let lanes = all.iter().enumerate().map(|(i, (l, cards))| lane::view(s, t, l, cards, &clock, (i > 0, i < last)));
    // `.kanban-board`: 16 24 of padding, 16 between lanes, scrolling sideways only.
    el().key("board-lanes").w_full().grow(1.0).min_h(0.0).scroll_x().child(
        row().key("board-lanes-row").h_full().items_stretch().gap(16.0).px(24.0).py(16.0).children(lanes).children(manage::adder(s, t)),
    )
}

/// `KanbanBoard`: fills the sheet edge to edge (`margin: -24px` cancels the centre's padding), so only the lanes scroll.
fn board(s: &State, t: &Theme) -> El {
    let page = col()
        .key("board-screen")
        .label("Kanban board")
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(header::bar(s, t))
        .child(header::tiles(s, t))
        .children(sources::view(s, t))
        .children(manage::lane_creator(s, t))
        .children(header::toast(s, t))
        .children(header::banner(s, t))
        .child(body(s, t));
    // Fleet's workspace sheet holds the page, then the gutter and the right panel; its width is what the panel's rules measure.
    sheet(t, "board")
        .on_layout(|s: &mut State, [_, _, w, h]| {
            s.sheet_w = w;
            s.sheet_h = h;
        })
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .items_stretch()
                .child(col().grow(1.0).min_w(0.0).h_full().min_h(0.0).child(page))
                .children(right_panel::columns(s, t)),
        )
}

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    labs_frame::page(s, t, Feature::Board, "board", labs_frame::panel(s, "board-controls", "Board controls"), board(s, t))
}
