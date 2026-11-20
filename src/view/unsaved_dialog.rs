//! Ports `components/canvas/UnsavedFileDialog.vue`, the alert before a file tab with unsaved changes closes: the path in mono, what is
//! lost, then Cancel, **Close without saving** (outline) and **Save and close** (primary; "Saving…" while it saves, all three off meanwhile).
//! The same dialog family asks before the app quits with unsaved files (`use-unsaved-files-guard.ts`; Fleet's browser asks, the native
//! app draws the question itself).
use crate::State;
use crate::fleet::unsaved::{CANCEL, Closing, DETAIL, DISCARD, QUIT_BUTTON, QUIT_TITLE, SAVE, SAVING, TITLE, quit_detail};
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::dialog::{Dialog, dialog};
use crate::ui::rem;
use rune::{El, Span, rich};

/// The dialog and its scrim, to mount over the window; `None` while neither question is open.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    if let Some(closing) = &s.unsaved.closing {
        return Some(closing_dialog(t, closing));
    }
    let files = s.unsaved.quitting.as_ref()?;
    // The quit question is an alert too: Cancel is the way out, Escape is Cancel.
    let cancel = button(t, Variant::Outline, Size::Default, None, CANCEL)
        .key("unsaved-quit-cancel")
        .autofocus()
        .on_press(|s: &mut State| s.quit_unsaved_cancel());
    let quit = button(t, Variant::Destructive, Size::Default, None, QUIT_BUTTON)
        .key("unsaved-quit-confirm")
        .on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.quit_unsaved_confirm(tx));
    Some(dialog(
        t,
        Dialog {
            key: "unsaved-quit",
            title: QUIT_TITLE.into(),
            description: Some(quit_detail(files.len())),
            footer: vec![cancel, quit],
            close_button: false,
            click_outside: false,
            dismiss: Some(|s: &mut State| s.quit_unsaved_cancel()),
            ..Default::default()
        },
    ))
}

fn closing_dialog(t: &Theme, closing: &Closing) -> Vec<El> {
    let words = format!("{} {DETAIL}", closing.path);
    let desc = rich(words, vec![Span::new(0, closing.path.len()).mono().color(t.text)])
        .font(sans(rem(0.875), 400.0, 1.4))
        .color(t.muted)
        .wrap()
        .w_full();
    let cancel = button(t, Variant::Outline, Size::Default, None, CANCEL).key("unsaved-cancel").autofocus();
    let discard = button(t, Variant::Outline, Size::Default, None, DISCARD).key("unsaved-discard");
    let save = button(t, Variant::Default, Size::Default, None, if closing.saving { SAVING } else { SAVE }).key("unsaved-save");
    let (cancel, discard, save) = if closing.saving {
        (disabled(cancel), disabled(discard), disabled(save))
    } else {
        (
            cancel.on_press(|s: &mut State| s.close_cancel()),
            discard.on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.close_discard(tx)),
            save.on_press_tx(|s: &mut State, tx: &mut crate::Tx| s.close_save(tx)),
        )
    };
    dialog(
        t,
        Dialog {
            key: "unsaved",
            title: TITLE.into(),
            description_el: Some(desc),
            footer: vec![cancel, discard, save],
            close_button: false,
            click_outside: false,
            // Escape is Cancel, which does nothing while the save is on its way (the buttons are off).
            dismiss: Some(|s: &mut State| s.close_cancel()),
            ..Default::default()
        },
    )
}
