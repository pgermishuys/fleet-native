//! What the finish bar above the composer holds and does (`WorkflowFinishBar.vue`): the run's step the user finishes in the open
//! session, the note for the next step, the press on its way and the words of a refusal. The bar reads the machine the session
//! is on, as the stepper and the card do.
use super::helpers::move_blocked_reason;
use super::{Run, RunStatus, WithYou};
use crate::State;
use rune::{TextEdit, Tx};

/// The key of the note's field.
pub const NOTE_KEY: &str = "workflow-finish-note";

/// What the bar holds for the step it is drawn for. Another step (or another session) starts it again, as the component's
/// `watch` on the step id does; `owner` says which step this is.
#[derive(Debug, Default)]
pub struct Finish {
    owner: Option<(String, String)>,
    pub note: TextEdit,
    /// The outcome of the button pressed while its request is on the way, or `end`.
    pub busy: Option<String>,
    pub error: Option<String>,
}

impl Finish {
    /// A note being written, as scenes show it.
    pub fn set_note(&mut self, words: &str) {
        self.note = TextEdit::new(words);
    }
}

/// Everything the bar says that doesn't depend on the state: `single`, the next step's title, the subtitle and the hand-off.
pub fn single(with_you: &WithYou) -> bool {
    with_you.moves.len() == 1
}

pub fn next_title(with_you: &WithYou) -> String {
    if single(with_you) {
        with_you.moves[0].to_title.clone().unwrap_or_else(|| "the end of the run".into())
    } else {
        "the next step".into()
    }
}

pub fn subtitle(with_you: &WithYou, declared: bool) -> &'static str {
    if !single(with_you) {
        "Pick the outcome. The run goes where it leads."
    } else if declared {
        "The agent can't end this step. Keep talking until it's right."
    } else {
        "You chose to check each step. Keep talking, or move on."
    }
}

pub fn hand_off(with_you: &WithYou) -> String {
    let next = next_title(with_you);
    if with_you.files.is_empty() {
        format!("Move on asks the agent, in this session, to write a short summary. Then {next} starts with it and your note.")
    } else {
        format!(
            "Move on asks the agent, in this session, to bring these files up to date with what you agreed and to write a short summary. Then {next} starts with the files, the summary and your note."
        )
    }
}

/// While the wrap-up runs: where it's going (`wrapping`).
pub fn wrapping(with_you: &WithYou) -> Option<String> {
    if !with_you.wrapping_up {
        return None;
    }
    let next = with_you
        .moves
        .iter()
        .find(|m| Some(&m.outcome) == with_you.outcome.as_ref())
        .and_then(|m| m.to_title.clone())
        .unwrap_or_else(|| "the end of the run".into());
    let title = &with_you.step_title;
    Some(if with_you.files.is_empty() {
        format!("Wrapping up {title}: the agent is writing a summary for {next}. Then Fleet starts {next}.")
    } else {
        format!(
            "Wrapping up {title}: the agent is updating its files and writing a summary for {next}. Then Fleet checks the files and starts {next}."
        )
    })
}

/// The first reason a way on is off (`blocked`).
pub fn blocked(with_you: &WithYou) -> Option<String> {
    with_you.moves.iter().find_map(|m| move_blocked_reason(&with_you.step_title, m))
}

/// The step the run is at is the user's here: running, and `withYou` names this session.
fn finish_of<'a>(run: &'a Run, session: &str) -> Option<&'a WithYou> {
    if run.status != RunStatus::Running {
        return None;
    }
    run.with_you.as_ref().filter(|w| w.session_id == session)
}

impl State {
    /// `withYou` for the open session.
    pub fn finish_with_you(&self) -> Option<(&Run, &WithYou)> {
        let run = self.stepper_run()?;
        Some((run, finish_of(run, &self.session().id)?))
    }

    /// Whether the workflow file says `finish: you` for the step (`declared`).
    pub fn finish_declared(&self) -> bool {
        self.finish_with_you().and_then(|(run, w)| run.steps.iter().find(|s| s.id == w.step_id)).is_some_and(|s| s.finish_you)
    }

    /// Called from `poll`: another step starts the bar again. True when its state was dropped.
    pub fn keep_workflow_finish(&mut self) -> bool {
        let owner = self.finish_with_you().map(|(_, w)| (self.session().id.clone(), w.step_id.clone()));
        if self.workflows_nav.finish.owner == owner {
            return false;
        }
        self.workflows_nav.finish = Finish { owner, ..Finish::default() };
        true
    }

    /// `moveOn` for the way on at `index`: a single way sends no outcome.
    pub fn finish_move_on(&mut self, index: usize, tx: &mut Tx) {
        let Some((run, with_you)) = self.finish_with_you() else { return };
        let Some(mv) = with_you.moves.get(index).filter(|m| m.allowed) else { return };
        if self.workflows_nav.finish.busy.is_some() {
            return;
        }
        let outcome = (!single(with_you)).then(|| mv.outcome.clone());
        let busy = mv.outcome.clone();
        let run_id = run.id.clone();
        let owner = self.workflows_nav.finish.owner.clone();
        let machine = self.machine_key_of(&self.session().id.clone()).to_string();
        let finish = &mut self.workflows_nav.finish;
        let note = Some(finish.note.text.trim().to_string()).filter(|n| !n.is_empty());
        finish.busy = Some(busy);
        finish.error = None;
        self.workflow_move_on(
            &machine,
            &run_id,
            outcome.as_deref(),
            note.as_deref(),
            tx,
            Box::new(move |s, _, result| {
                let finish = &mut s.workflows_nav.finish;
                if finish.owner != owner {
                    return;
                }
                finish.busy = None;
                match result {
                    Ok(_) => finish.note = TextEdit::new(""),
                    Err(words) => finish.error = Some(words),
                }
            }),
        );
    }

    /// `endRun`, from the link under a spent loop.
    pub fn finish_end_run(&mut self, tx: &mut Tx) {
        let Some((run, _)) = self.finish_with_you() else { return };
        if self.workflows_nav.finish.busy.is_some() {
            return;
        }
        let run_id = run.id.clone();
        let owner = self.workflows_nav.finish.owner.clone();
        let machine = self.machine_key_of(&self.session().id.clone()).to_string();
        self.workflows_nav.finish.busy = Some("end".into());
        self.workflows_nav.finish.error = None;
        self.workflow_end(
            &machine,
            &run_id,
            tx,
            Box::new(move |s, _, result| {
                let finish = &mut s.workflows_nav.finish;
                if finish.owner != owner {
                    return;
                }
                finish.busy = None;
                if let Err(words) = result {
                    finish.error = Some(words);
                }
            }),
        );
    }
}
