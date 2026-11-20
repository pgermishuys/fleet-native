//! Invented workflows for the Library's scenes and tests (component 18): Fleet's two built-ins with the step counts the server
//! gives them, and two files in the invented repository `kettle`.
use crate::fleet::workflows::{Library, Step, Workflow};
use serde_json::json;

fn steps(titles: &[&str]) -> Vec<Step> {
    titles.iter().map(|t| Step { id: t.to_lowercase().replace(' ', "-"), title: (*t).to_string(), ..Step::default() }).collect()
}

/// The repository the scenes' Run box has picked.
pub const REPOSITORY: &str = "/work/kettle";

/// What the Library holds: the built-ins always, and the repository's files when a repository is picked.
pub fn library(directory: Option<&str>) -> Library {
    let built_in = |id: &str, name: &str, description: &str, titles: &[&str]| Workflow {
        id: format!("builtin:{id}"),
        built_in: true,
        name: name.to_string(),
        description: Some(description.to_string()),
        steps: steps(titles),
        ..Workflow::default()
    };
    let mut workflows = vec![
        built_in(
            "build-a-feature",
            "Build a feature",
            "Turns a sentence into a reviewed pull request.",
            &["Explore", "Plan", "Approve the plan", "Implement", "Test", "Review", "Fix review notes", "Open the pull request"],
        ),
        built_in(
            "fix-a-bug",
            "Fix a bug",
            "Finds the cause of a bug, fixes it and proves the fix.",
            &["Reproduce", "Diagnose", "Approve the cause", "Fix", "Test", "Review", "Open the pull request"],
        ),
    ];
    let Some(directory) = directory else { return Library { repository: None, repository_name: None, workflows } };
    workflows.push(Workflow {
        id: "file:.weave/workflows/release-notes.yml".into(),
        file: Some(".weave/workflows/release-notes.yml".into()),
        name: "Write the release notes".into(),
        steps: steps(&["Collect changes", "Draft", "Review"]),
        ..Workflow::default()
    });
    workflows.push(Workflow {
        id: "file:.weave/workflows/triage.yml".into(),
        file: Some(".weave/workflows/triage.yml".into()),
        name: "Triage an issue".into(),
        steps: steps(&["Read", "Label"]),
        errors: vec!["triage.yml line 7: step \"label\" routes to \"close\", which isn't a step".into()],
        ..Workflow::default()
    });
    let name = directory.rsplit('/').next().unwrap_or(directory).to_string();
    Library { repository: Some(directory.to_string()), repository_name: Some(name), workflows }
}

/// A run for a workflow, as the server's `workflow_run` carries it: only what the Library's rows read.
pub fn run(id: &str, workflow: &str, status: &str, with_you: bool, created: &str) -> crate::fleet::workflows::Run {
    let mut v = json!({
        "id": id, "workflowId": workflow, "workflowName": "", "title": "A change", "request": "A change", "status": status,
        "currentStepId": null, "result": null, "repositoryPath": REPOSITORY, "branch": null, "baseBranch": null, "worktreePath": null,
        "harnessType": "opencode", "createdAt": created, "updatedAt": created, "endedAt": null, "steps": [], "sessions": [],
    });
    if with_you {
        v["withYou"] = json!({"stepId": "plan", "stepTitle": "Plan", "sessionId": "s1", "files": [], "wrappingUp": false, "outcome": null, "moves": []});
    }
    crate::fleet::workflows::Run::from_json(&v).expect("a run")
}

/// What a scene shows of the Workflows page.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// On, the Run box's repository picked, two runs that show in the rows' words.
    Library,
    /// Off, the notice.
    Off,
    /// The repository just picked is still loading.
    RepoLoading,
    /// On, no repository picked.
    NoRepo,
    /// The library failed to load.
    Failed,
    /// The first read is still on its way.
    Loading,
    /// Another machine is listed, so the switch is drawn; the library is the machine you're on.
    Machines,
    /// Another machine shown, where Workflows is off.
    MachineOff,
}

impl crate::State {
    /// The Workflows page for the pictures and tests: no server, so the library is the invented one.
    pub fn show_workflows(&mut self, dark: bool, scene: Scene) {
        use crate::fleet::new_session::request::Folder;
        use crate::fleet::settings::HOME_MACHINE_KEY;
        self.dark = Some(dark);
        if matches!(scene, Scene::Machines | Scene::MachineOff) {
            self.show_machines_only(super::machines::Case::Several);
        }
        self.pickers.prefs.set("Workflows", if scene == Scene::Off { "false" } else { "true" });
        self.enter_workflows();
        if scene == Scene::MachineOff {
            self.workflows_nav.choice.pick("m-falcon");
            self.feeds.prefs.entry("m-falcon".into()).or_default().set("Workflows", "false");
            return;
        }
        let nav = &mut self.workflows_nav;
        match scene {
            Scene::Off => {}
            Scene::Loading => nav.loading_library = true,
            Scene::Failed => nav.library_error = Some("Couldn't load the workflows.".into()),
            Scene::NoRepo => nav.library = Some(library(None)),
            Scene::RepoLoading => {
                nav.library = Some(library(None));
                nav.set_folder(Folder::Repository(REPOSITORY.into()), true);
                nav.loading_library = true;
            }
            Scene::Library | Scene::Machines | Scene::MachineOff => {
                nav.set_folder(Folder::Repository(REPOSITORY.into()), true);
                nav.library = Some(library(Some(REPOSITORY)));
                nav.library_repository = Some(REPOSITORY.into());
            }
        }
        // The scene is the state; nothing is read again.
        self.workflows_nav.reload_wanted = false;
        if matches!(scene, Scene::Library | Scene::Machines) {
            let runs = self.workflows.of_mut(HOME_MACHINE_KEY);
            runs.upsert(run("r1", "builtin:build-a-feature", "waiting", false, "2026-11-19T10:00:00Z"));
            runs.upsert(run("r2", "builtin:fix-a-bug", "running", true, "2026-11-19T11:00:00Z"));
            runs.upsert(run("r3", "file:.weave/workflows/release-notes.yml", "running", false, "2026-11-19T12:00:00Z"));
        }
    }
}
