//! The card a question the agent asks shows while it waits (amber, under its message). Ports the "waiting on you" half
//! of `components/session/QuestionCard.vue` (rows 1.1-1.17, 1.20 and 1.24 of `docs/parity/04-questions-and-permissions.md`).
//! `asks::question` holds the rules and `fleet::questions` the picks; this file draws them. Send, Skip and the keys that
//! answer are piece D's: the buttons are drawn, and press nothing yet.
use super::ask_card;
use crate::State;
use crate::asks::question::{QuestionInfo, key_hint, link_ranges};
use crate::fleet::questions::QuestionCard;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, El, Link, Span, Tx, col, el, icon, rich, row, text, text_input};

/// `.qcard` and what is in it, for one waiting question.
pub fn card(t: &Theme, card: &QuestionCard, first: bool) -> El {
    let title = card.input.questions.first().map(|q| q.header.as_str()).filter(|h| !h.is_empty()).unwrap_or("Question").to_string();
    let key = if first { "question-card".to_string() } else { format!("question-card-{}", card.id) };
    let mut out = ask_card::frame(t, key).label(format!("The agent asks: {title}")).child(head(t, &title));
    for (qi, q) in card.input.questions.iter().enumerate() {
        out = out.child(section(t, card, qi, q));
    }
    out.child(foot(t, card))
}

/// `.qcard__head`: the icon, the first question's header, and "Needs you" at the far end.
fn head(t: &Theme, title: &str) -> El {
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(icon(&MESSAGE_CIRCLE_QUESTION_MARK, 15.0).color(t.status_waiting()))
        .child(text(title.to_string()).key("question-heading").font(sans(13.0, 600.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0))
        .child(el().grow(1.0))
        .child(ask_card::needs_you(t, "question-needs-you"))
}

/// One question: its sub-heading, its text, "Pick any that apply." and the choices (`.qcard__question`).
fn section(t: &Theme, card: &QuestionCard, qi: usize, q: &QuestionInfo) -> El {
    let mut out = col().key(format!("question-{qi}")).w_full().gap(8.0);
    if qi > 0 && !q.header.is_empty() {
        out = out.child(text(q.header.clone()).key(format!("question-subhead-{qi}")).font(sans(12.0, 600.0, 1.3)).color(t.muted).mt(4.0));
    }
    out = out.child(question_text(t, qi, &q.question));
    if q.multiple {
        out =
            out.child(text("Pick any that apply.").key(format!("question-hint-{qi}")).font(sans(12.0, 400.0, 1.3)).color(t.muted).mt(-4.0));
    }
    let mut group = col()
        .key(format!("question-choices-{qi}"))
        .label(if q.header.is_empty() { q.question.clone() } else { q.header.clone() })
        .w_full()
        .gap(4.0);
    for (oi, o) in q.options.iter().enumerate() {
        group = group.child(choice(t, card, qi, oi, q, o.label.clone(), &o.description));
    }
    if q.custom {
        group = group.child(other(t, card, qi, q));
    }
    out.child(group)
}

/// `.qcard__text`: 14px at 1.5, line breaks kept, web addresses in the accent and underlined, and a press opens one.
fn question_text(t: &Theme, qi: usize, words: &str) -> El {
    let ranges = link_ranges(words);
    let spans = ranges.iter().map(|r| Span::new(r.start, r.end).color(t.accent_text).underline(t.accent_text)).collect::<Vec<_>>();
    let mut p = rich(words.to_string(), spans)
        .key(format!("question-text-{qi}"))
        .font(sans(14.0, 400.0, 1.5))
        .color(t.text)
        .w_full()
        .wrap()
        .selectable(true);
    for r in &ranges {
        p = p.link(Link::new(r.clone(), words[r.clone()].to_string()));
    }
    p.on_link(|_: &mut State, url: &str, tx: &mut Tx| {
        let _ = tx.open_url(url);
    })
}

/// `.qcard__choice` in its resting, picked and typed looks: a bordered row, tinted when it holds the answer.
fn box_of(t: &Theme, picked: bool) -> El {
    row()
        .w_full()
        .min_w(0.0)
        .gap(10.0)
        .px(10.0)
        .py(8.0)
        .radius(RADIUS_BTN)
        .border(1.0, if picked { t.accent } else { t.border })
        .border_layout()
        .fill(if picked { t.accent_dim } else { t.panel_bg })
}

/// `.qcard__key`: the number, or in a multiple question a check once the option is picked; filled when picked.
fn key_chip(t: &Theme, number: usize, picked: bool, check: bool) -> El {
    let chip = ask_card::chip_box(t).mt(1.0);
    match (picked, check) {
        (true, true) => chip.fill(t.accent).border(1.0, t.accent).border_layout().child(icon(&CHECK, 11.0).color(t.primary_fg)),
        (true, false) => chip
            .fill(t.accent)
            .border(1.0, t.accent)
            .border_layout()
            .child(text(number.to_string()).font(crate::theme::mono(11.0, 400.0, 1.0)).color(t.primary_fg)),
        _ => ask_card::chip(t, number.to_string()).mt(1.0),
    }
}

fn choice(t: &Theme, card: &QuestionCard, qi: usize, oi: usize, q: &QuestionInfo, label: String, description: &str) -> El {
    let picked = card.sel.is_picked(qi, &label);
    let mut copy = col().gap(1.0).grow(1.0).min_w(0.0).child(text(label.clone()).font(sans(13.0, 500.0, 1.5)).color(t.text).wrap());
    if !description.is_empty() {
        copy = copy.child(text(description.to_string()).font(sans(12.0, 400.0, 1.45)).color(t.muted).wrap());
    }
    let (id, pick) = (card.id.clone(), label.clone());
    box_of(t, picked)
        .key(format!("question-pill-{label}"))
        .role(if q.multiple { "checkbox" } else { "radio" })
        .checked(picked)
        .focusable(true)
        .focus_ring(2.0, -1.0, t.accent)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.pick_choice(&id, qi, &pick))
        .child(key_chip(t, oi + 1, picked, q.multiple))
        .child(copy)
}

/// `.qcard__choice--other`: a borderless field with its number; typed words give it the picked look.
fn other(t: &Theme, card: &QuestionCard, qi: usize, q: &QuestionInfo) -> El {
    let typed = !card.typed[qi].text.trim().is_empty();
    let (input, id) = (format!("question-other-input-{qi}"), card.id.clone());
    let field = text_input(&card.typed[qi], sans(13.0, 400.0, 1.7))
        .key(input.clone())
        .label("Type your own answer")
        .placeholder("Something else\u{2026}", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .on_edit(move |s: &mut State| s.questions.typed_mut(&id, qi));
    let number = ask_card::chip(t, (q.options.len() + 1).to_string());
    box_of(t, typed)
        .key(format!("question-other-{qi}"))
        .center_y()
        .focus_within_ring(1.0, -1.0, t.accent)
        .cursor(Cursor::Text)
        .on_press_tx(move |_: &mut State, tx: &mut Tx| tx.focus(&input, false))
        .child(number)
        .child(field)
}

/// `.qcard__foot`: Send answer, Skip and the key hint. Neither button answers yet (piece D).
fn foot(t: &Theme, card: &QuestionCard) -> El {
    let send = row()
        .key("question-submit-button")
        .h(28.0)
        .px(14.0)
        .center()
        .radius(999.0)
        .fill(t.accent)
        .opacity(if card.has_any_answer() { 1.0 } else { 0.45 })
        .child(text("Send answer").font(sans(12.0, 600.0, 1.3)).color(t.primary_fg));
    let skip = row()
        .key("question-dismiss-button")
        .h(28.0)
        .px(14.0)
        .center()
        .radius(999.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .child(text("Skip").font(sans(12.0, 500.0, 1.3)).color(t.muted).group_color(t.text));
    row()
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .child(send)
        .child(skip)
        .child(el().grow(1.0))
        .child(text(key_hint(&card.input)).key("question-key-hint").font(sans(11.0, 400.0, 1.4)).color(t.muted))
}
