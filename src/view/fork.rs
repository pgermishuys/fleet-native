//! `ForkSessionDialog.vue`: "Fork session", the source's title, a title field and one button. Drawn on `ui::dialog` (the
//! window is `sm:max-w-lg`, 32rem, which is the dialog's own width). The state is `fleet::fork`.
use crate::fleet::fork::{ForkDialog, TITLE_FIELD};
use crate::icons::CIRCLE_ALERT;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::controls::input;
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{El, Key, KeyEvent, col, icon, row, text};

pub const DESCRIPTION: &str = "A new session with a copy of this conversation, up to the last reply that finished. From there the two conversations go their own ways. Both work in the same folder, so each sees the files the other changes.";

/// `ErrorNote`: the server's words in a red box.
fn error_note(t: &Theme, message: &str) -> El {
    row()
        .key("fork-session-error")
        .role("alert")
        .w_full()
        .gap(rem(0.5))
        .items_start()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(col().shrink().min_w(0.0).child(text(message.to_string()).font(sans(rem(0.875), 400.0, 1.4)).color(t.error_text).wrap()))
}

fn body(t: &Theme, d: &ForkDialog) -> Vec<El> {
    let label = |words: &str| text(words.to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text);
    let source = col()
        .w_full()
        .gap(rem(0.5))
        .child(label("Source session"))
        .child(text(d.source_title.clone()).key("fork-session-source-title").font(sans(rem(0.875), 400.0, 1.4)).color(t.muted).wrap());
    let mut field = input(t, &d.title, "Enter a title").key(TITLE_FIELD);
    field = if d.forking {
        field.opacity(0.5).disabled(true)
    } else {
        field.on_edit(|s: &mut State| &mut s.head.bar.fork.as_mut().expect("the dialog is open").title).on_input_key(
            |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
                if ev.key == Key::Enter {
                    s.fork_submit(tx);
                    return true;
                }
                false
            },
        )
    };
    let title = col().w_full().gap(rem(0.5)).child(label("New session title")).child(field);
    let mut form = col().w_full().gap(rem(1.0)).child(source).child(title);
    if let Some(why) = &d.error {
        form = form.child(error_note(t, why));
    }
    vec![form]
}

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let d = s.head.bar.fork.as_ref()?;
    let submit = button(t, Variant::Default, Size::Default, None, if d.forking { "Forking\u{2026}" } else { "Fork session" })
        .key("fork-session-submit");
    let submit = if d.forking { disabled(submit) } else { submit.on_press_tx(|s: &mut State, tx: &mut Tx| s.fork_submit(tx)) };
    Some(dialog(
        t,
        Dialog {
            key: "fork-session",
            title: "Fork session".into(),
            description_el: Some(text(DESCRIPTION).font(sans(rem(0.875), 400.0, 1.4)).color(t.muted).wrap().w_full()),
            body: body(t, d),
            footer: vec![submit],
            close_button: true,
            click_outside: true,
            dismiss: Some(|s: &mut State| s.close_fork_dialog()),
            ..Default::default()
        },
    ))
}
