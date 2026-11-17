//! The Changes tab's body (`ChangesCanvas.vue`, rows 4.1 to 4.8 of docs/parity/12-changes-and-review.md): the base line, then
//! one of loading, the failure with Try again, git's refusal with its fix command, the empty note, or the review (piece E on).
//! What the changes are compared with, and its words, are `fleet::diffs::diff_base_label`.
use crate::State;
use crate::fleet::diffs::{Base, BaseLabel, Refusal, diff_base_label};
use crate::icons::{GIT_BRANCH, GIT_COMPARE, HISTORY};
use crate::theme::{Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::confirm::error_note_with;
use crate::ui::empty_state;
use rune::{Cursor, El, Tx, col, icon, row, text};

pub fn changes_body(s: &State, t: &Theme) -> El {
    let d = &s.diffs;
    let has_changes = !d.files().is_empty();
    let mut body = col().key("changes-canvas").w_full().grow(1.0).min_h(0.0);
    if !has_changes && let Some(label) = diff_base_label(d.base.as_ref()) {
        body = body.child(base_line(t, d.base.as_ref(), &label));
    }
    // The review's header names the base itself once there are changes.
    let state = if has_changes {
        super::review::review(s, t)
    } else if !d.has_loaded {
        text("Loading changes…").key("changes-loading").font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap().px(18.0).py(14.0)
    } else if let Some(why) = &d.error {
        let retry = button(t, Variant::Outline, Size::Sm, None, "Try again").key("changes-retry").on_press(|s: &mut State| s.retry_diffs());
        let retry = if d.is_loading() { retry.disabled(true) } else { retry };
        // `m-3`: a wrapper holds the margin so the note's full width stays inside the panel.
        col().w_full().px(12.0).py(12.0).child(error_note_with(
            t,
            "changes-load-failed",
            "Couldn't load the changes.",
            Some(why),
            Some(row().child(retry)),
        ))
    } else if let Some(refusal) = &d.refusal {
        refused(t, refusal, d.copied_fix)
    } else {
        let title =
            if matches!(d.base, Some(Base::Branch { .. })) { "No changes on this branch yet" } else { "No changes in this session yet" };
        empty_state::compact(t, "changes-empty", &GIT_COMPARE, title)
    };
    body.child(state)
}

/// `.changes-canvas__base`: lines up with the rows' icons below it; its tooltip says what the comparison means.
fn base_line(t: &Theme, base: Option<&Base>, label: &BaseLabel) -> El {
    let glyph = if matches!(base, Some(Base::Branch { .. })) { &GIT_BRANCH } else { &HISTORY };
    let mut line = row()
        .key("changes-base")
        .w_full()
        .items_center()
        .gap(6.0)
        .px(16.0)
        .pt(10.0)
        .help(label.detail.clone())
        .label(format!("{}{}", label.text, label.commit.as_deref().unwrap_or("")))
        .child(icon(glyph, 12.0).color(t.muted.alpha(0.75)))
        .child(text(label.text.clone()).font(sans(12.0, 400.0, 1.4)).color(t.muted));
    if let Some(commit) = &label.commit {
        line = line.child(text(commit.clone()).key("changes-base-commit").font(mono(11.5, 400.0, 1.4)).color(t.muted.alpha(0.8)));
    }
    line
}

/// Git won't read the folder: why, the one command that lets it, and a button that copies it.
fn refused(t: &Theme, refusal: &Refusal, copied: bool) -> El {
    let command = text(refusal.fix_command.clone())
        .key("git-fix-command")
        .font(mono(12.0, 400.0, 1.4))
        .color(t.text)
        .wrap()
        .selectable(true)
        .px(8.0)
        .py(6.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.muted.alpha(0.1));
    let copy = row()
        .key("changes-copy-command")
        .role("button")
        .label(if copied { "Copied" } else { "Copy command" })
        .min_h(28.0)
        .px(12.0)
        .py(4.0)
        .center()
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(text(if copied { "Copied" } else { "Copy command" }).font(sans(13.0, 400.0, 1.4)).color(t.text))
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.copy_fix_command(tx));
    col()
        .key("changes-git-refused")
        .w_full()
        .items_start()
        .gap(8.0)
        .px(18.0)
        .py(14.0)
        .child(text(refusal.message.clone()).font(sans(13.0, 400.0, 1.4)).color(t.text).wrap())
        .child(text("To let git read it, run this once:").font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap())
        .child(command)
        .child(copy)
}
