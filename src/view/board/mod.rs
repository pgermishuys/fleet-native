//! The Board (component 23): `[board-controls][board]`. Ports `routes/board.tsx` (the off notice or the board) and the
//! context's layout, with `KanbanBoard.vue`'s page: the header and its states (`header`). The lanes and cards (`lane`, `card`)
//! in work mode. Manage mode, dragging, the controls (`BoardControlsPanel`) and the right panel come with later pieces.
mod card;
mod header;
mod lane;

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
            None,
        ));
    }
    let clock = RowClock::now(s);
    let lanes = b.lanes_with_cards().into_iter().map(|(l, cards)| lane::view(s, t, &l, &cards, &clock));
    // `.kanban-board`: 16 24 of padding, 16 between lanes, scrolling sideways only.
    el().key("board-lanes")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll_x()
        .child(row().key("board-lanes-row").h_full().items_stretch().gap(16.0).px(24.0).py(16.0).children(lanes))
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
        .children(header::toast(s, t))
        .children(header::banner(s, t))
        .child(body(s, t));
    sheet(t, "board").child(page)
}

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    labs_frame::page(s, t, Feature::Board, "board", labs_frame::panel(s, "board-controls", "Board controls"), board(s, t))
}
