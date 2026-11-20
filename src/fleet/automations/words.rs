//! The words Automations uses (`lib/automations.ts`: row 14.10-14.15): what an event is called, how a run and a row are
//! described, and the plan line under the composer. Pure: the clock and its zone are passed in, so a test fixes them. A part
//! of the plan line carries what a view needs to draw it (strong, a caution, a path).
use super::model::{Automation, AutomationRun, RunState};
use super::schedule::{ScheduleHit, Tz, When, day_name, describe_date, next_run, next_short, when_sentence};
use crate::fleet::new_session::plan::tilde_path;
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::dream_nightly;

/// What each event an automation can wait for means, in the words the form and page use, in the server's order.
const EVENT_LABELS: [(&str, &str); 5] = [
    ("session_created", "A session starts"),
    ("session_archived", "A session is archived"),
    ("session_deleted", "A session is deleted"),
    ("delegation.created", "A sub agent starts"),
    ("delegation.updated", "A sub agent's status changes"),
];

/// The event types an automation can wait for (`EVENT_TYPES`).
pub fn event_types() -> Vec<&'static str> {
    EVENT_LABELS.iter().map(|(t, _)| *t).collect()
}

/// An event type in words; one the server no longer sends is marked, since that trigger can't fire (`describeEventType`).
pub fn describe_event_type(event_type: &str) -> String {
    match EVENT_LABELS.iter().find(|(t, _)| *t == event_type) {
        Some((_, words)) => (*words).to_string(),
        None => format!("{event_type} (never fires)"),
    }
}

/// The event type an event trigger waits for, from its stored config (`{"eventType": …}`); anything else as it is
/// (`eventTypeOf`).
pub fn event_type_of(trigger_config: &str) -> String {
    serde_json::from_str::<serde_json::Value>(trigger_config)
        .ok()
        .and_then(|v| v.get("eventType").and_then(|t| t.as_str().map(str::to_string)))
        .unwrap_or_else(|| trigger_config.to_string())
}

/// The machine's IANA zone, which new and edited schedules are saved in; `None` where it can't be told (`browserTimeZone`).
pub fn browser_time_zone() -> Option<String> {
    dream_nightly::local_time_zone()
}

/// The zone a saved schedule runs in, as the page says it: automations saved without one run in UTC (`scheduleTimeZone`).
pub fn schedule_time_zone(time_zone: Option<&str>) -> String {
    time_zone.filter(|z| !z.is_empty()).unwrap_or("UTC").to_string()
}

// ── Runs and rows ─────────────────────────────────────────────────────────────

/// What started a run, for the runs list; nothing for the schedule itself (`describeRunTrigger`).
pub fn describe_run_trigger(trigger: &str) -> String {
    match trigger {
        "schedule" => String::new(),
        "catch_up" => "Caught up".into(),
        "once" => "Once".into(),
        "manual" => "Run now".into(),
        other => describe_event_type(other),
    }
}

/// How a word is coloured (`RowTone`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Working,
    Error,
    Warn,
    Quiet,
}

/// A run's state in a word, and how it's coloured (`describeRunState`).
pub fn describe_run_state(run: &AutomationRun) -> (String, Tone) {
    let (label, tone) = match run.state {
        RunState::Starting => ("Starting".to_string(), Tone::Working),
        RunState::Running => ("Running".to_string(), Tone::Working),
        RunState::Waiting => ("Needs you".to_string(), Tone::Warn),
        RunState::Ended => ("Ended".to_string(), Tone::Quiet),
        RunState::Failed => ("Failed".to_string(), Tone::Error),
        RunState::Skipped => ("Skipped".to_string(), Tone::Warn),
        RunState::Unanswered => (format!("{} didn't answer", run.machine_name.as_deref().unwrap_or("Its machine")), Tone::Warn),
        RunState::Done => ("Done".to_string(), Tone::Quiet),
    };
    (label, tone)
}

/// The glyph before a row's status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Working,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowStatus {
    pub label: String,
    pub tone: Tone,
    pub glyph: Option<Glyph>,
}

fn status(label: &str, tone: Tone, glyph: Option<Glyph>) -> RowStatus {
    RowStatus { label: label.to_string(), tone, glyph }
}

/// A sidebar row's status, like a session row's: Running while its latest run is going, Failed when that run failed,
/// otherwise when it runs next. Off rows say so and dim (`automationRowStatus`).
pub fn automation_row_status(automation: &Automation, from: i64, tz: Tz) -> RowStatus {
    let last = automation.last_run.as_ref().map(|r| r.state);
    if matches!(last, Some(RunState::Running | RunState::Starting)) {
        return status("Running", Tone::Working, Some(Glyph::Working));
    }
    if last == Some(RunState::Waiting) {
        return status("Needs you", Tone::Warn, None);
    }
    if !automation.is_enabled {
        return status("Off", Tone::Quiet, None);
    }
    if last == Some(RunState::Failed) {
        return status("Failed", Tone::Error, Some(Glyph::Error));
    }
    if let Some(next) = automation.next_run_at.as_deref().and_then(crate::fleet::work::parse_iso_ms) {
        return RowStatus { label: next_short(next, from, tz), tone: Tone::Quiet, glyph: None };
    }
    status(if automation.trigger_type == "event" { "On event" } else { "" }, Tone::Quiet, None)
}

// ── The line under the composer ───────────────────────────────────────────────

/// A run of plan-line text (`PlanPart`): `strong` is the schedule, `code` a path or branch, `warn` a caution.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Part {
    pub text: String,
    pub strong: bool,
    pub code: bool,
    pub warn: bool,
}

fn plain(text: impl Into<String>) -> Part {
    Part { text: text.into(), ..Part::default() }
}
fn strong(text: impl Into<String>) -> Part {
    Part { text: text.into(), strong: true, ..Part::default() }
}
fn code(text: impl Into<String>) -> Part {
    Part { text: text.into(), code: true, ..Part::default() }
}
fn warn(text: impl Into<String>) -> Part {
    Part { text: text.into(), warn: true, ..Part::default() }
}

/// What's known when the line is made (`AutomationPlanInput`).
pub struct PlanInput<'a> {
    /// What's in the box.
    pub text: &'a str,
    pub when: Option<&'a When>,
    /// The schedule words found in the text; `None` when the When menu chose instead.
    pub hit: Option<&'a ScheduleHit>,
    /// What the agent will be asked.
    pub prompt: &'a str,
    /// The sentence said "on Friday" and "Just once" was chosen.
    pub force_once: bool,
    pub folder: Option<&'a Folder>,
    /// Each run gets a new worktree of the folder.
    pub worktree: bool,
    /// Where a worktree starts, once known.
    pub base: Option<&'a str>,
    pub same_session: bool,
    /// `None`: runs are sessions. `Some(None)`: a workflow target with none picked. `Some(Some(name))`: its name.
    pub workflow: Option<Option<&'a str>>,
    /// The zone the schedule will be saved in.
    pub time_zone: &'a str,
    /// An existing automation's zone, to say when saving moves it: `None` for a new one, `Some(None)` for one saved in UTC.
    pub saved_time_zone: Option<Option<&'a str>>,
    /// An automation made before folders were kept per run: no folder means the first workspace root.
    pub legacy_folderless: bool,
    /// Now, in Unix ms, and the offset of the clock the schedule is read on.
    pub from: i64,
    pub tz: Tz<'a>,
}

/// What a button after the schedule answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskAction {
    JustOnce,
    EveryWeek,
}

/// A question the schedule leaves open, answered by a button after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub question: String,
    pub answer: String,
    pub action: AskAction,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// When it runs.
    pub schedule: Vec<Part>,
    pub ask: Option<Ask>,
    /// Where each run happens and what the agent gets.
    pub rest: Vec<Part>,
}

/// The whole line as one string: the schedule, the question and its answer in brackets, a space, the rest. This is what Fleet's
/// tests compare.
pub fn plan_sentence(plan: &Plan) -> String {
    let mut s: String = plan.schedule.iter().map(|p| p.text.as_str()).collect();
    if let Some(ask) = &plan.ask {
        s.push_str(&format!(" {} [{}]", ask.question, ask.answer));
    }
    s.push(' ');
    s.extend(plan.rest.iter().map(|p| p.text.as_str()));
    s
}

/// One sentence (or three) saying what pressing Enter will make (`describeAutomationPlan`).
pub fn describe_automation_plan(input: &PlanInput) -> Plan {
    let mut schedule = Vec::new();
    let mut ask = None;
    let tz = input.tz;

    match input.when {
        None => schedule.push(plain(if input.text.trim().is_empty() {
            "Say what it should do and when, anywhere in the message, e.g. “every Monday at 9”."
        } else {
            "When should it run? Add it to the message (“every Monday at 9”) or pick it under When."
        })),
        Some(When::Event { event_type }) => {
            let words = describe_event_type(event_type);
            let words = words.strip_prefix("A ").map(|r| format!("a {r}")).unwrap_or(words);
            schedule.push(plain("Runs when "));
            schedule.push(plain(format!("{words}.")));
        }
        Some(when) => {
            let next = if matches!(when, When::Once { .. }) { None } else { next_run(when, input.from, tz) };
            schedule.push(plain("Runs "));
            schedule.push(strong(when_sentence(when, input.from, tz)));
            schedule.push(plain(format!(" ({} time)", input.time_zone)));
            if let Some(next) = next {
                schedule.push(plain(", next "));
                schedule.push(strong(describe_date(next, tz)));
            }
            schedule.push(plain(if matches!(when, When::Once { .. }) { ", then it switches off." } else { "." }));
            if let Some(saved) = input.saved_time_zone {
                let saved = saved.unwrap_or("UTC");
                if saved != input.time_zone {
                    schedule.push(warn(format!(" Saving moves it from {saved} time to {}.", input.time_zone)));
                }
            }
            let hit_ambiguous = |h: &ScheduleHit| matches!(&h.when, When::Weekly { ambiguous: true, .. });
            if let (When::Weekly { days, ambiguous: true, .. }, false, Some(_)) = (when, input.force_once, input.hit) {
                ask = Some(Ask {
                    question: format!("Every {}, or just once?", day_name(days[0])),
                    answer: "Just once".into(),
                    action: AskAction::JustOnce,
                });
            } else if input.force_once
                && let Some(hit) = input.hit.filter(|h| hit_ambiguous(h))
                && let When::Weekly { days, .. } = &hit.when
            {
                ask = Some(Ask {
                    question: String::new(),
                    answer: format!("Every {} instead", day_name(days[0])),
                    action: AskAction::EveryWeek,
                });
            }
        }
    }

    Plan { schedule, ask, rest: rest_of(input) }
}

fn rest_of(input: &PlanInput) -> Vec<Part> {
    let mut rest = Vec::new();
    let session = if input.same_session { "session" } else { "new session" };
    if let Some(workflow) = input.workflow {
        match (workflow, input.folder) {
            (None, _) => rest.push(plain("Pick the workflow it runs.")),
            (Some(name), Some(Folder::Repository(path))) => {
                rest.push(plain(format!("Each run: {name} in a new worktree of ")));
                rest.push(code(tilde_path(path)));
                if let Some(base) = input.base {
                    rest.push(plain(" from "));
                    rest.push(code(base));
                }
                rest.push(plain(
                    ", with your message as the request. Check with me is off; the run stops only where the workflow asks you.",
                ));
            }
            (Some(name), _) => rest.push(plain(format!("Each run: {name}. Pick the repository it runs in."))),
        }
        if input.hit.is_some() && input.prompt.is_empty() {
            rest.push(warn(" Say what it should do."));
        }
        return rest;
    }
    match input.folder {
        None => rest.push(plain("Choose where it runs.")),
        Some(Folder::None) => rest.push(plain(if input.legacy_folderless {
            format!("Each run: a {session} in your first workspace root.")
        } else {
            format!("Each run: a {session} with no folder.")
        })),
        Some(Folder::Repository(path)) if input.worktree => {
            rest.push(plain(format!("Each run: a {session} in a new worktree of ")));
            rest.push(code(tilde_path(path)));
            if let Some(base) = input.base {
                rest.push(plain(" from "));
                rest.push(code(base));
            }
            rest.push(plain("."));
        }
        Some(Folder::Repository(path) | Folder::Directory(path)) => {
            rest.push(plain(format!("Each run: a {session} in ")));
            rest.push(code(tilde_path(path)));
            rest.push(plain(" as it is."));
        }
    }
    if input.same_session {
        rest.push(plain(" Later runs continue the first run's session."));
    }
    if input.hit.is_some() || input.text.to_lowercase().contains("automation") {
        if input.prompt.is_empty() {
            rest.push(warn(" Say what it should do."));
        } else if input.prompt.chars().count() > 110 {
            rest.push(plain(format!(" The agent gets “{}…”", input.prompt.chars().take(108).collect::<String>())));
        } else {
            rest.push(plain(format!(" The agent gets “{}”", input.prompt)));
        }
    }
    rest
}
