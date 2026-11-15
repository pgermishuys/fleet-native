//! A project header's right-click menu (`ProjectGroup.vue`, `ContextMenu` with `w-56`): New session, Rename (F2), Move up,
//! Move down and Delete…. Every item waits while a request is on its way (`isAnyActionPending`).
use crate::State;
use crate::Tx;
use crate::icons::*;
use crate::theme::Theme;
use crate::ui::menu::{Entry, Item, context_w};
use rune::El;

pub fn context(s: &State, t: &Theme, p: usize) -> Vec<El> {
    let busy = s.proj.pending;
    let mut entries = vec![
        Entry::Item(Item::new("New session").icon(&PLUS).disabled(busy).on(move |s: &mut State, tx: &mut Tx| s.new_session_in(p, tx))),
        Entry::Separator,
        Entry::Item(
            Item::new("Rename").icon(&PENCIL).shortcut("F2").disabled(busy).on(move |s: &mut State, tx: &mut Tx| s.start_rename(p, tx)),
        ),
    ];
    if s.can_move(p, true) {
        entries.push(Entry::Item(
            Item::new("Move up").icon(&ARROW_UP).disabled(busy).on(move |s: &mut State, tx: &mut Tx| s.move_project(p, true, tx)),
        ));
    }
    if s.can_move(p, false) {
        entries.push(Entry::Item(
            Item::new("Move down").icon(&ARROW_DOWN).disabled(busy).on(move |s: &mut State, tx: &mut Tx| s.move_project(p, false, tx)),
        ));
    }
    // The count in the dialog is what the group shows: the filtered rows (row 2.60, a Fleet quirk).
    entries.push(Entry::Separator);
    entries.push(Entry::Item(Item::new("Delete…").icon(&TRASH_2).destructive().disabled(busy).on(move |s: &mut State, _: &mut Tx| {
        let count = s.shown_count(p);
        s.open_delete(p, count);
    })));
    context_w(t, "pctx", s.ui.point, entries, Some(196.0))
}
