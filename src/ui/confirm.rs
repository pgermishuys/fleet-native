//! `confirmAction` and `ConfirmHost.vue`: the one way the app asks "are you sure?" before something that can't be taken
//! back. A question (title, the subject it is about, what happens, the action's words), drawn on `ui::dialog`'s alert.
//! Focus starts on Cancel when the action is destructive, so Enter can't take it by accident; while the request runs both
//! buttons wait and Escape does nothing; when it fails the dialog stays open and says why, to try again or cancel.
use super::button::{Size, Variant, button, button_lead, disabled};
use super::dialog::{Dialog, dialog};
use super::rem;
use crate::fleet::actions::{CONFIRM_ACTION, CONFIRM_CANCEL, Call};
use crate::icons::{CIRCLE_ALERT, LOADER_CIRCLE};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::{State, Tx};
use rune::{Color, Cycle, El, col, icon, row, text};

/// What is asked, and what answers it.
#[derive(Clone, Debug)]
pub struct Confirm {
    pub title: String,
    /// The name the action is about, on its own line.
    pub subject: String,
    pub body: String,
    pub confirm_label: String,
    pub busy_label: String,
    /// What went wrong when the request fails; the server's words go under it.
    pub error_message: String,
    pub destructive: bool,
    /// The request is on its way.
    pub busy: bool,
    pub error: Option<String>,
    pub call: Call,
}

impl Confirm {
    /// `handleClearAttachments`.
    pub fn clear_attachments(subject: String, call: Call) -> Confirm {
        Confirm {
            title: "Clear attachments?".into(),
            subject,
            body: "This deletes the files attached to this session. The agent can't read them again. The conversation stays.".into(),
            confirm_label: "Clear attachments".into(),
            busy_label: "Clearing…".into(),
            error_message: "Couldn't clear the attachments.".into(),
            destructive: true,
            busy: false,
            error: None,
            call,
        }
    }

    /// `handleDelete`.
    pub fn delete_session(subject: String, call: Call) -> Confirm {
        Confirm {
            title: "Delete this session?".into(),
            subject,
            body: "It and its session data are deleted for good.".into(),
            confirm_label: "Delete session".into(),
            busy_label: "Deleting…".into(),
            error_message: "Couldn't delete the session.".into(),
            destructive: true,
            busy: false,
            error: None,
            call,
        }
    }
}

impl State {
    /// Asks. One at a time: a new ask replaces the one before it. The menu that asked closes.
    pub fn ask(&mut self, confirm: Confirm, _tx: &mut Tx) {
        self.ui.open = None;
        self.ui.confirm = Some(confirm);
    }

    /// Cancel and Escape: closes, unless the request is running.
    pub fn cancel_confirm(&mut self) {
        if self.ui.confirm.as_ref().is_some_and(|c| !c.busy) {
            self.ui.confirm = None;
        }
    }

    /// The action's button: runs the request with the dialog held open.
    pub fn take_confirm(&mut self, tx: &mut Tx) {
        let Some(c) = self.ui.confirm.as_mut().filter(|c| !c.busy) else { return };
        c.busy = true;
        c.error = None;
        let call = c.call.clone();
        // A machine's question has no request to hold the dialog open for: it closes and the page does the rest.
        match call {
            Call::ForgetMachine { key } => {
                self.ui.confirm = None;
                self.forget_machine(&key);
            }
            Call::ReplaceToken => {
                self.ui.confirm = None;
                self.replace_home_token();
            }
            call => self.send_session_call(call, None, tx),
        }
    }

    /// The request answered: the dialog closes and the session leaves the list, or stays open with the reason.
    pub fn confirm_done(&mut self, result: Result<(), String>, tx: &mut Tx) {
        let Some(c) = self.ui.confirm.as_mut() else { return };
        match result {
            Ok(()) => {
                let call = c.call.clone();
                self.ui.confirm = None;
                if let Call::Delete { id } = call {
                    self.forget_session(&id, tx);
                }
            }
            Err(why) => {
                c.busy = false;
                c.error = Some(why.clone());
                // The header's toolbar keeps the same words under its buttons (`deleteError`).
                if let Call::Delete { id } = c.call.clone() {
                    self.head.bar.delete_error = Some((id, why));
                }
            }
        }
    }
}

/// `Spinner` (`size="sm"`): 14px, turning.
pub(crate) fn spinner(ink: Color) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, 14.0).color(ink).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// `ErrorNote`: what went wrong in medium weight, the server's reason under it. With no reason the message is the error
/// itself, in regular weight.
pub(crate) fn error_note(t: &Theme, key: &str, message: &str, reason: Option<&str>) -> El {
    let font = |w| sans(rem(0.875), w, 1.43);
    let mut words = col()
        .shrink()
        .min_w(0.0)
        .child(text(message.to_string()).font(font(if reason.is_some() { 500.0 } else { 400.0 })).color(t.error_text).wrap());
    if let Some(reason) = reason {
        words = words.child(text(reason.to_string()).font(font(400.0)).color(t.error_text).wrap().mt(2.0));
    }
    row()
        .key(key.to_string())
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
        .child(words)
}

/// The dialog and its scrim, to mount over the window; `None` while nothing is asked.
pub fn view(s: &State, t: &Theme) -> Option<Vec<El>> {
    let c = s.ui.confirm.as_ref()?;
    let description = col()
        .gap(rem(0.25))
        .child(text(c.subject.clone()).key("confirm-subject").font(sans(rem(0.875), 500.0, 1.4)).color(t.text).wrap().w_full())
        .child(text(c.body.clone()).key("confirm-body").font(sans(rem(0.875), 400.0, 1.4)).color(t.muted).wrap().w_full());
    let body = c.error.iter().map(|why| error_note(t, "confirm-error", &c.error_message, Some(why))).collect();
    // The dialog opens with focus on Cancel when the action is destructive (`onOpenAutoFocus`).
    let start = |e: El, first: bool| if first { e.autofocus() } else { e };
    let cancel = start(button(t, Variant::Outline, Size::Default, None, "Cancel").key(CONFIRM_CANCEL), c.destructive);
    let kind = if c.destructive { Variant::Destructive } else { Variant::Default };
    let action = if c.busy {
        button_lead(t, kind, Size::Default, Some(spinner(Color::WHITE)), &c.busy_label).busy(true)
    } else {
        button(t, kind, Size::Default, None, &c.confirm_label)
    }
    .key(CONFIRM_ACTION);
    let action = start(action, !c.destructive);
    let (cancel, action) = if c.busy {
        (disabled(cancel), disabled(action))
    } else {
        (cancel.on_press(|s: &mut State| s.cancel_confirm()), action.on_press_tx(|s: &mut State, tx: &mut Tx| s.take_confirm(tx)))
    };
    Some(dialog(
        t,
        Dialog {
            key: "confirm",
            title: c.title.clone(),
            description_el: Some(description),
            body,
            footer: vec![cancel, action],
            close_button: false,
            click_outside: false,
            dismiss: Some(|s: &mut State| s.cancel_confirm()),
            ..Default::default()
        },
    ))
}

// ---- scenes ---------------------------------------------------------------------------------------------------------------

impl State {
    /// The Delete session dialog for the row at (`p`, `i`), as it stands while `busy` or after the server refused with `error`.
    pub fn show_confirm(&mut self, delete: bool, p: usize, i: usize, busy: bool, error: Option<&str>) {
        let session = &self.projects[p].sessions[i];
        let (title, id) = (crate::fleet::row_status::display_title(None, &session.title), session.id.clone());
        let mut c = if delete {
            Confirm::delete_session(title, Call::Delete { id })
        } else {
            Confirm::clear_attachments(title, Call::ClearAttachments { id })
        };
        c.busy = busy;
        c.error = error.map(str::to_string);
        self.ui.confirm = Some(c);
    }
}
