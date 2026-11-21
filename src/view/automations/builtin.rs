//! The built-in Dreaming automation's compact page (`BuiltinAutomationDetail.vue`): the bar with its pills, a description, the
//! buttons (Run now, Change time, Pause or Resume, Settings → Memory), the note a refusal leaves, and its runs. No composer, no
//! delete: Fleet keeps this automation, and only its time, Run now and Pause are the user's.
use crate::State;
use crate::fleet::automations::builtin::{description, meta, no_runs};
use crate::fleet::automations::model::Automation;
use crate::icons::PLAY;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::text_menu::text_input;
use crate::ui::{page_header, rem};
use rune::{El, Tx, col, row, text};

/// `.builtin-page__pill`: a 1 px border, full radius, 1/8 padding, 11.5 px 500 muted.
pub(super) fn pill(t: &Theme, key: &str, word: &str) -> El {
    row()
        .key(key.to_string())
        .px(8.0)
        .py(1.0)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text(word.to_string()).font(sans(11.5, 500.0, 1.5)).color(t.muted))
}

/// The bar: the name, Built in, Experimental, and the line of words.
pub fn header(t: &Theme, a: &Automation) -> El {
    let words = text(meta(a)).key("automation-header-meta").font(sans(12.0, 400.0, 1.5)).color(t.muted).shrink().min_w(0.0).truncate();
    page_header::bar_with(
        t,
        "automation-header",
        &a.name,
        vec![pill(t, "automation-builtin-pill", "Built in"), pill(t, "automation-builtin-experimental", "Experimental"), words],
        Vec::new(),
    )
}

/// `.builtin-page__time`: 28 px tall, 1 px border, 8 px padding, 13 px.
fn time_box(s: &State, t: &Theme) -> El {
    let edit = &s.automations_nav.builtin.time;
    text_input(edit, sans(13.0, 400.0, 1.3))
        .key("builtin-time")
        .label("Time each night")
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .on_edit(|s: &mut State| &mut s.automations_nav.builtin.time)
        .on_submit(|s: &mut State, _| s.builtin_save_time())
        .w(63.0)
        .h(28.0)
        .px(8.0)
        .radius(RADIUS_BTN)
        .border(1.0, if edit.focused { t.accent } else { t.border })
        .border_layout()
}

/// `.builtin-page__actions`.
fn actions(s: &State, t: &Theme, a: &Automation) -> El {
    let b = &s.automations_nav.builtin;
    let wait = |e: El| if b.busy { disabled(e) } else { e };
    let mut r = row().key("builtin-actions").w_full().flex_wrap().center_y().gap(6.0).child(wait(
        button(t, Variant::Outline, Size::Sm, Some(&PLAY), "Run now")
            .key("automation-run-now")
            .on_press_tx(|s: &mut State, tx: &mut Tx| s.builtin_run_now(tx)),
    ));
    if b.editing {
        let save =
            button(t, Variant::Default, Size::Sm, None, "Save").key("builtin-time-save").on_press(|s: &mut State| s.builtin_save_time());
        let save = if b.busy || b.time.text.is_empty() { disabled(save) } else { save };
        r = r.child(time_box(s, t)).child(save).child(
            button(t, Variant::Ghost, Size::Sm, None, "Cancel")
                .key("builtin-time-cancel")
                .on_press(|s: &mut State| s.builtin_cancel_time()),
        );
    } else {
        r = r.child(
            button(t, Variant::Outline, Size::Sm, None, "Change time")
                .key("builtin-change-time")
                .on_press(|s: &mut State| s.builtin_start_time()),
        );
    }
    let pause = button(t, Variant::Outline, Size::Sm, None, if a.is_enabled { "Pause" } else { "Resume" })
        .key("builtin-pause")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.builtin_toggle_paused(tx));
    r.child(wait(pause)).child(
        button(t, Variant::Ghost, Size::Sm, None, "Settings → Memory")
            .key("builtin-settings")
            .on_press(|s: &mut State| s.builtin_open_settings()),
    )
}

/// The body: 760 wide at most, centred, 14 between its parts.
pub fn body(t: &Theme, s: &State, a: &Automation) -> El {
    let words = |key: &str, w: String| text(w).key(key.to_string()).font(sans(rem(0.96429), 400.0, 1.5)).color(t.muted).wrap();
    let mut inner = col().w_full().max_w(760.0).gap(14.0).child(words("builtin-description", description(a))).child(actions(s, t, a));
    if let Some(note) = &s.automations_nav.notice {
        inner = inner.child(super::detail::notice(t, note, 0.0));
    }
    let runs = &s.automations_nav.runs;
    if runs.of.as_ref().map(|(_, id)| id.as_str()) == Some(a.id.as_str()) {
        if runs.loaded && runs.list.is_empty() {
            inner = inner.child(words("automation-no-runs", no_runs(a)));
        } else if let Some(list) = super::runs::section(s, t, a) {
            inner = inner.child(list);
        }
    }
    col().w_full().items_center().child(inner)
}
