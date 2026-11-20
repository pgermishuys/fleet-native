//! What the views share about workflows and their runs (`lib/workflows.ts`): names, counts, notes and the grouping of a
//! run's step sessions in the Sessions list. Plain Rust.
use super::{Move, Run, RunStatus, RunStep, StepKind, StepState, Workflow};
use std::collections::HashSet;

/// How a run's status shows next to its title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Wait,
    Run,
    With,
    Done,
    Fail,
}

/// `runStatusLabel`: what the Sessions list says about a run. A step the user finishes is With you, not Needs you.
pub fn run_status_label(run: &Run) -> (String, Tone) {
    let result = |fallback: &str| run.result.clone().unwrap_or_else(|| fallback.to_string());
    match run.status {
        RunStatus::Waiting => ("Needs you".into(), Tone::Wait),
        RunStatus::Running if run.with_you.is_some() => ("With you".into(), Tone::With),
        RunStatus::Running => ("Working".into(), Tone::Run),
        RunStatus::Failed => (result("Failed"), Tone::Fail),
        RunStatus::Ended => ("Ended".into(), Tone::Done),
        RunStatus::Done => (result("Done"), Tone::Done),
    }
}

/// `ordinal`: 1st, 2nd, 3rd, 4th, 11th, 12th, 13th, 21st.
pub fn ordinal(n: u32) -> String {
    let suffix = match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

/// `stepLabel`: a step's name in a run, with its pass when it ran more than once ("Implement · 2nd pass"). None for a
/// session the run doesn't hold.
pub fn step_label(run: &Run, session_id: &str) -> Option<String> {
    let session = run.sessions.iter().find(|s| s.session_id == session_id)?;
    let title = run.steps.iter().find(|s| s.id == session.step_id).map_or(session.step_id.as_str(), |s| s.title.as_str());
    Some(if session.visit > 1 { format!("{title} \u{b7} {} pass", ordinal(session.visit)) } else { title.to_string() })
}

/// `stepPosition`: the step number of a session's step among the steps that run in this run ("step 2 of 6"), from 1.
pub fn step_position(run: &Run, session_id: &str) -> Option<(usize, usize)> {
    let session = run.sessions.iter().find(|s| s.session_id == session_id)?;
    let running: Vec<&RunStep> = run.steps.iter().filter(|s| s.enabled).collect();
    let at = running.iter().position(|s| s.id == session.step_id)?;
    Some((at + 1, running.len()))
}

/// `modelShortName`: "opus-5.5" from "github-copilot/opus-5.5" when the catalog has no name for it.
pub fn model_short_name(model: Option<&str>) -> String {
    match model {
        None | Some("") => "Default model".into(),
        Some(m) => m.split_once('/').map_or(m, |(_, rest)| rest).to_string(),
    }
}

/// `skillsInUse`: the skills the enabled steps of a workflow use, in step order, for a run with these optional steps on.
/// Each is the step's id and the skill.
pub fn skills_in_use<'a>(workflow: &'a Workflow, optional_on: &HashSet<String>) -> Vec<(&'a str, &'a str)> {
    workflow
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::Agent && (!s.optional || optional_on.contains(&s.id)))
        .filter_map(|s| s.skill.as_deref().filter(|k| !k.is_empty()).map(|k| (s.id.as_str(), k)))
        .collect()
}

/// `loopNotes`: the loops of a workflow as the steps strip says them: "Review → Implement on changes, at most 2×".
pub fn loop_notes(workflow: &Workflow) -> Vec<String> {
    let mut notes = Vec::new();
    for (from, step) in workflow.steps.iter().enumerate() {
        for (outcome, target) in &step.routes {
            let Some(to) = workflow.steps.iter().position(|s| &s.id == target) else { continue };
            // Forward routes are not loops; a step with no maximum says nothing.
            let Some(max) = step.max_loops.filter(|_| to <= from) else { continue };
            notes.push(format!("{} \u{2192} {} on {outcome}, at most {max}\u{d7}", step.title, workflow.steps[to].title));
        }
    }
    notes
}

/// `isStepPrompt`: the prompt a step started with carries Fleet's footer.
pub fn is_step_prompt(text: &str) -> bool {
    text.contains(super::STEP_FOOTER_START)
}

/// `workflowMessageLabel`: what a user-side message in a step session is, when Fleet sent it: "Workflow · step 1 of 7 ·
/// you finish this step" on the prompt the step started with, "Fleet · you pressed Move on" on the wrap-up. A step the user
/// finishes has no footer, so its prompt is found by its id.
pub fn workflow_message_label(run: Option<&Run>, session_id: &str, message_id: &str, text: &str) -> Option<String> {
    let run = run?;
    let session = run.sessions.iter().find(|s| s.session_id == session_id)?;
    if session.wrap_up_message_id.as_deref().is_some_and(|w| !w.is_empty() && w == message_id) {
        return Some("Fleet \u{b7} you pressed Move on".into());
    }
    if session.prompt_message_id.as_deref() == Some(message_id) || is_step_prompt(text) {
        let (index, total) = step_position(run, session_id)?;
        let with_you = if session.with_you { " \u{b7} you finish this step" } else { "" };
        return Some(format!("Workflow \u{b7} step {index} of {total}{with_you}"));
    }
    None
}

/// `workflowMessageKey`: what the label reads from a run for one session, as a key that changes only when a label would.
pub fn workflow_message_key(run: Option<&Run>, session_id: &str) -> Option<String> {
    let run = run?;
    let session = run.sessions.iter().find(|s| s.session_id == session_id)?;
    let position = step_position(run, session_id).map_or("undefined/undefined".to_string(), |(i, n)| format!("{i}/{n}"));
    let id = |v: &Option<String>| v.clone().unwrap_or_else(|| "null".into());
    Some(format!("{position}|{}|{}|{}", id(&session.prompt_message_id), id(&session.wrap_up_message_id), session.with_you))
}

/// `fileName`: the last part of a path: "sheet.md" from "docs/design/sheet.md".
pub fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `listFiles`: "a.md", "a.md and b.html", "a.md, b.html and c.css".
pub fn list_files(files: &[String]) -> String {
    match files {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// `moveLabel`: what a way to move on says: "Move on to Plan", "Pass: on to Open the pull request", "Changes: back to
/// Implement (1 of 2)".
pub fn move_label(mv: &Move, single: bool) -> String {
    let where_ = mv.to_title.as_deref().unwrap_or("the end of the run");
    if single {
        return mv.to_title.as_ref().map_or("Finish the run".into(), |t| format!("Move on to {t}"));
    }
    let mut outcome = mv.outcome.chars();
    let outcome = outcome.next().map(|c| c.to_uppercase().chain(outcome).collect::<String>()).unwrap_or_default();
    if !mv.back {
        return format!("{outcome}: on to {where_}");
    }
    let count = match (mv.max_loops, mv.loops_used) {
        (Some(max), Some(used)) if mv.allowed => format!(" ({} of {max})", used + 1),
        (Some(max), Some(_)) => format!(" ({max} of {max} used)"),
        _ => String::new(),
    };
    format!("{outcome}: back to {where_}{count}")
}

/// `moveBlockedReason`: why a way to move on is off: its loop's maximum is used up.
pub fn move_blocked_reason(step_title: &str, mv: &Move) -> Option<String> {
    let max = mv.max_loops.filter(|_| !mv.allowed)?;
    let times = match max {
        1 => "once".to_string(),
        2 => "twice".to_string(),
        n => format!("{n} times"),
    };
    Some(format!("{step_title} has sent the work back {times}, the most this run allows."))
}

/// `runningStep`: the step the run is on, when it's an agent step that's started and not finished.
pub fn running_step(run: &Run) -> Option<&RunStep> {
    if !matches!(run.status, RunStatus::Running | RunStatus::Waiting) {
        return None;
    }
    let step = run.steps.iter().find(|s| Some(&s.id) == run.current_step_id.as_ref())?;
    (step.kind == StepKind::Agent && matches!(step.state, StepState::Running | StepState::Waiting)).then_some(step)
}

/// `nextStepTitle`: the title of the first enabled step after the one the run is on, for "Check with me is on from Review".
pub fn next_step_title(run: &Run) -> Option<&str> {
    let enabled: Vec<&RunStep> = run.steps.iter().filter(|s| s.enabled).collect();
    let at = enabled.iter().position(|s| Some(&s.id) == run.current_step_id.as_ref())?;
    enabled.get(at + 1).map(|s| s.title.as_str())
}

/// `checkWithMeNote`: the note under the header's switch when it differs from how the running step started: a change
/// applies from the next step, so the running one keeps the way it started.
pub fn check_with_me_note(run: &Run) -> Option<String> {
    let step = running_step(run)?;
    // A step whose file says who finishes it is that whatever the switch says.
    if step.finish_you || step.finish_agent || run.check_with_me == step.with_you {
        return None;
    }
    let next = next_step_title(run).unwrap_or("the next step");
    Some(if run.check_with_me {
        format!("Check with me is on from {next}. This step started on its own, so it still moves on when the agent reports it's done.")
    } else {
        format!("Check with me is off from {next}. You still finish this step, because it started with you.")
    })
}

// ─── The Sessions list ──────────────────────────────────────────────────────────────────────────────────────────

/// A row of a project's session list the grouping reads (`groupRunSessions`' `T`).
pub trait RunMember {
    fn id(&self) -> &str;
    fn title(&self) -> Option<&str>;
    fn workflow_run_id(&self) -> Option<&str>;
}

/// A row of a project's session list: a session, or a workflow run with its step sessions under it (`SessionListEntry`).
#[derive(Debug, PartialEq)]
pub enum ListEntry<'a, T> {
    Session(&'a T),
    Run { run_id: String, run: Option<&'a Run>, steps: Vec<(&'a T, String)> },
}

/// `groupRunSessions`: groups a run's step sessions under one row, where the run's newest session would have been, with
/// its steps in the order they ran. Sessions of a run the store doesn't know yet still group, under a plain header.
pub fn group_run_sessions<'a, T: RunMember>(sessions: &'a [T], runs: impl Fn(&str) -> Option<&'a Run>) -> Vec<ListEntry<'a, T>> {
    let run_id_of = |s: &T| s.workflow_run_id().map(str::to_string).or_else(|| runs(s.id()).map(|r| r.id.clone()));
    let mut entries = Vec::new();
    let mut grouped = HashSet::new();
    for session in sessions {
        let Some(run_id) = run_id_of(session) else {
            entries.push(ListEntry::Session(session));
            continue;
        };
        if !grouped.insert(run_id.clone()) {
            continue;
        }
        let run = runs(session.id());
        let mut members: Vec<&T> = sessions.iter().filter(|s| run_id_of(s).as_deref() == Some(run_id.as_str())).collect();
        let order = |s: &T| run.and_then(|r| r.sessions.iter().position(|x| x.session_id == s.id())).unwrap_or(usize::MAX);
        members.sort_by_key(|s| order(s));
        let steps = members
            .into_iter()
            .map(|s| (s, run.and_then(|r| step_label(r, s.id())).unwrap_or_else(|| step_title_from_session_title(s.title()))))
            .collect();
        entries.push(ListEntry::Run { run_id, run, steps });
    }
    entries
}

/// "Plan" from a step session's title, "<run title> · Plan", when the run isn't loaded.
fn step_title_from_session_title(title: Option<&str>) -> String {
    let text = title.unwrap_or_default();
    match text.rfind(" \u{b7} ") {
        Some(at) => text[at + " \u{b7} ".len()..].to_string(),
        None if text.is_empty() => "Step".into(),
        None => text.to_string(),
    }
}
