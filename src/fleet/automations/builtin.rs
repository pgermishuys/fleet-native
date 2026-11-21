//! The built-in Dreaming automation's page (`BuiltinAutomationDetail.vue`): its words, and what its buttons do. Run now, Pause
//! and Change time are the user's; deleting and editing are not. One action runs at a time (`isBusy`), and a refusal shows as
//! the error note. Every request goes to the machine the view shows, on a thread; with no server (scenes, the app with no
//! Fleet) a change is made on the invented list.
use super::model::Automation;
use super::schedule::describe_date;
use crate::fleet::automations::actions::words_of;
use crate::fleet::settings::dream_nightly::{self, BAD_TIME};
use crate::fleet::settings::memory::{self, Request};
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::json;

const RUN_FAILED: &str = "Couldn't start a run.";
const PAUSE_FAILED: &str = "Couldn't change it.";
const TIME_FAILED: &str = "Couldn't change the time.";
/// The time the box starts on when the schedule is not "every day".
const DEFAULT_TIME: &str = "03:00";

/// What the page holds besides the automation: the action in flight and the time box.
#[derive(Debug, Default)]
pub struct Builtin {
    /// An action is waiting for the server (`isBusy`).
    pub busy: bool,
    /// Change time is open (`editingTime`).
    pub editing: bool,
    pub time: TextEdit,
}

/// The clock time its cron "m h * * *" runs at, as HH:MM; none for a cron of any other shape (`clock`).
pub fn clock(trigger_config: &str) -> Option<String> {
    let parts: Vec<&str> = trigger_config.trim().split(' ').collect();
    let [m, h, "*", "*", "*"] = parts[..] else { return None };
    let digits = |s: &str| (1..=2).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit());
    (digits(m) && digits(h)).then(|| format!("{h:0>2}:{m:0>2}"))
}

/// The description under the bar (`description`).
pub fn description(a: &Automation) -> String {
    let when = match clock(&a.trigger_config) {
        Some(time) => format!("Every day at {time}."),
        None => format!("On the schedule {}.", a.trigger_config),
    };
    format!("{when} Starts one dreaming session for each repository with sessions since the last run. Skips the rest at no cost.")
}

/// What the bar says beside the pills (`meta`).
pub fn meta(a: &Automation) -> String {
    if !a.is_enabled {
        return "Paused. It won't run until you resume it.".into();
    }
    let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
    a.next_run_at
        .as_deref()
        .and_then(crate::fleet::work::parse_iso_ms)
        .map(|ms| format!("Next run {}", describe_date(ms, &tz)))
        .unwrap_or_default()
}

/// The empty runs' words (`automation-no-runs`).
pub fn no_runs(a: &Automation) -> String {
    let tz = |ms: i64| crate::conv::time::local_offset_at(ms);
    let first = a.next_run_at.as_deref().and_then(crate::fleet::work::parse_iso_ms).map(|ms| describe_date(ms, &tz));
    format!("No runs yet. The first is {}, or press Run now.", first.unwrap_or_else(|| "tonight".into()))
}

impl State {
    /// The open automation, when it is the built-in one.
    fn open_builtin(&self) -> Option<Automation> {
        self.open_automation().filter(|a| a.builtin.is_some())
    }

    /// `act`: one at a time, and the last note goes first. False when another action is running.
    fn builtin_begin(&mut self) -> bool {
        if self.automations_nav.builtin.busy {
            return false;
        }
        self.automations_nav.builtin.busy = true;
        self.automations_nav.notice = None;
        true
    }

    /// The end of an action: not busy, and a refusal is the error note if the same automation is still open.
    fn builtin_end(&mut self, id: &str, failure: Result<(), (String, &str)>) {
        self.automations_nav.builtin.busy = false;
        if let Err((e, fallback)) = failure
            && self.still_open(id)
        {
            self.automation_say(true, words_of(e, fallback));
        }
    }

    /// Run now (`runNow`): the run goes first in the list, and the list is read again.
    pub fn builtin_run_now(&mut self, tx: &mut Tx) {
        let Some(a) = self.open_builtin() else { return };
        if !self.builtin_begin() {
            return;
        }
        let machine = self.automations_machine();
        if self.link_for_machine(&machine).is_none() {
            self.automation_run_started(None);
            self.automations_nav.builtin.busy = false;
            return;
        }
        let id = a.id.clone();
        self.automation_run(
            &machine,
            &a.id,
            tx,
            Box::new(move |s, _, result| {
                let failure = result.map(|run| {
                    if s.still_open(&id) {
                        s.automation_run_started(Some(run));
                    }
                });
                s.builtin_end(&id, failure.map_err(|e| (e, RUN_FAILED)));
            }),
        );
    }

    /// Pause / Resume (`togglePaused`): disables or enables the automation, then the list is read again.
    pub fn builtin_toggle_paused(&mut self, tx: &mut Tx) {
        let Some(a) = self.open_builtin() else { return };
        if !self.builtin_begin() {
            return;
        }
        let machine = self.automations_machine();
        if self.link_for_machine(&machine).is_none() {
            if let Some(mine) = self.automations.of_mut(&machine).list.iter_mut().find(|x| x.id == a.id) {
                mine.is_enabled = !a.is_enabled;
            }
            self.automations_nav.builtin.busy = false;
            return;
        }
        let id = a.id.clone();
        let done: super::store::Done<()> = Box::new(move |s, _, result| s.builtin_end(&id, result.map_err(|e| (e, PAUSE_FAILED))));
        if a.is_enabled {
            self.automation_disable(&machine, &a.id, tx, done);
        } else {
            self.automation_enable(&machine, &a.id, tx, done);
        }
    }

    /// Change time (`startEditingTime`): the box opens on the clock time, or 03:00.
    pub fn builtin_start_time(&mut self) {
        let Some(a) = self.open_builtin() else { return };
        let time = clock(&a.trigger_config).unwrap_or_else(|| DEFAULT_TIME.into());
        let b = &mut self.automations_nav.builtin;
        b.time.replace_text(time);
        b.editing = true;
    }

    /// Cancel.
    pub fn builtin_cancel_time(&mut self) {
        self.automations_nav.builtin.editing = false;
    }

    /// Save (`saveTime`): `PUT /api/memory/dreaming/nightly {time, timeZone}` on the view's machine, the zone this computer is
    /// in (else UTC), then the list is read again. A time that isn't one is said and nothing is sent.
    pub fn builtin_save_time(&mut self) {
        let Some(a) = self.open_builtin() else { return };
        let typed = self.automations_nav.builtin.time.text.trim().to_string();
        if self.automations_nav.builtin.busy || typed.is_empty() {
            return;
        }
        if !self.builtin_begin() {
            return;
        }
        let Some(time) = dream_nightly::parse_time(&typed) else {
            self.automations_nav.builtin.busy = false;
            self.automation_say(true, BAD_TIME);
            return;
        };
        let machine = self.automations_machine();
        let Some(link) = self.link_for_machine(&machine) else {
            if let Some(mine) = self.automations.of_mut(&machine).list.iter_mut().find(|x| x.id == a.id)
                && let Some((h, m)) = time.split_once(':')
            {
                mine.trigger_config = format!("{} {} * * *", m.parse::<u32>().unwrap_or(0), h.parse::<u32>().unwrap_or(0));
            }
            self.automations_nav.builtin.busy = false;
            self.automations_nav.builtin.editing = false;
            return;
        };
        let body = json!({"time": time, "timeZone": dream_nightly::local_time_zone().unwrap_or_else(|| "UTC".into())});
        let call = Request { method: "PUT", path: "/api/memory/dreaming/nightly".into(), body: Some(body), fallback: TIME_FAILED.into() };
        let id = a.id.clone();
        link.off_thread(
            move |base| memory::send(base, &call).map(|_| ()),
            move |s: &mut State, tx: &mut Tx, got: Result<(), String>| {
                if got.is_ok() && s.still_open(&id) {
                    s.automations_nav.builtin.editing = false;
                }
                if got.is_ok() {
                    let machine = s.automations_machine();
                    s.automations_refresh(&machine, tx, None);
                }
                s.builtin_end(&id, got.map_err(|e| (e, TIME_FAILED)));
            },
        );
    }

    /// Settings → Memory (`openMemorySettings`).
    pub fn builtin_open_settings(&mut self) {
        self.open_settings("memory", None);
    }
}
