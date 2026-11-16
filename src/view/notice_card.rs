//! The notice card (`components/notices/NoticeCard.vue`): an icon tile, the title and body, a close button, an optional
//! list, a countdown bar and the action buttons. Fixed 12 from the right and 40 from the bottom, just above the status
//! bar. Which card is open, when it opens and how long it stays are `fleet::notices`; this file only draws it and
//! hands presses back (row 7.5 to 7.8). The enter and settle animations and the lift over a composer are piece M's.
use crate::State;
use crate::fleet::notices::{Action, ActionTone, ItemKind, Notice, Tone};
use crate::icons::{CIRCLE_ARROW_UP, X};
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, sans};
use rune::{Color, Cursor, El, Key, KeyEvent, Link, LinkPaint, Shadow, Span, Tx, col, el, icon, rich, row, text};

/// Where the card sits, from the bottom-right corner.
const RIGHT: f32 = 12.0;
const BOTTOM: f32 = 40.0;
/// 264 wide, 340 with a list to read.
const WIDTH: f32 = 264.0;
const WIDTH_LIST: f32 = 340.0;
/// The icon tile's width and the gap after it: the list, the countdown and the actions line up with the text.
const INDENT: f32 = 36.0;

/// The open card, or none while no notice is open.
pub fn view(s: &State, t: &Theme) -> Option<El> {
    let n = s.notices.open()?;
    let width = if n.items.is_empty() { WIDTH } else { WIDTH_LIST };
    let mut card = col()
        .key("notice-card")
        .role("status")
        .label(n.title.clone())
        .w(width)
        .pt(12.0)
        .pb(10.0)
        .px(12.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .radius(RADIUS_PANEL)
        .box_shadow(Shadow { x: 0.0, y: 1.0, blur: 0.0, spread: 0.0, color: t.wash(4.0), inset: true })
        .box_shadow(Shadow { x: 0.0, y: 16.0, blur: 36.0, spread: -16.0, color: Color::BLACK.alpha(0.4), inset: false })
        .on_hover_change(|s: &mut State, inside: bool, _: &mut Tx| s.notices.set_hover(inside))
        .child(top(t, n));
    if !n.items.is_empty() {
        card = card.child(items(t, n));
    }
    if let Some(more) = n.more.as_ref().filter(|_| !n.items.is_empty()) {
        card = card.child(text(more.clone()).key("notice-more").font(sans(12.0, 400.0, 1.3)).color(t.muted).mt(8.0).pl(INDENT));
    }
    if let Some(left) = s.notices.countdown(s.clock) {
        card = card.child(countdown(t, left, width - 24.0));
    }
    if !n.actions.is_empty() {
        card = card.child(actions(t, &n.actions));
    }
    Some(col().key("notice-frame").absolute(0.0, 0.0).w_full().h_full().justify_end().items_end().pr(RIGHT).pb(BOTTOM).child(card))
}

/// The window's key handler, for Esc while focus is inside the card (`onCardKeydown`). True when it took the key.
pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.key != Key::Escape || s.notices.open().is_none() || !tx.focused().is_some_and(|k| k.starts_with("notice-")) {
        return false;
    }
    s.notices.escape();
    true
}

/// × and Esc: the card settles now, into its chip if it has one.
fn dismiss(s: &mut State) {
    s.notices.settle();
}

fn top(t: &Theme, n: &Notice) -> El {
    let (tint, ink) = match n.tone {
        Tone::Accent => (t.accent_dim, t.accent),
        Tone::Warn => (t.status_waiting().alpha(0.15), t.status_waiting()),
    };
    let glyph = n.icon.map_or(&CIRCLE_ARROW_UP, |i| i.0);
    let tile = el().key("notice-icon").size(26.0, 26.0).center().radius(RADIUS_BTN).fill(tint).child(icon(glyph, 15.0).color(ink));
    let mut words =
        col().grow(1.0).shrink().child(text(n.title.clone()).key("notice-title").font(sans(13.0, 600.0, 1.35)).color(t.text).wrap());
    if n.body.is_some() || n.link.is_some() {
        words = words.child(body(t, n));
    }
    // The close button sits 4 px up and right of the row, so its corner meets the card's padding.
    let close = row()
        .key("notice-dismiss")
        .role("button")
        .label("Dismiss")
        .help("Dismiss (Esc)")
        .size(22.0, 22.0)
        .center()
        .radius(RADIUS_BTN - 2.0)
        .mt(-4.0)
        .mr(-4.0)
        .hover(t.wash(6.0))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .on_press(dismiss)
        .child(icon(&X, 14.0).color(t.muted));
    row().w_full().gap(10.0).child(tile).child(words).child(close)
}

/// The body in muted 12 px, with the link running on from its last word, underlined.
fn body(t: &Theme, n: &Notice) -> El {
    let mut words = n.body.clone().unwrap_or_default();
    let mut spans = Vec::new();
    let mut link = None;
    if let Some(l) = &n.link {
        if !words.is_empty() {
            words.push(' ');
        }
        let range = words.len()..words.len() + l.label.len();
        words.push_str(&l.label);
        spans.push(Span::new(range.start, range.end).color(t.muted).underline(t.muted.alpha(0.55)));
        link = Some(Link::new(range, l.href.clone()).hover(LinkPaint::new().color(t.text)).focused(LinkPaint::new().ring(t.accent, 2.0)));
    }
    let mut p = rich(words, spans).key("notice-body").font(sans(12.0, 400.0, 1.45)).color(t.muted).w_full().wrap().mt(2.0);
    if let Some(link) = link {
        p = p.link(link);
    }
    p.on_link(|s: &mut State, url: &str, tx: &mut Tx| {
        // A link Fleet handles itself runs instead of opening the browser, and the card settles (`followLink`).
        match s.notices.open().and_then(|n| n.link.as_ref()).and_then(|l| l.run.clone()) {
            Some(run) => {
                s.notices.tell(&run);
                s.notices.settle();
            }
            None => {
                let _ = tx.open_url(url);
            }
        }
    })
}

fn items(t: &Theme, n: &Notice) -> El {
    let mut list = col().key("notice-items").w_full().gap(7.0).mt(10.0).pl(INDENT);
    for (i, item) in n.items.iter().enumerate() {
        let dot = if item.kind == Some(ItemKind::Fixed) { t.running } else { t.accent };
        let mut words = String::new();
        let mut spans = Vec::new();
        if let Some(label) = &item.label {
            words.push_str(label);
            words.push(' ');
            spans.push(Span::new(0, words.len()).size(11.5).color(t.muted));
        }
        words.push_str(&item.text);
        let line = rich(words, spans).font(sans(12.5, 400.0, 1.4)).color(t.text).wrap().grow(1.0).shrink();
        let marker = el().key(format!("notice-item-dot-{i}")).size(5.0, 5.0).radius(999.0).fill(dot).mt(5.0);
        list = list.child(row().key(format!("notice-item-{i}")).w_full().gap(8.0).child(marker).child(line));
    }
    list
}

/// The time left to act (Undo), draining left to right.
fn countdown(t: &Theme, left: f32, width: f32) -> El {
    el().key("notice-countdown")
        .w(width)
        .h(3.0)
        .mt(10.0)
        .mb(2.0)
        .radius(3.0)
        .fill(t.border)
        .clip_rounded(3.0)
        .child(el().key("notice-countdown-bar").w(width * left).h(3.0).fill(t.accent))
}

fn actions(t: &Theme, actions: &[Action]) -> El {
    let mut r = row().key("notice-actions").w_full().wrap().gap(6.0).mt(10.0).pl(INDENT);
    for (i, a) in actions.iter().enumerate() {
        let (fill, hover, ink) = match a.tone {
            ActionTone::Primary => (t.accent, t.mix(t.accent, Color::BLACK, 0.12), t.primary_fg),
            ActionTone::Danger => (t.error.alpha(0.16), t.error.alpha(0.24), t.error_text),
            ActionTone::Quiet => (Color::TRANSPARENT, t.wash(6.0), t.muted),
        };
        // The text above it is level with a lone quiet button's label, not its edge.
        let lead = if i == 0 && a.tone == ActionTone::Quiet { -10.0 } else { 0.0 };
        let run = a.run.clone();
        let mut b = row()
            .key(format!("notice-action-{}", a.label))
            .role("button")
            .label(a.label.clone())
            .h(26.0)
            .px(10.0)
            .gap(5.0)
            .center_y()
            .ml(lead)
            .radius(RADIUS_BTN - 1.0)
            .fill(fill)
            .hover(hover)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 1.0, t.accent)
            .on_press(move |s: &mut State| s.notices.tell(&run));
        if let Some(i) = a.icon {
            b = b.child(icon(i.0, 12.0).color(ink));
        }
        r = r.child(b.child(text(a.label.clone()).font(sans(12.0, 500.0, 1.2)).color(ink)));
    }
    r
}
