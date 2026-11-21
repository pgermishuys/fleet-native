//! `WorkflowFinishBar.vue`: the bar above the composer while the user and the agent work through a step the user finishes. The
//! agent has no way to end the step; Move on (one button per outcome when the step has more than one) asks it, in this session,
//! to bring the step's files up to date and write a short summary. While it does, one line says where it is going. The state is
//! `fleet::workflows::finish`.
use crate::fleet::workflows::WithYou;
use crate::fleet::workflows::finish::{self, NOTE_KEY};
use crate::fleet::workflows::helpers::{move_blocked_reason, move_label};
use crate::icons::{ARROW_RIGHT, CHECK, CORNER_UP_LEFT, FILE_TEXT, LOADER_CIRCLE, USER_ROUND};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button_lead, disabled};
use crate::ui::rem;
use crate::view::{parts, ring_popover, toolcard::mix};
use crate::{State, Tx};
use rune::{Cursor, El, Span, col, icon, rich, row, text};

/// `size-3.5` on a button's icon.
const GLYPH: f32 = 12.25;

/// Nothing in a step the agent finishes, or in another session.
pub fn finish_bar(s: &State, t: &Theme) -> Option<El> {
    let (_, with_you) = s.finish_with_you()?;
    Some(match finish::wrapping(with_you) {
        Some(words) => wrapping(t, &words),
        None => bar(s, t, with_you),
    })
}

/// `.wf-finish--wrapping`: the working glyph and where the wrap-up goes.
fn wrapping(t: &Theme, words: &str) -> El {
    let line = row()
        .key("workflow-wrapping-up")
        .role("status")
        .label(words.to_string())
        .w_full()
        .gap(8.0)
        .center_y()
        .px(12.0)
        .py(9.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(t.accent, 40.0))
        .border_layout()
        .fill(mix(t.card_bg, t.accent, 0.06))
        .child(parts::status_glyph(t, "active", false))
        .child(text(words.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.text.alpha(0.85)).wrap().shrink().min_w(0.0));
    outer(line)
}

/// `margin: 0 16px 8px`.
fn outer(inner: El) -> El {
    col().w_full().px(16.0).mb(8.0).child(inner)
}

fn bar(s: &State, t: &Theme, with_you: &WithYou) -> El {
    let state = &s.workflows_nav.finish;
    let busy = state.busy.is_some();
    let single = finish::single(with_you);
    let next = finish::next_title(with_you);
    let small = sans(12.0, 400.0, 1.45);
    let top = row()
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(6.0)
        .child(icon(&USER_ROUND, 14.0).color(t.accent))
        .child(text(format!("You finish {}", with_you.step_title)).font(sans(13.5, 600.0, 1.4)).color(t.text))
        .child(text(format!("\u{b7} {}", finish::subtitle(with_you, s.finish_declared()))).font(sans(12.5, 400.0, 1.4)).color(t.muted));
    let note = {
        let field = crate::ui::text_menu::text_input(&state.note, sans(13.0, 400.0, 1.3))
            .key(NOTE_KEY)
            .label(format!("Note for {next}"))
            .placeholder(format!("Note for {next} (optional)"), t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .single_line()
            .w_full()
            .px(10.0)
            .on_edit(|s: &mut State| &mut s.workflows_nav.finish.note);
        let field = if busy { disabled(field) } else { field };
        // `flex: 1 1 220px; height: 32px`; the focus outline is drawn on the frame, over the border.
        row()
            .min_w(220.0)
            .grow(1.0)
            .h(32.0)
            .center_y()
            .radius(RADIUS_BTN)
            .fill(t.main_bg)
            .border(1.0, t.border)
            .border_layout()
            .focus_within_ring(2.0, -1.0, t.accent)
            .child(field)
    };
    let mut acts = row().key("workflow-finish-acts").w_full().flex_wrap().center_y().gaps(6.0, 6.0).child(note);
    for (index, mv) in with_you.moves.iter().enumerate() {
        let variant = if index == 0 { Variant::Default } else { Variant::Outline };
        let ink = if index == 0 { t.primary_fg } else { t.text };
        let lead = if state.busy.as_deref() == Some(mv.outcome.as_str()) {
            Some(icon(&LOADER_CIRCLE, GLYPH).color(ink).cycle(ring_popover::spinner()))
        } else if mv.back {
            Some(icon(&CORNER_UP_LEFT, GLYPH).color(ink))
        } else if !single {
            Some(icon(&CHECK, GLYPH).color(ink))
        } else {
            None
        };
        let label = move_label(mv, single);
        let mut b = button_lead(t, variant, Size::Sm, lead, &label).key(format!("workflow-move-{}", mv.outcome));
        if single {
            // `has-[>svg]:px-2.5`: the arrow after the label takes the narrower padding.
            b = b.px(rem(0.625)).child(icon(&ARROW_RIGHT, GLYPH).color(ink));
        }
        if let Some(why) = move_blocked_reason(&with_you.step_title, mv) {
            b = b.help(why);
        }
        let press = move |s: &mut State, tx: &mut Tx| s.finish_move_on(index, tx);
        acts = acts.child(if busy || !mv.allowed { disabled(b) } else { b.on_press_tx(press) });
    }
    let mut inner = col()
        .key("workflow-finish-bar")
        .role("region")
        .label(format!("You finish {}", with_you.step_title))
        .w_full()
        .gap(8.0)
        .px(14.0)
        .py(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border_mix(t.accent, 40.0))
        .border_layout()
        .fill(mix(t.card_bg, t.accent, 0.06))
        .child(top)
        .child(acts);
    if let Some(reason) = finish::blocked(with_you) {
        let mut underlined = Span::new(0, 11);
        underlined.underline = Some(t.accent_text);
        let end = rich("end the run", vec![underlined])
            .key("workflow-finish-end")
            .role("button")
            .font(small)
            .color(t.accent_text)
            .cursor(Cursor::Pointer)
            .focus_ring(2.0, 1.0, t.accent);
        let end = if busy { disabled(end) } else { end.on_press_tx(|s: &mut State, tx: &mut Tx| s.finish_end_run(tx)) };
        inner = inner.child(
            row()
                .key("workflow-finish-blocked")
                .w_full()
                .flex_wrap()
                .center_y()
                .child(text(format!("{reason} Pick another outcome, or ")).font(small).color(t.idle_text).wrap())
                .child(end)
                .child(text(".").font(small).color(t.idle_text)),
        );
    }
    inner = inner.child(hands_on(t, with_you));
    inner = inner.child(text(finish::hand_off(with_you)).key("workflow-finish-hint").font(small).color(t.muted).wrap().w_full());
    if let Some(error) = &state.error {
        inner = inner.child(
            text(error.clone()).key("workflow-finish-error").role("alert").font(sans(12.0, 400.0, 1.4)).color(t.error_text).wrap().w_full(),
        );
    }
    outer(inner)
}

/// "Hands on:" with a chip per file, or what the next step gets when the step declares none.
fn hands_on(t: &Theme, with_you: &WithYou) -> El {
    let small = sans(12.0, 400.0, 1.4);
    let line = row().key("workflow-finish-outs").w_full().flex_wrap().center_y().gaps(6.0, 6.0);
    if with_you.files.is_empty() {
        return line.child(
            text("This step declares no files. The next step gets its summary, your note, and the branch as it is.")
                .font(small)
                .color(t.muted)
                .wrap()
                .w_full(),
        );
    }
    let mut line = line.child(text("Hands on:").font(small).color(t.muted));
    for file in &with_you.files {
        line = line.child(
            row()
                .key(format!("workflow-finish-file-{file}"))
                .gap(4.0)
                .center_y()
                .px(7.0)
                .py(1.0)
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .child(icon(&FILE_TEXT, 12.0).color(t.muted))
                .child(text(file.clone()).font(mono(11.0, 400.0, 1.4)).color(t.text)),
        );
    }
    line
}
