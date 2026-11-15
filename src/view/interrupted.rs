//! The card at the end of a conversation a Fleet restart cut off. Ports `components/session/InterruptedCard.vue` and
//! `SessionInterruptedCard.vue` (what loads it is `fleet::interrupted`; what it says is `conv::working`).
use super::message::now_ms;
use super::toolcard::mix;
use crate::State;
use crate::conv::retry::format_when;
use crate::conv::time::local_offset_secs;
use crate::conv::working::interruption_lines;
use crate::fleet::rows::Interruption;
use crate::fleet::work::parse_iso_ms;
use crate::theme::{RADIUS_CARD, Theme, sans};
use rune::{Cursor, El, Span, Tx, col, el, rich, row, text};

/// `.interrupted` (rows 1.83-1.85).
pub fn card(t: &Theme, s: &State, i: &Interruption) -> El {
    let act = s.interruption_act();
    let now = now_ms(s);
    let clock = format_when(parse_iso_ms(&i.at).unwrap_or(now), now, local_offset_secs());
    let tone = t.idle;
    let mut card = col()
        .key("interrupted-card")
        .w_full()
        .mt(10.0)
        .px(12.0)
        .py(11.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(tone, 40.0))
        .border_layout()
        .fill(mix(t.panel_bg, tone, 0.09))
        .label("Cut off by a restart")
        .child(
            row()
                .gap(7.0)
                .center_y()
                .mb(3.0)
                .child(el().size(9.0, 9.0).radius(4.5).fill(tone))
                .child(text("Cut off by a restart").font(sans(13.0, 600.0, 1.3)).color(t.text)),
        );
    for (n, para) in interruption_lines(i, &clock).into_iter().enumerate() {
        let key = match (n, i.waiting) {
            (0, _) => "interrupted-text",
            (1, false) if i.last_call.is_some() => "interrupted-last-call",
            _ => "interrupted-files",
        };
        let spans = para.bold.iter().map(|r| Span::new(r.start, r.end).color(t.text).weight(500.0)).collect();
        card = card.child(rich(para.text, spans).key(key).font(sans(12.5, 400.0, 1.3)).color(t.muted).my(3.0).wrap().w_full());
    }
    if let Some(why) = &act.error {
        card =
            card.child(text(why.clone()).key("interrupted-error").font(sans(12.0, 400.0, 1.3)).color(t.error_text).my(3.0).wrap().w_full());
    }
    if i.auto {
        card = card
            .child(text("Fleet is continuing this by itself.").key("interrupted-auto").font(sans(12.5, 400.0, 1.3)).color(t.muted).my(3.0));
    }
    let mut actions = row().wrap().gap(6.0).mt(9.0);
    if !i.auto {
        let label = if i.waiting { "Ask again" } else { "Continue" };
        actions =
            actions.child(button(t, "interrupted-continue", label, true, !act.busy, |s: &mut State, _: &mut Tx| s.interruption_continue()));
    }
    actions =
        actions.child(button(t, "interrupted-dismiss", "Dismiss", false, !act.busy, |s: &mut State, _: &mut Tx| s.interruption_dismiss()));
    card.child(actions)
}

/// `.interrupted__button`: a 12.5px pill, 5px 12px. The primary one is solid accent; the other's border turns to the
/// accent on hover, drawn as a 1px ring that takes the hover fill, since Rune has no border colour for hover.
fn button(t: &Theme, key: &str, label: &'static str, primary: bool, enabled: bool, press: impl Fn(&mut State, &mut Tx) + 'static) -> El {
    let (fill, ink) = if primary { (t.accent, t.primary_fg) } else { (t.panel_bg, t.text) };
    let inner = row()
        .px(11.0)
        .py(4.0)
        .center_y()
        .radius(999.0)
        .fill(fill)
        .child(text(label).font(sans(12.5, 400.0, 1.3)).color(ink).group_color(if primary { t.primary_fg } else { t.accent_text }));
    let ring = row().key(key).px(1.0).py(1.0).radius(999.0).fill(if primary { t.accent } else { t.border }).label(label).child(inner);
    if !enabled {
        return ring.opacity(0.5).disabled(true);
    }
    // Hover: the solid button goes to 90% opacity over the card; the plain one's ring takes the accent.
    let hover = if primary { mix(t.panel_bg, t.accent, 0.9) } else { t.accent };
    ring.hover_group().hover(hover).cursor(Cursor::Pointer).on_press_tx(press)
}
