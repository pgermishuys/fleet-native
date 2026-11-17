//! The Harnesses page's invented data (component 16, piece G): a Fleet that lists OpenCode 2, Claude Code, GitHub Copilot and
//! Pi, worded as the server words them, so the page draws the same every time. OpenCode 1 isn't listed: Fleet drops it.
use crate::State;
use crate::fleet::new_session::feeds::Profile;
use crate::fleet::new_session::harness_choice::{Card, HarnessEntry, HarnessSetting};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::settings::harness_profiles::Editing;

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

/// What the Profiles group shows in a scene.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Profiles {
    /// The machine has said there are none.
    None,
    /// Asked, not answered.
    Loading,
    /// The machine couldn't be read.
    Failed,
    /// Three: the default, one two sessions use, one that doesn't read.
    Three,
    /// Three, and a new profile's editor open under them.
    New,
    /// Three, and the first one's editor open.
    Edit,
    /// Three, after "Set default" on the second.
    Chosen,
}

fn profile(id: &str, name: &str, content: &str, default: bool, open: u32) -> Profile {
    Profile { id: id.into(), name: name.into(), content: content.into(), is_default: default, open_sessions: open }
}

/// Three invented profiles: one is the default, one is used by two open sessions, one doesn't read.
pub fn profiles() -> Vec<Profile> {
    vec![
        profile(
            "p-work",
            "Work",
            "{\n  // Models through the company gateway\n  \"model\": \"gateway/large\",\n  \"provider\": { \"gateway\": {} },\n  \"mcp\": { \"docs\": {}, \"tickets\": {} },\n}",
            true,
            0,
        ),
        profile(
            "p-local",
            "Local only",
            "{ \"enabled_providers\": [\"ollama\"], \"default_agent\": \"review\", \"agent\": { \"review\": {} } }",
            false,
            2,
        ),
        profile("p-draft", "Draft", "{ \"model\": ", false, 0),
    ]
}

impl State {
    /// Settings → Harnesses on OpenCode 2's tab, scrolled to Profiles, with `case` in it.
    pub fn show_profiles(&mut self, dark: bool, case: Profiles) {
        self.show_harnesses(dark, Case::Mixed, Some("opencode2"));
        let key = (HOME_MACHINE_KEY.to_string(), "opencode2".to_string());
        match case {
            Profiles::None => drop(self.feeds.profiles.insert(key, Vec::new())),
            Profiles::Loading => {
                self.feeds.profiles.remove(&key);
            }
            Profiles::Failed => {
                self.feeds.profiles.remove(&key);
                self.settings.harnesses.profiles.mark_failed(key);
            }
            Profiles::Three | Profiles::New | Profiles::Edit | Profiles::Chosen => {
                let mut list = profiles();
                if case == Profiles::Chosen {
                    list = crate::fleet::settings::harness_profiles::with_default(&list, Some("p-local"));
                }
                self.feeds.profiles.insert(key, list);
            }
        }
        match case {
            Profiles::New => self.open_profile_editor("opencode2", Editing::New(None)),
            Profiles::Edit => self.open_profile_editor("opencode2", Editing::Edit("p-work".into())),
            Profiles::Chosen => self.settings.harnesses.profiles.notice = Some("New sessions start with Local only.".into()),
            _ => {}
        }
        self.open_harness("opencode2", Some("profiles"));
    }
}
