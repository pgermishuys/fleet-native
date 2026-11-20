//! What the open automation's header does (`AutomationDetailPanel.vue`): the switch, Run now and Delete, the note they leave
//! over the page, and the line of words beside the switch. Every request goes to the machine the view shows, on a thread;
//! with no server (scenes, the app with no Fleet) a change is made on the invented list, so the page can be tried.
use super::model::{Automation, RunState};
use super::nav::Notice;
use super::schedule::{Tz, describe_date};
use super::words::{describe_event_type, event_type_of};
use crate::fleet::actions::Call;
use crate::ui::confirm::Confirm;
use crate::{State, Tx};

/// How long an info note stays (`showNotice`).
pub const NOTICE_MS: i64 = 4000;

/// The words the page falls back on when the server gives none.
const SWITCH_ON_FAILED: &str = "Couldn't switch it on.";
const SWITCH_OFF_FAILED: &str = "Couldn't switch it off.";
const RUN_FAILED: &str = "Couldn't start a run.";
const DELETE_FAILED: &str = "Couldn't delete the automation.";
const SKIPPED: &str = "The run was skipped.";
pub const STARTED: &str = "Started a run. It shows below and in Sessions.";

/// What the header says beside the switch (`headerMeta`).
pub fn header_meta(a: &Automation, tz: Tz) -> String {
    let date = |iso: &str| crate::fleet::work::parse_iso_ms(iso).map(|ms| describe_date(ms, tz));
    if !a.is_enabled {
        if a.trigger_type == "once"
            && let Some(when) = a.last_run.as_ref().and_then(|r| date(&r.started_at))
        {
            return format!("Off. Its one run was {when}.");
        }
        return "Off. It won't run until you switch it on.".into();
    }
    if a.trigger_type == "event" {
        return format!("Runs when {}", describe_event_type(&event_type_of(&a.trigger_config)).to_lowercase());
    }
    a.next_run_at.as_deref().and_then(date).map(|when| format!("Next run {when}")).unwrap_or_default()
}

/// `messageOf`: the server's words, else `fallback`.
fn words_of(error: String, fallback: &str) -> String {
    if error.is_empty() { fallback.to_string() } else { error }
}

impl State {
    /// `showNotice`: an info note goes after 4 s, an error stays.
    pub fn automation_say(&mut self, error: bool, text: impl Into<String>) {
        let until = (!error).then(|| crate::view::message::now_ms(self) + NOTICE_MS);
        self.automations_nav.notice = Some(Notice { error, text: text.into(), until });
    }

    /// The open automation's id, while it is the one a request was made for (its answer is about it, not about the next one).
    fn still_open(&self, id: &str) -> bool {
        self.automations_nav.open_id() == Some(id)
    }

    /// The open automation, as the list has it now.
    fn open_automation(&self) -> Option<Automation> {
        let id = self.automations_nav.open_id()?;
        self.shown_automations().get(id).cloned()
    }

    /// `handleToggleEnabled`: on → enable, off → disable, then the list reads again (the row dims too). One at a time.
    pub fn automation_toggle(&mut self, tx: &mut Tx) {
        let Some(a) = self.open_automation() else { return };
        if self.automations_nav.toggling {
            return;
        }
        let (machine, enable) = (self.automations_machine(), !a.is_enabled);
        if self.link_for_machine(&machine).is_none() {
            if let Some(mine) = self.automations.of_mut(&machine).list.iter_mut().find(|x| x.id == a.id) {
                mine.is_enabled = enable;
            }
            return;
        }
        self.automations_nav.toggling = true;
        let id = a.id.clone();
        let done: crate::fleet::automations::store::Done<()> = Box::new(move |s, _, result| {
            s.automations_nav.toggling = false;
            if let Err(e) = result
                && s.still_open(&id)
            {
                s.automation_say(true, words_of(e, if enable { SWITCH_ON_FAILED } else { SWITCH_OFF_FAILED }));
            }
        });
        if enable {
            self.automation_enable(&machine, &a.id, tx, done);
        } else {
            self.automation_disable(&machine, &a.id, tx, done);
        }
    }

    /// `handleRunNow`: a skipped run is an error note with its reason; any other says it started. The run in the runs list is
    /// piece F's.
    pub fn automation_run_now(&mut self, tx: &mut Tx) {
        let Some(a) = self.open_automation() else { return };
        let machine = self.automations_machine();
        if self.link_for_machine(&machine).is_none() {
            self.automation_say(false, STARTED);
            return;
        }
        let id = a.id.clone();
        self.automation_run(
            &machine,
            &a.id,
            tx,
            Box::new(move |s, _, result| {
                if !s.still_open(&id) {
                    return;
                }
                match result {
                    Ok(run) if run.state == RunState::Skipped => s.automation_say(true, words_of(run.error.unwrap_or_default(), SKIPPED)),
                    Ok(_) => s.automation_say(false, STARTED),
                    Err(e) => s.automation_say(true, words_of(e, RUN_FAILED)),
                }
            }),
        );
    }

    /// `confirmDelete`: asks first, naming it.
    pub fn automation_delete_ask(&mut self, tx: &mut Tx) {
        let Some(a) = self.open_automation() else { return };
        let call = Call::DeleteAutomation { machine: self.automations_machine(), id: a.id };
        self.ask(Confirm::delete_automation(a.name, call), tx);
    }

    /// The delete, once confirmed: back to list mode when it is done, an error note when the server refused.
    pub(crate) fn automation_delete_now(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        if self.link_for_machine(machine).is_none() {
            self.automations.of_mut(machine).list.retain(|x| x.id != id);
            self.automations_nav.clear_selection();
            return;
        }
        self.automation_delete(
            machine,
            id,
            tx,
            Box::new(|s, _, result| match result {
                Ok(()) => s.automations_nav.clear_selection(),
                Err(e) => s.automation_say(true, words_of(e, DELETE_FAILED)),
            }),
        );
    }
}
