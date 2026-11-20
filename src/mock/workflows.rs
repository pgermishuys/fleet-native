//! Invented workflows for the Library's scenes and tests (component 18): Fleet's two built-ins with the step counts the server
//! gives them, and two files in the invented repository `kettle`.
use crate::fleet::workflows::{Library, Step, StepKind, Workflow};
use serde_json::json;

fn steps(titles: &[&str]) -> Vec<Step> {
    titles.iter().map(|t| Step { id: t.to_lowercase().replace(' ', "-"), title: (*t).to_string(), ..Step::default() }).collect()
}

/// Fix a bug's steps as the server gives them: who does each, the role it runs on, its skill and its loops.
fn fix_a_bug_steps() -> Vec<Step> {
    let agent = |id: &str, title: &str, model: &str| Step {
        id: id.into(),
        title: title.into(),
        kind: StepKind::Agent,
        model: Some(model.into()),
        ..Step::default()
    };
    let with = |mut s: Step, skill: Option<&str>, route: Option<(&str, &str)>, max: Option<u32>| {
        s.skill = skill.map(str::to_string);
        s.routes = route.map(|(o, to)| vec![(o.to_string(), to.to_string())]).unwrap_or_default();
        s.max_loops = max;
        s
    };
    vec![
        with(agent("investigate", "Find the cause", "strong"), Some("fleet-debug"), Some(("failed", "end")), None),
        Step { id: "ok-cause".into(), title: "Agree on the cause".into(), kind: StepKind::You, ..Step::default() },
        agent("fix", "Fix", "standard"),
        with(agent("review", "Review", "strong"), Some("fleet-code-review"), Some(("changes", "fix")), Some(2)),
        Step { optional: true, ..with(agent("verify", "Check it runs", "standard"), Some("fleet-run"), Some(("broken", "fix")), Some(2)) },
        Step { id: "ok-pr".into(), title: "Open the pull request".into(), kind: StepKind::You, ..Step::default() },
        with(agent("open-pr", "Push and open the PR", "fast"), None, Some(("failed", "end")), None),
    ]
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
            "Finds the root cause before any code changes, and stops once for you to agree with it. Then fixes it with a test that fails first, reviews the fix and opens a pull request.",
            &[],
        ),
    ];
    workflows[1].steps = fix_a_bug_steps();
    let Some(directory) = directory else { return Library { repository: None, repository_name: None, workflows } };
    let tidy = |id: &str, title: &str, model: &str, skill: Option<&str>, optional: bool| Step {
        id: id.into(),
        title: title.into(),
        kind: StepKind::Agent,
        model: Some(model.into()),
        skill: skill.map(str::to_string),
        optional,
        ..Step::default()
    };
    workflows.push(Workflow {
        id: "repo:tidy-up-a-flaky-test".into(),
        file: Some(".weave/workflows/tidy-up-a-flaky-test.yaml".into()),
        name: "Tidy up a flaky test".into(),
        description: Some("Finds why a test passes and fails on the same code, makes it steady and shows it stays green.".into()),
        steps: vec![
            tidy("find", "Find the cause", "strong", Some("fleet-debug"), false),
            tidy("steady", "Make it steady", "standard", None, false),
            tidy("check", "Run it many times", "fast", None, true),
        ],
        ..Workflow::default()
    });
    // The server gives a file it can't read no steps and no description, only what is wrong with it.
    workflows.push(Workflow {
        id: "repo:triage-an-issue".into(),
        file: Some(".weave/workflows/triage-an-issue.yaml".into()),
        name: "Triage an issue".into(),
        errors: [
            ".weave/workflows/triage-an-issue.yaml, line 7: \"routes\" isn't something an agent step (read) can have.",
            ".weave/workflows/triage-an-issue.yaml, line 4: read needs a model: strong, standard, fast, or an exact provider/model.",
            ".weave/workflows/triage-an-issue.yaml, line 4: read needs a prompt: what the agent should do.",
            ".weave/workflows/triage-an-issue.yaml, line 4: read needs outcomes, e.g. outcomes: [done]. The agent reports one of them when it's finished.",
        ]
        .map(String::from)
        .to_vec(),
        ..Workflow::default()
    });
    let name = directory.rsplit('/').next().unwrap_or(directory).to_string();
    Library { repository: Some(directory.to_string()), repository_name: Some(name), workflows }
}

/// A run for a workflow, as the server's `workflow_run` carries it: only what the Library's rows read.
pub fn run(id: &str, workflow: &str, status: &str, with_you: bool, created: &str) -> crate::fleet::workflows::Run {
    titled(id, workflow, status, with_you, created, "A change", None)
}

/// A run with a title, the step it is at and who started it (an automation's name), as the detail's recent runs read it.
pub fn titled(
    id: &str,
    workflow: &str,
    status: &str,
    with_you: bool,
    created: &str,
    title: &str,
    started_by: Option<&str>,
) -> crate::fleet::workflows::Run {
    let session = format!("s-{id}");
    let step = ("investigate", "Find the cause");
    let mut v = json!({
        "id": id, "workflowId": workflow, "workflowName": "", "title": title, "request": title, "status": status,
        "currentStepId": step.0, "result": null, "repositoryPath": REPOSITORY, "branch": null, "baseBranch": null, "worktreePath": null,
        "harnessType": "opencode", "createdAt": created, "updatedAt": created, "endedAt": null,
        "steps": [{"id": step.0, "title": step.1, "kind": "agent", "state": "running", "visits": 1, "sessionId": session}],
        "sessions": [{"sessionId": session, "stepId": step.0, "visit": 1}],
    });
    if with_you {
        v["withYou"] = json!({"stepId": step.0, "stepTitle": step.1, "sessionId": session, "files": [], "wrappingUp": false, "outcome": null, "moves": []});
    }
    if status == "waiting" {
        v["waiting"] =
            json!({"kind": "no-outcome", "stepId": step.0, "stepTitle": step.1, "message": "", "sessionId": session, "choices": []});
    }
    if status == "failed" {
        v["result"] = json!("Couldn't start the step");
    }
    if let Some(name) = started_by {
        v["startedBy"] = json!({"automationId": "a-nightly", "automationName": name});
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
    /// Fix a bug open with three runs, as the scratch Fleet makes them: with you, needs you, ended.
    Detail,
    /// Fix a bug open with a run in every state a row can say, one of them started by an automation.
    DetailStates,
    /// Fix a bug open with no repository picked: Repeat on a schedule… and Duplicate are off.
    DetailNoRepo,
    /// A repository's file that can't be read: its errors, no steps.
    DetailErrors,
    /// A repository's file that reads: its path, the optional step, the skill.
    DetailRepo,
    /// The runs are on their way.
    RunsLoading,
    /// The runs are read and there are none.
    RunsEmpty,
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
            Scene::NoRepo | Scene::DetailNoRepo => nav.library = Some(library(None)),
            Scene::RepoLoading => {
                nav.library = Some(library(None));
                nav.set_folder(Folder::Repository(REPOSITORY.into()), true);
                nav.loading_library = true;
            }
            _ => {
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
            runs.upsert(run("r3", "repo:tidy-up-a-flaky-test", "running", false, "2026-11-19T12:00:00Z"));
        }
        let open = |s: &mut crate::State, id: &str| s.workflows_nav.move_to(id);
        // The scenes' clock: the afternoon of the runs' day, so a run says its time, not its date.
        self.now_ms = crate::fleet::work::parse_iso_ms("2026-11-19T18:00:00Z");
        match scene {
            Scene::Detail => {
                open(self, "builtin:fix-a-bug");
                let runs = self.workflows.of_mut(HOME_MACHINE_KEY);
                let fix = "builtin:fix-a-bug";
                runs.loaded = true;
                runs.upsert(titled("d1", fix, "running", true, "2026-11-19T13:19:00Z", "Page size of zero divides by zero", None));
                runs.upsert(titled(
                    "d2",
                    fix,
                    "waiting",
                    false,
                    "2026-11-19T13:18:00Z",
                    "The pager skips the last page when the total is an exact\u{2026}",
                    None,
                ));
                runs.upsert(titled("d3", fix, "ended", false, "2026-11-19T13:24:00Z", "Tooltip stays open after the pointer leaves", None));
            }
            Scene::DetailStates => {
                open(self, "builtin:fix-a-bug");
                let runs = self.workflows.of_mut(HOME_MACHINE_KEY);
                let fix = "builtin:fix-a-bug";
                runs.loaded = true;
                for r in [
                    titled(
                        "d1",
                        fix,
                        "waiting",
                        false,
                        "2026-11-19T13:19:00Z",
                        "The pager skips the last page when the total is an exact\u{2026}",
                        None,
                    ),
                    titled("d2", fix, "running", true, "2026-11-19T11:41:00Z", "Page size of zero divides by zero", None),
                    titled(
                        "d3",
                        fix,
                        "running",
                        false,
                        "2026-11-19T10:05:00Z",
                        "Search keeps the old results after a new query",
                        Some("Nightly triage"),
                    ),
                    titled("d4", fix, "done", false, "2026-11-18T16:20:00Z", "The export button does nothing on an empty list", None),
                    titled("d5", fix, "failed", false, "2026-11-18T09:12:00Z", "Dates sort as text in the activity list", None),
                    titled("d6", fix, "ended", false, "2026-11-17T15:30:00Z", "Tooltip stays open after the pointer leaves", None),
                ] {
                    runs.upsert(r);
                }
            }
            Scene::DetailNoRepo => open(self, "builtin:fix-a-bug"),
            Scene::DetailErrors => open(self, "repo:triage-an-issue"),
            Scene::DetailRepo => {
                open(self, "repo:tidy-up-a-flaky-test");
                let runs = self.workflows.of_mut(HOME_MACHINE_KEY);
                runs.loaded = true;
                runs.upsert(titled(
                    "t1",
                    "repo:tidy-up-a-flaky-test",
                    "waiting",
                    false,
                    "2026-11-19T12:00:00Z",
                    "The slow-test timer test fails one run in ten",
                    None,
                ));
            }
            Scene::RunsLoading => {
                open(self, "builtin:fix-a-bug");
                self.workflows.of_mut(HOME_MACHINE_KEY).loading = true;
            }
            Scene::RunsEmpty => {
                open(self, "builtin:fix-a-bug");
                self.workflows.of_mut(HOME_MACHINE_KEY).loaded = true;
            }
            _ => {}
        }
    }
}

/// What a scene of the Run box shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunBoxScene {
    /// Fix a bug in the invented repository, nothing typed yet: Run is off.
    Empty,
    /// A request typed, the optional step and Check with me on.
    Filled,
    /// None of the harnesses that run workflows is installed and on.
    NoHarness,
    /// A step's built-in skill is off.
    SkillOff,
    /// Run was pressed and the server hasn't answered.
    Starting,
    /// The server refused the run.
    Refused,
}

impl crate::State {
    /// The Run box for the pictures and tests: Fix a bug open, the invented repository picked and its branches known.
    pub fn show_run_box(&mut self, dark: bool, which: RunBoxScene) {
        use crate::fleet::new_session::request::Folder;
        use crate::fleet::skills::BuiltInSkill;
        self.show_workflows(dark, Scene::DetailNoRepo);
        let path = super::feeds::ROCKET;
        self.feeds = super::feeds::feeds();
        let nav = &mut self.workflows_nav;
        nav.set_folder(Folder::Repository(path.into()), true);
        nav.library = Some(library(Some(path)));
        nav.library_repository = Some(path.into());
        nav.reload_wanted = false;
        let run_box = &mut nav.run_box;
        if matches!(which, RunBoxScene::Filled | RunBoxScene::Starting | RunBoxScene::Refused) {
            run_box.request = rune::TextEdit::new("The pager skips the last page when the total is an exact multiple of the page size");
        }
        if which == RunBoxScene::Filled {
            run_box.optional_on.insert("verify".into());
            run_box.check_with_me = true;
        }
        run_box.starting = which == RunBoxScene::Starting;
        if which == RunBoxScene::Refused {
            run_box.error = Some("Couldn't start the run: the branch origin/main isn't there.".into());
        }
        if which == RunBoxScene::NoHarness {
            let harnesses = self.feeds.harnesses.entry(crate::fleet::settings::HOME_MACHINE_KEY.into()).or_default();
            for h in &mut harnesses.harnesses.all {
                h.available = false;
                h.state = "not-installed".into();
            }
        }
        if which == RunBoxScene::SkillOff {
            let skill = |name: &str, enabled: bool| BuiltInSkill { name: name.into(), enabled, ..BuiltInSkill::default() };
            self.skills.list = Some(vec![skill("fleet-debug", false), skill("fleet-code-review", true), skill("fleet-run", true)]);
        }
    }
}
