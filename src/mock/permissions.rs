//! What agents can do's invented data (component 16, piece M): a machine with OpenCode 2, Claude Code and Pi, where Pi is
//! installed but Fleet can't hand it a permission level, so the page draws the same every time.
use crate::State;
use crate::fleet::machines::AgentWork;
use crate::fleet::new_session::harness_choice::Load;
use crate::fleet::settings::HOME_MACHINE_KEY;

/// What the preferences hold in a scene.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Nothing is set: edits and commands are allowed.
    Defaults,
    /// The default level asks, and Claude Code has `edits` of its own, so it is the one exception on Edit files.
    Custom,
    /// The default level is `edits`: Run commands asks, and Claude Code's own `all` lets it run commands.
    Edits,
    /// Messages on, nobody-watching at Stop & wait, and three other machines: falcon Always, harbor Never, lantern at Ask.
    Machines,
    /// One other machine, vega, at Ask (what a scratch Fleet with one paired machine shows).
    VegaAsk,
    /// The same with vega at Always, messages on and nobody-watching at Stop & wait.
    VegaAlways,
    /// The same, with falcon shown: its list of machines isn't on its page.
    Elsewhere,
}

impl State {
    /// Settings → What agents can do on the invented list, no server asked. `open` leaves a value list open: a chip's, or the
    /// "+ exception" one.
    pub fn show_permissions(&mut self, dark: bool, case: Case, open: Option<&'static str>) {
        self.dark = Some(dark);
        self.now_ms = Some(crate::mock::rows::NOW);
        let mut list = super::harnesses::harnesses("2026-11-17T09:00:00Z");
        list.retain(|h| h.kind != "copilot");
        for h in &mut list {
            h.available = true;
            h.state = "ready".into();
            h.reason = None;
        }
        list[0].card.permission_modes.insert("ask".into(), "Ask before running tools or editing".into());
        let claude = list.iter_mut().find(|h| h.kind == "claude-code").expect("Claude Code is listed");
        claude.card.permission_modes.insert("edits".into(), "Accept edits, ask for commands".into());
        claude.card.permission_modes.insert("all".into(), "Bypass permissions".into());
        let feed = self.feeds.harnesses.entry(HOME_MACHINE_KEY.to_string()).or_default();
        feed.harnesses.all = list;
        feed.harnesses.load = Load::Done;
        self.pickers.prefs.fetched = true;
        match case {
            Case::Defaults | Case::Machines | Case::Elsewhere | Case::VegaAsk | Case::VegaAlways => {}
            Case::Custom => {
                self.pickers.prefs.set("PermissionLevel", "ask");
                self.pickers.prefs.set("PermissionLevel.claude-code", "edits");
            }
            Case::Edits => {
                self.pickers.prefs.set("PermissionLevel", "edits");
                self.pickers.prefs.set("PermissionLevel.claude-code", "all");
            }
        }
        if matches!(case, Case::Machines | Case::Elsewhere) {
            self.pickers.prefs.set("SessionMessages", "true");
            self.pickers.prefs.set("PermissionUnattended", "same");
            self.show_machines_only(crate::mock::machines::Case::Several);
            self.machines.home = Some(super::machines_page::info("home-id", "atlas", "all", 1, 2));
            self.machines.identities.insert("m-falcon".into(), super::machines_page::info("m-falcon", "falcon", "edits", 0, 1));
            self.machines.identities.insert("m-harbor".into(), super::machines_page::info("m-harbor", "harbor", "ask", 0, 0));
            self.machines.identities.insert("m-lantern".into(), super::machines_page::info("m-lantern", "lantern", "all", 0, 0));
            self.machines.agent_work.insert("m-falcon".into(), AgentWork::Always);
            self.machines.agent_work.insert("m-harbor".into(), AgentWork::Never);
            self.machines.agent_work.insert("m-lantern".into(), AgentWork::Ask);
        }
        if matches!(case, Case::VegaAsk | Case::VegaAlways) {
            let always = case == Case::VegaAlways;
            if always {
                self.pickers.prefs.set("SessionMessages", "true");
                self.pickers.prefs.set("PermissionUnattended", "same");
            }
            self.show_machines_only(crate::mock::machines::Case::Empty);
            self.machines.list[0].name = "vega".into();
            self.machines.home = Some(super::machines_page::info("home-id", "atlas", "all", 1, 2));
            self.machines.identities.insert("m-quiet".into(), super::machines_page::info("m-quiet", "vega", "all", 0, 0));
            self.machines.agent_work.insert("m-quiet".into(), if always { AgentWork::Always } else { AgentWork::Ask });
        }
        self.open_settings("permissions", None);
        if case == Case::Elsewhere {
            self.settings.choice.pick("m-falcon");
        }
        match open {
            Some("chip") => {
                self.settings.permissions.chip = Some((crate::fleet::settings::permissions::Row::Edits, "claude-code".into()));
                self.ui.open = Some(crate::ui::Open::Select("permission-chip"));
            }
            Some("machine") => {
                self.settings.permissions.machine_chip = Some("m-falcon".into());
                self.ui.open = Some(crate::ui::Open::Select("permission-machine"));
            }
            Some("machine-add") => {
                self.settings.permissions.machine_add = true;
                self.ui.open = Some(crate::ui::Open::Select("permission-machine-add"));
            }
            Some("add") => {
                self.settings.permissions.add = Some(crate::fleet::settings::permissions::Row::Commands);
                self.ui.open = Some(crate::ui::Open::Select("permission-add"));
            }
            _ => {}
        }
    }
}
