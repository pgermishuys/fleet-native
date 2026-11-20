//! What the side conversation's panel needs beyond its state (`side`, `side_http`): the conversation it draws, the fold, and the
//! keys. Ports `SideConversationPanel.vue`'s script: the fold on minimize, Ctrl+/ and focus going back to the composer.
use super::side::SideState;
use super::tab_motion::ease_out;
use crate::conv::fold_plan::FoldPlan;
use crate::mock::Session;
use crate::{State, Tx};
use rune::KeyEvent;
use std::time::Duration;

/// `grid-template-rows 280ms ease-out`: how long the card takes to fold away or come back.
pub const FOLD_SECS: f64 = 0.28;

/// The panel's own bits of `State`.
#[derive(Default)]
pub struct SideView {
    /// The conversation of the side session the panel draws. Only a scene or the app with no server has one (invented); the
    /// live stream is piece C's.
    pub conv: Option<Session>,
    pub plan: FoldPlan,
    /// Which side session `conv` was built from, when it came from a live stream (a scene's has none).
    pub conv_of: Option<String>,
    /// The live side session's stream and what the link listens to.
    pub live: super::side_stream::SideStreams,
    /// Whether the panel was folded as of the last frame, and when that last changed: the fold runs towards it from then.
    folded: Option<bool>,
    changed_at: Option<f64>,
}

impl State {
    /// The conversation and fold plan the panel's stream draws; blank until there is one.
    pub fn side_conv(&self) -> (&Session, &FoldPlan) {
        let own = self.side_ui.conv_of.as_ref().is_none_or(|of| self.side_session_id().as_ref() == Some(of));
        (self.side_ui.conv.as_ref().filter(|_| own).unwrap_or(&self.blank.sessions[0]), &self.side_ui.plan)
    }

    /// The panel's state when it should be drawn at all: a side conversation, or the first question making one (`shown`).
    pub fn side_shown(&self) -> Option<&SideState> {
        self.side().filter(|st| st.side.is_some() || st.starting.is_some())
    }

    /// How much of the card shows, 0 (folded) to 1 (open), through the fold (`grid-template-rows` 1fr to 0fr, 280 ms, ease-out).
    /// A panel found in either state shows it at once.
    pub fn side_open_share(&self) -> f32 {
        let folded = self.side().is_some_and(|st| st.minimized());
        let (Some(to), Some(at)) = (self.side_ui.folded, self.side_ui.changed_at) else { return if folded { 0.0 } else { 1.0 } };
        let p = ease_out(((self.clock - at) / FOLD_SECS) as f32);
        if to { 1.0 - p } else { p }.clamp(0.0, 1.0)
    }

    /// Keeps the fold running (each frame): notes when it was folded or opened, and asks for frames until it has settled.
    pub fn keep_side_fold(&mut self, tx: &mut Tx) -> bool {
        let folded = self.side().is_some_and(|st| st.minimized());
        match self.side_ui.folded {
            None => self.side_ui.folded = Some(folded),
            Some(was) if was != folded => {
                self.side_ui.folded = Some(folded);
                self.side_ui.changed_at = Some(self.clock);
            }
            _ => {}
        }
        let running = self.side_ui.changed_at.is_some_and(|at| self.clock - at < FOLD_SECS);
        if running {
            tx.wake_after(Duration::from_millis(16));
        }
        running
    }

    /// Minimize, or open: the composer talks to the session or to the side conversation again, and opening puts the caret back
    /// in it (`watch(minimized)`).
    pub fn side_set_minimized(&mut self, value: bool, tx: &mut Tx) {
        let Some(owner) = self.draft_owner() else { return };
        self.set_side_minimized(&owner, value, tx);
        if !value {
            tx.focus("composer", false);
        }
    }

    /// Ctrl+/ (⌘/ on a Mac): folds it or opens it, anywhere in the window but a code editor, where it comments a line. Consumed
    /// when there is a side conversation (even a folded one), left alone otherwise (`handleKeydown`).
    pub fn side_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !ev.is_command('/') || ev.mods.alt || ev.mods.shift {
            return false;
        }
        if !self.side().is_some_and(|st| st.side.is_some()) {
            return false;
        }
        if self.code_edit.focused || (self.code_edit.on && tx.focused() == Some(self.edit_surface())) {
            return false;
        }
        let to = !self.side().is_some_and(|st| st.minimized());
        self.side_set_minimized(to, tx);
        true
    }

    /// × and the tab's ×: discards it at once, with Undo.
    pub fn side_discard(&mut self) {
        if let Some(owner) = self.draft_owner() {
            self.discard_side(&owner);
        }
    }

    /// Keep as session.
    pub fn side_keep(&mut self, tx: &mut Tx) {
        if let Some(owner) = self.draft_owner() {
            self.keep_side_as_session(&owner, tx);
        }
    }
}

/// The states the scenes draw the panel in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// The fork is made and answered; the card is open.
    Open,
    /// The first question is making the fork: "Forking the session at its last finished turn…".
    Starting,
}

impl State {
    /// A scene of the panel on the open session, from invented data (no server is asked).
    pub fn show_side(&mut self, case: Case, dark: bool) {
        use super::side::{Side, SideState};
        self.dark = Some(dark);
        self.set_open((0, 2));
        let Some(owner) = self.draft_owner() else { return };
        let mut st = SideState { loaded: true, ..Default::default() };
        match case {
            Case::Open => {
                st.side = Some(Side {
                    session_id: "mock.side.0".into(),
                    instance_id: "mock-instance".into(),
                    title: format!("btw: {}", crate::mock::side::QUESTION),
                    boundary_message_id: None,
                    created_at: String::new(),
                    minimized: false,
                    seen_answer_id: None,
                });
                self.side_ui.conv = Some(crate::mock::side::conversation());
            }
            Case::Starting => {
                st.asking = true;
                st.starting = Some(crate::mock::side::QUESTION.into());
            }
        }
        self.sides.insert(owner, st);
    }
}
