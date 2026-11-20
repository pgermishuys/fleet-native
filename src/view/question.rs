//! The card a question the agent asks shows while it waits (amber, under its message). Ports the "waiting on you" half
//! of `components/session/QuestionCard.vue` (rows 1.1-1.17, 1.20 and 1.24 of `docs/parity/04-questions-and-permissions.md`).
//! `asks::question` holds the rules and `fleet::questions` the picks and the requests; this file draws them, draws the
//! answered and skipped rows (rows 2.1, 2.2 and 2.4) and takes the card's keys (`on_key`, rows 1.18 and 1.19).
use super::ask_card;
use crate::State;
use crate::asks::focus;
use crate::asks::question::{Digit, QuestionInfo, QuestionPart, answer_text, digit_choice, key_hint, link_ranges};
use crate::fleet::questions::QuestionCard;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::text_menu::text_input;
use rune::{Cursor, El, Key, KeyEvent, Link, Span, Tx, col, el, icon, rich, row, text};

/// What a message draws for its `question` call: the answered row, the skipped row or the waiting card
/// (`QuestionCard.vue`'s `v-if` chain). Nothing for a call that finished without recorded answers, or one whose
/// input could not be read (row 2.5).
pub fn part(t: &Theme, s: &State, call: &str, q: &QuestionPart, first: bool) -> Option<El> {
    let input = q.input.as_ref()?;
    let card = s.questions.card(call);
    if !q.status.is_active()
        && let Some(answers) = &q.answers
    {
        return Some(answered(t, call, input, answers));
    }
    if q.status == crate::asks::question::QuestionStatus::Error || card.is_some_and(|c| c.hidden) {
        return Some(skipped(t, call, input));
    }
    if q.status.is_active() { card.map(|c| self::card(t, c, first)) } else { None }
}

/// Changes whenever what the message draws for its question calls changes: the card's picks, typed words, request and
/// error, which a `memo`d row would otherwise keep.
pub fn key(s: &State, msg: &super::conversation::Message) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut any = false;
    for part in &msg.parts {
        if let super::conversation::Part::Tool { call_id, question: Some(_), .. } = part
            && let Some(card) = s.questions.card(call_id)
        {
            any = true;
            // A text field's own hash covers its selection, composition and focus, which the memo needs (rune's memo docs).
            (&card.sel.picked, &card.typed, card.busy, &card.error, card.gone, card.hidden).hash(&mut hash);
        }
    }
    if any { hash.finish() | 1 } else { 0 }
}

/// `.qrow-group`: the frame the answered and skipped rows sit in, like the message's tool list.
fn group(t: &Theme, key: String) -> El {
    col().key(key).w_full().mt(10.0).mb(4.0).padding(4.0).radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.wash(3.0))
}

/// `.qrow`: the icon, "Asked" and the question; the caller adds what the row ends with.
fn qrow(t: &Theme, question: &str) -> El {
    row()
        .min_h(30.0)
        .px(8.0)
        .gap(9.0)
        .center_y()
        .min_w(0.0)
        .help(question.to_string())
        .child(icon(&MESSAGE_CIRCLE_QUESTION_MARK, 14.0).color(t.muted.alpha(0.75)))
        .child(text("Asked").font(sans(13.0, 500.0, 1.3)).color(t.text))
        .child(text(question.to_string()).font(sans(13.0, 400.0, 1.3)).color(t.muted).truncate().grow(1.0).shrink().min_w(0.0))
}

/// Answered: one row per question, with its answer and a tick.
fn answered(t: &Theme, call: &str, input: &crate::asks::question::QuestionInput, answers: &[Vec<String>]) -> El {
    let mut out = group(t, format!("question-answered-{call}"));
    for (qi, q) in input.questions.iter().enumerate() {
        // The answer is what the row is for: the question gives way first, and the answer stops at half the row.
        let answer = answer_text(answers.get(qi));
        out = out.child(
            qrow(t, &q.question)
                .key(format!("question-row-{call}-{qi}"))
                .child(text("\u{2192}").font(sans(13.0, 400.0, 1.3)).color(t.muted))
                .child(
                    text(answer)
                        .key(format!("question-answer-{call}-{qi}"))
                        .font(sans(13.0, 500.0, 1.3))
                        .color(t.text)
                        .truncate()
                        .shrink()
                        .min_w(0.0)
                        .max_w((super::conversation::COLUMN - 10.0 - 16.0) / 2.0),
                )
                .child(icon(&CHECK, 13.0).color(t.running_text).label("Answered")),
        );
    }
    out
}

/// Skipped: one row with the first question and "Skipped".
fn skipped(t: &Theme, call: &str, input: &crate::asks::question::QuestionInput) -> El {
    let question = input.questions.first().map_or("Question", |q| q.question.as_str());
    group(t, format!("question-skipped-{call}"))
        .child(qrow(t, question).key(format!("question-row-{call}-0")).child(text("Skipped").font(sans(12.0, 400.0, 1.3)).color(t.muted)))
}

/// `.qcard` and what is in it, for one waiting question.
pub fn card(t: &Theme, card: &QuestionCard, first: bool) -> El {
    let title = card.input.questions.first().map(|q| q.header.as_str()).filter(|h| !h.is_empty()).unwrap_or("Question").to_string();
    let key = if first { "question-card".to_string() } else { format!("question-card-{}", card.id) };
    // `tabindex="-1"`: the card itself can hold the keyboard when it has no choice to give it to.
    let mut out = ask_card::frame(t, key)
        .label(format!("The agent asks: {title}"))
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .child(head(t, &title));
    for (qi, q) in card.input.questions.iter().enumerate() {
        out = out.child(section(t, card, qi, q));
    }
    if let Some(error) = &card.error {
        // `.qcard__error`, role alert: Fleet's own words for why it didn't go.
        out = out.child(text(error.clone()).key("question-error").label("alert").font(sans(12.0, 400.0, 1.4)).color(t.error_text).wrap());
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
    crate::ui::text_menu::selectable(p.on_link_secondary(crate::ui::text_menu::on_link).on_link(|_: &mut State, url: &str, tx: &mut Tx| {
        let _ = tx.open_url(url);
    }))
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
        .focusable(!card.busy)
        .focus_ring(2.0, -1.0, t.accent)
        .opacity(if card.busy { 0.55 } else { 1.0 })
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| {
            if !s.questions.card(&id).is_some_and(|c| c.busy) {
                s.pick_choice(&id, qi, &pick)
            }
        })
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
        .on_edit({
            let id = id.clone();
            move |s: &mut State| s.questions.typed_mut(&id, qi)
        })
        .disabled(card.busy)
        .on_input_key({
            let id = id.clone();
            move |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
                let sends = matches!(ev.key, Key::Enter) && !ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl;
                let ready = s.questions.card(&id).is_some_and(|c| c.has_any_answer() && !c.busy);
                if sends && ready {
                    s.submit_question(&id);
                }
                sends && ready
            }
        });
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

/// `.qcard__foot`: Send answer, Skip (Dismiss once the question is known gone) and the key hint.
fn foot(t: &Theme, card: &QuestionCard) -> El {
    let ready = card.has_any_answer() && !card.busy;
    let (sent, skipped) = (card.id.clone(), card.id.clone());
    let send = row()
        .key("question-submit-button")
        .label("Send answer")
        .h(28.0)
        .px(14.0)
        .center()
        .radius(999.0)
        .fill(t.accent)
        .opacity(if ready { 1.0 } else { 0.45 })
        .focusable(ready)
        .focus_ring(2.0, 2.0, t.accent)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.submit_question(&sent))
        .child(text(if card.busy { "Sending\u{2026}" } else { "Send answer" }).font(sans(12.0, 600.0, 1.3)).color(t.primary_fg));
    let skip = row()
        .key("question-dismiss-button")
        .h(28.0)
        .px(14.0)
        .center()
        .radius(999.0)
        .hover(t.wash(5.0))
        .hover_group()
        .opacity(if card.busy { 0.45 } else { 1.0 })
        .focusable(!card.busy)
        .focus_ring(2.0, 2.0, t.accent)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.skip_question(&skipped))
        .child(text(if card.gone { "Dismiss" } else { "Skip" }).font(sans(12.0, 500.0, 1.3)).color(t.muted).group_color(t.text));
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

/// The key of a choice button: `question-pill-` and the label.
const PILL: &str = "question-pill-";
const OTHER: &str = "question-other-input-";

/// The window's key handler, for a key while the waiting card holds the keyboard (rows 1.19). True when it took it.
/// `handleCardKeydown` is on the card, so only keys from inside it count: a choice, a button, the card itself. The
/// "Something else\u{2026}" field is left to itself, except Enter (its own `on_input_key`).
pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(card) = s.active_question() else { return false };
    let Some(focused) = tx.focused().map(str::to_string) else { return false };
    let on_pill = focused.starts_with(PILL);
    let inside = on_pill || focused == "question-submit-button" || focused == "question-dismiss-button" || focused == "question-card";
    if !inside || card.busy || ev.mods.cmd || ev.mods.alt || ev.mods.ctrl || ev.mods.logo {
        return false;
    }
    let (id, input, ready) = (card.id.clone(), card.input.clone(), card.has_any_answer());
    let labels: Vec<&str> = input.questions.iter().flat_map(|q| q.options.iter().map(|o| o.label.as_str())).collect();
    if let Some(step) = focus::step_of(&ev.key) {
        // Arrows move only from a choice button, and wrap round.
        let from = focused.strip_prefix(PILL).unwrap_or_default();
        return match focus::next_choice(&labels, from, step) {
            Some(next) => {
                tx.focus(format!("{PILL}{next}"), true);
                true
            }
            None => false,
        };
    }
    match &ev.key {
        Key::Text(d) if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() && d != "0" => {
            let digit = usize::from(d.as_bytes()[0] - b'0');
            // The question holding focus; the first when focus is on a button or the card.
            let qi = focused
                .strip_prefix(PILL)
                .and_then(|label| input.questions.iter().position(|q| q.options.iter().any(|o| o.label == label)))
                .unwrap_or(0);
            match digit_choice(&input, qi, digit) {
                Some(Digit::Pick(label)) => s.pick_choice(&id, qi, &label),
                Some(Digit::Custom) => tx.focus(format!("{OTHER}{qi}"), false),
                None => {}
            }
            // Fleet consumes a digit whenever the question exists.
            input.questions.get(qi).is_some()
        }
        Key::Enter => {
            // Enter on a button is that button's press; anywhere else it sends when something is answered.
            if on_pill {
                if let Some(label) = focused.strip_prefix(PILL) {
                    let qi = input.questions.iter().position(|q| q.options.iter().any(|o| o.label == label)).unwrap_or(0);
                    s.pick_choice(&id, qi, label);
                }
                true
            } else if focused == "question-dismiss-button" {
                s.skip_question(&id);
                true
            } else if ready {
                s.submit_question(&id);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
