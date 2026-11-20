//! `components/canvas/BrowserOpenDialog.vue` (component 14, piece J): "Open a browser tab", from the + menu's Browser entry. A
//! command to run in the session's folder, or an address on Fleet's machine. Drawn on `ui::dialog`; the window is
//! `sm:max-w-md` (28rem). The state is `web::browser_open`.
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::web::browser_open::{BrowserOpen, FIELD, hint, submit_word};
use crate::{State, Tx};
use rune::{Cursor, El, Key, KeyEvent, col, row, text};

/// The field (`Input` with `font-mono` at 12.5 px): the kit's input, in the mono face.
fn field(t: &Theme, d: &BrowserOpen) -> El {
    let f = text_input(&d.value, mono(12.5, 400.0, 1.3))
        .key(FIELD)
        .single_line()
        .placeholder("npm run dev", t.muted)
        .label("Command or address")
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .h(rem(2.25))
        .px(rem(0.75))
        .py((rem(2.25) - 12.5 * 1.3) / 2.0 - 1.0)
        .border(1.0, t.input())
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5));
    if d.busy {
        return f.opacity(0.5).disabled(true);
    }
    f.on_edit(|s: &mut State| &mut s.web.open.as_mut().expect("the dialog is open").value).on_input_key(
        |s: &mut State, ev: &KeyEvent, _: &mut Tx| match &ev.key {
            Key::Enter => {
                s.browser_open_submit();
                true
            }
            Key::Text(_) | Key::Backspace => {
                s.browser_open_typed();
                false
            }
            _ => false,
        },
    )
}

/// `.browser-open__suggestion`: a mono 11.5 chip; the one that is the field's value is on.
fn chip(t: &Theme, at: usize, command: &str, on: bool) -> El {
    let picked = command.to_string();
    let mut c = row()
        .key(format!("browser-open-suggestion-{at}"))
        .help(command.to_string())
        .shrink()
        .px(8.0)
        .py(2.0)
        .radius(RADIUS_BTN)
        .border(1.0, if on { t.accent } else { t.border })
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .hover(t.accent_dim)
        .child(text(command.to_string()).font(mono(11.5, 400.0, 1.4)).color(t.text).truncate().min_w(0.0))
        .on_press(move |s: &mut State| s.browser_open_pick(&picked));
    if on {
        c = c.fill(t.accent_dim);
    }
    c
}

/// `.browser-open__error`: padding 6 10, a border and ground of the error colour at 30 % and 8 %.
fn error_box(t: &Theme, why: &str) -> El {
    row()
        .key("browser-open-error")
        .role("alert")
        .w_full()
        .mt(4.0)
        .px(10.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.08))
        .child(text(why.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.error_text).wrap().w_full())
}

fn form(t: &Theme, d: &BrowserOpen) -> El {
    let target = d.target();
    let mut all = col()
        .w_full()
        .gap(8.0)
        .child(text("Command or address").font(sans(13.0, 500.0, 1.4)).color(t.text))
        .child(field(t, d))
        .child(text(hint(target.as_ref())).key("browser-open-hint").font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full());
    if !d.suggestions.is_empty() {
        let typed = d.value.text.trim();
        let mut chips = row()
            .key("browser-open-suggestions")
            .w_full()
            .mt(4.0)
            .flex_wrap()
            .gap(6.0)
            .center_y()
            .child(text("Used here before").font(sans(11.5, 400.0, 1.3)).color(t.muted));
        for (at, command) in d.suggestions.iter().enumerate() {
            chips = chips.child(chip(t, at, command, command == typed));
        }
        all = all.child(chips);
    }
    if let Some(why) = &d.error {
        all = all.child(error_box(t, why));
    }
    let submit = button(t, Variant::Default, Size::Default, None, submit_word(target.as_ref())).key("browser-open-submit");
    let submit = if target.is_none() || d.busy { disabled(submit) } else { submit.on_press(|s: &mut State| s.browser_open_submit()) };
    all.child(row().w_full().justify_end().child(submit))
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let d = s.web.open.as_ref()?;
    Some(dialog(
        t,
        Dialog {
            key: "browser-open",
            title: "Open a browser tab".into(),
            body: vec![form(t, d)],
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_browser_dialog()),
            width: Some(rem(28.0)),
            ..Default::default()
        },
    ))
}
