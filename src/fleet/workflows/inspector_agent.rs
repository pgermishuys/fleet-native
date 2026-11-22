//! The inspector's agent-step form (component 18, piece Q; `WorkflowInspector.vue`): the fields of Runs as, the instructions, the
//! outcomes with their max, the declared files and the optional hint, and the edits the buttons make. Like piece P's fields they
//! are read once a frame by `Editor::sync_inspector` (`AgentFields::sync`), which hands back the draft they make.
use super::draft::{
    StepPatch, WorkflowDraft, drop_unused_max, loop_outcomes, new_outcome, remove_outcome, rename_outcome, set_route, update_step,
};
use super::editor::Editor;
use super::{Role, Step, StepKind};
use crate::State;
use rune::TextEdit;

pub const PROMPT_FIELD: &str = "workflow-step-prompt";

/// A new declared file, as Fleet's "Add a file" writes it.
pub const NEW_FILE: &str = "docs/{{slug}}.md";

/// The list open in the agent form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentMenu {
    Model,
    /// The "Leads to" of the outcome at this place.
    Route(usize),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Mirror {
    prompt: String,
    agent: String,
    skill: String,
    effort: String,
    pinned: String,
    hint: String,
    outcomes: Vec<String>,
    max: String,
    writes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct AgentFields {
    pub prompt: TextEdit,
    pub agent: TextEdit,
    pub skill: TextEdit,
    pub effort: TextEdit,
    /// The `provider/model` of a pinned model.
    pub pinned: TextEdit,
    pub hint: TextEdit,
    pub outcomes: Vec<TextEdit>,
    /// One per outcome (only a loop's is drawn); they all show the step's one max.
    pub maxes: Vec<TextEdit>,
    pub writes: Vec<TextEdit>,
    pub menu: Option<AgentMenu>,
    /// "Pinned model…" was picked, or the step's model is not a role: the `provider/model` field shows.
    pub pinning: bool,
    /// An outcome whose name Enter commits now; otherwise it is committed when the field is left.
    commit_now: Option<usize>,
    mirror: Mirror,
}

fn set(field: &mut TextEdit, words: &str) {
    if field.text != words {
        field.replace_text(words);
    }
}

fn edits(words: &[String]) -> Vec<TextEdit> {
    words.iter().map(|w| TextEdit::new(w.clone())).collect()
}

/// `Number.parseInt(value, 10)` for a max: the digits it starts with, and a number above 0 or none.
fn parse_max(words: &str) -> Option<u32> {
    let digits: String = words.trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u32>().ok().filter(|n| *n > 0)
}

/// `isRole` on a model.
pub fn is_role(model: Option<&str>) -> bool {
    Role::parse(model).is_some()
}

/// One field against its mirror, as in `inspector::two_way`.
fn two_way(field: &mut TextEdit, mirror: &mut String, from_draft: &str) -> Option<String> {
    if field.text != *mirror {
        *mirror = field.text.clone();
        return Some(field.text.clone());
    }
    if from_draft != mirror {
        *mirror = from_draft.to_string();
        set(field, from_draft);
    }
    None
}

fn some(words: &str) -> Option<String> {
    Some(words.trim().to_string()).filter(|w| !w.is_empty())
}

fn max_words(step: &Step) -> String {
    step.max_loops.map(|n| n.to_string()).unwrap_or_default()
}

impl AgentFields {
    /// The fields for a step that was just opened.
    pub fn load(&mut self, step: Option<&Step>) {
        let Some(step) = step.filter(|s| s.kind == StepKind::Agent) else {
            *self = AgentFields::default();
            return;
        };
        let model = step.model.clone().unwrap_or_default();
        self.pinning = !model.is_empty() && !is_role(Some(&model));
        self.menu = None;
        self.commit_now = None;
        self.mirror = Mirror {
            prompt: step.prompt.clone().unwrap_or_default(),
            agent: step.agent.clone().unwrap_or_default(),
            skill: step.skill.clone().unwrap_or_default(),
            effort: step.effort.clone().unwrap_or_default(),
            pinned: if is_role(Some(&model)) { String::new() } else { model },
            hint: step.optional_hint.clone().unwrap_or_default(),
            outcomes: step.outcomes.clone(),
            max: max_words(step),
            writes: step.writes.clone(),
        };
        let m = self.mirror.clone();
        for (field, words) in [
            (&mut self.prompt, &m.prompt),
            (&mut self.agent, &m.agent),
            (&mut self.skill, &m.skill),
            (&mut self.effort, &m.effort),
            (&mut self.pinned, &m.pinned),
            (&mut self.hint, &m.hint),
        ] {
            set(field, words);
        }
        self.outcomes = edits(&m.outcomes);
        self.maxes = m.outcomes.iter().map(|_| TextEdit::new(m.max.clone())).collect();
        self.writes = edits(&m.writes);
    }

    /// What the form's fields changed in step `i` of `draft`, as the draft they make (`next` is that draft so far), and the
    /// fields follow a draft that changed under them.
    pub fn sync(&mut self, draft: &WorkflowDraft, i: usize, next: &mut WorkflowDraft) {
        let step = &draft.steps[i];
        let m = &mut self.mirror;
        let patch = |next: &mut WorkflowDraft, p: StepPatch| *next = drop_unused_max(&update_step(next, i, &p), i);
        if let Some(words) = two_way(&mut self.prompt, &mut m.prompt, step.prompt.as_deref().unwrap_or("")) {
            patch(next, StepPatch { prompt: Some(Some(words)), ..StepPatch::default() });
        }
        if let Some(words) = two_way(&mut self.agent, &mut m.agent, step.agent.as_deref().unwrap_or("")) {
            patch(next, StepPatch { agent: Some(some(&words)), ..StepPatch::default() });
        }
        if let Some(words) = two_way(&mut self.skill, &mut m.skill, step.skill.as_deref().unwrap_or("")) {
            patch(next, StepPatch { skill: Some(some(&words)), ..StepPatch::default() });
        }
        if let Some(words) = two_way(&mut self.effort, &mut m.effort, step.effort.as_deref().unwrap_or("")) {
            patch(next, StepPatch { effort: Some(some(&words)), ..StepPatch::default() });
        }
        let pinned_now = step.model.clone().filter(|model| !is_role(Some(model))).unwrap_or_default();
        if let Some(words) = two_way(&mut self.pinned, &mut m.pinned, &pinned_now) {
            patch(next, StepPatch { model: Some(Some(words.trim().to_string())), ..StepPatch::default() });
        }
        if let Some(words) = two_way(&mut self.hint, &mut m.hint, step.optional_hint.as_deref().unwrap_or("")) {
            patch(next, StepPatch { optional_hint: Some(Some(words).filter(|w| !w.is_empty())), ..StepPatch::default() });
        }
        // The declared files change as they are typed; one that came or went reads them all again.
        if self.writes.len() != step.writes.len() {
            self.writes = edits(&step.writes);
            m.writes = step.writes.clone();
        } else {
            for (at, field) in self.writes.iter_mut().enumerate() {
                if let Some(words) = two_way(field, &mut m.writes[at], &step.writes[at]) {
                    let writes = next.steps[i].writes.iter().enumerate().map(|(n, w)| if n == at { words.clone() } else { w.clone() });
                    patch(next, StepPatch { writes: Some(writes.collect()), ..StepPatch::default() });
                }
            }
        }
        // The max: every loop's box shows the one number, and the one the user typed in wins.
        if let Some(words) = self.maxes.iter().map(|f| f.text.clone()).find(|w| *w != m.max) {
            m.max = words.clone();
            // The other loops' boxes show the same number now, or the next read takes them for an edit.
            for field in &mut self.maxes {
                set(field, &words);
            }
            patch(next, StepPatch { max_loops: Some(parse_max(&words)), ..StepPatch::default() });
        } else if max_words(step) != m.max {
            m.max = max_words(step);
            for field in &mut self.maxes {
                set(field, &m.max);
            }
        }
        // The outcomes' names are committed when the field is left or on Enter (`@change`): a rename carries the route.
        if step.outcomes != m.outcomes || self.outcomes.len() != step.outcomes.len() {
            self.outcomes = edits(&step.outcomes);
            self.maxes = step.outcomes.iter().map(|_| TextEdit::new(max_words(step))).collect();
            m.outcomes = step.outcomes.clone();
            m.max = max_words(step);
            self.commit_now = None;
        } else if let Some(at) = (0..self.outcomes.len()).find(|n| {
            let f = &self.outcomes[*n];
            f.text != m.outcomes[*n] && (!f.focused || self.commit_now == Some(*n))
        }) {
            self.commit_now = None;
            let to = self.outcomes[at].text.trim().to_string();
            let current = next.steps[i].clone();
            patch(next, rename_outcome(&current, &m.outcomes[at], &to));
        }
    }

    /// Where an outcome leads: the same patch as picking it in the list.
    pub fn commit_now(&mut self, at: usize) {
        self.commit_now = Some(at);
    }
}

impl Editor {
    fn agent_step(&self) -> Option<(WorkflowDraft, usize)> {
        let draft = self.draft.clone()?;
        let i = self.canvas.selected.filter(|i| draft.steps.get(*i).is_some_and(|s| s.kind == StepKind::Agent))?;
        Some((draft, i))
    }

    fn agent_patch(&mut self, patch: &StepPatch) {
        self.inspector_patch(patch);
    }

    /// `pickModel`: a role, or "Pinned model…" (which keeps a pinned model and shows its field).
    pub fn inspector_pick_model(&mut self, value: &str) {
        let Some((draft, i)) = self.agent_step() else { return };
        self.inspector.agent.menu = None;
        if value == "pinned" {
            self.inspector.agent.pinning = true;
            let model = draft.steps[i].model.clone().filter(|m| !is_role(Some(m))).unwrap_or_default();
            self.agent_patch(&StepPatch { model: Some(Some(model)), ..StepPatch::default() });
        } else {
            self.inspector.agent.pinning = false;
            self.agent_patch(&StepPatch { model: Some(Some(value.to_string())), ..StepPatch::default() });
        }
    }

    /// `pickFinisher`: who moves the step on.
    pub fn inspector_pick_finisher(&mut self, you: bool, agent: bool) {
        self.agent_patch(&StepPatch { finish_you: Some(you), finish_agent: Some(agent), ..StepPatch::default() });
    }

    /// "Always runs" and "Off unless switched on for a run".
    pub fn inspector_pick_optional(&mut self, optional: bool) {
        let patch = StepPatch { optional: Some(optional), optional_hint: if optional { None } else { Some(None) }, ..StepPatch::default() };
        self.agent_patch(&patch);
    }

    /// `pickRoute`: where an outcome leads; an empty target is the next step.
    pub fn inspector_pick_route(&mut self, at: usize, target: &str) {
        let Some((draft, i)) = self.agent_step() else { return };
        let Some(outcome) = draft.steps[i].outcomes.get(at).cloned() else { return };
        self.inspector.agent.menu = None;
        self.agent_patch(&set_route(&draft.steps[i], &outcome, Some(target).filter(|t| !t.is_empty())));
    }

    pub fn inspector_add_outcome(&mut self) {
        let Some((draft, i)) = self.agent_step() else { return };
        let step = &draft.steps[i];
        let mut outcomes = step.outcomes.clone();
        outcomes.push(new_outcome(step));
        self.agent_patch(&StepPatch { outcomes: Some(outcomes), ..StepPatch::default() });
    }

    pub fn inspector_remove_outcome(&mut self, at: usize) {
        let Some((draft, i)) = self.agent_step() else { return };
        let Some(outcome) = draft.steps[i].outcomes.get(at).cloned() else { return };
        self.inspector.agent.menu = None;
        self.agent_patch(&remove_outcome(&draft.steps[i], &outcome));
    }

    pub fn inspector_add_write(&mut self) {
        let Some((draft, i)) = self.agent_step() else { return };
        let mut writes = draft.steps[i].writes.clone();
        writes.push(NEW_FILE.to_string());
        self.agent_patch(&StepPatch { writes: Some(writes), ..StepPatch::default() });
    }

    pub fn inspector_remove_write(&mut self, at: usize) {
        let Some((draft, i)) = self.agent_step() else { return };
        let writes = draft.steps[i].writes.iter().enumerate().filter(|(n, _)| *n != at).map(|(_, w)| w.clone()).collect();
        self.agent_patch(&StepPatch { writes: Some(writes), ..StepPatch::default() });
    }

    /// `insertVariable`: `{{variable}}` goes in where the caret is, or over what is selected. The field was not typed in, so the
    /// next frame's read takes it into the draft.
    pub fn inspector_insert_variable(&mut self, variable: &str) {
        if self.agent_step().is_none() {
            return;
        }
        self.inspector.agent.prompt.insert(&format!("{{{{{variable}}}}}"));
    }

    /// The outcomes of the open step that are loops.
    pub fn inspector_loops(&self) -> Vec<String> {
        self.agent_step().map(|(draft, i)| loop_outcomes(&draft, i)).unwrap_or_default()
    }
}

impl State {
    pub fn workflow_agent_menu(&mut self, menu: Option<AgentMenu>) {
        self.workflows_nav.editor.inspector.agent.menu = menu;
    }

    pub fn workflow_outcome_commit(&mut self, at: usize) {
        self.workflows_nav.editor.inspector.agent.commit_now(at);
    }
}
