//! `components/mods/review/ModReviewDialog.vue` and `ModDialogShell.vue` (desktop form) (component 22, piece I): "Keep <name>?",
//! what the check read from the draft in plain words, a note, and Keep for all my sessions / Cancel (or Discard). The check runs when
//! the dialog opens, and only then.
use crate::State;
use crate::fleet::mods::check_summary::{CheckRow, label_of, summarize_check, warning_words};
use crate::fleet::mods::drafts::{Busy, Checked};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, RADIUS_PANEL, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead, disabled};
use crate::ui::confirm::spinner;
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use rune::{El, KeyEvent, ScrollSpec, col, row, text};

/// `sm:max-w-xl`: 36rem.
const WIDTH: f32 = 36.0 * 14.0;
/// What the dialog's frame, title, description and footer take of the 88% it may grow to, so the body scrolls under the footer.
const CHROME: f32 = 150.0;

fn note(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().w_full()
}

/// One row of the list (`.mod-review__row`): a 6.5 rem label, then the words; `code` is on its own line in 12 px.
fn list_row(t: &Theme, i: usize, r: &CheckRow) -> El {
    let ink = if r.muted { t.muted } else { t.text };
    let mut words = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text(r.text.clone()).key(format!("mod-review-row-{i}-text")).font(sans(13.0, 400.0, 1.5)).color(ink).wrap().w_full());
    if let Some(code) = &r.code {
        words =
            words.child(text(code.clone()).key(format!("mod-review-row-{i}-code")).font(mono(12.0, 400.0, 1.5)).color(ink).wrap().w_full());
    }
    let mut line = row()
        .key(format!("mod-review-row-{i}"))
        .w_full()
        .gap(12.0)
        .items_start()
        .px(12.0)
        .py(7.0)
        .child(
            text(label_of(&r.key).to_string()).key(format!("mod-review-row-{i}-label")).font(sans(13.0, 400.0, 1.5)).color(t.muted).w(91.0),
        )
        .child(words);
    if i > 0 {
        line = line.border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    }
    line
}

fn report_body(s: &State, t: &Theme, report: &serde_json::Value) -> Vec<El> {
    let mut out = Vec::new();
    let problems = s.mod_drafts.dialog.problems();
    if let Some(e) = &problems.error {
        let at = e.place.as_ref().map(|p| format!(" (line {p})")).unwrap_or_default();
        out.push(crate::ui::error_note::message(t, "mod-review-error", &format!("Fix this first: {}{at}", e.message)));
    }
    if problems.warnings > 0 {
        let n = problems.warnings;
        let words = format!("{n} {}: {}", if n == 1 { "warning" } else { "warnings" }, warning_words(report));
        out.push(note(t, "mod-review-warnings", &words));
    }
    let rows = summarize_check(report);
    let mut list =
        col().key("mod-review-list").w_full().radius(RADIUS_CARD).border(1.0, t.border).border_layout().clip_corners([RADIUS_CARD; 4]);
    for (i, r) in rows.iter().enumerate() {
        list = list.child(list_row(t, i, r));
    }
    out.push(list);
    out
}

fn body(s: &State, t: &Theme) -> El {
    let d = &s.mod_drafts.dialog;
    let mut b = col().key("mod-review-body").w_full().gap(8.0);
    match &d.checked {
        Checked::Loading => {
            b = b.child(
                row()
                    .key("mod-review-loading")
                    .items_center()
                    .gap(8.0)
                    .child(spinner(t.muted))
                    .child(text("Checking the mod…").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted)),
            );
        }
        Checked::Failed(why) => b = b.child(note(t, "mod-review-unchecked", why)),
        Checked::Report(None) => {
            let words = "Fleet couldn't check this mod yet: the mod runtime isn't running. Keep will check it.";
            b = b.child(note(t, "mod-review-unchecked", words));
        }
        Checked::Report(Some(report)) => b = b.children(report_body(s, t, report)),
    }
    let field = text_input(&d.note, sans(13.0, 400.0, 1.4))
        .key("mod-review-note")
        .label("Why keep it? (optional)")
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .h(2.0 * 13.0 * 1.4 + 12.0 + 2.0)
        .px(10.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .focus_ring(2.0, 1.0, t.accent)
        .on_edit(|s: &mut State| &mut s.mod_drafts.dialog.note)
        .on_input_key(|_: &mut State, _: &KeyEvent, _: &mut crate::Tx| false);
    b.child(
        col()
            .w_full()
            .gap(4.0)
            .mt(4.0)
            .child(text("Why keep it? (optional)").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted))
            .child(field),
    )
    .children(d.refused.as_ref().map(|why| crate::ui::error_note::message(t, "mod-review-refused", why)))
}

fn foot(s: &State, t: &Theme, requested: bool) -> El {
    let d = &s.mod_drafts.dialog;
    let keep = button_lead(
        t,
        Variant::Default,
        Size::Default,
        (d.busy == Some(Busy::Keep)).then(|| spinner(t.primary_fg)),
        "Keep for all my sessions",
    )
    .key("mod-review-keep");
    let keep = if d.can_keep() { keep.on_press(|s: &mut State| s.keep_reviewed_draft()) } else { disabled(keep) };
    let decline = button(t, Variant::Ghost, Size::Default, None, if requested { "Discard" } else { "Cancel" }).key("mod-review-decline");
    let decline = if d.busy.is_some() { disabled(decline) } else { decline.on_press(|s: &mut State| s.decline_reviewed_draft()) };
    let mut f = col().key("mod-review-foot").w_full().child(row().w_full().flex_wrap().gap(8.0).child(keep).child(decline));
    if requested {
        f = f.child(
            text("Discard tells the agent you didn't keep it. The draft stays on in this session.")
                .key("mod-review-hint")
                .font(sans(12.0, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .w_full()
                .mt(8.0),
        );
    }
    f
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let (session, name) = s.mod_drafts.review.as_ref()?;
    let draft = s.mod_drafts.list(session).iter().find(|d| &d.name == name);
    let requested = draft.is_some_and(|d| d.keep_request.is_some());
    let mut parts = vec![format!("v{}", draft.and_then(|d| d.kept).unwrap_or(0) + 1)];
    if let Checked::Report(Some(r)) = &s.mod_drafts.dialog.checked
        && let Some(lines) = r.get("lines").and_then(serde_json::Value::as_i64)
    {
        parts.push(format!("{lines} lines"));
    }
    if let Some(title) = s.title_of(session) {
        parts.push(format!("written in “{title}”"));
    }
    let title_font = |w| sans(rem(1.125), w, 1.0);
    let title_el = row()
        .key("mod-review-title")
        .child(text("Keep ").font(title_font(600.0)).color(t.text))
        .child(text(name.clone()).font(mono(rem(1.125), 600.0, 1.0)).color(t.text))
        .child(text("?").font(title_font(600.0)).color(t.text));
    let max = (s.ui.sel.window.1 * 0.88 - CHROME).max(120.0);
    let scroller = body(s, t).max_h(max).scroll(ScrollSpec { stick_bottom: false });
    Some(dialog(
        t,
        Dialog {
            key: "mod-review",
            title: format!("Keep {name}?"),
            title_el: Some(title_el),
            description: Some(parts.join(" · ")),
            body: vec![scroller],
            footer: vec![foot(s, t, requested)],
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_draft_review()),
            width: Some(WIDTH),
            radius: Some(RADIUS_PANEL),
            ..Default::default()
        },
    ))
}
