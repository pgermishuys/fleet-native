//! The card under a message whose turn stopped on a failure, and the bar inside it that says when Fleet tries again.
//! Ports the `.turn-failure` block of `components/session/ActivityStream.vue`, `TurnFailureRetry.vue` and
//! `TurnRetryBar.vue`. `conv::turn_error` and `conv::retry` say what it reads; `fleet::retry` is what its buttons do.
use super::conversation::{Failure, Message, Role};
use super::message::now_ms;
use super::toolcard::mix;
use crate::State;
use crate::conv::retry::{limit_title, retry_when, retry_why};
use crate::conv::time::local_offset_secs;
use crate::conv::turn_error::{split_turn_error_message, url_ranges};
use crate::fleet::rows::ScheduledRetry;
use crate::icons::*;
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, El, Icon, Link, ScrollSpec, Span, Tx, col, el, icon, rich, row, text};

/// What a card knows about the conversation around it.
pub struct Around {
    /// It is the newest message: only that failure can be tried again.
    pub latest: bool,
    /// There is a user prompt to send again.
    pub has_prompt: bool,
    /// The composer would take a prompt (`canSend`).
    pub can_send: bool,
}

/// `.turn-failure` (rows 1.62-1.70).
pub fn card(t: &Theme, s: &State, msg: &Message, f: &Failure, around: &Around) -> El {
    let id = &msg.id;
    let limit = limit_title(f.kind.as_deref());
    let tone = if limit.is_some() { t.idle } else { t.error };
    let text_of = split_turn_error_message(&f.message);
    let mut card = col()
        .key(format!("turn-failure-{id}"))
        .w_full()
        .mt(4.0)
        .mb(10.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(tone, 35.0))
        .border_layout()
        .fill(mix(t.card_bg, tone, if limit.is_some() { 0.06 } else { 0.07 }))
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon(&TRIANGLE_ALERT, 14.0).color(tone))
                .child(text(limit.unwrap_or("This turn stopped early")).font(sans(13.0, 600.0, 1.3)).color(t.text)),
        )
        .child(summary(t, id, &text_of.summary));
    if let Some(details) = &text_of.details {
        card = card.child(details_block(t, id, details, s.failure_open.contains(id)));
    }
    // The bar is only for a limit, on the newest message, and only while Fleet has a retry waiting.
    let retry = if f.kind.is_some() && around.latest { s.retry() } else { None };
    if let Some(retry) = retry {
        card = card.child(retry_bar(t, s, retry));
    }
    card.child(foot(t, id, f, &text_of.summary, around, retry.is_some()))
}

/// The paragraph: 13px at 1.45, web addresses underlined in the accent (`.turn-failure__message`).
fn summary(t: &Theme, id: &str, message: &str) -> El {
    let ranges = url_ranges(message);
    let spans = ranges.iter().map(|r| Span::new(r.start, r.end).color(t.accent_text).underline(t.accent_text)).collect();
    let mut p =
        rich(message, spans).key(format!("turn-failure-{id}-message")).font(sans(13.0, 400.0, 1.45)).color(t.text).mt(6.0).selectable(true);
    for r in &ranges {
        p = p.link(Link::new(r.clone(), message[r.clone()].to_string()));
    }
    p.on_link(|_: &mut State, url: &str, tx: &mut Tx| {
        let _ = tx.open_url(url);
    })
}

/// `<details>`: "Details" in 12px muted, opening to the stack in 11px mono, 180px tall at most.
fn details_block(t: &Theme, id: &str, details: &str, open: bool) -> El {
    let key = id.to_string();
    let marker = icon(&CHEVRON_RIGHT, 11.0).color(t.muted).group_color(t.text).rotate(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 });
    let toggle = row()
        .key(format!("turn-failure-{id}-details"))
        .gap(4.0)
        .center_y()
        .hover_group()
        .cursor(Cursor::Pointer)
        .label("Details")
        .on_press(move |s: &mut State| {
            if !s.failure_open.remove(&key) {
                s.failure_open.insert(key.clone());
            }
        })
        .child(marker)
        .child(text("Details").font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text));
    let mut block = col().mt(6.0).items_start().child(toggle);
    if open {
        block = block.child(
            el().key(format!("turn-failure-{id}-stack"))
                .w_full()
                .mt(6.0)
                .max_h(180.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(text(details).font(mono(11.0, 400.0, 1.5)).color(t.muted).wrap().w_full().selectable(true)),
        );
    }
    block
}

/// The foot (`.turn-failure__foot`): the error's name on the left, Report this and Try again on the right.
fn foot(t: &Theme, id: &str, f: &Failure, summary: &str, around: &Around, retry_waits: bool) -> El {
    let name = if f.name == "UnknownError" { "" } else { f.name.as_str() };
    let report = summary.to_string();
    let mut actions = row().gap(6.0).center_y().ml(8.0).child(pill(
        t,
        format!("turn-failure-{id}-report"),
        &BUG,
        "Report this",
        true,
        move |s: &mut State, tx: &mut Tx| s.report_turn_failure(&report, tx),
    ));
    // Only the latest failure can be tried again, and not while Fleet's own retry waits (its Try now stands in).
    if around.has_prompt && around.latest && !retry_waits {
        actions = actions.child(pill(
            t,
            format!("turn-failure-{id}-retry"),
            &ROTATE_CW,
            "Try again",
            around.can_send,
            |s: &mut State, tx: &mut Tx| crate::retry_turn(s, tx),
        ));
    }
    row()
        .w_full()
        .mt(8.0)
        .gap(12.0)
        .center_y()
        .justify_between()
        .child(text(name).font(mono(11.0, 400.0, 1.3)).color(t.muted).truncate().shrink().min_w(0.0))
        .child(actions)
}

/// `.turn-failure__retry` and `.turn-retry__button`: a pill, 12px, whose border turns to the accent on hover. The border
/// is a 1px ring around the pill that takes the hover fill, since Rune has no border colour for hover.
fn pill(
    t: &Theme,
    key: String,
    glyph: &'static Icon,
    label: &'static str,
    enabled: bool,
    press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    let inner = row()
        .gap(5.0)
        .px(10.0)
        .py(4.0)
        .center_y()
        .radius(999.0)
        .fill(t.panel_bg)
        .child(icon(glyph, 12.0).color(t.text).group_color(t.accent_text))
        .child(text(label).font(sans(12.0, 400.0, 1.3)).color(t.text).group_color(t.accent_text));
    let ring = row().key(key).px(1.0).py(1.0).radius(999.0).fill(t.border).label(label).child(inner);
    if enabled { ring.hover_group().hover(t.accent).cursor(Cursor::Pointer).on_press_tx(press) } else { ring.opacity(0.5).disabled(true) }
}

/// `TurnRetryBar` (rows 1.71-1.75): the rotate icon, when Fleet tries again and why, and the two pills.
fn retry_bar(t: &Theme, s: &State, retry: &ScheduledRetry) -> El {
    let act = s.retry_act();
    let (when, why) = (retry_when(retry, now_ms(s), local_offset_secs()), retry_why(retry));
    let why = act.error.clone().unwrap_or(why);
    let words = col()
        .grow(1.0)
        .min_w(220.0)
        .gap(1.0)
        .child(text(when).key("turn-retry-when").font(sans(12.5, 500.0, 1.3).features("'tnum' 1")).color(t.text).wrap())
        .child(text(why).key("turn-retry-why").font(sans(11.5, 400.0, 1.3)).color(t.muted).wrap());
    let buttons = |key: &str, glyph, label, send: bool| {
        pill(
            t,
            key.to_string(),
            glyph,
            label,
            !act.busy,
            move |s: &mut State, _: &mut Tx| if send { s.retry_now() } else { s.retry_cancel() },
        )
    };
    row()
        .key("turn-retry")
        .w_full()
        .mt(8.0)
        .px(10.0)
        .py(8.0)
        .gap(8.0)
        .center_y()
        .wrap()
        .radius(RADIUS_CARD - 2.0)
        .fill(mix(t.panel_bg, t.idle, 0.10))
        .child(icon(&ROTATE_CW, 14.0).color(t.idle))
        .child(words)
        .child(row().gap(6.0).ml(8.0).child(buttons("turn-retry-now", &PLAY, "Try now", true)).child(buttons(
            "turn-retry-cancel",
            &X,
            "Don't retry",
            false,
        )))
}

/// Whether `msg` is the message a card's Try again would answer: the user's last prompt, for `Around::has_prompt`.
pub fn last_prompt(messages: &[Message]) -> Option<String> {
    let body = messages.iter().rev().find(|m| m.role == Role::User)?.text();
    let body = body.trim();
    (!body.is_empty()).then(|| body.to_string())
}
