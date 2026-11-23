//! What the run card above the composer holds and does (`WorkflowRunCard.vue`): the wait the open session is the place of, the
//! note a "Send back" choice takes, the answer on its way, the words of a refusal, the plan that opens beside a You step (once
//! per run, step and visit) and the line that says where the run went once this session's step is over. The card reads the
//! machine the session is on, as the stepper does.
use super::helpers::{file_name, list_files};
use super::runs::Choice;
use super::{MOVE_ON_ANYWAY, Run, RunChoice, RunSession, RunStatus, WITHOUT_SKILL, Waiting, WaitingKind};
use crate::State;
use crate::fleet::canvases::OpenFile;
use rune::{TextEdit, Tx};
use std::collections::HashSet;

/// The key of the note's field, which a "Send back" choice focuses.
pub const NOTE_KEY: &str = "workflow-card-note";

/// What the card holds for the wait it is drawn for. A new wait (another step, or another session) starts it again, as the
/// component's `watch` on the step id does; `owner` says which wait this is.
#[derive(Debug, Default)]
pub struct Card {
    owner: Option<(String, String)>,
    /// The choice whose note is being written.
    pub note_for: Option<String>,
    pub note: TextEdit,
    /// The button pressed while its answer is on the way: a choice's id, `<id>:<check with me>` for the two ways, or `end`.
    pub busy: Option<String>,
    pub error: Option<String>,
    /// The note's field has to be drawn before it can take the keyboard: 1 asked, 2 drawn once.
    focus_note: u8,
    /// `run:step:visit` of the waits whose files were opened already.
    opened: HashSet<String>,
}

impl Card {
    /// A note being written for `choice`, as scenes show it.
    pub fn set_note(&mut self, choice: &str, words: &str) {
        self.note_for = Some(choice.to_string());
        self.note = TextEdit::new(words);
    }
}

/// The words when the server gave none.
pub const ANSWER_FALLBACK: &str = "Couldn't answer the run.";
pub const END_FALLBACK: &str = "Couldn't end the run.";

/// The line under a You step's question once the step before it declared files.
const INVOLVEMENT: &str = " Then choose how involved you want to be in the rest of the run; you can change it later from the header.";

/// "Send back to Plan", or "Implement" for a choice leading there (`stepTitle`).
pub fn step_title(run: &Run, id: Option<&str>) -> String {
    match id {
        None | Some("end") | Some("") => "the end".to_string(),
        Some(id) => run.steps.iter().find(|s| s.id == id).map(|s| s.title.clone()).unwrap_or_else(|| id.to_string()),
    }
}

/// What the card asks (`ask`): the server's message, or at a You step its question, the files open beside it and, when a
/// choice asks how involved to be, the line that says so.
pub fn ask(waiting: &Waiting) -> String {
    if waiting.kind != WaitingKind::You {
        return waiting.message.clone();
    }
    let names: Vec<String> = waiting.files.iter().map(|f| file_name(f).to_string()).collect();
    let open = match names.len() {
        0 => String::new(),
        n => format!(" {} {} open on the right.", list_files(&names), if n == 1 { "is" } else { "are" }),
    };
    let involvement = if waiting.choices.iter().any(|c| c.involvement) { INVOLVEMENT } else { "" };
    format!("{}{open}{involvement}", waiting.question.as_deref().unwrap_or(""))
}

/// What "let it run" means here (`letItRun`).
pub fn let_it_run(waiting: &Waiting) -> String {
    let alone = if waiting.then_alone.is_empty() {
        "The next steps go ahead on their own.".to_string()
    } else {
        format!("{} go ahead on their own.", list_files(&waiting.then_alone))
    };
    match waiting.next_you_title.as_deref().filter(|t| !t.is_empty()) {
        Some(next) => format!("{alone} You're asked again at {next}."),
        None => alone,
    }
}

/// `describe`: the title of a choice's button, none for a wait that says it in its message.
pub fn describe(run: &Run, waiting: &Waiting, choice: &RunChoice) -> Option<String> {
    if matches!(waiting.kind, WaitingKind::StartFailed | WaitingKind::SkillOff) {
        return None;
    }
    let to = choice.to.as_deref().filter(|t| !t.is_empty())?;
    Some(if to == "end" { "Ends the run".to_string() } else { format!("Starts {}", step_title(run, Some(to))) })
}

/// The label of a choice's button: as given for a You step and for the three the server names, else "Continue as …".
pub fn choice_label(waiting: &Waiting, choice: &RunChoice) -> String {
    let own = waiting.kind == WaitingKind::You || choice.id == "retry" || choice.id == MOVE_ON_ANYWAY || choice.id == WITHOUT_SKILL;
    if own { choice.label.clone() } else { format!("Continue as {}", choice.label) }
}

/// Where the run went, after this session's step (`movedOn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovedOn {
    pub text: String,
    /// The step the run is at and the session to open for it.
    pub next: Option<(String, String)>,
    pub files: Vec<String>,
    pub commit: Option<String>,
    pub commit_error: Option<String>,
}

/// `movedOn` for `session`. Nothing while the run waits here, or before this session's step has an outcome.
pub fn moved_on(run: &Run, session: &str) -> Option<MovedOn> {
    if is_waiting_here(run, session) {
        return None;
    }
    let mine = run.sessions.iter().find(|s| s.session_id == session)?;
    let outcome = mine.outcome.as_deref().filter(|o| !o.is_empty())?;
    let title = run.steps.iter().find(|s| s.id == mine.step_id).map(|s| s.title.clone()).unwrap_or_else(|| mine.step_id.clone());
    let made = |text: String, next: Option<(String, String)>| {
        let (files, commit, commit_error) = checked_files(mine);
        MovedOn { text, next, files, commit, commit_error }
    };
    if let Some(later) = run.sessions.last().filter(|l| l.session_id != session) {
        return Some(made(
            format!("{title} finished: {outcome}."),
            Some((step_title(run, Some(&later.step_id)), later.session_id.clone())),
        ));
    }
    if matches!(run.status, RunStatus::Done | RunStatus::Ended) {
        let result = run.result.as_deref().filter(|r| !r.is_empty()).unwrap_or("The run is done.");
        return Some(made(format!("{title} finished: {outcome}. {result}"), None));
    }
    if run.status == RunStatus::Waiting
        && let Some(waiting) = &run.waiting
        && let Some(id) = waiting.session_id.as_deref().filter(|s| !s.is_empty())
    {
        return Some(made(format!("{title} finished: {outcome}."), Some((waiting.step_title.clone(), id.to_string()))));
    }
    None
}

/// The files Fleet found before the next step started, and what came of committing them (`checkedFiles`).
fn checked_files(visit: &RunSession) -> (Vec<String>, Option<String>, Option<String>) {
    if !visit.files_checked {
        return (Vec::new(), None, None);
    }
    (visit.files.clone(), visit.files_commit.clone(), visit.files_commit_error.clone())
}

/// "Files checked" or "Files checked and committed as a1b2c3d", then the names.
pub fn checked_text(moved: &MovedOn) -> String {
    let names: Vec<String> = moved.files.iter().map(|f| file_name(f).to_string()).collect();
    let head = match &moved.commit {
        Some(sha) => format!("Files checked and committed as {sha}"),
        None => "Files checked".to_string(),
    };
    format!("{head}: {}", list_files(&names))
}

/// "Fleet couldn't commit it: …" / "them".
pub fn commit_failed_text(moved: &MovedOn, why: &str) -> String {
    format!("Fleet couldn't commit {}: {why}", if moved.files.len() == 1 { "it" } else { "them" })
}

/// The run waits on the user in `session`.
fn is_waiting_here(run: &Run, session: &str) -> bool {
    run.status == RunStatus::Waiting && run.waiting.as_ref().is_some_and(|w| w.session_id.as_deref() == Some(session))
}

impl State {
    /// The wait the open session is the place of (`waiting`).
    pub fn card_waiting(&self) -> Option<(&Run, &Waiting)> {
        let run = self.stepper_run()?;
        let session = self.session().id.as_str();
        if !is_waiting_here(run, session) {
            return None;
        }
        Some((run, run.waiting.as_ref()?))
    }

    /// Where the run went, for the open session.
    pub fn card_moved_on(&self) -> Option<MovedOn> {
        moved_on(self.stepper_run()?, &self.session().id)
    }

    /// Called from `poll`: the card starts again for a new wait, and the plan beside a You step opens. True when the card's
    /// state was dropped (so what is drawn changes).
    pub fn keep_workflow_card(&mut self, tx: &mut Tx) -> bool {
        let owner = self.card_waiting().map(|(_, w)| (self.session().id.clone(), w.step_id.clone()));
        let mut changed = false;
        if self.workflows_nav.card.owner != owner {
            let opened = std::mem::take(&mut self.workflows_nav.card.opened);
            self.workflows_nav.card = Card { owner, opened, ..Card::default() };
            changed = true;
        }
        self.open_card_files(tx);
        match self.workflows_nav.card.focus_note {
            1 => {
                self.workflows_nav.card.focus_note = 2;
                changed = true;
            }
            2 => {
                self.workflows_nav.card.focus_note = 0;
                tx.focus(NOTE_KEY, false);
            }
            _ => {}
        }
        changed
    }

    /// "Approve the plan" with the plan open next to it: the files the step before a You step declares open as kept tabs and
    /// the right panel opens (unless it is a sheet), once per run, step and visit. Waits for the sheet to be measured, so
    /// "is it a sheet" is known.
    fn open_card_files(&mut self, tx: &mut Tx) {
        if self.rail != crate::Rail::Sessions || self.sheet_w <= 0.0 {
            return;
        }
        let Some((run, waiting)) = self.card_waiting() else { return };
        if waiting.kind != WaitingKind::You || waiting.files.is_empty() {
            return;
        }
        let visits = run.steps.iter().find(|s| s.id == waiting.step_id).map_or(0, |s| s.visits);
        let key = format!("{}:{}:{visits}", run.id, waiting.step_id);
        let files = waiting.files.clone();
        if !self.workflows_nav.card.opened.insert(key) {
            return;
        }
        let session = self.session().id.clone();
        for path in files {
            self.open_file(&session, &path, OpenFile { keep: true, view: None }, tx);
        }
        if crate::fleet::right_panel::fits(self.sheet_w) {
            self.set_content_open(true);
        }
    }

    /// `choose`: a choice with a note asks for the note first; any other is sent, as the pressed button's `way` says (the two
    /// ways to go on say whether to check with the user after each step).
    pub fn card_choose(&mut self, index: usize, check_with_me: Option<bool>, tx: &mut Tx) {
        let Some((run, waiting)) = self.card_waiting() else { return };
        let Some(choice) = waiting.choices.get(index).cloned() else { return };
        let (run_id, owner) = (run.id.clone(), self.workflows_nav.card.owner.clone());
        let session = self.session().id.clone();
        let machine = self.machine_key_of(&session).to_string();
        let card = &mut self.workflows_nav.card;
        if choice.note && card.note_for.as_deref() != Some(&choice.id) {
            card.note_for = Some(choice.id.clone());
            card.focus_note = 1;
            return;
        }
        if card.busy.is_some() {
            return;
        }
        card.busy = Some(match check_with_me {
            None => choice.id.clone(),
            Some(on) => format!("{}:{on}", choice.id),
        });
        card.error = None;
        let note = choice.note.then(|| card.note.text.trim().to_string());
        self.workflow_answer(
            &machine,
            &run_id,
            &Choice { id: choice.id, note, check_with_me },
            tx,
            Box::new(move |s, _, result| {
                let card = &mut s.workflows_nav.card;
                if card.owner != owner {
                    return;
                }
                card.busy = None;
                match result {
                    Ok(_) => {
                        card.note_for = None;
                        card.note = TextEdit::new("");
                    }
                    Err(words) => card.error = Some(words),
                }
            }),
        );
    }

    /// The note's Send back button: the choice whose note it is.
    pub fn card_send_note(&mut self, tx: &mut Tx) {
        let Some(id) = self.workflows_nav.card.note_for.clone() else { return };
        if self.workflows_nav.card.note.text.trim().is_empty() {
            return;
        }
        let Some(index) = self.card_waiting().and_then(|(_, w)| w.choices.iter().position(|c| c.id == id)) else { return };
        self.card_choose(index, None, tx);
    }

    pub fn card_cancel_note(&mut self) {
        if self.workflows_nav.card.busy.is_none() {
            self.workflows_nav.card.note_for = None;
        }
    }

    /// `endRun`.
    pub fn card_end_run(&mut self, tx: &mut Tx) {
        let Some((run, _)) = self.card_waiting() else { return };
        let run_id = run.id.clone();
        let owner = self.workflows_nav.card.owner.clone();
        let machine = self.machine_key_of(&self.session().id.clone()).to_string();
        let card = &mut self.workflows_nav.card;
        if card.busy.is_some() {
            return;
        }
        card.busy = Some("end".into());
        card.error = None;
        self.workflow_end(
            &machine,
            &run_id,
            tx,
            Box::new(move |s, _, result| {
                let card = &mut s.workflows_nav.card;
                if card.owner != owner {
                    return;
                }
                card.busy = None;
                if let Err(words) = result {
                    card.error = Some(words);
                }
            }),
        );
    }

    /// The link in a skill-off wait: Settings → Skills & tools.
    pub fn card_open_skills(&mut self) {
        self.open_settings("skills", None);
    }

    /// The link in the resolved line: the session the run is in now, on the machine this one is on.
    pub fn card_open_next(&mut self, session: &str, tx: &mut Tx) {
        let machine = self.machine_key_of(&self.session().id.clone()).to_string();
        self.open_run_session(&machine, session, tx);
    }
}
