//! Invented drafting states for the scenes (component 18, piece S): the model being asked, a draft that failed, and a draft that
//! came. The scripted model can't answer a draft, so the success path is only ever a scene.
use crate::State;
use crate::fleet::workflows::draft::{DraftTokens, DraftedWorkflow};
use crate::fleet::workflows::drafting::{Ask, Drafting};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Asked from a session, waiting.
    Drafting,
    /// Asked from a description, waiting.
    DraftingDescription,
    /// The model didn't answer in time: the words and Try again.
    Failed,
    /// The draft came: open unsaved, with the banner.
    Drafted,
    /// The draft came with errors: the File view, Save off.
    DraftedErrors,
}

const TITLE: &str = "Fix the pager's last page";

/// What the model gave for the invented session: the invented repository's workflow, counted as one ask.
fn drafted(from_session: bool) -> DraftedWorkflow {
    let file = super::workflow_editor::file("repo:tidy-up-a-flaky-test");
    DraftedWorkflow {
        repository: super::workflows::REPOSITORY.into(),
        repository_name: "kettle".into(),
        session_title: from_session.then(|| TITLE.to_string()),
        check: file.check,
        asks: 1,
        tokens: Some(DraftTokens { total: 3412, from_cache: 3100 }),
    }
}

impl State {
    pub fn show_workflow_draft(&mut self, dark: bool, case: Case) {
        self.show_workflows(dark, super::workflows::Scene::Library);
        let nav = &mut self.workflows_nav;
        let session = Ask::Session { id: "s1".into(), title: TITLE.into() };
        match case {
            Case::Drafting | Case::Failed => {
                let error = (case == Case::Failed).then(|| "The model took too long to answer. Try again.".to_string());
                nav.drafting = Some(Drafting { ask: session, error });
                nav.show_draft();
            }
            Case::DraftingDescription => {
                let ask = Ask::Description {
                    directory: super::workflows::REPOSITORY.into(),
                    description: "Triage a bug report.".into(),
                    harness_type: None,
                };
                nav.drafting = Some(Drafting { ask, error: None });
                nav.show_draft();
            }
            Case::Drafted | Case::DraftedErrors => {
                let mut d = drafted(true);
                if case == Case::DraftedErrors {
                    d.asks = 2;
                    d.tokens = Some(DraftTokens { total: 7300, from_cache: 6800 });
                    d.check.errors = vec![crate::fleet::workflows::draft::WorkflowProblem {
                        line: 12,
                        message: "check sends work back to an earlier step, so it needs a max: how many times it may do that in a run."
                            .into(),
                        step: None,
                    }];
                }
                nav.drafted = Some(d.clone());
                nav.editor.adopt_draft(crate::fleet::settings::HOME_MACHINE_KEY, d);
                nav.show_draft();
            }
        }
    }
}
