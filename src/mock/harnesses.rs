//! The Harnesses page's invented data (component 16, piece G): a Fleet that lists OpenCode 2, Claude Code, GitHub Copilot and
//! Pi, worded as the server words them, so the page draws the same every time. OpenCode 1 isn't listed: Fleet drops it.
use crate::State;
use crate::fleet::new_session::feeds::Profile;
use crate::fleet::new_session::harness_choice::{Card, HarnessEntry, HarnessSetting};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::settings::harness_profiles::{Check, Editing};

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
    opencode.card.profile_note = Some(
        "Fleet hands this to OpenCode 2 as `OPENCODE_CONFIG`, on top of your own opencode.json. Fleet's own settings (every tool allowed, the Fleet plugin) still apply after it. Keep API keys out of it: sign in to providers in OpenCode 2 itself, or refer to a variable Fleet runs with as `{env:NAME}`. Sessions on a profile run on an OpenCode 2 server of their own, which stops after a few minutes unused."
            .into(),
    );
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
    /// The editor of the one that doesn't read: the note about its text.
    EditBroken,
    /// The first one's editor after Test passed.
    TestOk,
    /// The first one's editor after Test failed, with the harness's lines.
    TestFailed,
    /// The first one's editor, Delete pressed once.
    DeleteAsk,
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
            "{\n  // Models through the company gateway\n  \"model\": \"gateway/large\",\n  \"mcp\": { \"docs\": { \"type\": \"remote\", \"url\": \"https://docs.example.test/mcp\" } },\n}",
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
            Profiles::Three
            | Profiles::New
            | Profiles::Edit
            | Profiles::Chosen
            | Profiles::EditBroken
            | Profiles::TestOk
            | Profiles::TestFailed
            | Profiles::DeleteAsk => {
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
            Profiles::EditBroken => self.open_profile_editor("opencode2", Editing::Edit("p-draft".into())),
            Profiles::TestOk | Profiles::TestFailed | Profiles::DeleteAsk => {
                self.open_profile_editor("opencode2", Editing::Edit("p-work".into()));
                if let Some(d) = self.settings.harnesses.profiles.draft.as_mut() {
                    d.confirming_delete = case == Profiles::DeleteAsk;
                    d.result = match case {
                        Profiles::TestOk => Some(Check { ok: true, ..Check::default() }),
                        Profiles::TestFailed => Some(Check {
                            ok: false,
                            error: Some("OpenCode 2 couldn't load this profile: unknown key \"provider.gateway.url\".".into()),
                            details: Some("line 4, column 22: expected a string\nline 5: \"mcp\" has no entry named \"docs\"".into()),
                        }),
                        _ => None,
                    };
                }
            }
            _ => {}
        }
        self.open_harness("opencode2", Some("profiles"));
    }
}

/// What the Install panel and the Copilot path field show in a scene (component 16, piece J).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// OpenCode 2 installed, with its folders, a note and the sign-in command.
    Installed,
    /// Not installed, with two places to put it.
    TwoPlaces,
    /// Not installed, one installer.
    OneInstaller,
    /// What is there stopped working: the installer again.
    Broken,
    /// Copilot: Fleet found weave-copilot.
    PathFound,
    /// Copilot: Fleet couldn't find it and says why.
    PathReason,
    /// Copilot: a path was typed and Fleet refused to save it.
    PathError,
}

fn folder(label: &str, path: &str) -> crate::fleet::harness_setup_rows::Folder {
    crate::fleet::harness_setup_rows::Folder { label: label.into(), path: path.into() }
}

fn choice(
    id: &str,
    label: &str,
    description: &str,
    command: &str,
    recommended: bool,
    program: &str,
) -> crate::fleet::harness_setup_rows::Choice {
    crate::fleet::harness_setup_rows::Choice {
        id: id.into(),
        label: label.into(),
        description: description.into(),
        command: command.into(),
        recommended,
        folders: vec![folder("Program", program)],
    }
}

const SEPARATE: &str = "curl -fsSL https://opencode.ai/v2/install | HOME=/home/you/.weave/harnesses/opencode2 bash -s -- --no-modify-path";

impl State {
    /// Settings → Harnesses on the tab the case is about.
    pub fn show_install(&mut self, dark: bool, case: Install) {
        let copilot = matches!(case, Install::PathFound | Install::PathReason | Install::PathError);
        self.show_harnesses(dark, Case::Mixed, Some(if copilot { "copilot" } else { "opencode2" }));
        let feed = self.feeds.harnesses.entry(HOME_MACHINE_KEY.to_string()).or_default();
        let h = &mut feed.harnesses.all[if copilot { 2 } else { 0 }];
        if copilot {
            h.state = "ready".into();
            if case == Install::PathReason {
                h.state = "not-installed".into();
                h.available = false;
                h.version = None;
                h.executable_path = None;
                h.reason = Some("GitHub Copilot isn't installed: Fleet couldn't find weave-copilot on PATH.".into());
            }
            if case == Install::PathError {
                self.settings.harnesses.path.draft = rune::TextEdit::new("/opt/nowhere/weave-copilot");
                self.settings.harnesses.path.error = Some("Fleet couldn't save the path to weave-copilot.".into());
            }
            return;
        }
        h.setup.mode = Some("In its own folder".into());
        h.setup.sign_in_command = Some("OPENCODE_CONFIG_DIR=/c opencode2 auth login".into());
        h.setup.notes = vec!["OpenCode 2 keeps its own provider sign-ins here.".into()];
        h.setup.folders = vec![
            folder("Program", "/home/you/.weave/harnesses/opencode2/.opencode/bin"),
            folder("Settings", "/home/you/.weave/harnesses/opencode2/config"),
            folder("Sessions", "/home/you/.weave/harnesses/opencode2/data/opencode.db"),
        ];
        h.setup.install_command = Some(SEPARATE.into());
        match case {
            Install::TwoPlaces | Install::OneInstaller | Install::Broken => {
                h.state = if case == Install::Broken { "not-working" } else { "not-installed" }.into();
                h.available = false;
                if case != Install::Broken {
                    h.version = None;
                    h.executable_path = None;
                }
                h.reason = Some("OpenCode 2 isn't installed: Fleet couldn't find it on this machine.".into());
                if case == Install::TwoPlaces {
                    h.setup.install_choices = vec![
                        choice(
                            "separate",
                            "In its own folder",
                            "OpenCode 1 can be installed next to it at any time.",
                            SEPARATE,
                            true,
                            "/home/you/.weave/harnesses/opencode2/.opencode/bin",
                        ),
                        choice(
                            "default",
                            "As your main opencode",
                            "Installing OpenCode 1 later replaces OpenCode 2.",
                            "curl -fsSL https://opencode.ai/v2/install | bash",
                            false,
                            "/home/you/.opencode/bin",
                        ),
                    ];
                }
            }
            _ => {}
        }
        h.card.describes_install = true;
    }
}

/// What an update strip shows in a scene.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Strip {
    Offer,
    TooOld,
    Waiting,
    Running,
    Succeeded,
    Failed,
    Install,
    Downloading,
    Refused,
}

impl State {
    /// The setup wizard's rows with GitHub Copilot (the one Fleet installs itself) in each state of its update.
    pub fn show_strip(&mut self, dark: bool, case: Strip) {
        use crate::fleet::harness_setup::Scene;
        use crate::fleet::settings::harness_update::{Job, Update};
        self.dark = Some(dark);
        self.show_harness_setup_scene(Scene::RowsCopilot);
        let plan = "Fleet downloads weave-copilot 0.2.2 and the Copilot CLI 1.0.105 from GitHub into ~/.weave/runtimes/copilot.";
        let job = |phase: &str, message: Option<&str>| Job {
            phase: phase.into(),
            working_sessions: 2,
            message: message.map(str::to_string),
            from_version: Some("0.2.1".into()),
            output: None,
        };
        let mut update = Update { install_plan: Some(plan.into()), latest_version: Some("0.2.2".into()), ..Update::default() };
        let (mut state, mut version) = ("ready", Some("0.2.1"));
        match case {
            Strip::Offer | Strip::Refused => update.available = true,
            Strip::TooOld => {
                update.available = true;
                state = "update-needed";
            }
            Strip::Waiting => update.job = Some(job("waiting", None)),
            Strip::Running => update.job = Some(job("running", Some("Updating weave-copilot to 0.2.2\u{2026}"))),
            Strip::Succeeded => update.job = Some(job("succeeded", Some("Updated weave-copilot from 0.2.1 to 0.2.2."))),
            Strip::Failed => {
                let mut failed = job(
                    "failed",
                    Some("The update failed (exit code 1). weave-copilot is still on 0.2.1. Run it yourself: weave-copilot update"),
                );
                failed.output = Some(
                    "error: could not replace /home/you/.weave/runtimes/copilot/weave-copilot\nPermission denied (os error 13)".into(),
                );
                update.job = Some(failed);
                update.command = Some("weave-copilot update".into());
            }
            Strip::Install | Strip::Downloading => {
                (state, version) = ("not-installed", None);
                update.can_install = true;
                update.available = false;
                if case == Strip::Downloading {
                    update.job = Some(job("running", Some("Downloading the Copilot CLI 1.0.105\u{2026} 42%")));
                }
            }
        }
        if let Some(feed) = self.feeds.harnesses.get_mut(HOME_MACHINE_KEY) {
            let h = &mut feed.harnesses.all[1];
            h.state = state.into();
            h.available = state == "ready";
            h.version = version.map(str::to_string);
            h.reason = (state == "update-needed").then(|| "Fleet needs weave-copilot 0.2.0 or newer. You have 0.1.4.".into());
            if state == "update-needed" {
                h.version = Some("0.1.4".into());
            }
            h.card.update = update;
        }
        if case == Strip::Refused {
            self.settings.harnesses.strips.error = Some(("copilot".into(), "GitHub Copilot is already being updated.".into()));
        }
    }
}
