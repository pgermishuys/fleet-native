//! Component 18, piece R: New workflow and Duplicate. `WorkflowCreateDialog.test.ts`'s four dialog tests under their names (the
//! Describe it form's is piece S's), then what the native dialog adds: the buttons that open it, a blank name, Enter, the spinner
//! and the refusal's words clearing as you type, a repository picked elsewhere. A fake Fleet answers (`tests_workflow_editor`).
use crate::fleet::new_session::request::Folder;
use crate::fleet::workflows::draft::WorkflowFile;
use crate::fleet::workflows::editor::EditorView;
use crate::fleet::workflows::{Library, Workflow};
use crate::mock::workflows::Scene;
use crate::tests::{FRAME, click};
use crate::tests_workflow_editor::{FILES, REPO, Rig, check_json, ed, file_json, review_draft, rig};
use rune::{Key, KeyEvent, Mods};
use serde_json::{Value, json};

const TAKEN: &str = "There's already a workflow called Build a feature. Pick another name.";

fn library() -> Library {
    Library { repository: Some(REPO.into()), repository_name: Some("weave-fleet".into()), workflows: built_ins() }
}

fn built_ins() -> Vec<Workflow> {
    crate::mock::workflows::library(None).workflows
}

fn created_json(id: &str) -> Value {
    let slug = id.trim_start_matches("repo:");
    json!({"workflowId": id, "file": format!(".weave/workflows/{slug}.yaml"), "hash": "h1",
        "check": check_json(Some(review_draft()), "name: Tidy up a flaky test\n", json!([]), json!([]))})
}

impl Rig {
    /// The Workflows page on a repository picked in the Run box, with nothing open in the designer.
    fn page(&mut self) {
        self.rt.update(|s, _| {
            s.show_workflows(true, Scene::Library);
            let nav = &mut s.workflows_nav;
            nav.set_folder(Folder::Repository(REPO.into()), true);
            nav.library = Some(library());
            nav.library_repository = Some(REPO.into());
            nav.reload_wanted = false;
        });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn enter_name(&mut self, words: &str) {
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    fn press_enter(&mut self) {
        self.rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
        self.rt.frame(FRAME);
    }

    fn open(&self) -> bool {
        self.rt.find("workflow-create-dialog").is_some()
    }

    fn words_of(&self, key: &str) -> String {
        let all = self.rt.elements();
        let Some(root) = all.iter().find(|e| e.key.as_deref() == Some(key)) else { return String::new() };
        let inside = format!("{}/", root.path);
        all.iter()
            .filter(|e| e.path == root.path || e.path.starts_with(&inside))
            .filter_map(|e| e.text.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn wait_closed(&mut self) {
        self.wait("the dialog to close", |s| s.workflows_nav.create.is_none());
    }
}

#[test]
fn new_creates_the_file_in_the_repo_and_opens_it() {
    let mut r = rig();
    r.fleet.always("POST", FILES, 200, created_json("repo:tidy-up-a-flaky-test"));
    r.fleet.always(
        "GET",
        "/api/workflows",
        200,
        json!({"repository": REPO, "repositoryName": "weave-fleet", "workflows": [
            {"id": "builtin:build-a-feature", "builtIn": true, "name": "Build a feature", "steps": []},
            {"id": "repo:tidy-up-a-flaky-test", "builtIn": false, "file": ".weave/workflows/tidy-up-a-flaky-test.yaml", "name": "Tidy up a flaky test", "steps": []}]}),
    );
    r.page();
    click(&mut r.rt, "workflow-new");
    assert!(r.open());
    assert!(r.words_of("workflow-create-dialog").contains("New workflow"));
    r.enter_name("Tidy up a flaky test");
    assert_eq!(r.words_of("workflow-create-file"), "weave-fleet/.weave/workflows/tidy-up-a-flaky-test.yaml");
    click(&mut r.rt, "workflow-create-submit");
    r.wait_closed();

    assert_eq!(r.fleet.bodies("POST", FILES), vec![json!({"directory": REPO, "name": "Tidy up a flaky test", "workflowId": null})]);
    // The file opens as the server made it, with no read of it: the editor was given it.
    r.wait("the library to be read again", |s| {
        s.workflows_nav.library.as_ref().is_some_and(|l| l.workflows.iter().any(|w| w.id == "repo:tidy-up-a-flaky-test"))
    });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    let s = &r.rt.state;
    assert_eq!(s.workflows_nav.active_id(), "repo:tidy-up-a-flaky-test");
    assert_eq!(ed(s).file.as_ref().map(|f| f.workflow_id.as_str()), Some("repo:tidy-up-a-flaky-test"));
    assert_eq!(ed(s).view, EditorView::Designer);
    assert!(!s.workflows_nav.showing_runs);
    assert_eq!(r.fleet.count("POST", "/api/workflows/files/open"), 0, "it was not read again");
    assert!(r.rt.find("workflow-editor-header").is_some(), "the designer is showing");
}

#[test]
fn duplicate_copies_the_built_in_under_a_new_name() {
    let mut r = rig();
    r.fleet.always("POST", FILES, 200, created_json("repo:build-a-feature-our-way"));
    r.page();
    // Build a feature is the workflow the page opens on.
    click(&mut r.rt, "workflow-duplicate");
    assert!(r.open());
    assert!(r.words_of("workflow-create-dialog").contains("Duplicate Build a feature"));
    assert_eq!(r.rt.state.workflows_nav.create.as_ref().map(|c| c.name.text.as_str()), Some("Build a feature, our way"));
    assert_eq!(r.words_of("workflow-create-submit"), "Duplicate and open");
    click(&mut r.rt, "workflow-create-submit");
    r.wait_closed();
    assert_eq!(
        r.fleet.bodies("POST", FILES),
        vec![json!({"directory": REPO, "name": "Build a feature, our way", "workflowId": "builtin:build-a-feature"})]
    );
    assert_eq!(r.rt.state.workflows_nav.active_id(), "repo:build-a-feature-our-way");
}

#[test]
fn says_so_when_the_name_is_taken_and_stays_open() {
    let mut r = rig();
    r.fleet.always("POST", FILES, 409, json!({ "error": TAKEN }));
    r.page();
    click(&mut r.rt, "workflow-new");
    r.enter_name("Build a feature");
    click(&mut r.rt, "workflow-create-submit");
    r.wait("the refusal", |s| s.workflows_nav.create.as_ref().is_some_and(|c| c.error.is_some()));
    r.rt.frame(FRAME);
    assert!(r.open(), "the dialog stays open");
    assert_eq!(r.words_of("workflow-create-error"), TAKEN);
    assert!(r.rt.state.workflows_nav.editor.file.is_none(), "nothing was opened");
    // Another name is a new try: the words go when you type, and the button is on again.
    r.enter_name("!");
    assert!(r.rt.find("workflow-create-error").is_none());
    r.fleet.always("POST", FILES, 200, created_json("repo:build-a-feature"));
    click(&mut r.rt, "workflow-create-submit");
    r.wait_closed();
}

#[test]
fn duplicate_has_no_describe_it() {
    let mut r = rig();
    r.page();
    click(&mut r.rt, "workflow-duplicate");
    assert!(r.open());
    assert!(r.rt.find("workflow-create-describe").is_none());
    assert!(r.rt.find("workflow-create-blank").is_none());
    click(&mut r.rt, "workflow-create-cancel");
    click(&mut r.rt, "workflow-new");
    assert!(r.rt.find("workflow-create-describe").is_some(), "New has both tabs");
    assert!(r.rt.find("workflow-create-blank").is_some());
}

// ─── What the native dialog adds ───────────────────────────────────────────────────────────────────────────────

#[test]
fn describe_it_is_a_tab_of_new_and_its_form_is_the_next_piece() {
    let mut r = rig();
    r.page();
    click(&mut r.rt, "workflow-new");
    click(&mut r.rt, "workflow-create-describe");
    assert!(r.words_of("workflow-create-dialog").contains("Say what it should do."));
    assert!(r.rt.find("workflow-create-submit").is_none(), "no Create on this tab");
    click(&mut r.rt, "workflow-create-blank");
    assert!(r.rt.find("workflow-create-submit").is_some());
    assert!(r.words_of("workflow-create-dialog").contains("one agent step"));
}

#[test]
fn a_blank_name_makes_nothing_and_enter_does_what_the_button_does() {
    let mut r = rig();
    r.fleet.always("POST", FILES, 200, created_json("repo:tidy-up-a-flaky-test"));
    r.page();
    click(&mut r.rt, "workflow-new");
    assert_eq!(r.words_of("workflow-create-file"), "weave-fleet/.weave/workflows/workflow.yaml");
    assert!(!r.rt.state.workflow_can_create());
    r.enter_name("   ");
    r.press_enter();
    assert!(r.fleet.bodies("POST", FILES).is_empty(), "a blank name is not sent");
    r.enter_name("Tidy up a flaky test");
    r.press_enter();
    r.wait_closed();
    assert_eq!(r.fleet.bodies("POST", FILES).len(), 1);
}

#[test]
fn escape_and_cancel_close_it_and_the_next_one_starts_empty() {
    let mut r = rig();
    r.page();
    click(&mut r.rt, "workflow-new");
    r.enter_name("Half a name");
    r.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    r.rt.frame(FRAME);
    assert!(!r.open());
    click(&mut r.rt, "workflow-new");
    assert_eq!(r.rt.state.workflows_nav.create.as_ref().map(|c| c.name.text.clone()), Some(String::new()));
}

#[test]
fn the_buttons_are_off_without_a_repository() {
    let mut r = rig();
    r.rt.update(|s, _| s.show_workflows(true, Scene::NoRepo));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    r.rt.update(|s, tx| s.workflow_create_open(None, tx));
    r.rt.frame(FRAME);
    assert!(!r.open(), "nothing opens with no repository to put the file in");
}

#[test]
fn the_file_made_opens_even_when_it_is_not_the_workflow_the_server_named_first() {
    // The created file is adopted as the server sent it, so a hash and text the editor shows are the server's.
    let mut r = rig();
    r.fleet.always("POST", FILES, 200, file_json(check_json(Some(review_draft()), "name: X\n", json!([]), json!([])), "h9"));
    r.page();
    click(&mut r.rt, "workflow-new");
    r.enter_name("Build it our way");
    click(&mut r.rt, "workflow-create-submit");
    r.wait_closed();
    let file: &WorkflowFile = ed(&r.rt.state).file.as_ref().expect("a file is open");
    assert_eq!(file.hash, "h9");
    assert!(!ed(&r.rt.state).is_dirty());
}
