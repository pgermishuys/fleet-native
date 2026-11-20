//! What the Automations page shows, shared by its list and its detail (`composables/use-automations-nav.ts`,
//! `use-automations-machine.ts`): which automation is open, the list / create / edit mode, the new automation's draft, and the
//! machine the view shows. Plain Rust apart from the `State` methods that start a request on a thread.
use super::model::Automation;
use crate::State;
use crate::fleet::machine_choice::MachineChoice;

/// `AutomationViewMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    List,
    Create,
    Edit,
}

/// The new automation being written. It survives leaving the page, and the list's draft row shows it. Piece H gives it the
/// rest of Fleet's `AutomationComposerState`; until then it holds what the row's title reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    pub name: String,
}

/// A short note over the open automation: that a run started, or why the switch, Run now or Delete failed (`notice`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub error: bool,
    pub text: String,
    /// When an info note goes (4 s after it came); an error stays.
    pub until: Option<i64>,
}

/// The words of a load that failed while the list is empty and a retry is running.
pub const RETRYING: &str = "Trying again…";

/// The row's relative times follow the clock this often (`useRelativeTime`).
const TICK_MS: i64 = 30_000;

#[derive(Debug, Default)]
pub struct Nav {
    /// The machine the list and the pane show; the machine you're on until the view's switch picks another.
    pub choice: MachineChoice,
    pub active: Option<String>,
    pub mode: ViewMode,
    pub draft: Draft,
    /// Try again is running (`retrying`).
    pub retrying: bool,
    /// The note over the open automation (`notice`); opening another one clears it.
    pub notice: Option<Notice>,
    /// The switch is waiting for the server (`isTogglingEnabled`).
    pub toggling: bool,
    /// The open automation's runs.
    pub runs: super::runs::Runs,
    /// The machine whose list was asked for since the page opened; another machine, or the next visit, asks again.
    asked_for: Option<String>,
    last_tick: i64,
}

impl Nav {
    /// The open automation, while one is open in edit mode.
    pub fn open_id(&self) -> Option<&str> {
        self.active.as_deref().filter(|_| self.mode == ViewMode::Edit)
    }

    /// The draft row's title: the name typed, else "New automation". Piece H reads the sentence for it (`draftTitle`).
    pub fn draft_title(&self) -> String {
        let name = self.draft.name.trim();
        if name.is_empty() { "New automation".to_string() } else { name.to_string() }
    }

    /// The scenes say the list was asked for, so the page doesn't read it again over what they set.
    pub fn mark_asked(&mut self, machine: &str) {
        self.asked_for = Some(machine.to_string());
    }

    /// Opening another automation, or changing mode, drops what the page said about the one before (`watch([activeAutomationId,
    /// viewMode])`); the same one opened again changes nothing.
    fn moved(&mut self, active: Option<String>, mode: ViewMode) {
        if self.active != active || self.mode != mode {
            self.notice = None;
        }
        self.active = active;
        self.mode = mode;
    }

    /// `setActiveAutomation`.
    pub fn set_active(&mut self, id: &str) {
        self.moved(Some(id.to_string()), ViewMode::Edit);
    }

    /// `startCreate`.
    pub fn start_create(&mut self) {
        self.moved(None, ViewMode::Create);
    }

    /// `clearSelection`.
    pub fn clear_selection(&mut self) {
        self.moved(None, ViewMode::List);
    }

    /// `resetDraft`.
    pub fn reset_draft(&mut self) {
        self.draft = Draft::default();
    }
}

impl State {
    /// The machine the view shows (`useAutomationsMachine().key`).
    pub fn automations_machine(&self) -> String {
        self.automations_nav.choice.key(&self.machines).to_string()
    }

    /// The automations of the machine the view shows.
    pub fn shown_automations(&self) -> &super::store::MachineAutomations {
        self.automations_of(&self.automations_machine())
    }

    /// Opening the page: it starts on the machine you're on and reads the list again (`resetAutomationsMachine`, and the
    /// list's `refresh` on mount).
    pub fn enter_automations(&mut self) {
        if self.rail != crate::Rail::Automations {
            self.automations_nav.choice.reset();
            self.automations_nav.asked_for = None;
            // The runs stop with the page and are read again when it opens (the poll's timer ends on unmount).
            self.automations_nav.runs = Default::default();
        }
        self.rail = crate::Rail::Automations;
    }

    /// `setActiveAutomation`.
    pub fn set_active_automation(&mut self, id: &str) {
        self.automations_nav.set_active(id);
    }

    /// `startCreate`.
    pub fn start_automation_create(&mut self) {
        self.automations_nav.start_create();
    }

    /// The list's Try again: reads again while the failed note stays up and its button waits (`retry`).
    pub fn automations_retry(&mut self, tx: &mut rune::Tx) {
        if self.automations_nav.retrying {
            return;
        }
        self.automations_nav.retrying = true;
        let machine = self.automations_machine();
        self.automations_refresh(
            &machine,
            tx,
            Some(Box::new(|s, _| {
                s.automations_nav.retrying = false;
            })),
        );
    }

    /// The page's own upkeep, once a frame (`poll`): reads the machine's list when the page opens or the machine changes, and
    /// moves the clock the rows' times follow. With no Fleet to ask (scenes, the app with no server) the list is the invented
    /// one. True when something changed.
    pub fn keep_automations(&mut self, tx: &mut rune::Tx) -> bool {
        if self.rail != crate::Rail::Automations {
            return false;
        }
        let machine = self.automations_machine();
        let mut changed = false;
        if self.automations_nav.asked_for.as_deref() != Some(machine.as_str()) {
            self.automations_nav.asked_for = Some(machine.clone());
            if self.link_for_machine(&machine).is_some() {
                self.automations_refresh(&machine, tx, None);
            } else {
                let store = self.automations.of_mut(&machine);
                store.list = crate::mock::automations::automations();
                store.is_loading = false;
            }
            changed = true;
        }
        let now = crate::view::message::now_ms(self);
        // An info note goes after 4 s.
        if self.automations_nav.notice.as_ref().and_then(|n| n.until).is_some_and(|until| now >= until) {
            self.automations_nav.notice = None;
            changed = true;
        }
        changed |= self.keep_automation_runs(tx);
        if now - self.automations_nav.last_tick >= TICK_MS {
            self.automations_nav.last_tick = now;
            changed = true;
        }
        changed
    }
}

/// `machineOf`: the other machine an automation's runs go to, by the name it has in the list now; none for this machine.
pub fn machine_of(s: &State, automation: &Automation) -> Option<String> {
    let id = automation.target_machine_id.as_deref().filter(|id| !id.is_empty())?;
    let last = automation.last_run.as_ref().and_then(|r| r.machine_name.clone());
    // The id is in the list of the machine that keeps it; this machine's list names only its own.
    if s.automations_nav.choice.is_other(&s.machines) {
        return Some(last.unwrap_or_else(|| "another machine".into()));
    }
    let named = s.machines.entries().into_iter().find(|e| e.key == id).map(|e| e.name);
    Some(named.or(last).unwrap_or_else(|| "another machine".into()))
}
