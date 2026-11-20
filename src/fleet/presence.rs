//! Telling the server whether this window is on screen and whether it is looking at the open session
//! (`use-desk-presence.ts`, `use-session-recap.ts`, rows 6.5 and 6.6). The server uses the first to keep a
//! phone quiet while a computer shows Fleet, and the second to hold back a recap and a desktop notification
//! for a session someone is already looking at.
//!
//! Pure: no socket, no clock of its own. The link thread feeds it what happened and when, and sends the
//! calls it returns.
use std::time::{Duration, Instant};

/// How often a window says it is still there; the server forgets one after 90 s (`PRESENCE_INTERVAL_MS`).
pub const PRESENCE_INTERVAL: Duration = Duration::from_secs(30);

/// Focus flickers when the pointer crosses into another surface of the same window; wait for it to settle
/// (`FOCUS_SETTLE_MS`).
pub const FOCUS_SETTLE: Duration = Duration::from_millis(250);

/// A hub call to make.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// `SetPresenceAsync(visible, "desktop")`: the form factor is always a computer here.
    Presence(bool),
    /// `SetSessionFocusAsync(sessionId, focused)`.
    Focus(String, bool),
}

#[derive(Debug)]
pub struct Presence {
    /// Whether the window has the keyboard (`window` `focus` / `blur`). A new window counts as focused.
    focused: bool,
    /// Whether the window is on screen (`visibilitychange`).
    visible: bool,
    /// When presence is due again.
    next_tick: Option<Instant>,
    /// The session the server holds a subscription for, once its snapshot is in.
    subscribed: Option<String>,
    /// What the server was last told about focus: the session and whether it is looked at.
    reported: Option<(String, bool)>,
    /// When a focus or visibility change has settled enough to report.
    settle_at: Option<Instant>,
    /// Fleet reports focus only while session recaps (on this machine) or desktop notifications (here) are
    /// switched on (`use-session-recap.ts`'s `enabled`). On until `set_enabled` says otherwise, so a link that
    /// starts before the preferences are read still reports.
    enabled: bool,
}

impl Default for Presence {
    fn default() -> Self {
        Presence { focused: true, visible: true, next_tick: None, subscribed: None, reported: None, settle_at: None, enabled: true }
    }
}

impl Presence {
    /// Visible and focused: `isLooking`.
    fn looking(&self) -> bool {
        self.visible && self.focused
    }

    /// A hub connection opened, a first one or a new one: say where this window is. The server has forgotten
    /// what it was told, so focus is reported again once the snapshot is back.
    pub fn connected(&mut self, now: Instant) -> Vec<Call> {
        self.next_tick = Some(now + PRESENCE_INTERVAL);
        self.subscribed = None;
        self.reported = None;
        vec![Call::Presence(self.visible)]
    }

    /// The connection is gone: nothing can be reported until `connected`.
    pub fn disconnected(&mut self) {
        self.next_tick = None;
        self.subscribed = None;
        self.reported = None;
        self.settle_at = None;
    }

    /// The window gained or lost the keyboard.
    pub fn focus_changed(&mut self, focused: bool, now: Instant) {
        if self.focused != focused {
            self.focused = focused;
            self.settle_at = Some(now + FOCUS_SETTLE);
        }
    }

    /// The window went off screen or came back. Presence says so at once; focus waits to settle.
    pub fn visibility_changed(&mut self, visible: bool, now: Instant) -> Vec<Call> {
        if self.visible == visible {
            return vec![];
        }
        self.visible = visible;
        self.settle_at = Some(now + FOCUS_SETTLE);
        if self.next_tick.is_some() { vec![Call::Presence(visible)] } else { vec![] }
    }

    /// The open session's snapshot arrived (a first one, or one restored after a reconnect): the server
    /// has forgotten this window, so say again.
    pub fn snapshot(&mut self, session: &str) -> Vec<Call> {
        self.subscribed = Some(session.to_string());
        self.reported = None;
        self.report()
    }

    /// The two preferences changed: nothing is reported while both are off. Turning it on says where the window is.
    pub fn set_enabled(&mut self, enabled: bool) -> Vec<Call> {
        if self.enabled == enabled {
            return vec![];
        }
        self.enabled = enabled;
        self.report()
    }

    /// Another session is now the open one. Tells the server the old one is no longer looked at.
    pub fn left(&mut self) -> Vec<Call> {
        let calls = match self.reported.take() {
            Some((id, true)) => vec![Call::Focus(id, false)],
            _ => vec![],
        };
        self.subscribed = None;
        calls
    }

    /// Time passed: the 30 s presence, and a focus change that has settled.
    pub fn tick(&mut self, now: Instant) -> Vec<Call> {
        let mut calls = vec![];
        if let Some(due) = self.next_tick
            && now >= due
        {
            self.next_tick = Some(now + PRESENCE_INTERVAL);
            calls.push(Call::Presence(self.visible));
        }
        if let Some(at) = self.settle_at
            && now >= at
        {
            self.settle_at = None;
            calls.extend(self.report());
        }
        calls
    }

    /// When `tick` next has something to do, for a thread that waits.
    pub fn next_due(&self) -> Option<Instant> {
        [self.next_tick, self.settle_at].into_iter().flatten().min()
    }

    /// `report()`: the open session's focus, unless it is what the server already has.
    fn report(&mut self) -> Vec<Call> {
        let Some(id) = self.subscribed.clone() else { return vec![] };
        if !self.enabled {
            return vec![];
        }
        let looking = self.looking();
        if self.reported.as_ref().is_some_and(|(s, f)| *s == id && *f == looking) {
            return vec![];
        }
        self.reported = Some((id.clone(), looking));
        vec![Call::Focus(id, looking)]
    }
}
