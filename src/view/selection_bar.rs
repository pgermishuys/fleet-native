//! The bar that replaces the footer while rows are picked (`SessionsPanel.vue` `.selection-bar`, rows 1.54-1.57): how many are
//! picked, Archive (Restore in the Archived view) and a button that lets them go.
use crate::icons::{ARCHIVE, ARCHIVE_RESTORE, X};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button_lead};
use crate::{State, Tx};
use rune::{Cursor, El, el, icon, row, text};

pub fn selection_bar(s: &State, t: &Theme) -> El {
    let restore = s.store.retention == crate::fleet::store::Retention::Archived;
    let (glyph, label, key) = if restore {
        (&ARCHIVE_RESTORE, "Restore", "session-selection-restore")
    } else {
        (&ARCHIVE, "Archive", "session-selection-archive")
    };
    // `.selection-bar__action`: 28 high, a pill, 6 between the icon (13) and the word.
    let action = button_lead(t, Variant::Default, Size::Sm, Some(icon(glyph, 13.0).color(t.primary_fg)), label)
        .key(key)
        .h(28.0)
        .radius(999.0)
        .gap(6.0)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| if restore { s.restore_selected() } else { s.archive_selected(tx) });
    let clear = el()
        .key("session-selection-clear")
        .role("button")
        .label("Clear selection")
        .help("Clear selection (Esc)")
        .size(28.0, 28.0)
        .center()
        .radius(7.0)
        .hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .on_press(|s: &mut State| s.selection.clear())
        .child(icon(&X, 14.0).color(t.muted).group_color(t.text));
    let bar = row()
        .key("session-selection-bar")
        .role("toolbar")
        .label("Selected sessions")
        .w_full()
        .padding(5.0)
        .pl(12.0)
        .gap(6.0)
        .center_y()
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_CARD)
        .fill(t.card_bg)
        .box_shadow(t.selection_shadow())
        .child(
            text(format!("{} selected", s.selection.count()))
                .key("session-selection-count")
                .font(sans(12.0, 600.0, 1.3).features("'tnum' 1"))
                .color(t.text)
                .grow(1.0),
        )
        .child(action)
        .child(clear);
    // `margin: 6px 8px 8px`, kept as the wrapper's padding so the bar's width is the panel's less the margins.
    el().w_full().px(8.0).pt(6.0).pb(8.0).child(bar)
}
