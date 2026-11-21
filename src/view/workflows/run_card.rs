//! `WorkflowRunCard.vue`: the run's card above the composer, in a step session's conversation. When the run waits on the user
//! here it asks: a You step's choices (a "Send back" choice takes a note), the two ways to go on, an outcome for a step that
//! stopped without one, or whether to move on anyway. Once the step is over and the run has moved on, a line says where it
//! went. The state is `fleet::workflows::card`.
use crate::fleet::workflows::card::{self, MovedOn, NOTE_KEY};
use crate::fleet::workflows::{Run, RunChoice, Waiting, WaitingKind};
use crate::icons::{
    ARROW_RIGHT, CHECK, CORNER_UP_LEFT, FILE_CHECK_CORNER, LOADER_CIRCLE, PLAY, ROTATE_CCW, SEND, TRIANGLE_ALERT, USER_ROUND,
};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button_ink, button_lead, disabled};
use crate::view::{ring_popover, toolcard::mix};
use crate::{State, Tx};
use rune::{Color, Cursor, El, auto_grow, col, icon, row, text};

/// `size-3.5` on a button's icon.
const GLYPH: f32 = 12.25;

/// Nothing in a session the run isn't waiting in and that hasn't finished its step.
pub fn run_card(s: &State, t: &Theme) -> Option<El> {
    if let Some((run, waiting)) = s.card_waiting() {
        return Some(waiting_card(s, t, run, waiting));
    }
    s.card_moved_on().map(|moved| resolved(t, &moved))
}

fn waiting_card(s: &State, t: &Theme, run: &Run, w: &Waiting) -> El {
    let state = &s.workflows_nav.card;
    let busy = state.busy.is_some();
    let title = if w.kind == WaitingKind::You { w.step_title.clone() } else { format!("{} needs you", w.step_title) };
    let top = row()
        .w_full()
        .gap(7.0)
        .center_y()
        .child(icon(&USER_ROUND, 14.0).color(t.idle))
        .child(text(title).font(sans(13.5, 600.0, 1.4)).color(t.text).grow(1.0).min_w(0.0).wrap())
        .child(
            row()
                .px(8.0)
                .py(1.0)
                .radius(999.0)
                .border(1.0, t.idle.alpha(t.idle.a * 0.5))
                .border_layout()
                .child(text("Needs you").font(sans(11.0, 500.0, 1.4)).color(t.idle)),
        );
    let mut inner = col()
        .key("workflow-card")
        .role("region")
        .label(w.step_title.clone())
        .w_full()
        .gap(8.0)
        .px(14.0)
        .py(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(t.idle, 45.0))
        .border_layout()
        .fill(mix(t.card_bg, t.idle, 0.06));
    inner = inner
        .child(top)
        .child(text(card::ask(w)).key("workflow-card-ask").font(sans(13.0, 400.0, 1.5)).color(t.text.alpha(0.85)).wrap().w_full());
    if state.note_for.is_some() {
        inner = inner.child(note_field(s, t, run));
    }
    let mut acts = row().key("workflow-card-acts").w_full().flex_wrap().center_y().gaps(6.0, 6.0);
    if let Some(id) = &state.note_for {
        let to = w.choices.iter().find(|c| c.id == *id).and_then(|c| c.to.as_deref());
        let sending = state.busy.as_deref() == Some(id.as_str());
        let lead = if sending { spinner(t.primary_fg) } else { icon(&SEND, GLYPH).color(t.primary_fg) };
        let send = button_lead(t, Variant::Default, Size::Sm, Some(lead), &format!("Send back to {}", card::step_title(run, to)))
            .key("workflow-card-send-note");
        let ready = !busy && !state.note.text.trim().is_empty();
        acts = acts.child(if ready { send.on_press_tx(|s: &mut State, tx: &mut Tx| s.card_send_note(tx)) } else { disabled(send) });
        let cancel = button_lead(t, Variant::Ghost, Size::Sm, None, "Cancel").key("workflow-card-cancel-note");
        acts = acts.child(if busy { disabled(cancel) } else { cancel.on_press(|s: &mut State| s.card_cancel_note()) });
    } else {
        let involvement = w.choices.iter().any(|c| c.involvement);
        for (index, choice) in w.choices.iter().enumerate().filter(|(_, c)| c.involvement) {
            acts = acts.child(way(s, t, w, choice, index, false)).child(way(s, t, w, choice, index, true));
        }
        for (n, (index, choice)) in w.choices.iter().enumerate().filter(|(_, c)| !c.involvement).enumerate() {
            acts = acts.child(choice_button(s, t, run, w, choice, index, n, involvement));
        }
    }
    let end = button_ink(t, Variant::Ghost, Size::Sm, None, "End run", Some(t.muted)).key("workflow-card-end");
    let end = if busy { disabled(end) } else { end.on_press_tx(|s: &mut State, tx: &mut Tx| s.card_end_run(tx)) };
    // `margin-left: auto`: a growing box as wide as the button, so it takes the end of whichever line it lands on.
    acts = acts.child(row().min_w(76.0).grow(1.0).justify_end().child(end));
    inner = inner.child(acts);
    if matches!(w.kind, WaitingKind::NoOutcome | WaitingKind::LoopLimit) {
        inner = inner.child(
            hint(t, "workflow-card-hint").child(plain(t, "Or reply to the agent below: if it finishes the step, the run carries on.")),
        );
    }
    if w.kind == WaitingKind::SkillOff {
        inner = inner.child(
            hint(t, "workflow-card-skill-off")
                .child(plain(t, "Turn it on in "))
                .child(
                    text("Settings \u{2192} Skills & tools")
                        .key("workflow-card-skills-link")
                        .role("button")
                        .font(sans(12.0, 400.0, 1.4))
                        .color(t.accent)
                        .cursor(Cursor::Pointer)
                        .focus_ring(2.0, 1.0, t.accent)
                        .on_press(|s: &mut State| s.card_open_skills()),
                )
                .child(plain(t, ", then retry. Or start the step without it for the rest of this run.")),
        );
    }
    if let Some(error) = &state.error {
        inner = inner.child(
            text(error.clone()).key("workflow-card-error").role("alert").font(sans(12.0, 400.0, 1.4)).color(t.error_text).wrap().w_full(),
        );
    }
    // `margin: 0 16px 8px` around the card.
    col().w_full().px(16.0).mb(8.0).child(inner)
}

/// `.wf-card__hint`: 12 px, muted, wrapping.
fn hint(t: &Theme, key: &str) -> El {
    let _ = t;
    row().key(key.to_string()).w_full().flex_wrap().center_y()
}

fn plain(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(12.0, 400.0, 1.4)).color(t.muted)
}

/// A turning ring in the colour of the button's ink.
fn spinner(ink: Color) -> El {
    icon(&LOADER_CIRCLE, GLYPH).color(ink).cycle(ring_popover::spinner())
}

/// `.wf-card__note`: the field the "Send back" choice takes, at least 56 high, focused when it opens.
fn note_field(s: &State, t: &Theme, run: &Run) -> El {
    let card = &s.workflows_nav.card;
    let to =
        s.card_waiting().and_then(|(_, w)| w.choices.iter().find(|c| Some(&c.id) == card.note_for.as_ref())).and_then(|c| c.to.clone());
    let field = crate::ui::text_menu::text_input(&card.note, sans(13.0, 400.0, 1.5))
        .key(NOTE_KEY)
        .label("What should change?")
        .placeholder(format!("What should change? This goes into {}'s next prompt.", card::step_title(run, to.as_deref())), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(10.0)
        .py(8.0)
        .on_edit(|s: &mut State| &mut s.workflows_nav.card.note);
    // `:focus { outline: 2px solid var(--accent); outline-offset: -1px }` shows for any focus, so the ring is on the frame.
    col()
        .w_full()
        .min_h(56.0)
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .focus_within_ring(2.0, -1.0, t.accent)
        .child(auto_grow(field, 2, 8).min_h(54.0))
}

/// `.wf-opt`: one of the two ways to go on, as a bigger option with a line under it.
fn way(s: &State, t: &Theme, w: &Waiting, choice: &RunChoice, index: usize, check: bool) -> El {
    let busy = s.workflows_nav.card.busy.as_deref();
    let mine = busy == Some(format!("{}:{check}", choice.id).as_str());
    let ink = t.text;
    let lead = if mine {
        spinner(if check { t.accent } else { ink })
    } else if check {
        icon(&USER_ROUND, GLYPH).color(t.accent)
    } else {
        icon(&PLAY, GLYPH).color(ink)
    };
    let (label, line) = if check {
        (
            format!("{}, check with me after each step", choice.label),
            "Each step waits for you to move it on, and you can talk to the agent in between.".to_string(),
        )
    } else {
        (format!("{}, let it run", choice.label), card::let_it_run(w))
    };
    let head =
        row().gap(6.0).center_y().child(lead).child(text(label.clone()).font(sans(13.0, 600.0, 1.4)).color(ink).wrap().shrink().min_w(0.0));
    let mut b = col()
        .key(if check { "workflow-card-check-with-me" } else { "workflow-card-let-it-run" })
        .role("button")
        .label(label)
        // `flex: 1 1 240px`: Rune's grow has a basis of 0, so the 240 is a floor, which is what wraps the line.
        .min_w(240.0)
        .grow(1.0)
        .gap(3.0)
        .px(11.0)
        .py(9.0)
        .radius(RADIUS_BTN)
        .border(1.0, if check { t.border_mix(t.accent, 55.0) } else { t.border })
        .border_layout()
        .fill(if check { mix(t.main_bg, t.accent, 0.10) } else { t.main_bg })
        .focus_ring(2.0, 1.0, t.accent)
        .child(head)
        .child(text(line).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full());
    b = if busy.is_some() {
        disabled(b)
    } else {
        b.cursor(Cursor::Pointer)
            .hover(t.accent.alpha(0.06))
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.card_choose(index, Some(check), tx))
    };
    b
}

/// One of the other choices as a small button: the first is the main one unless the card offers the two ways.
#[allow(clippy::too_many_arguments)]
fn choice_button(s: &State, t: &Theme, run: &Run, w: &Waiting, choice: &RunChoice, index: usize, n: usize, involvement: bool) -> El {
    let busy = s.workflows_nav.card.busy.as_deref();
    let variant = if n == 0 && !involvement { Variant::Default } else { Variant::Outline };
    let ink = if variant == Variant::Default { t.primary_fg } else { t.text };
    let lead = if busy == Some(choice.id.as_str()) {
        Some(spinner(ink))
    } else if choice.note {
        Some(icon(&CORNER_UP_LEFT, GLYPH).color(ink))
    } else if choice.id == "retry" {
        Some(icon(&ROTATE_CCW, GLYPH).color(ink))
    } else if n == 0 {
        Some(icon(&CHECK, GLYPH).color(ink))
    } else {
        None
    };
    let mut b = button_lead(t, variant, Size::Sm, lead, &card::choice_label(w, choice)).key(format!("workflow-card-choice-{index}"));
    if let Some(tip) = card::describe(run, w, choice) {
        b = b.help(tip);
    }
    if busy.is_some() { disabled(b) } else { b.on_press_tx(move |s: &mut State, tx: &mut Tx| s.card_choose(index, None, tx)) }
}

/// `.wf-card--resolved`: where the run went, once this session's step is over.
fn resolved(t: &Theme, moved: &MovedOn) -> El {
    let small = sans(12.5, 400.0, 1.4);
    let mut line = row()
        .key("workflow-card-resolved")
        .w_full()
        .flex_wrap()
        .center_y()
        .gaps(8.0, 8.0)
        .px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .child(icon(&CHECK, 14.0).color(t.running_text))
        .child(text(moved.text.clone()).font(small).color(t.muted));
    if !moved.files.is_empty() {
        line = line.child(
            row()
                .gap(4.0)
                .center_y()
                .child(icon(&FILE_CHECK_CORNER, 12.0).color(t.running_text))
                .child(text(card::checked_text(moved)).key("workflow-card-files-checked").font(small).color(t.muted)),
        );
        if let Some(why) = &moved.commit_error {
            line = line.child(
                row().gap(4.0).center_y().min_w(0.0).help(why.clone()).child(icon(&TRIANGLE_ALERT, 12.0).color(t.idle)).child(
                    text(card::commit_failed_text(moved, why))
                        .key("workflow-card-commit-failed")
                        .font(small)
                        .color(t.muted)
                        .wrap()
                        .shrink()
                        .min_w(0.0),
                ),
            );
        }
    }
    if let Some((next, session)) = &moved.next {
        let session = session.clone();
        // `margin-left: auto`: a growing box as wide as the link, so the link takes the end of whichever line it lands on.
        let width = next.chars().count() as f32 * 6.6 + 24.0;
        line = line.child(
            row().min_w(width).grow(1.0).justify_end().child(
                row()
                    .key("workflow-card-next")
                    .role("button")
                    .label(next.clone())
                    .gap(4.0)
                    .center_y()
                    .cursor(Cursor::Pointer)
                    .focus_ring(2.0, 2.0, t.accent)
                    .child(text(next.clone()).font(small).color(t.accent))
                    .child(icon(&ARROW_RIGHT, 12.0).color(t.accent))
                    .on_press_tx(move |s: &mut State, tx: &mut Tx| s.card_open_next(&session, tx)),
            ),
        );
    }
    col().w_full().px(16.0).mb(8.0).child(line)
}
