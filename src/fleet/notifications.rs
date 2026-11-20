//! Session notifications (`use-session-notifications.ts`, rows 24.91-24.95): the server sends `session_notification` on the
//! sessions topic when a session needs the person or finishes and nobody is looking at it. If Settings › General › Desktop
//! notifications is on, this shows it as the system's notification, tagged with the session's id so a second one for the same
//! session replaces the first. A click brings the window forward on that session; opening the session (or the window coming
//! forward on it) takes its notification down.
use super::events::{Event, Kind};
use crate::State;
use crate::fleet::settings::switches::DESKTOP_NOTIFICATIONS;
use rune::Tx;
use serde_json::Value;

/// The title when the event brings none (`payload.title || "Weave Fleet"`).
pub const DEFAULT_TITLE: &str = "Weave Fleet";

/// The tag of the note posted as the window goes to the tray; a click on it only brings the window back.
pub const STILL_RUNNING_TAG: &str = "fleet-still-running";

impl State {
    /// Whether this window is looking at the session: it is open, and the window is on screen and has the keyboard
    /// (`isLookingAt`).
    pub fn looking_at(&self, session: &str) -> bool {
        self.open_id().as_deref() == Some(session) && self.quiet.visible && self.quiet.focused
    }

    /// `session_notification` `{sessionId, title, body, ...}` on the sessions topic. Any other event is ignored.
    pub fn on_notification_event(&mut self, event: &Event) {
        let Event::Other(Kind::SessionNotification, props) = event else { return };
        if !DESKTOP_NOTIFICATIONS.enabled(&self.pickers.prefs) {
            return;
        }
        let Some(notifier) = self.notify.notifier.clone().filter(|n| n.can_post()) else { return };
        let Some(session) = props.get("sessionId").and_then(Value::as_str).filter(|s| !s.is_empty()) else { return };
        // The server sent this because no window was looking; make sure that's still true of this one.
        if self.looking_at(session) {
            return;
        }
        let text = |k: &str| props.get(k).and_then(Value::as_str).unwrap_or_default();
        let title = if text("title").is_empty() { DEFAULT_TITLE } else { text("title") };
        notifier.post(session, title, text("body"), false);
        self.notify.shown.insert(session.to_string());
    }

    /// Once the person is looking at a session, what it told them is old: take its notification off the screen
    /// (`clearForOpenSession`). Runs when the route changes and when the window gains the keyboard or comes on screen.
    pub fn clear_notification_for_open(&mut self) {
        let Some(id) = self.open_id().filter(|id| self.looking_at(id)) else { return };
        if self.notify.shown.remove(&id)
            && let Some(notifier) = &self.notify.notifier
        {
            notifier.take_down(&id);
        }
    }

    /// A click on a notification: the window comes forward and the session opens. The note about the tray only shows the
    /// window.
    pub fn notification_clicked(&mut self, tag: &str, tx: &mut Tx) {
        if let Some(surface) = &self.close.surface {
            surface.show_window();
        }
        self.window_visible();
        if tag == STILL_RUNNING_TAG {
            return;
        }
        self.notify.shown.remove(tag);
        self.open_dreaming_session(tag, tx);
    }

    /// "Fleet is still running", as the window goes to the tray (`hideToTray`): silent, only where the system can post.
    pub fn post_still_running(&self, working: usize) {
        let Some(notifier) = self.notify.notifier.as_ref().filter(|n| n.can_post()) else { return };
        let (title, body) = crate::desktop::close::still_running(working);
        notifier.post(STILL_RUNNING_TAG, &title, &body, true);
    }
}
