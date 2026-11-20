//! The side conversation's panel (`/btw`): a card docked above the composer. Ports `components/session/SideConversationPanel.vue`
//! (its header, its body and the fold); the tab it folds into, the peek and Undo are `side_tab`. The body draws the side session's
//! conversation with the same stream as the main one (`conversation::stream`, `Which::Side`).
use super::conversation::{COLUMN, Scroll, Which, stream};
use crate::State;
use crate::icons::*;
use crate::theme::{RADIUS_PANEL, Theme, sans};
use crate::ui::button::disabled;
use crate::ui::rem;
use rune::{Cursor, El, Icon, col, el, icon, row, text};
use std::rc::Rc;

/// `.side-conversation__card`: `height: min(42vh, 400px)`.
pub const CARD_MAX_H: f32 = 400.0;
const CARD_SHARE: f32 = 0.42;
/// `@media (max-width: 640px)`.
const NARROW: f32 = 640.0;

pub const HINT: &str = "A fork at the last finished turn \u{b7} the session carries on";
pub const FORKING: &str = "Forking the session at its last finished turn\u{2026}";
pub const KEEP_TIP: &str = "Keep as a session of its own";
pub const DISCARD_LABEL: &str = "Discard the side conversation";
pub const DISCARD_TIP: &str = "Discard (you can undo it for a few seconds)";

/// The title without its `btw: ` prefix (`/^btw:\s*/`).
pub fn plain_title(title: &str) -> &str {
    title.strip_prefix("btw:").map_or(title, str::trim_start)
}

/// `Ctrl /`, or `⌘ /` on a Mac: what the minimize button's tip names.
pub fn shortcut() -> &'static str {
    if cfg!(target_os = "macos") { "\u{2318} /" } else { "Ctrl /" }
}

fn narrow(s: &State) -> bool {
    let w = s.ui.sel.window.0;
    w > 0.0 && w <= NARROW
}

/// The card's height: 42 % of the window's, at most 400. Before the window is measured, the most.
fn card_height(s: &State) -> f32 {
    let h = s.ui.sel.window.1;
    if h <= 0.0 { CARD_MAX_H } else { (h * CARD_SHARE).min(CARD_MAX_H) }
}

/// A header button with a label (`variant="toolbar-icon" size="sm"` at 26 high and 12px): the icon, then the words.
fn action(t: &Theme, key: &'static str, glyph: &'static Icon, words: &str, tip: String, show_words: bool) -> El {
    let mut b = row()
        .key(key)
        .h(26.0)
        .px(rem(0.625))
        .gap(rem(0.375))
        .center()
        .radius(crate::theme::RADIUS_BTN)
        .hover(t.wash(6.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .label(words.to_string())
        .help(tip)
        .child(icon(glyph, 14.0).color(t.muted).group_color(t.text));
    if show_words {
        b = b.child(text(words.to_string()).font(sans(12.0, 500.0, 1.4)).color(t.muted).group_color(t.text));
    }
    b
}

/// The × (`size="toolbar"`, 28 square); dimmed and without a press while the fork is being made.
fn close_button(t: &Theme, key: &'static str, can: bool) -> El {
    let b = el()
        .key(key)
        .size(rem(1.75), rem(1.75))
        .center()
        .radius(crate::theme::RADIUS_BTN)
        .label(DISCARD_LABEL)
        .help(DISCARD_TIP)
        .child(icon(&X, 14.0).color(t.muted).group_color(t.text));
    if !can {
        return disabled(b);
    }
    b.hover(t.wash(6.0)).hover_group().cursor(Cursor::Pointer).on_press(|s: &mut State| s.side_discard())
}

fn header(s: &State, t: &Theme, title: &str, forked: bool) -> El {
    let words = !narrow(s);
    let font = sans(12.0, 400.0, 1.4);
    let mut head = row()
        .key("side-head")
        .w_full()
        .items_center()
        .gap(8.0)
        .pl(14.0)
        .pr(8.0)
        .py(8.0)
        .min_w(0.0)
        .child(icon(&MESSAGE_CIRCLE_QUESTION_MARK, 14.0).color(t.accent))
        .child(text("btw").font(sans(12.0, 600.0, 1.4)).color(t.accent_text))
        .child(
            text(title.to_string())
                .key("side-title")
                .font(sans(12.0, 500.0, 1.4))
                .color(t.text)
                .truncate()
                .shrink()
                .help(title.to_string()),
        );
    // The hint grows to fill; hidden at 640 wide and under, the buttons keep only their icons.
    head = if words {
        head.child(text(HINT).key("side-hint").font(font).color(t.muted).truncate().grow(1.0).shrink())
    } else {
        head.child(el().grow(1.0))
    };
    if forked {
        head = head
            .child(
                action(t, "side-keep", &SQUARE_ARROW_OUT_UP_RIGHT, "Keep as session", KEEP_TIP.into(), words)
                    .on_press_tx(|s: &mut State, tx| s.side_keep(tx)),
            )
            .child(
                action(
                    t,
                    "side-minimize",
                    &MINUS,
                    "Minimize",
                    format!("Minimize ({}); the composer goes back to the session", shortcut()),
                    words,
                )
                .on_press_tx(|s: &mut State, tx| s.side_set_minimized(true, tx)),
            );
    }
    head.child(close_button(t, "side-close", forked))
}

fn body(s: &State, t: &Theme, forked: bool) -> El {
    if !forked {
        return col()
            .key("side-body")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .center()
            .child(text(FORKING).key("side-forking").role("status").font(sans(13.0, 400.0, 1.4)).color(t.muted));
    }
    let (conv, plan) = s.side_conv();
    // Only what follows the boundary shows; with an invented conversation there is nothing before it. Not here (a side conversation's
    // own): the find bar, the page-pick card, "older messages" (rows 3.4, 3.5).
    // The side session's own permission asks wait in the panel and are answered there; its questions are cards in its messages.
    let asks = s.side_session_id().map(|id| s.asks.of(&id).to_vec()).unwrap_or_default();
    let asks = asks.iter().enumerate().map(|(n, ask)| super::permission::card(t, s, ask, n == 0 && s.open_asks().is_empty())).collect();
    let scroll = Scroll { jump: false, probe: Rc::default(), find: None, asks };
    let stream = stream(t, Which::Side, conv, plan, false, None, scroll);
    col().key("side-body").w_full().grow(1.0).min_h(0.0).child(stream)
}

/// The section above the composer: the card while it is open, folding to nothing (the tab is `side_tab`'s). Nothing at all with no
/// side conversation.
pub fn panel(s: &State, t: &Theme) -> Option<El> {
    let st = s.side_shown()?;
    let forked = st.side.is_some();
    let title = st.side.as_ref().map(|side| plain_title(&side.title).to_string()).or_else(|| st.starting.clone()).unwrap_or_default();
    let share = s.side_open_share();
    let folded = st.minimized();
    let radius = RADIUS_PANEL + 2.0;
    let card = col()
        .key("side-card")
        .w_full()
        .max_w(COLUMN)
        .h(card_height(s))
        .fill(t.card_bg)
        .border(1.0, t.accent_border(45.0))
        .border_layout()
        .radius(radius)
        .clip_rounded(radius)
        .child(header(s, t, &title, forked))
        .child(el().w_full().h(1.0).fill(t.border))
        .child(body(s, t, forked));
    // The fold: the card keeps its size and the box around it shrinks to nothing, so the stream stays where it was.
    let fold = col().key("side-fold").w_full().items_center().h(card_height(s) * share).min_h(0.0).clip_rounded(0.0).child(card);
    // Folded all the way it leaves nothing, not even the card's top border in the 1 px its box keeps.
    let fold = if share <= 0.0 { fold.opacity(0.0) } else { fold };
    let section = col()
        .key("side-conversation")
        .role("region")
        .label("Side conversation")
        .w_full()
        .items_center()
        .pt(if folded { 0.0 } else { 6.0 })
        .px(if narrow(s) { 12.0 } else { 24.0 })
        .child(fold);
    // Folded, the tab takes its place on the composer's edge (piece D, `view::side_tab`).
    // The tab paints over the composer's top edge (z-index 2): the composer comes after it in the column.
    let section = if folded && forked {
        section.paint_order(2).child(super::side_tab::tab(s, t, st, (s.sheet_w - 2.0 * if narrow(s) { 12.0 } else { 24.0 }).max(0.0)))
    } else {
        section
    };
    Some(section)
}
