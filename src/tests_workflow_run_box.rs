//! Component 18, piece D: the Run box. Fleet's `WorkflowRunBox.test.ts` (6) under their names, against a fake Fleet that records
//! what Run sends; then what the native box keeps: Enter runs and Shift+Enter doesn't, Run with no folder opens the folder
//! menu, another workflow clears the box, the chips that show only sometimes, a refused run keeps the words, and a run that
//! starts opens its first session.
use crate::fleet::groups::{ProjectInfo, ProjectKind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses, Load};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::run_box::Warning;
use crate::mock::feeds::ROCKET;
use crate::mock::workflows::RunBoxScene;
use crate::tests::{FRAME, app_tall, click};
use crate::tests_workflow_runs::{Fake, fake};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

const POST_RUNS: &str = "/api/workflows/runs";

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
}

fn run_json() -> Value {
    json!({
        "id": "run-1", "workflowId": "builtin:fix-a-bug", "workflowName": "Fix a bug", "title": "Press ?", "request": "Press ?",
        "status": "running", "currentStepId": "investigate", "result": null, "repositoryPath": ROCKET, "branch": null, "baseBranch": null,
        "worktreePath": null, "harnessType": "opencode2", "createdAt": "2026-09-23T10:00:00Z", "updatedAt": "2026-09-23T10:00:00Z",
        "endedAt": null, "steps": [{"id": "investigate", "title": "Find the cause", "kind": "agent", "state": "running", "visits": 1, "enabled": true}],
        "sessions": [{"sessionId": "run-1-s1", "stepId": "investigate", "visit": 1}]
    })
}

/// The Workflows page on a fake Fleet, the box drawn from the invented feeds (the repository, its branches, two harnesses).
fn rig(which: RunBoxScene) -> Rig {
    let home = fake();
    home.route("GET", "/api/preferences", 200, json!({"Workflows": "true"}));
    home.route("POST", POST_RUNS, 200, run_json());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app_tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.load_preferences();
    });
    rt.frame(FRAME);
    test_wait::drive(&mut rt, &queue, "the preferences", |s| s.pickers.prefs.fetched, |_| String::new());
    rt.update(move |s, _| {
        s.show_run_box(true, which);
        // The scene is the state: nothing of the box is read from the fake.
        s.workflows_nav.run_box.asked = true;
        s.workflows_nav.run_box.initial_done = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    Rig { rt, queue, home }
}

impl Rig {
    fn text(&mut self, words: &str) {
        click(&mut self.rt, "workflow-request");
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn enter(&mut self, mods: Mods) {
        self.rt.key(KeyEvent { key: Key::Enter, mods });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("box {:?}", s.workflows_nav.run_box));
    }

    /// The bodies of the runs started so far, once at least `n` were heard.
    fn started(&mut self, n: usize) -> Vec<Value> {
        let calls = self.home.calls_handle();
        self.wait("a request to start a run", move |_| calls().iter().filter(|(m, p)| m == "POST" && p == POST_RUNS).count() >= n);
        self.home.workflow_calls().into_iter().filter(|(m, p, _)| m == "POST" && p == POST_RUNS).map(|(.., b)| b).collect()
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn words(&self) -> Vec<String> {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect()
    }

    fn say(&self, words: &str) -> bool {
        self.words().iter().any(|w| w.contains(words))
    }

    fn run(&mut self) {
        click(&mut self.rt, "workflow-run");
    }
}

fn harness(kind: &str, name: &str, workflow_steps: bool, ready: bool) -> HarnessEntry {
    HarnessEntry {
        kind: kind.into(),
        display_name: name.into(),
        available: ready,
        user_enabled: true,
        supports_workflow_steps: workflow_steps,
        state: if ready { "ready" } else { "not-installed" }.into(),
        ..HarnessEntry::default()
    }
}

fn set_harnesses(rt: &mut Runtime<State>, all: Vec<HarnessEntry>, default: &str) {
    let default = default.to_string();
    rt.update(move |s, _| {
        let feed = s.feeds.harnesses.entry(HOME_MACHINE_KEY.into()).or_default();
        feed.harnesses = Harnesses { all, load: Load::Done, preferred: Some(default) };
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

// ─── WorkflowRunBox.test.ts ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn starts_a_run_with_check_with_me_after_each_step_switched_on() {
    let mut r = rig(RunBoxScene::Empty);
    assert!(r.say("off") && !r.rt.state.workflows_nav.run_box.check_with_me);
    click(&mut r.rt, "workflow-check-with-me-start");
    assert!(r.rt.state.workflows_nav.run_box.check_with_me);
    let chip = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-check-with-me-start")).unwrap();
    assert!(chip.help.as_deref().unwrap().starts_with("Check with me after each step"));
    r.text("Press ? to see every keyboard shortcut");
    r.run();
    let bodies = r.started(1);
    assert_eq!(bodies[0]["workflowId"], "builtin:fix-a-bug");
    assert_eq!(bodies[0]["directory"], ROCKET);
    assert_eq!(bodies[0]["checkWithMe"], true);
}

#[test]
fn leaves_it_off_unless_it_s_switched_on() {
    let mut r = rig(RunBoxScene::Empty);
    r.text("Press ?");
    r.run();
    assert_eq!(r.started(1)[0]["checkWithMe"], false);
}

#[test]
fn sends_no_project_by_default_scratch() {
    let mut r = rig(RunBoxScene::Empty);
    let chip = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-project")).unwrap();
    assert!(chip.label.as_deref().unwrap().contains("Scratch"), "{:?}", chip.label);
    r.text("Press ?");
    r.run();
    // Scratch is no project: the body leaves it out, as the server reads a missing one.
    assert!(r.started(1)[0].get("projectId").is_none_or(Value::is_null));
}

#[test]
fn sends_the_chosen_project() {
    let mut r = rig(RunBoxScene::Empty);
    r.rt.update(|s, _| {
        s.project_load.projects = vec![
            ProjectInfo { id: "proj-1".into(), name: "Dependencies".into(), kind: ProjectKind::User, position: 0 },
            ProjectInfo { id: "scratch".into(), name: "Scratch".into(), kind: ProjectKind::Scratch, position: 1 },
        ];
    });
    click(&mut r.rt, "workflow-project");
    assert!(r.has("workflow-project-scratch") && r.has("workflow-project-proj-1"), "Scratch and the user's project");
    assert!(!r.has("workflow-project-scratch-project"), "a scratch project isn't listed twice");
    click(&mut r.rt, "workflow-project-proj-1");
    assert!(!r.has("workflow-project-pop"), "picking closes the menu");
    r.text("Press ?");
    r.run();
    assert_eq!(r.started(1)[0]["projectId"], "proj-1");
}

#[test]
fn says_no_harness_that_runs_workflows_is_ready_by_name_and_points_to_settings_harnesses() {
    let mut r = rig(RunBoxScene::Empty);
    // OpenCode 2 is the default but isn't installed; only Pi, which can't run workflows, is ready.
    let mut claude = harness("claude-code", "Claude Code", true, true);
    claude.user_enabled = false;
    set_harnesses(
        &mut r.rt,
        vec![
            harness("opencode2", "OpenCode 2", true, false),
            harness("opencode", "OpenCode", true, false),
            claude,
            harness("copilot", "GitHub Copilot", true, false),
            harness("pi", "Pi", false, true),
        ],
        "opencode2",
    );
    r.text("Press ?");
    assert!(r.has("workflow-no-harness"));
    let said = "Workflows run on OpenCode 2, OpenCode, Claude Code or GitHub Copilot, and none of them is installed and on.";
    assert!(r.say(said), "{:?}", r.words());
    assert!(!r.say("opencode2") && !r.say("Pick"));
    click(&mut r.rt, "workflow-no-harness-link");
    assert_eq!(r.rt.state.rail, crate::Rail::Settings);
    assert_eq!(r.rt.state.settings.page.id(), "harnesses");
    r.rt.update(|s, _| s.enter_workflows());
    r.rt.frame(FRAME);
    let run = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-run")).unwrap();
    assert!(run.opacity < 1.0, "Run is off");
    r.run();
    assert!(r.home.workflow_calls().iter().all(|(m, ..)| m != "POST"), "nothing was sent");
}

#[test]
fn says_nothing_about_harnesses_while_one_that_runs_workflows_is_ready() {
    let mut r = rig(RunBoxScene::Empty);
    set_harnesses(&mut r.rt, vec![harness("pi", "Pi", false, true), harness("opencode", "OpenCode", true, true)], "pi");
    assert!(!r.has("workflow-no-harness"));
    // The default (Pi) can't run workflows: the box runs on the one that can.
    assert_eq!(r.rt.state.run_box_harness_type(), "opencode");
}

// ─── what the native box keeps ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn enter_runs_and_shift_enter_is_a_new_line() {
    let mut r = rig(RunBoxScene::Empty);
    r.text("first line");
    r.enter(Mods { shift: true, ..Mods::default() });
    assert!(r.home.workflow_calls().iter().all(|(m, ..)| m != "POST"), "Shift+Enter sends nothing");
    assert!(r.rt.state.workflows_nav.run_box.request.text.contains('\n'));
    r.enter(Mods::default());
    let bodies = r.started(1);
    assert_eq!(bodies[0]["request"], "first line", "the request is trimmed");
}

#[test]
fn enter_with_nothing_typed_sends_nothing() {
    let mut r = rig(RunBoxScene::Empty);
    click(&mut r.rt, "workflow-request");
    r.enter(Mods::default());
    r.run();
    assert!(r.home.workflow_calls().iter().all(|(m, ..)| m != "POST"));
}

#[test]
fn run_with_no_folder_opens_the_folder_menu_instead() {
    let mut r = rig(RunBoxScene::Filled);
    r.rt.update(|s, _| {
        s.workflows_nav.folder = None;
        s.workflows_nav.run_box.request = rune::TextEdit::new("Press ?");
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    // The button is the folder's way in: pressing Enter in the field does the same.
    click(&mut r.rt, "workflow-request");
    r.enter(Mods::default());
    assert!(r.rt.state.folder_menu_open(), "the folder menu opened");
    assert!(r.home.workflow_calls().iter().all(|(m, ..)| m != "POST"));
}

#[test]
fn another_workflow_clears_the_optional_steps_the_overrides_and_the_error() {
    let mut r = rig(RunBoxScene::Refused);
    r.rt.update(|s, _| {
        s.workflows_nav.run_box.optional_on.insert("verify".into());
        s.workflows_nav.run_box.overrides.push((crate::fleet::workflows::Role::Strong, Default::default()));
    });
    r.rt.update(|s, _| s.set_active_workflow("builtin:build-a-feature"));
    let run_box = &r.rt.state.workflows_nav.run_box;
    assert!(run_box.optional_on.is_empty() && run_box.overrides.is_empty() && run_box.error.is_none());
    assert!(!run_box.request.text.is_empty(), "what was typed stays");
}

#[test]
fn shows_a_chip_for_each_optional_step_that_flips_and_says_what_it_does() {
    let mut r = rig(RunBoxScene::Empty);
    let chip = |r: &Rig| r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-optional-verify")).unwrap();
    assert!(chip(&r).label.as_deref().unwrap().ends_with("off"));
    click(&mut r.rt, "workflow-optional-verify");
    assert!(chip(&r).label.as_deref().unwrap().ends_with("on"));
    r.text("Press ?");
    r.run();
    assert_eq!(r.started(1)[0]["optionalSteps"], json!(["verify"]));
}

#[test]
fn the_harness_chip_shows_only_with_more_than_one_harness_that_runs_workflows() {
    let mut r = rig(RunBoxScene::Empty);
    assert!(r.has("new-session-harness"), "the invented machine has two");
    set_harnesses(&mut r.rt, vec![harness("opencode", "OpenCode", true, true), harness("pi", "Pi", false, true)], "opencode");
    assert!(!r.has("new-session-harness"));
}

#[test]
fn picking_a_harness_runs_on_it() {
    let mut r = rig(RunBoxScene::Empty);
    click(&mut r.rt, "new-session-harness");
    click(&mut r.rt, "new-session-harness-claude-code");
    assert!(!r.rt.state.harness_menu_open());
    r.text("Press ?");
    r.run();
    assert_eq!(r.started(1)[0]["harnessType"], "claude-code");
}

#[test]
fn the_folder_menu_lists_repositories_only() {
    let mut r = rig(RunBoxScene::Empty);
    click(&mut r.rt, "new-session-folder-chip");
    let rows = r.rt.state.folder_rows();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|o| o.group != crate::fleet::new_session::folder::Group::New), "no create or clone");
    assert!(rows.iter().all(|o| o.group != crate::fleet::new_session::folder::Group::Folders), "no plain folders");
    // The draft of the New session page is untouched by what is picked here.
    r.rt.update(|s, tx| s.run_folder_row(0, tx));
    assert!(matches!(r.rt.state.workflows_nav.folder(), Some(crate::fleet::new_session::request::Folder::Repository(_))));
}

#[test]
fn another_repository_forgets_the_base() {
    let mut r = rig(RunBoxScene::Empty);
    r.rt.update(|s, _| s.run_box_choose_base("release/2.0"));
    assert_eq!(r.rt.state.workflows_nav.run_box.base_branch.as_deref(), Some("release/2.0"));
    r.rt.update(|s, _| s.set_folder(crate::fleet::new_session::request::Folder::Repository("/home/ada/src/comet".into()), true));
    assert_eq!(r.rt.state.workflows_nav.run_box.base_branch, None);
    // The same repository again keeps what was picked.
    r.rt.update(|s, _| s.run_box_choose_base("release/2.0"));
    r.rt.update(|s, _| s.set_folder(crate::fleet::new_session::request::Folder::Repository("/home/ada/src/comet".into()), true));
    assert_eq!(r.rt.state.workflows_nav.run_box.base_branch.as_deref(), Some("release/2.0"));
}

#[test]
fn the_default_base_is_no_choice_and_another_is_sent() {
    let mut r = rig(RunBoxScene::Empty);
    r.rt.update(|s, _| s.run_box_choose_base("origin/main"));
    assert_eq!(r.rt.state.workflows_nav.run_box.base_branch, None, "the default is sent as no choice");
    r.rt.update(|s, _| s.run_box_choose_base("release/2.0"));
    r.text("Press ?");
    r.run();
    assert_eq!(r.started(1)[0]["baseBranch"], "release/2.0");
}

#[test]
fn says_a_skill_that_is_off_and_waits_for_it() {
    let mut r = rig(RunBoxScene::SkillOff);
    assert_eq!(
        r.rt.state.run_box_warning(r.rt.state.workflows_nav.active_workflow().unwrap()),
        Some(Warning::SkillOff { step: "Find the cause".into(), skill: "fleet-debug".into() })
    );
    assert!(r.say("Find the cause uses fleet-debug, which is off."));
    click(&mut r.rt, "workflow-skill-off-link");
    assert_eq!(r.rt.state.settings.page.id(), "skills");
}

#[test]
fn a_skill_of_an_optional_step_that_is_off_counts_only_with_the_step_on() {
    let mut r = rig(RunBoxScene::Empty);
    r.rt.update(|s, _| {
        s.skills.list = Some(vec![crate::fleet::skills::BuiltInSkill { name: "fleet-run".into(), enabled: false, ..Default::default() }]);
    });
    let w = r.rt.state.workflows_nav.active_workflow().unwrap().clone();
    assert_eq!(r.rt.state.run_box_warning(&w), None, "Check it runs is off");
    r.rt.update(|s, _| s.workflows_nav.run_box.toggle_optional("verify"));
    assert!(matches!(r.rt.state.run_box_warning(&w), Some(Warning::SkillOff { .. })));
}

#[test]
fn a_file_with_errors_says_to_fix_it_first_and_wont_run() {
    let mut r = rig(RunBoxScene::Filled);
    // The run view of a repository's file, which Try it shows (its designer is the editor's).
    r.rt.update(|s, _| {
        s.set_active_workflow("repo:triage-an-issue");
        s.workflows_nav.showing_runs = true;
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(r.say("Fix the workflow file first."));
    r.run();
    // The editor reads the file whenever a repository's workflow is open (Fleet's watch does too); a run is the other POST.
    assert!(r.home.workflow_calls().iter().all(|(m, path, _)| m != "POST" || path == "/api/workflows/files/open"));
}

#[test]
fn a_refused_run_shows_the_server_s_words_and_keeps_what_was_typed() {
    let mut r = rig(RunBoxScene::Empty);
    r.home.route("POST", POST_RUNS, 409, json!({"error": "origin/main isn't there."}));
    r.text("Press ?");
    r.run();
    r.started(1);
    r.wait("the words", |s| s.workflows_nav.run_box.error.is_some() && !s.workflows_nav.run_box.starting);
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(r.has("workflow-start-error"), "{:?}", r.words());
    assert!(r.say("origin/main isn't there."), "{:?}", r.words());
    assert_eq!(r.rt.state.workflows_nav.run_box.request.text, "Press ?");
}

#[test]
fn the_box_is_read_only_and_runs_nothing_more_while_it_starts() {
    let mut r = rig(RunBoxScene::Starting);
    assert!(r.rt.state.workflows_nav.run_box.starting);
    r.run();
    assert!(r.home.workflow_calls().iter().all(|(m, ..)| m != "POST"), "a second Run is not sent");
}

#[test]
fn a_run_that_starts_clears_the_request_and_opens_its_first_session_once_the_list_has_it() {
    let mut r = rig(RunBoxScene::Empty);
    r.text("Press ?");
    r.run();
    r.started(1);
    r.wait("the run's answer", |s| s.workflows_nav.run_box.open_when_listed.is_some());
    assert!(r.rt.state.workflows_nav.run_box.request.text.is_empty(), "the request is cleared");
    assert_eq!(r.rt.state.rail, crate::Rail::Workflows, "the session isn't in the list yet");
    // The list gets it (an event or a read): the box opens it.
    r.rt.update(|s, _| {
        let id = "run-1-s1";
        let mut made = s.projects[0].sessions[0].row_copy();
        made.id = id.into();
        s.projects[0].sessions.insert(0, made);
    });
    r.wait("the session opens", |s| s.rail == crate::Rail::Sessions);
    assert_eq!(r.rt.state.open_id().as_deref(), Some("run-1-s1"));
    assert!(r.rt.state.workflows_nav.run_box.open_when_listed.is_none());
}

#[test]
fn the_schedule_draft_carries_the_request_the_optional_steps_the_base_and_the_harness() {
    let mut r = rig(RunBoxScene::Filled);
    r.rt.update(|s, _| {
        s.run_box_choose_base("release/2.0");
        s.run_box_choose_harness("claude-code");
    });
    let (request, steps, base, harness) = r.rt.state.run_box_schedule_draft();
    assert!(request.starts_with("The pager"));
    assert_eq!(steps, vec!["verify".to_string()]);
    assert_eq!(base.as_deref(), Some("release/2.0"));
    assert_eq!(harness.as_deref(), Some("claude-code"));
}

#[test]
fn the_models_chip_is_off_when_no_enabled_step_asks_for_a_role() {
    let mut r = rig(RunBoxScene::Empty);
    let models = |r: &Rig| r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-models")).unwrap();
    assert!(models(&r).opacity >= 1.0, "Fix a bug's steps ask for roles");
    r.rt.update(|s, _| {
        let w = s.workflows_nav.library.as_mut().unwrap().workflows.iter_mut().find(|w| w.id == "builtin:fix-a-bug").unwrap();
        w.steps.iter_mut().for_each(|st| st.model = None);
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(models(&r).opacity < 1.0);
}
