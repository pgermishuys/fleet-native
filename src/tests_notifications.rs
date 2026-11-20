//! Session notifications (component 24, piece F): `use-session-notifications.test.ts` under its names, against a notifier
//! that writes down what it was asked. Fleet's tests mount the composable with a fake `Notification`; here the sessions
//! topic's event goes through `State::dispatch`, the preference is read from the app's preferences, and "the tab is looking"
//! is the open session on a window that is on screen and has the keyboard.
use crate::desktop::notify::Recorder;
use crate::fleet::events::{Event, Kind};
use crate::fleet::settings::switches as sw;
use crate::tests::app;
use rune::Runtime;
use serde_json::json;
use std::sync::Arc;

struct Rig {
    rt: Runtime<crate::State>,
    notifier: Arc<Recorder>,
}

/// A window with notifications on (or off) and a system that can (or can't) post. Two real-looking ids are open sessions.
fn rig(enabled: bool, can_post: bool) -> Rig {
    let mut rt = app();
    let notifier = Arc::new(Recorder::new(can_post));
    let n = notifier.clone();
    rt.update(move |s, _| {
        s.notify.notifier = Some(n);
        s.pickers.prefs.fetched = true;
        s.pickers.prefs.set(sw::DESKTOP_NOTIFICATIONS.key, if enabled { "true" } else { "false" });
    });
    Rig { rt, notifier }
}

fn event(session: &str) -> Event {
    Event::Other(
        Kind::SessionNotification,
        json!({"sessionId": session, "reason": "needs_you", "title": "Make the dashboard true", "body": "Waiting on your answer."}),
    )
}

impl Rig {
    fn send(&mut self, e: Event) {
        self.rt.update(move |s, _| s.dispatch(&crate::fleet::state::Topic::Sessions, &[e]));
    }
    fn open(&mut self, id: &str) {
        let id = id.to_string();
        self.rt.update(move |s, tx| {
            s.go(crate::fleet::store::Route::Session(id));
            s.after_open(tx);
        });
    }
    fn window(&mut self, focused: bool) {
        self.rt.update(move |s, _| {
            s.quiet.focused = focused;
            s.clear_notification_for_open();
        });
    }
    fn calls(&self) -> Vec<String> {
        self.notifier.all()
    }
}

// use-session-notifications.test.ts: "shows a notification for a session that needs you"
#[test]
fn shows_a_notification_for_a_session_that_needs_you() {
    let mut r = rig(true, true);
    r.send(event("s1"));
    // Tagged with the session's id, so a second one for the session replaces the first.
    assert_eq!(r.calls(), ["post s1 | Make the dashboard true | Waiting on your answer."]);
}

// use-session-notifications.test.ts: "stays quiet when the setting is off"
#[test]
fn stays_quiet_when_the_setting_is_off() {
    let mut r = rig(false, true);
    r.send(event("s1"));
    assert!(r.calls().is_empty());
}

// use-session-notifications.test.ts: "stays quiet when the browser has not granted permission" (native: the system can't post)
#[test]
fn stays_quiet_when_the_system_cannot_post() {
    let mut r = rig(true, false);
    r.send(event("s1"));
    assert!(r.calls().is_empty());
}

// use-session-notifications.test.ts: "stays quiet about the session this tab is looking at"
#[test]
fn stays_quiet_about_the_session_this_window_is_looking_at() {
    let mut r = rig(true, true);
    r.open("s1");
    r.send(event("s1"));
    assert!(r.calls().is_empty());
}

// use-session-notifications.test.ts: "still speaks up about the open session when the window is in the background"
#[test]
fn still_speaks_up_about_the_open_session_when_the_window_is_in_the_background() {
    let mut r = rig(true, true);
    r.open("s1");
    r.window(false);
    r.send(event("s1"));
    assert_eq!(r.calls().len(), 1);
    // ...and also when the window is hidden to the tray with the keyboard still "on" it.
    r.window(true);
    r.rt.update(|s, _| s.quiet.visible = false);
    r.send(event("s1"));
    assert_eq!(r.calls().len(), 3, "a post, a take-down when it came forward, a post: {:?}", r.calls());
}

// use-session-notifications.test.ts: "takes down its own notification when you open that session"
#[test]
fn takes_down_its_own_notification_when_you_open_that_session() {
    let mut r = rig(true, true);
    r.send(event("s1"));
    r.open("s2");
    assert_eq!(r.calls().len(), 1, "another session opening takes nothing down");
    r.open("s1");
    assert_eq!(r.calls().last().unwrap(), "down s1");
    assert_eq!(r.calls().len(), 2);
}

// use-session-notifications.test.ts: "takes down a pushed notification for the session once it is open and in front"
#[test]
fn takes_down_the_notification_once_the_open_session_comes_in_front() {
    let mut r = rig(true, true);
    r.window(false);
    r.send(event("s1"));
    r.open("s1");
    assert_eq!(r.calls().len(), 1, "open but not in front: still up");
    r.window(true);
    assert_eq!(r.calls().last().unwrap(), "down s1");
    r.send(event("s2"));
    r.window(true);
    assert!(!r.calls().iter().any(|c| c == "down s2"), "another session's stays");
}

// use-session-notifications.test.ts: "ignores other events on the sessions topic"
#[test]
fn ignores_other_events_on_the_sessions_topic() {
    let mut r = rig(true, true);
    r.send(Event::Other(Kind::ActivityStatus, json!({"sessionId": "s1", "activityStatus": "idle"})));
    r.send(Event::Other(Kind::SessionNotification, json!({"reason": "finished"})));
    assert!(r.calls().is_empty(), "no session id, nothing to tag");
}

#[test]
fn a_title_the_event_lacks_is_weave_fleet() {
    let mut r = rig(true, true);
    r.send(Event::Other(Kind::SessionNotification, json!({"sessionId": "s1", "body": "Finished its turn."})));
    assert_eq!(r.calls(), ["post s1 | Weave Fleet | Finished its turn."]);
}

// Row 24.92: a click brings the window forward and opens that session.
#[test]
fn a_click_shows_the_window_and_opens_that_session() {
    let mut r = rig(true, true);
    r.send(event("s9"));
    r.rt.update(|s, tx| s.notification_clicked("s9", tx));
    assert_eq!(r.rt.state.open_id().as_deref(), Some("s9"));
    assert!(!r.rt.state.notify.shown.contains("s9"));
}

// Row 24.90: the note as the window goes to the tray is silent, and says how many are working.
#[test]
fn the_note_about_the_tray_is_silent() {
    let mut r = rig(false, true);
    r.rt.update(|s, _| s.hide_to_tray(2));
    assert_eq!(
        r.calls(),
        [
            "post fleet-still-running | Fleet is still running | 2 sessions are still working. Quit from the tray icon when you're done. | silent"
        ]
    );
    // A click on it only shows the window.
    r.rt.update(|s, tx| s.notification_clicked("fleet-still-running", tx));
    assert_ne!(r.rt.state.open_id().as_deref(), Some("fleet-still-running"));
}

// Row 24.97: where the system won't take a post, the note under the switch says so.
#[test]
fn the_settings_line_shows_only_where_the_system_cannot_post() {
    for (can, shown) in [(true, false), (false, true)] {
        let mut r = rig(false, can);
        r.rt.update(|s, _| {
            s.rail = crate::Rail::Settings;
            s.settings.set("general", None);
        });
        r.rt.frame(0.0);
        r.rt.frame(crate::tests::FRAME);
        assert_eq!(r.rt.find("notifications-later").is_some(), shown, "can_post={can}");
    }
}

#[test]
fn the_freedesktop_call_asks_for_a_default_action_and_replaces_by_id() {
    let call = crate::desktop::notify::notify_call(7, "T", "B", true);
    assert_eq!((call.replaces_id, call.actions.as_slice(), call.silent), (7, ["default", "Open"].as_slice(), true));
}
