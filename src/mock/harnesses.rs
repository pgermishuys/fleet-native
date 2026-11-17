//! The Harnesses page's invented data (component 16, piece G): a Fleet that lists OpenCode 2, Claude Code, GitHub Copilot and
//! Pi, worded as the server words them, so the page draws the same every time. OpenCode 1 isn't listed: Fleet drops it.
use crate::State;
use crate::fleet::new_session::harness_choice::{Card, HarnessEntry, HarnessSetting};
use crate::fleet::settings::HOME_MACHINE_KEY;

/// How the harnesses are doing in a scene.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// OpenCode 2 and Claude Code ready, Copilot needs a sign-in, Pi isn't installed; OpenCode 2 is the default.
    Mixed,
    /// The same, with Claude Code turned off.
    ClaudeOff,
}

/// The checked time of the scenes: two minutes before the scenes' clock (`mock::rows::NOW`).
const CHECKED_MINUTES_AGO: i64 = 2;

fn card(eyebrow: &str, description: &str, levels: bool) -> Card {
    Card {
        eyebrow: Some(eyebrow.into()),
        description: Some(description.into()),
        supports_permission_levels: levels,
        supports_agent_browser: levels,
        supports_provider_sign_in: levels,
        ..Card::default()
    }
}

fn entry(kind: &str, name: &str, state: &str, card: Card) -> HarnessEntry {
    let ready = state == "ready";
    HarnessEntry {
        kind: kind.into(),
        display_name: name.into(),
        available: ready,
        user_enabled: true,
        reason: (!ready).then(|| format!("{name} isn't installed: Fleet couldn't find it on this machine.")),
        state: state.into(),
        card,
        ..HarnessEntry::default()
    }
}

/// The list as `GET /api/harnesses` answers it.
pub fn harnesses(checked_at: &str) -> Vec<HarnessEntry> {
    let cli = "CLI harness";
    let mut opencode =
        entry("opencode2", "OpenCode 2", "ready", card(cli, "Harness for sessions backed by OpenCode 2's server, on its own.", true));
    opencode.version = Some("2.0.6".into());
    opencode.executable_path = Some("/usr/local/bin/opencode2".into());
    opencode.supports_profiles = true;
    let mut claude = entry(
        "claude-code",
        "Claude Code",
        "ready",
        card(cli, "Harness for Anthropic Claude Code sessions and project-aware coding workflows.", true),
    );
    claude.version = Some("2.1.276".into());
    claude.executable_path = Some("/usr/local/bin/claude".into());
    let mut copilot_card = card(cli, "GitHub Copilot through weave-copilot, with HydraFusion's passes when you turn them on.", true);
    copilot_card.cli_version = Some("1.0.4".into());
    copilot_card.settings = vec![HarnessSetting {
        key: "copilot.hydrafusion".into(),
        label: "HydraFusion (research preview)".into(),
        description:
            "Let Copilot choose a model or combine work from multiple models for your task. Applies to sessions started afterwards.".into(),
        default: false,
    }];
    let mut copilot = entry("copilot", "GitHub Copilot", "sign-in-required", copilot_card);
    copilot.available = true;
    copilot.reason = None;
    copilot.version = Some("0.9.2".into());
    copilot.executable_path = Some("/usr/local/bin/weave-copilot".into());
    let pi =
        entry("pi", "Pi", "not-installed", card(cli, "Harness for sessions backed by the Pi command-line runtime from pi.dev.", false));
    let mut all = vec![opencode, claude, copilot, pi];
    all[0].is_default = true;
    all[0].card.checked_at = Some(checked_at.into());
    all
}

impl State {
    /// Settings → Harnesses on the invented list, no server asked.
    pub fn show_harnesses(&mut self, dark: bool, case: Case, tab: Option<&str>) {
        self.dark = Some(dark);
        self.now_ms = Some(crate::mock::rows::NOW);
        let checked = crate::mock::rows::NOW - CHECKED_MINUTES_AGO * 60_000;
        let checked_at = crate::fleet::session_commands::iso_utc(checked);
        let feed = self.feeds.harnesses.entry(HOME_MACHINE_KEY.to_string()).or_default();
        feed.harnesses.all = harnesses(&checked_at);
        feed.harnesses.load = crate::fleet::new_session::harness_choice::Load::Done;
        self.pickers.prefs.fetched = true;
        if case == Case::ClaudeOff {
            self.pickers.prefs.set("claude-code.enabled", "false");
        }
        self.settings.harness = tab.map(str::to_string);
        self.open_settings("harnesses", None);
    }
}
