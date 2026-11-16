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
    /// The rows' own state (piece O).
    pub rows: super::harness_setup_rows::Rows,
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
        self.close_setup_terminal();
        self.harness_setup.close();
        if !self.harness_setup_done() {
            let before = self.pickers.prefs.get(HARNESS_SETUP_DONE_KEY).map(str::to_string);
            self.put_preference(HARNESS_SETUP_DONE_KEY, "true", before);
        }
    }

    /// `toReady()`: a harness set up here is wanted on, so turn on every working one that was off, check them all again, and
    /// show the last step. Doesn't write the done flag; leaving does.
    pub fn harness_setup_continue(&mut self) {
        self.close_setup_terminal();
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
    /// One row in each state: ready, sign-in needed, update needed, not working.
    RowsStates,
    /// The install terminal open under OpenCode 2's row, the shell just asked for.
    RowsStarting,
    /// The terminal open and the harness found: the banner.
    RowsFound,
    /// The terminal couldn't open: the command to run elsewhere.
    RowsFailure,
    /// GitHub Copilot, which Fleet downloads itself, under OpenCode.
    RowsCopilot,
}

fn demo(kind: &str, name: &str, state: &str, icon: &str) -> HarnessEntry {
    use super::harness_setup_rows::Setup;
    let ready = state == "ready";
    HarnessEntry {
        kind: kind.into(),
        display_name: name.into(),
        user_enabled: true,
        available: ready,
        state: state.into(),
        reason: (!ready).then(|| format!("{name} isn't installed.")),
        version: ready.then(|| "1.18.30".into()),
        executable_path: ready.then(|| format!("/usr/local/bin/{kind}")),
        setup: Setup {
            icon: Some(icon.into()),
            install_command: Some(format!("curl -fsSL https://example.com/{kind}/install | bash")),
            ..Setup::default()
        },
        ..HarnessEntry::default()
    }
}

/// OpenCode 2 the way a Fleet with OpenCode 1 on it offers it: two places, the first recommended, and two notes.
fn opencode2() -> HarnessEntry {
    use super::harness_setup_rows::{Choice, Setup};
    let separate = "curl -fsSL https://opencode.ai/v2/install | HOME=$HOME/.weave/harnesses/opencode2 bash -s -- --no-modify-path";
    let mut h = demo("opencode2", "OpenCode 2", "not-installed", "terminal");
    h.is_default = true;
    h.reason = Some("OpenCode 2 isn't installed: Fleet couldn't find it on this machine.".into());
    h.setup = Setup {
        icon: Some("terminal".into()),
        install_command: Some(separate.into()),
        sign_in_command: Some("opencode2 auth login --standalone".into()),
        install_choices: vec![
            Choice {
                id: "separate".into(),
                label: "In its own folder".into(),
                description: "Fleet runs it from a folder of its own, with its own settings and sessions. OpenCode 1 can be installed next to it at any time.".into(),
                command: separate.into(),
                recommended: true,
            },
            Choice {
                id: "default".into(),
                label: "As your main opencode".into(),
                description: "OpenCode 2's usual install, added to your PATH. Installing OpenCode 1 later replaces OpenCode 2.".into(),
                command: "curl -fsSL https://opencode.ai/v2/install | bash".into(),
                recommended: false,
            },
        ],
        notes: vec!["OpenCode 1 is installed here, so OpenCode 2 goes into a folder of its own.".into(), "OpenCode 2 keeps its own provider sign-ins here.".into()],
        ..Setup::default()
    };
    h
}

impl State {
    pub(crate) fn show_harness_setup_scene(&mut self, scene: Scene) {
        let mut claude = demo("claude-code", "Claude Code", "not-installed", "hexagon");
        claude.setup.sign_in_command = Some("claude auth login".into());
        let mut all = vec![opencode2(), claude];
        match scene {
            Scene::HarnessesReady | Scene::Ready => {
                all[0].state = "ready".into();
                all[0].available = true;
                all[0].reason = None;
                all[0].version = Some("1.18.30".into());
            }
            Scene::RowsStates => {
                let mut ready = demo("opencode", "OpenCode", "ready", "terminal");
                ready.is_default = true;
                let mut claude = demo("claude-code", "Claude Code", "sign-in-required", "hexagon");
                claude.setup.sign_in_command = Some("claude auth login".into());
                claude.version = Some("2.1.276".into());
                let mut old = demo("codex", "Codex", "update-needed", "plug");
                old.reason = Some("Fleet needs Codex 0.30.0 or newer. You have 0.21.4.".into());
                let mut broken = demo("gemini", "Gemini CLI", "not-working", "infinity");
                broken.reason = Some("Gemini CLI is installed but didn't answer `gemini --version`.".into());
                all = vec![ready, claude, old, broken];
            }
            Scene::RowsStarting | Scene::RowsFound | Scene::RowsFailure => {
                if scene == Scene::RowsFound {
                    all = vec![demo("opencode", "OpenCode", "ready", "terminal")];
                    all[0].is_default = true;
                } else {
                    all.truncate(1);
                }
                let rows = &mut self.harness_setup.rows;
                let kind = all[0].kind.clone();
                let command = super::harness_setup_rows::Rows::default().command_for(&all[0], super::harness_setup_rows::Action::Install);
                let command = command.unwrap_or_default();
                if scene == Scene::RowsFailure {
                    rows.active = None;
                    rows.failure = Some(super::harness_setup_rows::Failure {
                        kind,
                        message: "Terminals are turned off in this Fleet.".into(),
                        command,
                    });
                } else {
                    let terminal_id = (scene == Scene::RowsFound).then(|| "setup-1".to_string());
                    rows.active = Some(super::harness_setup_rows::Active {
                        kind,
                        action: super::harness_setup_rows::Action::Install,
                        command,
                        terminal_id,
                    });
                }
            }
            Scene::RowsCopilot => {
                let mut opencode = demo("opencode", "OpenCode", "not-installed", "terminal");
                opencode.is_default = true;
                opencode.setup.pitch = Some("Open source. Includes free models, or sign in to your own provider.".into());
                let mut copilot = demo("copilot", "GitHub Copilot", "not-installed", "hexagon");
                copilot.setup.install_command = None;
                copilot.setup.pitch = Some("GitHub's coding agent. Needs a GitHub Copilot subscription.".into());
                copilot.setup.install_plan = Some("Fleet downloads weave-copilot 0.2.2 and the Copilot CLI 1.0.105 from GitHub.".into());
                all = vec![opencode, copilot];
            }
            _ => {}
        }
        if let Some(feed) = self.feeds.harnesses.get_mut(HOME_MACHINE_KEY) {
            feed.harnesses.all = all;
        }
        self.harness_setup.open(match scene {
            Scene::Welcome => Step::Welcome,
            Scene::Ready => Step::Ready,
            _ => Step::Harnesses,
        });
    }
}
