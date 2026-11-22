//! The designer's canvas for the scenes (component 18, piece O): Fix a bug as a repository's copy, with its two loops, in the
//! states the canvas has: nothing picked, a step picked, a loop with no max and its parser error, and a + menu open.
use super::workflows::{REPOSITORY, Scene};
use crate::State;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::draft::{WorkflowCheck, WorkflowDraft, WorkflowFile, WorkflowProblem};
use crate::fleet::workflows::{Choice, Step, StepKind, Workflow};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// A step picked: its node has the accent ring.
    Picked,
    /// Nothing picked, and the loop of Check it runs has no max: a red arc "needs a max".
    Loops,
    /// The + after Review asking what to add.
    Add,
    /// The parser's error on Review's node and in the strip, with the settings box picked.
    Error,
}

pub const ID: &str = "repo:fix-a-bug-our-way";

fn agent(id: &str, title: &str, model: &str, skill: Option<&str>) -> Step {
    Step {
        id: id.into(),
        title: title.into(),
        kind: StepKind::Agent,
        agent: Some("build".into()),
        model: Some(model.into()),
        skill: skill.map(str::to_string),
        prompt: Some("The bug: {{request}}".into()),
        ..Step::default()
    }
}

fn you(id: &str, title: &str, ask: &str, choices: &[(&str, &str, bool)]) -> Step {
    Step {
        id: id.into(),
        title: title.into(),
        kind: StepKind::You,
        ask: Some(ask.into()),
        choices: choices.iter().map(|(label, to, note)| Choice { label: (*label).into(), to: (*to).into(), note: *note }).collect(),
        ..Step::default()
    }
}

/// What duplicating Fix a bug into the repository makes.
pub fn draft() -> WorkflowDraft {
    let route = |step: Step, outcomes: &[&str], routes: &[(&str, &str)], max: Option<u32>| Step {
        outcomes: outcomes.iter().map(|o| (*o).to_string()).collect(),
        routes: routes.iter().map(|(o, to)| ((*o).to_string(), (*to).to_string())).collect(),
        max_loops: max,
        ..step
    };
    let investigate =
        Step { writes: vec![".weave/bugs/{{slug}}.md".into()], ..agent("investigate", "Find the cause", "strong", Some("fleet-debug")) };
    let verify = Step { optional: true, ..agent("verify", "Check it runs", "standard", Some("fleet-run")) };
    let open_pr = Step { finish_agent: true, ..agent("open-pr", "Push and open the PR", "fast", None) };
    WorkflowDraft {
        name: "Fix a bug our way".into(),
        description: Some("Finds the root cause before any code changes, and stops once for you to agree with it.".into()),
        placeholder: None,
        steps: vec![
            route(investigate, &["found", "failed"], &[("failed", "end")], None),
            you(
                "ok-cause",
                "Agree on the cause",
                "Fix it this way?",
                &[("Approve", "fix", false), ("Send back with a note", "investigate", true), ("Stop here", "end", false)],
            ),
            route(agent("fix", "Fix", "standard", None), &["done"], &[], None),
            route(agent("review", "Review", "strong", Some("fleet-code-review")), &["pass", "changes"], &[("changes", "fix")], Some(2)),
            route(verify, &["fixed", "broken"], &[("broken", "fix")], Some(2)),
            you(
                "ok-pr",
                "Open the pull request",
                "Push the branch and open a PR?",
                &[("Open PR", "open-pr", false), ("Keep the branch only", "end", false)],
            ),
            route(open_pr, &["opened", "failed"], &[("failed", "end")], None),
        ],
    }
}

impl State {
    pub fn show_workflow_canvas(&mut self, dark: bool, case: Case) {
        self.show_workflows(dark, Scene::Library);
        let mut draft = draft();
        if case == Case::Loops || case == Case::Error {
            let i = if case == Case::Loops { 4 } else { 3 };
            draft.steps[i].max_loops = None;
        }
        if let Some(lib) = self.workflows_nav.library.as_mut() {
            lib.workflows.push(Workflow {
                id: ID.into(),
                file: Some(".weave/workflows/fix-a-bug-our-way.yaml".into()),
                name: "Fix a bug our way".into(),
                description: draft.description.clone(),
                steps: draft.steps.clone(),
                ..Workflow::default()
            });
        }
        self.workflows_nav.move_to(ID);
        let errors = if case == Case::Error {
            vec![WorkflowProblem {
                line: 38,
                message: "review sends work back to an earlier step, so it needs a max: how many times it may do that in a run.".into(),
                step: Some(3),
            }]
        } else {
            Vec::new()
        };
        let opened = WorkflowFile {
            workflow_id: ID.into(),
            file: ".weave/workflows/fix-a-bug-our-way.yaml".into(),
            hash: "h1".into(),
            check: WorkflowCheck {
                text: "name: Fix a bug our way\n".into(),
                errors: errors.clone(),
                draft: Some(draft),
                comments: Vec::new(),
            },
        };
        let ed = &mut self.workflows_nav.editor;
        ed.adopt(HOME_MACHINE_KEY, REPOSITORY, opened);
        match case {
            Case::Picked => ed.canvas.selected = Some(2),
            Case::Add => ed.canvas.add_at = Some(4),
            Case::Error => {
                ed.canvas.selected = None;
                ed.source = Some(crate::fleet::workflows::editor::Source::Draft);
            }
            Case::Loops => {}
        }
        self.workflows_nav.reload_wanted = false;
    }
}
