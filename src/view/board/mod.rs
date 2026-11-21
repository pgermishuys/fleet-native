//! The Board (component 23): `[board-controls][board]`. Ports `routes/board.tsx` (the off notice or the board) and the
//! context's layout. The controls (`BoardControlsPanel`), the board and its right panel come with later pieces.
use crate::State;
use crate::fleet::labs::Feature;
use crate::theme::Theme;
use crate::view::labs_frame;

pub fn page(s: &State, t: &Theme) -> Vec<rune::El> {
    labs_frame::page(
        s,
        t,
        Feature::Board,
        "board",
        labs_frame::panel(s, "board-controls", "Board controls"),
        labs_frame::empty_page(t, "board"),
    )
}
