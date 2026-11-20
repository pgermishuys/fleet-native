//! An invented side conversation (`/btw`): a question and its answer, drawn in the panel above the composer.
use super::{Session, Status, session};
use crate::view::conversation::{Message, Part};

pub const QUESTION: &str = "Which file sets the page size?";

/// The side session as the panel draws it: what was asked and answered after the fork.
pub fn conversation() -> Session {
    session(
        "btw: Which file sets the page size?",
        Status::Idle,
        "",
        "",
        vec![
            Message::user("q1", QUESTION),
            Message::assistant(
                "r1",
                vec![Part::text(
                    "r1-t",
                    "The default is set in `src/paginate.py`, as `PAGE_SIZE = 25`. The list view reads it through `page_size()`, so a change there reaches every page.\n\n\
                     Say `show-limit` and I'll show you where it is used.",
                )],
            ),
        ],
    )
}
