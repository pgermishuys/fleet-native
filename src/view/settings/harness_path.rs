//! The "Path to weave-copilot" box (`HarnessPathField.vue`), under GitHub Copilot's Profiles: a path, a Check button, and what
//! Fleet found.
use super::kit;
use crate::State;
use crate::fleet::new_session::harness_choice::HarnessEntry;
use crate::fleet::settings::harness_path::{COPILOT_EMPTY_MEANING, COPILOT_PROGRAM, Shown, result};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::{confirm::spinner, rem};
use rune::{Cursor, El, col, icon, row, text, text_input};

pub const INPUT_KEY: &str = "harness-path-input";
pub const CHECK_KEY: &str = "harness-path-check";

pub fn field(s: &State, t: &Theme, h: &HarnessEntry) -> El {
    let page = &s.settings.harnesses.path;
    let input = text_input(&page.draft, mono(rem(0.75), 400.0, 1.33))
        .key(INPUT_KEY)
        .label(format!("Path to {COPILOT_PROGRAM}"))
        .single_line()
        .placeholder(COPILOT_PROGRAM.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .min_w(0.0)
        .px(rem(0.625))
        .py(rem(0.375))
        .radius(RADIUS_BTN)
        .border(1.0, if page.draft.focused { t.accent } else { t.border })
        .border_layout()
        .fill(t.card_bg)
        .on_edit(|s: &mut State| &mut s.settings.harnesses.path.draft)
        .on_submit(|s: &mut State, _| s.check_harness_path());
    let mut check = row()
        .key(CHECK_KEY)
        .role("button")
        .label("Check")
        .center_y()
        .gap(6.0)
        .px(rem(0.75))
        .py(rem(0.375))
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .hover(t.accent.alpha(0.08));
    if page.checking {
        check = check.child(spinner(t.muted));
    }
    check = check.child(text("Check").font(sans(rem(0.75), 500.0, 1.33)).color(t.text));
    check = if page.checking { crate::ui::button::disabled(check) } else { check.on_press(|s: &mut State| s.check_harness_path()) };
    let mut c = col()
        .key("harness-path-field")
        .w_full()
        .px(14.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(text(format!("Path to {COPILOT_PROGRAM}")).font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(
            row().mt(3.5).child(
                text(format!("Leave it empty to {COPILOT_EMPTY_MEANING}."))
                    .font(sans(rem(0.75), 400.0, 1.33))
                    .color(t.muted)
                    .wrap()
                    .shrink()
                    .min_w(0.0),
            ),
        )
        .child(row().w_full().mt(7.0).flex_wrap().center_y().gap(7.0).child(input).child(check));
    let small = |key: &str, words: String, font| {
        row().mt(7.0).child(text(words).key(key.to_string()).font(font).color(t.muted).wrap().shrink().min_w(0.0))
    };
    match result(h, page.error.as_deref()) {
        Shown::Error(why) => c = c.child(kit::error_note(t, "harness-path-error", &why)),
        Shown::Found(found) => c = c.child(small("harness-path-found", format!("Found: {found}"), mono(11.0, 400.0, 1.5))),
        Shown::Reason(why) => c = c.child(small("harness-path-reason", why, sans(rem(0.75), 400.0, 1.33))),
        Shown::Nothing => {}
    }
    let _ = icon;
    c
}
