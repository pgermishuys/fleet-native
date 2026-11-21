//! An open automation's page (`AutomationDetailPanel.vue`, the existing-automation branch): the bar with its name, the switch
//! and the line beside it, Run now and the ⋯ menu, and the body under it, which starts with the note the actions leave. The
//! runs go under it; the composer goes under the body (`composer`), with the form error over it.
use crate::State;
use crate::fleet::automations::actions::header_meta;
use crate::fleet::automations::model::Automation;
use crate::fleet::automations::nav::Notice;
use crate::icons::{CIRCLE_ALERT, ELLIPSIS, PLAY, TRASH_2};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled, icon_button};
use crate::ui::controls::switch_with;
use crate::ui::menu::{self, Entry, Item};
use crate::ui::{overlay, page_header, rem};
use crate::{Tx, ui};
use rune::{Align, El, Side, col, icon, row, text};

/// The ⋯ button's key, which the menu anchors to.
pub const MORE: &str = "automation-more-menu";

/// The bar: name, then the switch and its words, and at the end Run now and ⋯.
pub fn header(s: &State, t: &Theme, a: &Automation) -> El {
    // `.automation-page__switch[data-state="unchecked"]`: the off track is the text colour at 22 %, so "off" shows.
    let track = switch_with(t, a.is_enabled, t.text.alpha(0.22)).key("automation-enabled").label(if a.is_enabled { "On" } else { "Off" });
    let track =
        if s.automations_nav.toggling { disabled(track) } else { track.on_press_tx(|s: &mut State, tx: &mut Tx| s.automation_toggle(tx)) };
    let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
    let words =
        text(header_meta(a, &tz)).key("automation-header-meta").font(sans(12.0, 400.0, 1.5)).color(t.muted).shrink().min_w(0.0).truncate();
    let run_now = button(t, Variant::Outline, Size::Sm, Some(&PLAY), "Run now")
        .key("automation-run-now")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.automation_run_now(tx));
    let more = icon_button(t, Variant::Outline, Size::IconSm, &ELLIPSIS, "More")
        .key(MORE)
        .on_press(|s: &mut State| s.ui.open = Some(ui::Open::Menu("automation-more")));
    page_header::bar_with(t, "automation-header", &a.name, vec![track, words], vec![run_now, more])
}

/// The ⋯ menu: Delete, in the error colour, aligned to the button's end and 6 px under it.
pub fn more_menu(t: &Theme) -> Vec<El> {
    let entries =
        vec![Entry::Item(Item::new("Delete").id("automation-delete").icon(&TRASH_2).destructive().on(|s: &mut State, tx: &mut Tx| {
            s.ui.open = None;
            s.automation_delete_ask(tx);
        }))];
    let width = menu::fit_width(&entries);
    let panel = menu::panel(t, "automation-more-pop", entries)
        .w(width)
        .focus_return(MORE.to_string())
        .modal(true, |s: &mut State| s.ui.open = None);
    vec![
        overlay::scrim("automation-more-pop-scrim", |s: &mut State| s.ui.open = None),
        overlay::anchored(panel, MORE, Side::Bottom, Align::End, 6.0),
    ]
}

/// What the body holds, from the top: the note (`.automation-page__notice`) and what later pieces add.
pub fn body(t: &Theme, s: &State, a: &Automation) -> El {
    let mut body = col().w_full().items_center();
    if let Some(note) = &s.automations_nav.notice {
        body = body.child(notice(t, note, 12.0));
    }
    if let Some(runs) = super::runs::section(s, t, a) {
        body = body.child(runs);
    }
    body
}

/// The note: 760 wide at most, centred, 12 under; info in the muted colours, an error in the error ones with its glyph.
pub(super) fn notice(t: &Theme, note: &Notice, below: f32) -> El {
    note_box(t, "automation-action-message", note, below)
}

/// `.automation-page__error`: why the server refused a create or save, over the composer, in the error colours; as wide as
/// the composer's parts (100 % less its 24 px sides, 760 at most) and 8 above it (row 4.11).
pub(super) fn form_error(t: &Theme, words: &str) -> El {
    let note = Notice { error: true, text: words.to_string(), until: None };
    row().w_full().px(24.0).justify_center().child(note_box(t, "automation-form-error", &note, 8.0))
}

fn note_box(t: &Theme, key: &str, note: &Notice, below: f32) -> El {
    let ink = if note.error { t.error_text } else { t.muted };
    let mut note_row = row()
        .key(key.to_string())
        .role(if note.error { "alert" } else { "status" })
        .w_full()
        .max_w(760.0)
        .gap(8.0)
        .items_start()
        .mb(below)
        .px(12.0)
        .py(9.0)
        .radius(RADIUS_CARD)
        .border(1.0, if note.error { t.error.alpha(0.3) } else { t.border })
        .border_layout();
    if note.error {
        note_row = note_row.fill(t.error.alpha(0.1)).child(icon(&CIRCLE_ALERT, rem(1.0)).color(ink).mt(2.0));
    }
    note_row.child(
        text(note.text.clone()).key(format!("{key}-text")).font(sans(12.5, 400.0, 1.5)).color(ink).wrap().grow(1.0).shrink().min_w(0.0),
    )
}
