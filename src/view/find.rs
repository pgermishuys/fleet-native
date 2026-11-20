//! The find bar of a conversation. Ports `components/session/ConversationFindBar.vue`: the query, "n of m", previous and
//! next, the scope chips, and "searching older…" while history pages load. It floats at the top right of the
//! conversation and doesn't push the stream. The logic is `conv::find` and `fleet::find`.
use crate::State;
use crate::conv::find::{FIELD, Paging, Scope};
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::text_menu::text_input;
use rune::{Color, Cursor, Cycle, El, Key, KeyEvent, Shadow, Span, Tx, col, el, icon, rich, row, text};

/// `.find-bar`: 420px at most, 18px from the right edge and 10px from the top.
const WIDTH: f32 = 420.0;

/// `.find-bar__icon`: 28 square, a 16px glyph, which hover tints with the accent at 14%.
fn icon_button(t: &Theme, key: &'static str, glyph: &'static rune::Icon, label: &'static str, off: bool) -> El {
    let b = el()
        .key(key)
        .size(28.0, 28.0)
        .center()
        .radius(6.0)
        .label(label)
        .help(label)
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(glyph, 16.0).color(t.text));
    if off {
        return b.opacity(0.4).disabled(true);
    }
    b.hover(t.accent.alpha(0.14)).cursor(Cursor::Pointer)
}

/// `.find-bar__chip`: a pill that reads as pressed with the accent border and a 14% accent fill.
fn chip(t: &Theme, key: &'static str, label: &'static str, pressed: bool) -> El {
    let (edge, ink) = if pressed { (t.accent, t.text) } else { (t.border, t.idle_text) };
    row()
        .key(key)
        .px(9.0)
        .py(2.0)
        .radius(999.0)
        .border(1.0, edge)
        .border_layout()
        .fill(if pressed { t.accent.alpha(0.14) } else { Color::TRANSPARENT })
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .label(label)
        .child(text(label).font(sans(12.0, 400.0, 1.5)).color(ink))
}

/// `.find-bar__stop`: an accent link, underlined.
fn link_button(t: &Theme, key: &'static str, label: &'static str, on_press: impl Fn(&mut State, &mut Tx) + 'static) -> El {
    let spans = vec![Span::new(0, label.len()).color(t.accent_text).underline(t.accent_text)];
    row()
        .key(key)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 0.0, t.accent)
        .label(label)
        .on_press_tx(on_press)
        .child(rich(label, spans).font(sans(12.0, 400.0, 1.5)).color(t.accent_text))
}

/// The status in the second row: older pages loading for the search, or the one that failed.
fn older(t: &Theme, paging: Paging) -> Option<El> {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    let font = sans(12.0, 400.0, 1.5);
    match paging {
        Paging::Quiet => None,
        Paging::Searching => Some(
            row()
                .key("find-older")
                .gap(5.0)
                .center_y()
                .child(icon(&LOADER_CIRCLE, 12.0).color(t.accent_text).cycle(Cycle {
                    period: 1.2,
                    phase: 0.0,
                    stops: &FADE,
                    still: 1.0,
                    fps: 30.0,
                }))
                .child(text("searching older\u{2026}").font(font).color(t.idle_text))
                .child(link_button(t, "find-stop-older", "Stop", |s, _| s.find_stop_older())),
        ),
        Paging::Failed => Some(
            row()
                .key("find-older-failed")
                .role("alert")
                .shrink()
                .min_w(0.0)
                .gap(5.0)
                .center_y()
                .child(text("couldn't load older messages \u{b7}").font(font).color(t.idle_text).wrap().shrink().min_w(0.0))
                .child(link_button(t, "find-retry-older", "Try again", |s, _| s.find_retry_older())),
        ),
    }
}

/// The bar, to lay over the top right of the conversation; `None` while it is closed.
pub fn bar(s: &State, t: &Theme) -> Option<El> {
    if !s.find.open {
        return None;
    }
    let find = &s.find;
    let none = find.hits.is_empty();
    let field = text_input(&find.query, sans(13.0, 400.0, 1.3))
        .key(FIELD)
        .label("Find in this conversation")
        .placeholder("Find in this conversation", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .h(30.0)
        .px(8.0)
        .py(5.5)
        .radius(6.0)
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(2.0, -1.0, t.accent)
        .on_edit(|s: &mut State| &mut s.find.query)
        .on_input_change(|s: &mut State, tx: &mut Tx| s.find_query_changed(tx))
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Escape => {
                s.find_close(tx);
                true
            }
            Key::Enter => {
                s.find_step(if ev.mods.shift { -1 } else { 1 }, tx);
                true
            }
            _ => false,
        });
    let count = row().min_w(56.0).justify_end().child(
        text(find.count_label()).key("find-count").font(sans(12.0, 400.0, 1.5).features("'tnum' 1")).color(t.idle_text).role("status"),
    );
    let top = row()
        .w_full()
        .gap(4.0)
        .center_y()
        .child(field)
        .child(count)
        .child(
            icon_button(t, "find-prev", &CHEVRON_UP, "Previous match", none).on_press_tx(|s: &mut State, tx: &mut Tx| s.find_step(-1, tx)),
        )
        .child(icon_button(t, "find-next", &CHEVRON_DOWN, "Next match", none).on_press_tx(|s: &mut State, tx: &mut Tx| s.find_step(1, tx)))
        .child(icon_button(t, "find-close", &X, "Close find", false).on_press_tx(|s: &mut State, tx: &mut Tx| s.find_close(tx)));
    let meta = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .child(
            chip(t, "find-scope-messages", "Messages", find.scope == Scope::Messages)
                .on_press_tx(|s: &mut State, tx: &mut Tx| s.find_set_scope(Scope::Messages, tx)),
        )
        .child(
            chip(t, "find-scope-tools", "+ Tool output", find.scope == Scope::Tools)
                .on_press_tx(|s: &mut State, tx: &mut Tx| s.find_toggle_tools(tx)),
        )
        .child(el().grow(1.0))
        .children(older(t, s.find_paging()));
    let card = col()
        .key("find-bar")
        .role("search")
        .label("Find in this conversation")
        .w_full()
        .max_w(WIDTH)
        .padding(8.0)
        .gap(6.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .box_shadow(Shadow { x: 0.0, y: 6.0, blur: 24.0, spread: 0.0, color: Color::BLACK.alpha(0.25), inset: false })
        .child(top)
        .child(meta);
    // Over the stream, not in the flow of it: `top: 10px; right: 18px`.
    Some(row().key("find-layer").absolute(0.0, 0.0).w_full().justify_end().pt(10.0).px(18.0).child(card))
}
