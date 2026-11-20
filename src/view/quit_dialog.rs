//! The question before the app quits with sessions working in the Fleet it started (component 24, piece E). Fleet's desktop
//! app asks it with the system's message box (`confirmStoppingSessions` in `desktop/src/main.ts`: a warning, the message, the
//! detail, **Quit Fleet** and **Cancel**, Cancel the default); the native app has no system box, so it is the kit's alert.
use crate::State;
use crate::desktop::close::{CANCEL_BUTTON, QUIT_BUTTON, QUIT_DETAIL, quit_message};
use crate::icons::TRIANGLE_ALERT;
use crate::theme::Theme;
use crate::ui::button::{Size, Variant, button};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;

/// The dialog and its scrim, to mount over the window; `None` while it is closed.
pub fn view(s: &State, t: &Theme) -> Option<Vec<rune::El>> {
    let working = s.close.quit_dialog?;
    // Cancel is the default (`defaultId: 1`): the dialog opens with focus on it, so Enter cancels.
    let cancel = button(t, Variant::Outline, Size::Default, None, CANCEL_BUTTON)
        .key("quit-cancel")
        .autofocus()
        .on_press(|s: &mut State| s.quit_cancel());
    let quit = button(t, Variant::Destructive, Size::Default, None, QUIT_BUTTON)
        .key("quit-confirm")
        .on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.quit_confirm(tx));
    Some(dialog(
        t,
        Dialog {
            key: "quit",
            title: quit_message(working),
            title_icon: Some((&TRIANGLE_ALERT, t.error_text)),
            description: Some(QUIT_DETAIL.into()),
            footer: vec![cancel, quit],
            // An alert: no close button, a press outside does nothing; Escape is Cancel.
            close_button: false,
            click_outside: false,
            dismiss: Some(|s: &mut State| s.quit_cancel()),
            width: Some(rem(28.0)),
            ..Default::default()
        },
    ))
}
