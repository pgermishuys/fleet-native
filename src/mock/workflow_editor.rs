//! Invented workflow files for the editor's scenes and for the app with no server (component 18, piece M). The scenes are the
//! frame's states: edited, the comments confirmation, a file changed on disk, the designer blocked, and just saved.
use super::workflows::{REPOSITORY, library, run};
use crate::State;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::draft::{WorkflowCheck, WorkflowComment, WorkflowDraft, WorkflowFile, WorkflowProblem};
use crate::fleet::workflows::editor::{EditorView, Source};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Opened and edited: Unsaved, the runs note, Save on.
    Edited,
    /// A file with comments, edited in the designer: the banner and the question Save asks.
    Comments,
    /// A save refused because the file changed on disk.
    Conflict,
    /// The text can't be shown in the designer: the banner, the error under the text, Save off.
    Blocked,
    /// A save just done: the toast.
    Saved,
    /// The File view of the file as it is (piece N).
    File,
    /// The File view with a typing mistake: two lines the parser marks, and the problems under them.
    FileErrors,
}

const TEXT: &str = "name: Tidy up a flaky test\ndescription: Finds why a test passes and fails on the same code.\nsteps:\n  - id: find\n    title: Find the cause\n    model: strong\n    skill: fleet-debug\n  - id: steady\n    title: Make it steady\n    model: standard\n  - id: check\n    title: Run it many times\n    model: fast\n    optional: true\n";

/// What the server's open would answer for a repository workflow of the invented library.
pub fn file(id: &str) -> WorkflowFile {
    let lib = library(Some(REPOSITORY));
    let w = lib.workflows.iter().find(|w| w.id == id).cloned().unwrap_or_default();
    let readable = w.errors.is_empty();
    let errors = w
        .errors
        .iter()
        .map(|e| {
            let at = e.split("line ").nth(1).and_then(|r| r.split(':').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
            WorkflowProblem { line: at, message: e.split(": ").skip(1).collect::<Vec<_>>().join(": "), step: None }
        })
        .collect();
    let draft = readable.then(|| WorkflowDraft {
        name: w.name.clone(),
        description: w.description.clone(),
        placeholder: None,
        steps: w.steps.clone(),
    });
    WorkflowFile {
        workflow_id: w.id.clone(),
        file: w.file.clone().unwrap_or_default(),
        hash: "h1".into(),
        check: WorkflowCheck {
            text: if readable { TEXT.into() } else { "name: Triage an issue\n".into() },
            errors,
            draft,
            comments: Vec::new(),
        },
    }
}

impl State {
    pub fn show_workflow_editor(&mut self, dark: bool, case: Case) {
        use crate::mock::workflows::Scene;
        self.show_workflows(dark, Scene::Library);
        let id = "repo:tidy-up-a-flaky-test";
        self.workflows_nav.move_to(id);
        // The run of this file the Library scene already has, so the runs note has something to say.
        self.workflows.of_mut(HOME_MACHINE_KEY).upsert(run("r4", id, "waiting", false, "2026-11-19T12:30:00Z"));
        let mut opened = file(id);
        if case == Case::Comments {
            opened.check.text = format!("# Kept by the platform team.\n{TEXT}# Minor versions only.\n");
            opened.check.comments = vec![
                WorkflowComment { line: 1, text: "Kept by the platform team.".into() },
                WorkflowComment { line: 15, text: "Minor versions only.".into() },
            ];
        }
        let ed = &mut self.workflows_nav.editor;
        ed.adopt(HOME_MACHINE_KEY, REPOSITORY, opened);
        // Edited in the designer, without the check a server would send: the scene is the state it would reach.
        if matches!(case, Case::Edited | Case::Comments | Case::Conflict) {
            let mut next = ed.draft.clone().expect("a draft");
            next.description = Some("Finds why a test passes and fails on the same code, then keeps it green.".into());
            ed.draft = Some(next);
            ed.source = Some(Source::Draft);
        }
        match case {
            Case::Comments => ed.asking_to_remove_comments = true,
            Case::Conflict => ed.conflict = Some("This file changed on disk since you opened it.".into()),
            Case::Blocked => {
                ed.view = EditorView::File;
                ed.text = "name: [Tidy up\nsteps:\n".into();
                ed.source = Some(Source::Text);
                ed.check = Some(WorkflowCheck {
                    text: ed.text.clone(),
                    errors: vec![WorkflowProblem {
                        line: 1,
                        message: "This isn't valid YAML: the flow list isn't closed.".into(),
                        step: None,
                    }],
                    draft: None,
                    comments: Vec::new(),
                });
                ed.designer_blocked = Some(crate::fleet::workflows::editor::DESIGNER_BLOCKED_MESSAGE.into());
            }
            Case::Saved => {
                self.workflows_nav.frame.notice = Some((
                    "Saved .weave/workflows/tidy-up-a-flaky-test.yaml. Commit it with your other changes to share it. 2 runs in progress keep the version they started with.".into(),
                    3600.0,
                ));
            }
            Case::File => ed.view = EditorView::File,
            Case::FileErrors => {
                ed.view = EditorView::File;
                ed.text = TEXT
                    .replace("    model: standard\n", "    modle: standard\n")
                    .replace("    skill: fleet-debug\n", "    skill fleet-debug\n");
                ed.source = Some(Source::Text);
                ed.check = Some(WorkflowCheck {
                    text: ed.text.clone(),
                    errors: vec![
                        WorkflowProblem {
                            line: 7,
                            message: "This line isn't a key and a value: add the colon after skill.".into(),
                            step: None,
                        },
                        WorkflowProblem {
                            line: 10,
                            message: "Unknown field \"modle\" on a step. Did you mean model?".into(),
                            step: Some(1),
                        },
                    ],
                    draft: None,
                    comments: Vec::new(),
                });
            }
            Case::Edited => {}
        }
        // Nothing is read again, and the page doesn't try to open the file itself.
        self.workflows_nav.reload_wanted = false;
    }
}
