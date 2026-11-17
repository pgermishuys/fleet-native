//! What agents can do: the level model and the Edit files and Run commands rows (`lib/permissions.ts` `reachOf`, `levelWith`;
//! `composables/use-harness-permissions.ts`; the script of `PermissionsSection.vue`). Plain Rust over the machine Settings
//! shows: its harness list and its preferences, never home by default.
use crate::asks::rules::{PERMISSION_LEVEL_KEY, PermissionLevel, harness_permission_key};
use crate::fleet::new_session::harness_choice::HarnessEntry;
use crate::{State, Tx};

/// Whether something runs without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Allowed,
    Ask,
}

impl Reach {
    pub fn name(self) -> &'static str {
        match self {
            Reach::Allowed => "Allowed",
            Reach::Ask => "Ask",
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Reach::Allowed => "allowed",
            Reach::Ask => "ask",
        }
    }

    pub fn parse(value: &str) -> Option<Reach> {
        match value {
            "allowed" => Some(Reach::Allowed),
            "ask" => Some(Reach::Ask),
            _ => None,
        }
    }

    pub fn opposite(self) -> Reach {
        match self {
            Reach::Allowed => Reach::Ask,
            Reach::Ask => Reach::Allowed,
        }
    }
}

/// A level as its two rows: file edits, and commands (the shell, the web and the browser).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelReach {
    pub edits: Reach,
    pub commands: Reach,
}

/// The two rows one level is shown as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Edits,
    Commands,
}

impl Row {
    /// The id Fleet's test ids use (`reach-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Row::Edits => "edits",
            Row::Commands => "commands",
        }
    }

    pub fn parse(id: &str) -> Option<Row> {
        match id {
            "edits" => Some(Row::Edits),
            "commands" => Some(Row::Commands),
            _ => None,
        }
    }

    /// The row's title.
    pub fn title(self) -> &'static str {
        match self {
            Row::Edits => "Edit files",
            Row::Commands => "Run commands & go online",
        }
    }

    /// The label of its segments and the end of a chip's label ("<harness>: edit files").
    pub fn label(self) -> &'static str {
        match self {
            Row::Edits => "Edit files",
            Row::Commands => "Run commands and go online",
        }
    }

    pub fn action(self) -> &'static str {
        match self {
            Row::Edits => "edit files",
            Row::Commands => "run commands",
        }
    }

    /// The row's name in "Add an exception to <name>".
    pub fn short(self) -> &'static str {
        match self {
            Row::Edits => "Edit files",
            Row::Commands => "Run commands",
        }
    }
}

impl LevelReach {
    pub fn on(self, row: Row) -> Reach {
        match row {
            Row::Edits => self.edits,
            Row::Commands => self.commands,
        }
    }
}

/// `reachOf`.
pub fn reach_of(level: PermissionLevel) -> LevelReach {
    match level {
        PermissionLevel::Ask => LevelReach { edits: Reach::Ask, commands: Reach::Ask },
        PermissionLevel::Edits => LevelReach { edits: Reach::Allowed, commands: Reach::Ask },
        PermissionLevel::All => LevelReach { edits: Reach::Allowed, commands: Reach::Allowed },
    }
}

/// `levelWith`: the level after one row changes. No level lets commands run unasked while edits ask (a command can edit files),
/// so the other row follows: Edit files Ask makes commands ask too, and Run commands Allowed allows edits too.
pub fn level_with(level: PermissionLevel, row: Row, value: Reach) -> PermissionLevel {
    let mut reach = reach_of(level);
    match row {
        Row::Edits => reach.edits = value,
        Row::Commands => reach.commands = value,
    }
    if row == Row::Edits && value == Reach::Ask {
        return PermissionLevel::Ask;
    }
    if reach.commands == Reach::Allowed {
        return PermissionLevel::All;
    }
    if reach.edits == Reach::Allowed { PermissionLevel::Edits } else { PermissionLevel::Ask }
}

/// `defaultTag`: "default" when the row holds Fleet's default, else "default: <label>".
pub fn default_tag(is_default: bool, label: &str) -> String {
    if is_default { "default".to_string() } else { format!("default: {label}") }
}

/// What the page keeps: the machine it last asked, and the chip whose list is open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionsPage {
    /// The machine the page asked when it opened; a different one under the Settings switch is asked again.
    pub asked: Option<String>,
    /// The exception chip whose value list is open: its row and its harness.
    pub chip: Option<(Row, String)>,
    /// The "+ exception" list that is open.
    pub add: Option<Row>,
    /// A change of "Message other sessions" is on its way: the choice waits.
    pub messages_saving: bool,
    /// The machine chip whose value list is open, and whether the machines' "+ exception" list is.
    pub machine_chip: Option<String>,
    pub machine_add: bool,
}

impl State {
    /// The machine's preferences as the page reads them.
    fn permission_pref(&self, key: &str) -> Option<String> {
        self.harness_prefs().get(key).map(str::to_string)
    }

    /// The level every harness uses unless one is set for it: unset, nothing asks (`defaultLevel`).
    pub fn permission_default(&self) -> PermissionLevel {
        PermissionLevel::parse(self.permission_pref(PERMISSION_LEVEL_KEY).as_deref()).unwrap_or(PermissionLevel::All)
    }

    /// `effectiveLevel`: a harness's own level, else the default.
    pub fn permission_level_of(&self, kind: &str) -> PermissionLevel {
        PermissionLevel::parse(self.permission_pref(&harness_permission_key(kind)).as_deref()).unwrap_or_else(|| self.permission_default())
    }

    /// The harnesses that can start a session on the machine shown: installed and turned on.
    pub fn permission_harnesses(&self) -> Vec<HarnessEntry> {
        self.harness_list().all.into_iter().filter(|h| h.available && self.harness_enabled(h)).collect()
    }

    /// Harnesses Fleet can hand a level.
    pub fn permission_leveled(&self) -> Vec<HarnessEntry> {
        self.permission_harnesses().into_iter().filter(|h| h.card.supports_permission_levels).collect()
    }

    /// Harnesses Fleet can't hand a level: they ask, or don't, as their own settings say.
    pub fn permission_unleveled(&self) -> Vec<HarnessEntry> {
        self.permission_harnesses().into_iter().filter(|h| !h.card.supports_permission_levels).collect()
    }

    pub fn permission_reach(&self, kind: &str, row: Row) -> Reach {
        reach_of(self.permission_level_of(kind)).on(row)
    }

    pub fn permission_default_reach(&self, row: Row) -> Reach {
        reach_of(self.permission_default()).on(row)
    }

    /// The harnesses whose level differs from the default on this row: its exceptions.
    pub fn permission_exceptions(&self, row: Row) -> Vec<HarnessEntry> {
        let base = self.permission_default_reach(row);
        self.permission_leveled().into_iter().filter(|h| self.permission_reach(&h.kind, row) != base).collect()
    }

    /// The harnesses that could be made an exception on this row: those that follow the default on it.
    pub fn permission_candidates(&self, row: Row) -> Vec<HarnessEntry> {
        let base = self.permission_default_reach(row);
        self.permission_leveled().into_iter().filter(|h| self.permission_reach(&h.kind, row) == base).collect()
    }

    /// `handedText`: what Fleet hands the harness at its level, as the harness says it, or the level's name.
    pub fn permission_handed(&self, h: &HarnessEntry) -> String {
        let level = self.permission_level_of(&h.kind);
        h.card.permission_modes.get(level.value()).cloned().unwrap_or_else(|| level.name().to_string())
    }

    /// A row's default changes: the level is written to the machine shown (`setDefault`).
    pub fn set_permission_default(&mut self, row: Row, value: Reach) {
        let level = level_with(self.permission_default(), row, value);
        self.save_harness_pref(PERMISSION_LEVEL_KEY, level.value(), |_, _| {});
    }

    /// A harness's own level after one row changes; back at the default, it follows the default again (`setHarness`).
    pub fn set_permission_harness(&mut self, kind: &str, row: Row, value: Reach) {
        let level = level_with(self.permission_level_of(kind), row, value);
        let own = if level == self.permission_default() { "" } else { level.value() };
        self.save_harness_pref(&harness_permission_key(kind), own, |_, _| {});
    }

    /// The Browser chip's state (`browserChip`): whether agents get Fleet's browser on this machine.
    pub fn permission_browser_chip(&self) -> String {
        if self.settings.harnesses.browser_off {
            return "Off".to_string();
        }
        let all = self.permission_harnesses();
        let (with, without): (Vec<_>, Vec<_>) = all.iter().partition(|h| h.card.supports_agent_browser);
        if !with.is_empty() && !without.is_empty() {
            let names: Vec<&str> = without.iter().map(|h| h.display_name.as_str()).collect();
            format!("On, not in {}", names.join(", "))
        } else if !with.is_empty() {
            "On".to_string()
        } else {
            "No harness here can use it".to_string()
        }
    }

    /// Opening the page asks the machine, as `onMounted` does: the preferences, the harness list and the browser's switch. The
    /// machine under the Settings switch changing asks again.
    pub fn keep_permissions_page(&mut self, _tx: &mut Tx) -> bool {
        let here = self.rail == crate::Rail::Settings && self.settings.page == crate::fleet::settings_nav::Page::Permissions;
        if !here {
            return std::mem::take(&mut self.settings.permissions.asked).is_some();
        }
        let machine = self.harness_machine();
        if self.settings.permissions.asked.as_deref() == Some(machine.as_str()) {
            return false;
        }
        self.settings.permissions.asked = Some(machine.clone());
        self.settings.harnesses.browser_off = false;
        self.read_preferences(&machine);
        self.read_harnesses(&machine, false);
        self.read_browser_switch(&machine);
        self.read_permission_machines();
        true
    }
}
