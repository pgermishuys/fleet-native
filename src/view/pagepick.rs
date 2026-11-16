//! The card the conversation shows while the agent waits for a pick on a page it showed. Ports
//! `components/session/PagePickCard.vue` (rows 6.1-6.4 and 8.1's slot of `docs/parity/04-questions-and-permissions.md`).
//! The pages are web content (component 14): the pick is made on the page in its tab, so the card is the way to it.
use crate::State;
use crate::fleet::pages::PageCanvas;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, col, el, icon, row, text};

/// Whether the card shows at all: a page waits, and this is the session's own conversation (`v-if="!props.after"`: the side
/// conversation has no card of its own).
pub fn shown(waiting: Option<&PageCanvas>, in_side: bool) -> Option<&PageCanvas> {
    waiting.filter(|_| !in_side)
}

/// The card for the open session, if a page waits on a pick in its own conversation.
pub fn card(t: &Theme, s: &State) -> Option<El> {
    // The main stream is the only one drawn here; the side conversation (component 15) passes `true` from its own.
    shown(s.waiting_page(), false).map(|page| frame(t, page))
}

/// `.page-pick`: icon, the two lines, and Open page.
fn frame(t: &Theme, page: &PageCanvas) -> El {
    let id = page.id.clone();
    // Fleet shows `waitingPage.page?.title`: a page without one leaves the line starting at the dot.
    let line = format!("{} \u{b7} or type a message to answer instead", page.title);
    // Not `ask_card::frame`: that is a grid with a gap of 12, this one is a row with a gap of 10.
    let body = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text("Pick on the page").key("page-pick-title").font(sans(13.0, 600.0, 1.3)).color(t.text))
        .child(text(line).key("page-pick-line").font(sans(12.0, 400.0, 1.4)).color(t.muted).truncate().mt(2.0));
    let open = el()
        .key("page-pick-open")
        .px(10.0)
        .py(4.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .hover(t.accent_dim)
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .on_press(move |s: &mut State| s.open_page(&id))
        .child(text("Open page").font(sans(12.0, 400.0, 1.3)).color(t.text));
    row()
        .key("page-pick-card")
        .w_full()
        .gap(10.0)
        .center_y()
        .mt(10.0)
        .padding(12.0)
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.border_mix(t.status_waiting(), 40.0))
        .border_layout()
        .fill(super::toolcard::mix(t.card_bg, t.status_waiting(), 0.05))
        .child(icon(&PANELS_TOP_LEFT, 16.0).color(t.status_waiting()))
        .child(body)
        .child(open)
}
