//! The Board's sources scenes: what GitHub's bookmarks are while the form is drawn (`mock::board::Case::Sources*`).
use super::bookmarks::Bookmark;
use super::state;
use crate::State;
use crate::mock::board::Case;

pub fn seed(s: &mut State, case: Case) {
    let g = state(&mut s.places);
    // Nothing asks a server in a scene: the bookmarks have been read already.
    g.bookmarks.has_loaded = true;
    let mark = |n: &str| Bookmark::of(n).expect("an owner and a name");
    match case {
        Case::Sources => g.bookmarks.list = vec![mark("acme-labs/spool"), mark("acme-labs/kite"), mark("acme-labs/windsock")],
        Case::SourcesError => g.bookmarks.error = Some("Failed to fetch bookmarks from server.".into()),
        _ => {}
    }
}
