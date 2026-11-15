//! What the composer knows about the session: busy or not, what its harness can do, whether it takes
//! input at all (`use-composer-actions.ts`, `use-send-prompt.ts` `canSend`).

/// The composer's view of the session: busy while a turn runs (or right after sending), waiting when it asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerStatus {
    Idle,
    Busy,
    WaitingInput,
}

/// `composerStatus`: an optimistic send counts as busy; `busy`, `delegating` and `retry` are busy.
pub fn composer_status(activity: Option<&str>, optimistic_busy: bool) -> ComposerStatus {
    if optimistic_busy {
        return ComposerStatus::Busy;
    }
    match activity {
        Some("busy" | "delegating" | "retry") => ComposerStatus::Busy,
        Some("waiting_input") => ComposerStatus::WaitingInput,
        _ => ComposerStatus::Idle,
    }
}

/// `canInterruptSession`: while a turn runs, retries or delegates, and while the session waits on a question or
/// permission; not when idle or once an abort is on its way.
pub fn can_interrupt(status: ComposerStatus, aborting: bool) -> bool {
    status != ComposerStatus::Idle && !aborting
}

/// When the agent reads a steered message (`steerDelivery`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteerDelivery {
    NextStep,
    Immediate,
}

/// What a harness says about itself in `GET /api/harnesses` (`capabilities`); a missing flag is false.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessInfo {
    pub kind: String,
    pub supports_steering: bool,
    pub steers_by_default: bool,
    pub steer_delivery: Option<SteerDelivery>,
    pub supports_shell_commands: bool,
    pub supports_side_conversations: bool,
    pub supports_image_attachments: bool,
}

/// What the composer may offer for the session's harness (`HarnessCapabilities`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Caps {
    /// It takes a message into a running turn, read at the agent's next step.
    pub can_steer: bool,
    /// Enter steers and Queue waits; other harnesses that steer keep Enter for the queue.
    pub steers_by_default: bool,
    pub steer_delivery: Option<SteerDelivery>,
    pub supports_images: bool,
    pub supports_shell: bool,
    pub supports_side: bool,
}

/// `harnessCapabilities`: no session, or none found, turns everything harness-specific off.
pub fn harness_capabilities(harness_type: Option<&str>, harnesses: &[HarnessInfo]) -> Caps {
    let Some(h) = harness_type.and_then(|t| harnesses.iter().find(|h| h.kind == t)) else {
        return Caps::default();
    };
    let can_steer = h.supports_steering;
    Caps {
        can_steer,
        steers_by_default: can_steer && h.steers_by_default,
        steer_delivery: if can_steer { h.steer_delivery } else { None },
        supports_shell: h.supports_shell_commands,
        supports_side: h.supports_side_conversations,
        supports_images: h.supports_image_attachments,
    }
}

/// What `canSend` and `isDisabled` read from the session row (rows 1.9 and 14.9).
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionGate<'a> {
    /// `retentionStatus == "archived"`.
    pub archived: bool,
    /// `capabilities.canPrompt`, when the row has capabilities.
    pub can_prompt: Option<bool>,
    pub lifecycle: &'a str,
}

/// `canSend` with the archived rule of `isDisabled`: an archived session never takes a prompt; the row's own
/// `canPrompt` decides when it has one; without it the session must be `running`. No session at all: true.
pub fn can_send(session: Option<SessionGate>) -> bool {
    let Some(s) = session else { return true };
    if s.archived {
        return false;
    }
    s.can_prompt.unwrap_or(s.lifecycle == "running")
}

/// `hasContent` (Send's enabled look, row 15.1), for the parts piece A knows: the draft's text, and the
/// command after the `!` in shell mode. Attachments and terminal lines count once they exist (pieces N, O).
/// A `!` draft is held while the harness list is still loading (`shell_pending`).
pub fn has_content(text: &str, shell_mode: bool, shell_pending: bool, extras: usize) -> bool {
    if shell_mode {
        return !super::route::shell_draft_command(text).is_empty();
    }
    !shell_pending && (!text.trim().is_empty() || extras > 0)
}

/// `optimisticBusy` (row 2.11): after a send the composer reads busy until the session says it is, so the
/// buttons don't flicker back to idle in the gap before the server's first `busy`. It ends when the session
/// has been busy and is not any more, or after `GIVE_UP` seconds (a prompt the server never turned into a turn).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Optimistic {
    pub since: f64,
    pub seen: bool,
}

impl Optimistic {
    pub const GIVE_UP: f64 = 15.0;

    pub fn new(now: f64) -> Optimistic {
        Optimistic { since: now, seen: false }
    }

    /// One look at the session: whether the guess still holds.
    pub fn keep(&mut self, busy: bool, now: f64) -> bool {
        self.seen |= busy;
        !(self.seen && !busy) && now - self.since < Self::GIVE_UP
    }
}

/// What the draft is: a message for the agent, a shell command (`!`), or a question for the side conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Prompt,
    Shell,
    Side,
}

/// `isShellMode` and `isSideMode`: the side conversation being open wins; a `!` draft is a command only where
/// the harness runs commands.
pub fn mode_of(text: &str, caps: &Caps, side_open: bool) -> Mode {
    if side_open {
        Mode::Side
    } else if caps.supports_shell && super::route::is_shell_draft(text) {
        Mode::Shell
    } else {
        Mode::Prompt
    }
}

/// Send's accessible label and its title while a turn runs (row 2.7): a prompt says what Enter does now. Shell and side
/// mode keep their own words; idle is "Send".
pub fn send_words(mode: Mode, busy: bool, steers_by_default: bool) -> (&'static str, &'static str) {
    match (mode, busy, steers_by_default) {
        (Mode::Prompt, true, true) => ("Send now", "Send now (Enter): the agent reads it at its next step"),
        (Mode::Prompt, true, false) => ("Queue", "Queue (Enter): sent when the agent finishes"),
        _ => {
            let label = send_label(mode);
            (label, label)
        }
    }
}

/// Send's label and title when idle (row 2.7): "Send", "Run command" in shell mode and "Ask in the side
/// conversation" in side mode.
pub fn send_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Prompt => "Send",
        Mode::Shell => "Run command",
        Mode::Side => "Ask in the side conversation",
    }
}
