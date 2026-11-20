//! Component 18, piece B: the Workflows page, its off notice and the Library list. Fleet's `WorkflowsPageOff` (3),
//! `WorkflowsPageMachine` (1), `WorkflowsPage` (2), `WorkflowsLoading` "shows the repo's workflows loading…" and
//! `use-workflows-nav-scope` under their names, against a fake Fleet (and a second one for another machine); then the rules the
//! native list keeps: a row opens its workflow, the words at the end of a row, and what another machine forgets.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::request::Folder;
use crate::fleet::new_session::starters::WorkflowRequest;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::nav::{BUILD_A_FEATURE, MetaTone, row_meta};
use crate::fleet::workflows::runs::{RECENT_RUNS, library_path};
use crate::mock::machines::Case;
use crate::mock::workflows::{REPOSITORY, Scene};
use crate::tests::{FRAME, app, click};
use crate::tests_workflow_runs::{Fake, fake};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

const KETTLE: &str = "/work/kettle";

/// The server's library: the two built-ins, and the repository's files when one is asked for.
fn library_json(repository: Option<&str>) -> Value {
    let step = |t: &str| json!({"id": t, "title": t, "kind": "agent"});
    let built = |id: &str, name: &str, steps: &[&str]| json!({"id": id, "builtIn": true, "file": null, "name": name, "description": null, "steps": steps.iter().map(|t| step(t)).collect::<Vec<_>>(), "errors": []});
    let mut workflows = vec![
        built("builtin:build-a-feature", "Build a feature", &["Explore", "Plan", "Build"]),
        built("builtin:fix-a-bug", "Fix a bug", &["Diagnose", "Fix"]),
    ];
    if repository.is_some() {
        workflows.push(json!({"id": "file:.weave/workflows/notes.yml", "builtIn": false, "file": ".weave/workflows/notes.yml",
            "name": "Write the notes", "steps": [step("Draft")], "errors": []}));
    }
    json!({"repository": repository, "repositoryName": repository.map(|r| r.rsplit('/').next().unwrap()), "workflows": workflows})
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
    other: Fake,
}

/// The app connected to a fake Fleet whose preferences are `prefs`; another machine, `m-harbor`, answers at a second fake
/// with its own. Nothing has been asked yet.
fn rig(prefs: Value, other_prefs: Value) -> Rig {
    let (home, other) = (fake(), fake());
    for (fleet, prefs) in [(&home, &prefs), (&other, &other_prefs)] {
        fleet.route("GET", "/api/preferences", 200, prefs.clone());
        fleet.route("PUT", "/api/preferences/Workflows", 200, json!({}));
        fleet.route("GET", &library_path(None), 200, library_json(None));
        fleet.route("GET", &library_path(Some(KETTLE)), 200, library_json(Some(KETTLE)));
        fleet.route("GET", RECENT_RUNS, 200, json!([]));
    }
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    let base = other.base.clone();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.show_machines_only(Case::Several);
        s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor is listed").base_url = base;
    });
    rt.frame(FRAME);
    Rig { rt, queue, home, other }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            format!("library {:?}, error {:?}", s.workflows_nav.library, s.workflows_nav.library_error)
        });
    }

    fn frames(&mut self) {
        while let Ok(job) = self.queue.try_recv() {
            self.rt.update(job);
        }
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    /// The preferences are read and the page is open.
    fn open(&mut self) {
        self.rt.update(|s, _| s.load_preferences());
        self.wait("the preferences", |s| s.pickers.prefs.fetched);
        self.rt.update(|s, _| s.enter_workflows());
        self.frames();
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }

    fn heard(&self, fleet: &Fake, method: &str, path: &str) -> usize {
        fleet.calls().iter().filter(|(m, p)| m == method && p == path).count()
    }

    fn wait_heard(&mut self, what: &str, home: bool, method: &'static str, path: &str) {
        let path = path.to_string();
        let calls = if home { self.home.calls_handle() } else { self.other.calls_handle() };
        self.wait(what, move |_| calls().iter().any(|(m, p)| m == method && *p == path));
    }
}

// ─── WorkflowsPageOff.test.ts ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn says_workflows_is_off_and_turns_it_on_from_the_button() {
    let mut r = rig(json!({}), json!({}));
    r.open();
    assert!(r.words().contains("Workflows is off"), "{}", r.words());
    assert!(!r.has("workflows-list"), "no list beside the notice");
    assert_eq!(r.heard(&r.home, "GET", &library_path(None)), 0, "nothing is read while it is off");

    click(&mut r.rt, "workflows-turn-on");
    r.wait_heard("the preference to be saved", true, "PUT", "/api/preferences/Workflows");
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    r.wait("the runs", |s| s.workflow_runs(HOME_MACHINE_KEY).loaded);
    r.frames();
    assert_eq!(r.heard(&r.home, "GET", &library_path(None)), 1);
    assert_eq!(r.heard(&r.home, "GET", RECENT_RUNS), 1);
    assert!(r.has("workflows-list") && !r.has("workflows-off"));
}

#[test]
fn says_nothing_until_the_switches_have_loaded_so_a_page_that_s_on_doesn_t_flash_is_off() {
    let mut r = rig(json!({"Workflows": "true"}), json!({}));
    r.rt.update(|s, _| s.enter_workflows());
    r.frames();
    assert!(!r.has("workflows-off"), "{}", r.words());
    assert!(!r.words().contains("is off"));
    r.open();
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    r.frames();
    assert!(!r.has("workflows-off"));
}

#[test]
fn shows_the_workflows_themselves_once_they_re_on() {
    let mut r = rig(json!({"Workflows": "true"}), json!({}));
    r.open();
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    r.frames();
    assert!(r.has("workflows-list") && r.has("workflows-detail"));
    // Model roles are on this page, in a dialog opened from the list.
    assert!(r.has("workflow-model-roles"));
    click(&mut r.rt, "workflow-model-roles");
    assert!(r.rt.state.workflows_nav.roles_open);
    assert!(!r.has("workflows-off"));
}

// ─── WorkflowsPageMachine.test.ts ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn says_workflows_is_off_for_the_machine_the_view_shows_and_follows_the_switch_both_ways() {
    let mut r = rig(json!({"Workflows": "true"}), json!({"Workflows": "false"}));
    r.open();
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    r.frames();
    assert!(r.has("workflows-list"));

    r.rt.update(|s, _| s.workflows_nav.choice.pick("m-harbor"));
    r.wait("harbor's preferences", |s| s.feeds.prefs.get("m-harbor").is_some_and(|p| p.fetched));
    r.frames();
    assert!(r.has("workflows-off"), "{}", r.words());
    assert!(!r.has("workflows-list"));

    r.rt.update(|s, _| s.workflows_nav.choice.pick(HOME_MACHINE_KEY));
    r.frames();
    r.wait("the library of the machine you're on", |s| s.workflows_nav.library.is_some());
    r.frames();
    assert!(!r.has("workflows-off"));
    assert!(r.has("workflows-list"));
}

#[test]
fn another_machine_forgets_the_last_one_s_library_folder_and_open_workflow() {
    let mut r = rig(json!({"Workflows": "true"}), json!({"Workflows": "true"}));
    r.open();
    r.rt.update(|s, _| {
        s.workflows_nav.set_folder(Folder::Repository(KETTLE.into()), true);
    });
    r.wait("the repository's library", |s| s.workflows_nav.library.as_ref().is_some_and(|l| l.repository.is_some()));
    r.rt.update(|s, _| s.set_active_workflow("file:.weave/workflows/notes.yml"));
    r.rt.update(|s, _| s.workflows_nav.choice.pick("m-harbor"));
    r.wait("harbor's library", |s| {
        s.workflows_nav.library.as_ref().is_some_and(|l| l.repository.is_none()) && !s.workflows_nav.loading_library
    });
    let nav = &r.rt.state.workflows_nav;
    assert_eq!((nav.repository_path(), nav.has_chosen_folder, nav.active_id()), (None, false, BUILD_A_FEATURE));
    assert!(r.heard(&r.other, "GET", &library_path(None)) >= 1, "harbor was asked, not the machine you're on");
    assert_eq!(r.heard(&r.other, "GET", &library_path(Some(KETTLE))), 0);
}

// ─── WorkflowsPage.test.ts ──────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn opens_the_workflow_a_link_names_in_its_folder_then_drops_them_from_the_address() {
    let mut r = rig(json!({"Workflows": "true"}), json!({}));
    r.open();
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    // The link is the Fix a bug starter's request; the address (a request that is taken) goes back to plain /workflows.
    r.rt.update(|s, _| {
        s.starters.workflow_request = Some(WorkflowRequest { workflow: "builtin:fix-a-bug".into(), folder: Some(KETTLE.into()) })
    });
    r.frames();
    r.wait("the repository's library", |s| s.workflows_nav.library.as_ref().is_some_and(|l| l.repository.as_deref() == Some(KETTLE)));
    let nav = &r.rt.state.workflows_nav;
    assert_eq!(nav.folder(), Some(&Folder::Repository(KETTLE.into())));
    assert!(nav.has_chosen_folder);
    assert_eq!(nav.active_id(), "builtin:fix-a-bug");
    assert!(r.rt.state.starters.workflow_request.is_none(), "the request is taken");
    assert_eq!(r.heard(&r.home, "GET", &library_path(Some(KETTLE))), 1);
}

#[test]
fn leaves_the_open_workflow_and_folder_alone_without_a_link() {
    let mut r = rig(json!({"Workflows": "true"}), json!({}));
    r.open();
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    r.frames();
    let nav = &r.rt.state.workflows_nav;
    assert_eq!((nav.folder(), nav.has_chosen_folder, nav.active_id()), (None, false, BUILD_A_FEATURE));
    assert_eq!(r.heard(&r.home, "GET", &library_path(None)), 1, "read once");
}

// ─── WorkflowsLoading.test.ts ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_the_repo_s_workflows_loading_not_pick_a_repository_until_the_picked_repo_answers() {
    let mut r = rig(json!({"Workflows": "true"}), json!({}));
    r.home.hold("GET", &library_path(Some(KETTLE)));
    r.open();
    r.wait("the library", |s| s.workflows_nav.library.is_some());
    r.frames();
    assert!(r.words().contains("Pick a repository in the Run box to see its workflows"));

    r.rt.update(|s, _| s.workflows_nav.set_folder(Folder::Repository(KETTLE.into()), true));
    r.wait_heard("the picked repository to be asked for", true, "GET", &library_path(Some(KETTLE)));
    r.frames();
    assert!(!r.words().contains("Pick a repository"), "{}", r.words());
    assert!(!r.words().contains("No workflow files in this repo yet"));
    assert!(r.has("workflows-repo-loading"));

    r.home.route("GET", &library_path(Some(KETTLE)), 200, library_json(Some(KETTLE)));
    r.wait("the answer", |s| !s.workflows_nav.loading_library);
    r.frames();
    assert!(!r.has("workflows-repo-loading"));
    assert!(r.words().contains("kettle · .weave/workflows"), "{}", r.words());
    assert!(r.has("workflow-row-file:.weave/workflows/notes.yml"));
}

// ─── use-workflows-nav-scope.test.ts ────────────────────────────────────────────────────────────────────────────────

#[test]
fn keeps_following_the_run_box_s_repository_while_another_page_is_open() {
    let mut r = rig(json!({"Workflows": "true"}), json!({}));
    r.rt.update(|s, _| s.load_preferences());
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    // Sessions is the page open: a session row (Save as workflow…) can be the first to ask.
    assert_eq!(r.rt.state.rail, Rail::Sessions);
    r.rt.update(|s, _| s.workflows_nav.set_folder(Folder::Repository(KETTLE.into()), true));
    r.wait("the library for the repository", |s| {
        s.workflows_nav.library.as_ref().is_some_and(|l| l.repository_name.as_deref() == Some("kettle"))
    });
    assert_eq!(r.heard(&r.home, "GET", &library_path(Some(KETTLE))), 1);
}

// ─── The native list ────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_row_opens_its_workflow_and_leaves_the_run_view() {
    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::Library));
    rt.frame(FRAME);
    assert_eq!(rt.state.workflows_nav.active_id(), BUILD_A_FEATURE);
    rt.update(|s, _| s.workflows_nav.showing_runs = true);
    click(&mut rt, "workflow-row-builtin:fix-a-bug");
    assert_eq!(rt.state.workflows_nav.active_id(), "builtin:fix-a-bug");
    assert!(!rt.state.workflows_nav.showing_runs);
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-row-builtin:fix-a-bug")).unwrap();
    assert_eq!(row.label.as_deref(), Some("Fix a bug"));
}

#[test]
fn a_leave_guard_that_says_no_keeps_the_workflow_open() {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_workflows(true, Scene::Library);
        s.workflows_nav.leave_guard = Some(|_| false);
    });
    rt.frame(FRAME);
    click(&mut rt, "workflow-row-builtin:fix-a-bug");
    assert_eq!(rt.state.workflows_nav.active_id(), BUILD_A_FEATURE);
    rt.update(|s, _| s.workflows_nav.leave_guard = Some(|_| true));
    click(&mut rt, "workflow-row-builtin:fix-a-bug");
    assert_eq!(rt.state.workflows_nav.active_id(), "builtin:fix-a-bug");
}

#[test]
fn a_row_says_the_first_that_holds_edited_can_t_read_needs_you_with_you_running_or_its_steps() {
    use crate::mock::workflows::{library, run};
    let lib = library(Some(REPOSITORY));
    let (feature, bug, notes, triage) = (&lib.workflows[0], &lib.workflows[1], &lib.workflows[2], &lib.workflows[3]);
    let waiting = run("a", &feature.id, "waiting", false, "2026-11-19T10:00:00Z");
    let with_you = run("b", &bug.id, "running", true, "2026-11-19T10:00:00Z");
    let running = run("c", &notes.id, "running", false, "2026-11-19T10:00:00Z");
    let done = run("d", &notes.id, "done", false, "2026-11-19T10:00:00Z");
    assert_eq!(row_meta(triage, Some(&waiting), true), ("Edited".into(), MetaTone::Wait));
    assert_eq!(row_meta(triage, Some(&waiting), false), ("Can't read".into(), MetaTone::Error));
    assert_eq!(row_meta(feature, Some(&waiting), false), ("Needs you".into(), MetaTone::Wait));
    assert_eq!(row_meta(bug, Some(&with_you), false), ("With you".into(), MetaTone::With));
    assert_eq!(row_meta(notes, Some(&running), false), ("Running".into(), MetaTone::Working));
    assert_eq!(row_meta(notes, Some(&done), false), ("3 steps".into(), MetaTone::Plain));
    let mut one = notes.clone();
    one.steps.truncate(1);
    assert_eq!(row_meta(&one, None, false), ("1 step".into(), MetaTone::Plain));
    assert_eq!(row_meta(feature, None, false), ("8 steps".into(), MetaTone::Plain));

    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::Library));
    rt.frame(FRAME);
    let words: Vec<String> = rt.elements().iter().filter_map(|e| e.text.clone()).collect();
    for want in ["Needs you", "With you", "Running", "Can't read", "Built into Fleet", "kettle · .weave/workflows"] {
        assert!(words.iter().any(|w| w == want), "no {want:?} in {words:?}");
    }
}

#[test]
fn the_new_workflow_button_waits_for_a_repository_and_says_why() {
    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::NoRepo));
    rt.frame(FRAME);
    let button = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-new")).unwrap();
    assert_eq!(button.help.as_deref(), Some("Pick a repository in the Run box first"));
    assert!(button.opacity < 1.0, "dimmed without a repository");
    rt.update(|s, _| s.show_workflows(true, Scene::Library));
    rt.frame(FRAME);
    let button = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-new")).unwrap();
    assert_eq!(button.help, None);
    assert!((button.opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_failed_library_says_the_server_s_words_and_the_first_read_shows_a_spinner() {
    let mut rt = app();
    rt.update(|s, _| s.show_workflows(true, Scene::Failed));
    rt.frame(FRAME);
    assert!(rt.find("workflows-list-error").is_some() && rt.find("workflows-error").is_some());
    rt.update(|s, _| s.show_workflows(true, Scene::Loading));
    rt.frame(FRAME);
    assert!(rt.find("workflows-loading").is_some());
}
