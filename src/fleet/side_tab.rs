//! The side conversation's tab, its peek and the Undo toast: what they need beyond the state (`side`, `side_http`), which is
//! their motion, from the app's clock. Ports the animations of `SideConversationPanel.vue`: the tab's entrance (260 ms), the
//! blinking dots (1.2 s), the unread dot's pulse (1.6 s, twice), the peek's fade (160 ms) and the toast's entrance (180 ms).
use super::side::SideState;
use super::tab_motion::ease_out;
use crate::{State, Tx};
use std::time::Duration;

pub const TAB_ENTER_SECS: f64 = 0.26;
pub const PEEK_SECS: f64 = 0.16;
pub const TOAST_ENTER_SECS: f64 = 0.18;
const DOTS_SECS: f64 = 1.2;
const PULSE_SECS: f64 = 1.6;
const PULSE_TIMES: f64 = 2.0;
/// The key of the tab's open button: keyboard focus on it shows the peek (`:focus-within`).
pub const OPEN_KEY: &str = "side-open";
/// The tab's × is inside the same wrap, so focus on it shows the peek too.
pub const CLOSE_KEY: &str = "side-tab-close";

/// The tab's own bits of `State`.
#[derive(Default)]
pub struct TabView {
    /// The pointer is over the tab (`:hover`).
    pub hover: bool,
    /// Whether the peek was wanted as of the last frame, and when that last changed.
    peek: bool,
    peek_at: Option<f64>,
    /// When the tab appeared (it is drawn only while folded), and whether it was there on the last frame.
    shown: bool,
    tab_at: Option<f64>,
    /// The answer whose unread dot pulses, and when it started.
    pulse: Option<(String, f64)>,
    /// When the Undo toast appeared, and whether it was there on the last frame.
    toast: bool,
    toast_at: Option<f64>,
}

/// What the tab says of its side conversation (`side-tab__state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    Thinking,
    NewAnswer { pulse: bool },
    Answered,
    Nothing,
}

impl Word {
    pub fn text(self) -> &'static str {
        match self {
            Word::Thinking => "Thinking",
            Word::NewAnswer { .. } => "New answer",
            Word::Answered => "Answered",
            Word::Nothing => "",
        }
    }
}

impl SideState {
    /// Thinking while it works, else New answer when unread, else Answered when there is an answer.
    pub fn word(&self) -> Word {
        if self.working {
            Word::Thinking
        } else if self.unread() {
            Word::NewAnswer { pulse: self.pulse() }
        } else if self.latest_answer.is_some() {
            Word::Answered
        } else {
            Word::Nothing
        }
    }

    /// What the peek says (`side-peek__text`): the answer is plain text, its line breaks kept.
    pub fn peek_text(&self) -> String {
        if self.working {
            "Thinking about your question\u{2026}".into()
        } else {
            self.latest_answer.clone().unwrap_or_else(|| "No answer yet.".into())
        }
    }
}

/// `@keyframes side-dots`: 0, 80 and 100 % at .25, 40 % at 1; the dots start .2 s apart.
pub fn dot_opacity(clock: f64, index: usize) -> f32 {
    let p = ((clock - 0.2 * index as f64).rem_euclid(DOTS_SECS) / DOTS_SECS) as f32;
    let (lo, hi) = (0.25, 1.0);
    if p < 0.4 {
        lo + (hi - lo) * p / 0.4
    } else if p < 0.8 {
        hi - (hi - lo) * (p - 0.4) / 0.4
    } else {
        lo
    }
}

impl State {
    /// The tab is drawn: folded, with the fork made.
    pub fn side_tab_shown(&self) -> bool {
        self.side().is_some_and(|st| st.minimized() && st.side.is_some())
    }

    /// The tab's entrance, 0 to 1 over 260 ms, or `None` once it has arrived.
    pub fn side_tab_enter(&self) -> Option<f32> {
        let at = self.side_ui.tab.tab_at?;
        let p = (self.clock - at) / TAB_ENTER_SECS;
        (0.0..1.0).contains(&p).then(|| ease_out(p as f32))
    }

    /// How much of the peek shows, 0 to 1: it fades in and out over 160 ms.
    pub fn side_peek_amount(&self) -> f32 {
        let tab = &self.side_ui.tab;
        let to = if tab.peek { 1.0 } else { 0.0 };
        let Some(at) = tab.peek_at else { return to };
        let p = ease_out(((self.clock - at) / PEEK_SECS) as f32);
        if tab.peek { p } else { 1.0 - p }
    }

    /// How far the unread dot's ring has come in its cycle (0 to 1), while it pulses: twice, for an answer that landed on this run.
    pub fn side_pulse_phase(&self) -> Option<f32> {
        let (_, at) = self.side_ui.tab.pulse.as_ref()?;
        let age = self.clock - at;
        (0.0..PULSE_SECS * PULSE_TIMES).contains(&age).then(|| (age.rem_euclid(PULSE_SECS) / PULSE_SECS) as f32)
    }

    /// The toast's entrance, 0 to 1 over 180 ms.
    pub fn side_toast_enter(&self) -> f32 {
        match self.side_ui.tab.toast_at {
            Some(at) => ease_out(((self.clock - at) / TOAST_ENTER_SECS) as f32),
            None => 1.0,
        }
    }

    /// The toast's bar: the share of Undo's window that is left, shrinking linearly to nothing.
    pub fn side_drain(&self) -> f32 {
        let Some(st) = self.side() else { return 0.0 };
        let Some(until) = st.undo_until else { return 0.0 };
        (((until - self.clock) * 1000.0) / st.undo_ms.max(1) as f64).clamp(0.0, 1.0) as f32
    }

    /// Undo is offered for the open session's last discarded side conversation.
    pub fn side_undo_offered(&self) -> bool {
        self.side().is_some_and(|st| st.discarded.is_some() && st.undo_until.is_some())
    }

    /// Undo.
    pub fn side_undo(&mut self) {
        if let Some(owner) = self.draft_owner() {
            self.undo_side_discard(&owner);
        }
    }

    /// Keeps the tab's motion running (each frame): notes when the tab, the peek, the pulse and the toast began, and asks for frames
    /// until they settle. The dots and the drain are drawn from the clock, so they keep frames coming while they show.
    pub fn keep_side_tab(&mut self, tx: &mut Tx) -> bool {
        let clock = self.clock;
        let shown = self.side_tab_shown();
        let focus = shown && tx.focused().is_some_and(|k| k == OPEN_KEY || k == CLOSE_KEY);
        let want = shown && (self.side_ui.tab.hover || focus);
        let toast = self.side_undo_offered();
        let pulse_of = self.side().filter(|_| shown).filter(|st| st.pulse()).and_then(|st| st.latest_answer_id.clone());
        let thinking = shown && self.side().is_some_and(|st| st.word() == Word::Thinking);
        let tab = &mut self.side_ui.tab;
        if shown != tab.shown {
            tab.shown = shown;
            tab.tab_at = shown.then_some(clock);
        }
        if want != tab.peek {
            tab.peek = want;
            tab.peek_at = Some(clock);
        }
        if !shown {
            tab.hover = false;
        }
        if toast != tab.toast {
            tab.toast = toast;
            tab.toast_at = toast.then_some(clock);
        }
        match (pulse_of, tab.pulse.as_ref()) {
            (Some(id), Some((was, _))) if *was == id => {}
            (Some(id), _) => tab.pulse = Some((id, clock)),
            (None, _) => tab.pulse = None,
        }
        let running = self.side_tab_enter().is_some()
            || self.side_pulse_phase().is_some()
            || self.side_ui.tab.peek_at.is_some_and(|at| clock - at < PEEK_SECS)
            || self.side_ui.tab.toast_at.is_some_and(|at| toast && clock - at < TOAST_ENTER_SECS);
        if running || thinking || toast {
            tx.wake_after(Duration::from_millis(16));
        }
        running || thinking || toast
    }
}
