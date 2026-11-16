//! The harness setup wizard's rules (`stores/harness-setup.ts`, `HarnessSetupWizard.vue`; rows C12, C17.9-C17.16): when it
//! opens by itself, which step shows, what leaving and continuing write. The dialog is `view::harness_setup`; the rows of
//! the harnesses step are piece O's. No Rune types except `State`'s own helpers.
use super::new_session::harness_choice::{HarnessEntry, Harnesses};
use super::settings::HOME_MACHINE_KEY;
use crate::State;

/// The server preference that says setup was finished or skipped (`HARNESS_SETUP_DONE_PREFERENCE`), per machine.
pub const HARNESS_SETUP_DONE_KEY: &str = "harnessSetup.done";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Welcome,
    Harnesses,
    Ready,
}

/// `useHarnessSetupStore` plus the wizard's `offered` flag.
#[derive(Debug, Default)]
pub struct Setup {
    pub is_open: bool,
    pub step: Step,
    /// The auto-open rule already ran once on this connection (C17.12): closing the wizard never brings it back by itself.
    offered: bool,
    /// Fleet's cloud mode never offers it (C17.11). The native app talks to a machine's own Fleet, so this stays false; it is
    /// kept so the rule reads as Fleet's does.
    pub cloud: bool,
}

impl Setup {
    /// `open(at = "harnesses")`: sets the step and shows the dialog, whatever the done flag says.
    pub fn open(&mut self, at: Step) {
        self.step = at;
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// The watcher (C17.12): when the preferences are known and no harness can start a session, the wizard is offered once per
    /// connection, and opens at "welcome" unless setup was finished or skipped before.
    pub fn offer(&mut self, prefs_fetched: bool, no_harness: bool, done: bool) {
        if self.cloud || self.offered || !prefs_fetched || !no_harness {
            return;
        }
        self.offered = true;
        if !done {
            self.open(Step::Welcome);
        }
    }

    /// A new connection starts the rule again (Fleet's wizard is mounted once per page load).
    pub fn reset_offer(&mut self) {
        self.offered = false;
    }
}

/// `readyHarnesses`: every harness that works, turned on or not.
pub fn ready_harnesses(h: &Harnesses) -> Vec<&HarnessEntry> {
    h.all.iter().filter(|e| e.is_ready()).collect()
}

/// "OpenCode 1.18 is ready." / "OpenCode 1.18 and Claude Code are ready." (C12.32).
pub fn ready_line(h: &Harnesses) -> String {
    let ready = ready_harnesses(h);
    let names: Vec<String> = ready
        .iter()
        .map(|e| match e.version.as_deref().filter(|v| !v.is_empty()) {
            Some(v) => format!("{} {v}", e.display_name),
            None => e.display_name.clone(),
        })
        .collect();
    format!(
        "{} {} ready. Add or update harnesses any time in Settings → Harnesses.",
        names.join(" and "),
        if ready.len() > 1 { "are" } else { "is" }
    )
}

impl State {
    /// The live machine's harnesses as the wizard reads them (`useEnabledHarnesses` with the live target).
    pub fn setup_harnesses(&self) -> Harnesses {
        self.harness_choice(HOME_MACHINE_KEY)
    }

    pub fn harness_setup_done(&self) -> bool {
        self.pickers.prefs.get(HARNESS_SETUP_DONE_KEY) == Some("true")
    }

    /// The watch of `HarnessSetupWizard.vue`: run after the harness list or the preferences change.
    pub fn harness_setup_check(&mut self) {
        if self.is_mock() {
            return;
        }
        let no_harness = self.setup_harnesses().no_harness_reason().is_some();
        let (fetched, done) = (self.pickers.prefs.fetched, self.harness_setup_done());
        self.harness_setup.offer(fetched, no_harness, done);
    }

    /// The banner, Settings and the new-session box's fix open it at the harness step on this machine (C17.15).
    pub fn open_harness_setup(&mut self) {
        self.harness_setup.open(Step::Harnesses);
    }

    /// `leave()`: closes, and remembers that setup was dealt with so it doesn't open again by itself.
    pub fn leave_harness_setup(&mut self) {
        self.harness_setup.close();
        if !self.harness_setup_done() {
            let before = self.pickers.prefs.get(HARNESS_SETUP_DONE_KEY).map(str::to_string);
            self.put_preference(HARNESS_SETUP_DONE_KEY, "true", before);
        }
    }

    /// `toReady()`: a harness set up here is wanted on, so turn on every working one that was off, check them all again, and
    /// show the last step. Doesn't write the done flag; leaving does.
    pub fn harness_setup_continue(&mut self) {
        let harnesses = self.setup_harnesses();
        if ready_harnesses(&harnesses).is_empty() {
            return;
        }
        for h in ready_harnesses(&harnesses).into_iter().filter(|h| !h.user_enabled) {
            let key = format!("{}.enabled", h.kind);
            let before = self.pickers.prefs.get(&key).map(str::to_string);
            self.put_preference(&key, "true", before);
        }
        self.read_harnesses(HOME_MACHINE_KEY, true);
        self.harness_setup.step = Step::Ready;
    }

    /// `startSession()`: leave, then the New session page with nothing chosen.
    pub fn harness_setup_start_session(&mut self) {
        self.leave_harness_setup();
        self.open_new_session_page(None);
    }
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// A state of the wizard for the pictures; the harnesses are invented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Welcome,
    /// Two harnesses, neither installed: Continue is off and the left button says "Skip for now".
    Harnesses,
    /// One installed: Continue is on and the left button says "Close".
    HarnessesReady,
    Ready,
}

impl State {
    pub(crate) fn show_harness_setup_scene(&mut self, scene: Scene) {
        let mut opencode = HarnessEntry { display_name: "OpenCode 2".into(), state: "not-installed".into(), ..HarnessEntry::default() };
        opencode.kind = "opencode2".into();
        opencode.user_enabled = true;
        opencode.is_default = true;
        opencode.reason = Some("OpenCode 2 isn't installed: Fleet couldn't find it on this machine.".into());
        let mut claude = HarnessEntry { display_name: "Claude Code".into(), state: "not-installed".into(), ..opencode.clone() };
        claude.kind = "claude-code".into();
        claude.is_default = false;
        claude.reason = Some("Claude Code isn't installed.".into());
        if matches!(scene, Scene::HarnessesReady | Scene::Ready) {
            opencode.state = "ready".into();
            opencode.available = true;
            opencode.reason = None;
            opencode.version = Some("1.18.30".into());
        }
        if let Some(feed) = self.feeds.harnesses.get_mut(HOME_MACHINE_KEY) {
            feed.harnesses.all = vec![opencode, claude];
        }
        self.harness_setup.open(match scene {
            Scene::Welcome => Step::Welcome,
            Scene::Harnesses | Scene::HarnessesReady => Step::Harnesses,
            Scene::Ready => Step::Ready,
        });
    }
}
