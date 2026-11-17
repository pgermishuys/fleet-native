//! What agents can do, the rows after Run commands (`PermissionsSection.vue`): messages between sessions, work sent to other
//! machines, and runs nobody is watching. Plain Rust over the machine Settings shows; the machine list is home's own.
use crate::State;
use crate::fleet::machine_actions::parse_agent_work;
use crate::fleet::machine_card::level_name;
use crate::fleet::machines::{AgentWork, Entry, Source};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::settings::permissions::default_tag;

pub const SESSION_MESSAGES_KEY: &str = "SessionMessages";
pub const PERMISSION_UNATTENDED_KEY: &str = "PermissionUnattended";

/// What a run does at an ask when nobody is there to answer (`UnattendedPermission`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unattended {
    Allow,
    Wait,
    Refuse,
}

impl Unattended {
    pub const ALL: [Unattended; 3] = [Unattended::Allow, Unattended::Wait, Unattended::Refuse];

    pub fn value(self) -> &'static str {
        match self {
            Unattended::Allow => "all",
            Unattended::Wait => "same",
            Unattended::Refuse => "deny",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Unattended::Allow => "Allow",
            Unattended::Wait => "Stop & wait",
            Unattended::Refuse => "Refuse",
        }
    }

    /// `toUnattendedPermission`: anything that isn't one of the three reads as Allow.
    pub fn parse(value: Option<&str>) -> Unattended {
        match value {
            Some("same") => Unattended::Wait,
            Some("deny") => Unattended::Refuse,
            _ => Unattended::Allow,
        }
    }

    /// `UNATTENDED_NOTES`.
    pub fn note(self) -> &'static str {
        match self {
            Unattended::Allow => "Nobody is there to answer, so these runs ask about nothing.",
            Unattended::Wait => "A run stops at its first ask and shows Needs you until you answer.",
            Unattended::Refuse => "Anything that would ask is refused, and the agent is told why.",
        }
    }
}

/// The words of a machine's setting (`MACHINE_CHOICES`).
pub fn work_label(work: AgentWork) -> &'static str {
    match work {
        AgentWork::Ask => "Ask",
        AgentWork::Always => "Always",
        AgentWork::Never => "Never",
    }
}

impl State {
    pub fn permission_unattended(&self) -> Unattended {
        Unattended::parse(self.harness_prefs().get(PERMISSION_UNATTENDED_KEY))
    }

    pub fn unattended_tag(&self) -> String {
        default_tag(self.permission_unattended() == Unattended::Allow, "Allow")
    }

    /// `setUnattended`.
    pub fn set_permission_unattended(&mut self, value: Unattended) {
        self.save_harness_pref(PERMISSION_UNATTENDED_KEY, value.value(), |_, _| {});
    }

    /// Whether sessions may message each other: off until it is turned on (`SessionMessages` = "true").
    pub fn permission_messages(&self) -> bool {
        self.harness_prefs().get(SESSION_MESSAGES_KEY) == Some("true")
    }

    pub fn messages_tag(&self) -> String {
        default_tag(!self.permission_messages(), "Off")
    }

    /// `setMessages`: the value already shown does nothing; while a save runs the choice waits (`usePreferenceSwitch`).
    pub fn set_permission_messages(&mut self, on: bool) {
        if on == self.permission_messages() || self.settings.permissions.messages_saving {
            return;
        }
        self.settings.permissions.messages_saving = true;
        self.save_harness_pref(SESSION_MESSAGES_KEY, if on { "true" } else { "false" }, |s, _| {
            s.settings.permissions.messages_saving = false;
        });
    }

    /// Whether the machine shown is the live one: only its page can list the machines it sends work to.
    pub fn permission_machine_is_live(&self) -> bool {
        self.harness_machine() == HOME_MACHINE_KEY
    }

    /// `shownName`: the machine Settings shows, by name.
    pub fn permission_shown_name(&self) -> String {
        let key = self.harness_machine();
        self.machines.entries().into_iter().find(|e| e.key == key).map_or_else(|| "this machine".to_string(), |e| e.name)
    }

    /// `agentWorkEntries`: the machines agents here can send work to, each with its setting. Only the owner sees home's list.
    fn permission_machine_entries(&self) -> Vec<(Entry, AgentWork)> {
        if self.machines.source != Source::Owner {
            return Vec::new();
        }
        self.machines
            .entries()
            .into_iter()
            .filter(|e| !e.is_home)
            .filter_map(|e| self.machines.agent_work.get(&e.key).copied().map(|w| (e, w)))
            .collect()
    }

    pub fn permission_machines_listed(&self) -> bool {
        !self.permission_machine_entries().is_empty()
    }

    /// The machines set to Always or Never: the row's exceptions.
    pub fn permission_machine_exceptions(&self) -> Vec<(Entry, AgentWork)> {
        self.permission_machine_entries().into_iter().filter(|(_, w)| *w != AgentWork::Ask).collect()
    }

    /// The machines at Ask, which could be made exceptions.
    pub fn permission_machine_candidates(&self) -> Vec<Entry> {
        self.permission_machine_entries().into_iter().filter(|(_, w)| *w == AgentWork::Ask).map(|(e, _)| e).collect()
    }

    /// "runs at <level name>" (`agentWorkLevelName`): the level the machine says it holds.
    pub fn permission_machine_level(&self, entry: &Entry) -> &'static str {
        level_name(entry.capabilities().and_then(|c| c.permission_level.as_deref()))
    }

    /// `setMachine`: the value is sent to home; a failure shows under the row, and the next try clears it.
    pub fn set_permission_machine(&mut self, key: &str, value: AgentWork) {
        for (e, _) in self.permission_machine_entries() {
            self.machine_page.errors.remove(&e.key);
        }
        self.set_agent_work(key, value);
    }

    /// The error a failed change of a machine's value left, for the row.
    pub fn permission_machine_error(&self) -> Option<String> {
        self.permission_machine_entries().iter().find_map(|(e, _)| self.machine_page.errors.get(&e.key).cloned())
    }

    /// Picking from the "+ exception" list: `<key>|always` or `<key>|never`.
    pub fn add_permission_machine(&mut self, pick: &str) {
        let Some((key, word)) = pick.split_once('|') else { return };
        if let Some(work) = parse_agent_work(word) {
            self.set_permission_machine(key, work);
        }
    }

    /// Opening the page on the live machine reads home's machines again and their settings (`loadHome`, `syncFromServer`): an
    /// answer in a conversation may have changed one since the list was read. Another machine's page asks for none of it.
    pub fn read_permission_machines(&mut self) {
        if !self.permission_machine_is_live() {
            return;
        }
        if let Some(link) = &self.link {
            link.refresh_machines();
        }
    }
}
