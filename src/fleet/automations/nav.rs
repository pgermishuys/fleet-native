//! What the Automations page shows, shared by its list and its detail (`composables/use-automations-nav.ts`,
//! `use-automations-machine.ts`): which automation is open, the list / create / edit mode, the new automation's draft, and the
//! machine the view shows. Plain Rust apart from the `State` methods that start a request on a thread.
use super::composer::ComposerState;
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
    /// The new automation being written (`draft`). It survives leaving the page, and the list's draft row shows it.
    pub draft: ComposerState,
    /// The open automation's changes (`editState` of `AutomationDetailPanel.vue`): the automation as saved until edited.
    pub edit: ComposerState,
    /// Try again is running (`retrying`).
    pub retrying: bool,
    /// The note over the open automation (`notice`); opening another one clears it.
    pub notice: Option<Notice>,
    /// The switch is waiting for the server (`isTogglingEnabled`).
    pub toggling: bool,
    /// The built-in automation's page: the action in flight and the time box.
    pub builtin: super::builtin::Builtin,
    /// The open automation's runs.
    pub runs: super::runs::Runs,
    /// The composer's message box for the draft and for the open automation; their text is `draft.text` and `edit.text`,
    /// kept equal through [`Nav::set_text`] and the box's change handler.
    pub draft_box: rune::TextEdit,
    pub edit_box: rune::TextEdit,
    /// A create or save is waiting for the server (`isSubmitting`): the box is read-only and Send spins.
    pub submitting: bool,
    /// Why the server refused the last create or save, over the composer (`formError`).
    pub form_error: Option<String>,
    /// The When menu's boxes and the cron's error.
    pub when: super::when::WhenMenu,
    /// The automation `edit` was made from; another one (or none) makes it again from the list (`resetEditState`).
    pub(super) edit_for: Option<String>,
    /// The composer's upkeep: the machine whose repositories were asked for, the paths last seen, and whether the draft
    /// was given its first folder.
    pub(super) composer: super::submit::Upkeep,
    /// The machine whose list was asked for since the page opened; another machine, or the next visit, asks again.
    asked_for: Option<String>,
    last_tick: i64,
}

impl Nav {
    /// The open automation, while one is open in edit mode.
    pub fn open_id(&self) -> Option<&str> {
        self.active.as_deref().filter(|_| self.mode == ViewMode::Edit)
    }

    /// The draft row's title: the name typed, else the name the sentence gives, else "New automation" (`draftTitle`).
    pub fn draft_title(&self) -> String {
        self.draft.title()
    }

    /// A folder read before the repository list arrived is a repository after all, in the draft and the edit state (row
    /// 7.14). Call it when the repositories of the machine the composer works on change.
    pub fn repositories_changed(&mut self, repositories: &[String]) {
        self.draft.upgrade_folder(repositories);
        self.edit.upgrade_folder(repositories);
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
            self.form_error = None;
            self.builtin.editing = false;
            // The unsaved changes belong to the automation they were made on: the next one starts as it is saved.
            self.edit_for = None;
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
        self.draft = ComposerState::fresh();
        self.draft_box = rune::TextEdit::new("");
        self.composer.draft_folder_done = false;
    }

    /// The composer's state: the open automation's changes, or the draft.
    pub fn composer(&self, editing: bool) -> &ComposerState {
        if editing { &self.edit } else { &self.draft }
    }

    /// Sets the composer's text, in its box too (a scene, a test, or a draft read from a session).
    pub fn set_text(&mut self, editing: bool, text: &str) {
        let (state, field) = if editing { (&mut self.edit, &mut self.edit_box) } else { (&mut self.draft, &mut self.draft_box) };
        field.replace_text(text);
        state.set_text(text);
    }

    /// The box changed: the state takes its text (and "Just once" goes with a new phrase).
    pub fn box_changed(&mut self, editing: bool) {
        let (state, field) = if editing { (&mut self.edit, &self.edit_box) } else { (&mut self.draft, &self.draft_box) };
        if state.text != field.text {
            state.set_text(field.text.clone());
        }
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

    /// `startCreate`, and the box takes the keys (`focusMessage` on mount).
    pub fn start_automation_create(&mut self, tx: &mut rune::Tx) {
        self.automations_nav.start_create();
        tx.focus(crate::view::automations::composer::MESSAGE, false);
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
        changed |= self.keep_automation_composer(tx);
        changed |= self.keep_when_menu();
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
