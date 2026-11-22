//! Component 18, piece M: the editor's frame, drawn. The view halves of Fleet's `WorkflowEditor.test.ts` ("WorkflowEditor") under
//! their names (the canvas's halves are piece O's), "useWorkflowsNav: leaving unsaved edits" (4), and what the native frame
//! adds: Ctrl/Cmd+S, the row saying Edited, Edit in the run view, a failed open. A fake Fleet answers (`tests_workflow_editor`).
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::editor::EditorView;
use crate::fleet::workflows::{Library, Workflow};
use crate::mock::workflows::{Scene, run};
use crate::tests::{FRAME, click, press_key};
use crate::tests_workflow_editor::{CHECK, FILES, MISSING_MAX, REPO, Rig, check_json, ed, review_draft, rig};
use crate::{Rail, State};
use rune::{Key, Mods};
use serde_json::json;

const ID: &str = "repo:build-it-our-way";

fn library() -> Library {
    let wf = |id: &str, name: &str| Workflow {
        id: id.into(),
        name: name.into(),
        file: Some(format!(".weave/workflows/{}.yaml", id.trim_start_matches("repo:"))),
        ..Workflow::default()
    };
    Library {
        repository: Some(REPO.into()),
        repository_name: Some("repo".into()),
        workflows: vec![wf(ID, "Build it our way"), wf("repo:ship-it", "Ship it")],
    }
}

impl Rig {
    /// The Workflows page on a repository with two files, `ID` open in the editor as `adopt` leaves it.
    pub(crate) fn mount(&mut self, comments: bool) {
        self.rt.update(|s, _| {
            s.show_workflows(true, Scene::Library);
            let nav = &mut s.workflows_nav;
            nav.set_folder(Folder::Repository(REPO.into()), true);
            nav.library = Some(library());
            nav.library_repository = Some(REPO.into());
            nav.reload_wanted = false;
            nav.move_to(ID);
        });
        self.adopt(comments);
        self.frames();
    }

    pub(crate) fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    /// The text of an element and of everything inside it, in order, joined by a space.
    pub(crate) fn words(&self, key: &str) -> String {
        let all = self.rt.elements();
        let Some(root) = all.iter().find(|e| e.key.as_deref() == Some(key)) else { return String::new() };
        let inside = format!("{}/", root.path);
        all.iter()
            .filter(|e| e.path == root.path || e.path.starts_with(&inside))
            .filter_map(|e| e.text.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn press(&mut self, key: &str) {
        click(&mut self.rt, key);
    }

    pub(crate) fn save_enabled(&self) -> bool {
        self.rt.state.workflows_nav.editor.can_save() && self.has("workflow-save")
    }

    /// The wait after the last key ends and the check answers; the frame is drawn after.
    pub(crate) fn settled(&mut self) {
        self.settle();
        self.frames();
    }

    pub(crate) fn drain(&mut self) {
        self.saved();
        self.frames();
    }
}

fn open_runs(r: &mut Rig, statuses: &[&str]) {
    let statuses: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
    r.rt.update(move |s, _| {
        for (i, status) in statuses.iter().enumerate() {
            s.workflows.of_mut(HOME_MACHINE_KEY).upsert(run(&format!("x{i}"), ID, status, false, "2026-11-19T10:00:00Z"));
        }
    });
}

// ─── WorkflowEditor.test.ts, "WorkflowEditor" ───────────────────────────────────────────────────────────────────

#[test]
fn shows_the_parser_s_error_on_the_step_and_under_the_canvas_and_save_waits_until_it_s_fixed() {
    let mut r = rig();
    r.mount(false);
    assert!(!r.save_enabled(), "nothing edited");

    let broken = json!({"line": 38, "message": MISSING_MAX, "step": 3});
    r.fleet.on("POST", CHECK, move |body| (200, check_json(body.get("draft").cloned(), "written", json!([broken.clone()]), json!([]))));
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.steps[3].max_loops = None;
        e.edit_draft(next);
    });
    r.settled();
    assert_eq!(r.words("workflow-problem"), format!("line 38 \u{b7} {MISSING_MAX}"));
    assert_eq!(r.words("workflow-unsaved"), "Unsaved");
    assert!(!r.save_enabled());

    r.fleet.on("POST", CHECK, |body| (200, check_json(body.get("draft").cloned(), "written", json!([]), json!([]))));
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.steps[3].max_loops = Some(3);
        e.edit_draft(next);
    });
    r.settled();
    assert_eq!(r.words("workflow-problems-ok"), "No problems. Checked by the same parser runs use.");
    assert!(r.save_enabled());

    r.press("workflow-save");
    r.drain();
    let put = &r.fleet.bodies("PUT", FILES)[0];
    assert_eq!(put["workflowId"], json!(ID));
    assert_eq!(put["draft"]["steps"][3]["maxLoops"], json!(3));
    assert_eq!(r.words("workflow-notice"), "Saved .weave/workflows/build-it-our-way.yaml. Commit it with your other changes to share it.");
    assert!(r.rt.state.workflows_nav.reload_wanted || r.rt.state.workflows_nav.loading_library, "the library reloads");
}

#[test]
fn says_runs_in_progress_keep_the_version_they_started_with() {
    let mut r = rig();
    open_runs(&mut r, &["running", "waiting", "done"]);
    r.mount(false);
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.name = "Build it, our way".into();
        e.edit_draft(next);
    });
    r.settled();
    assert_eq!(r.words("workflow-runs-note"), "2 runs in progress keep the version they started with.");

    r.press("workflow-save");
    r.drain();
    assert!(r.words("workflow-notice").ends_with("2 runs in progress keep the version they started with."));
    assert!(!r.has("workflow-runs-note"), "the note shows while there are edits");
}

#[test]
fn warns_about_comments_and_the_first_save_from_the_designer_asks_and_lists_them() {
    let mut r = rig();
    r.mount(true);
    assert!(r.words("workflow-comments-banner").contains("This file has 2 comments."));

    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.steps[0].title = "Outline".into();
        e.edit_draft(next);
    });
    r.settled();
    r.press("workflow-save");
    r.wait("the question", |s| ed(s).asking_to_remove_comments);
    r.frames();
    assert!(r.has("workflow-comments-confirm-dialog"));
    let said = r.words("workflow-comments-confirm-dialog");
    assert!(r.rt.elements().iter().any(|e| e.label.as_deref() == Some("Save and remove 2 comments?")), "{said}");
    assert!(said.contains("# Kept by the platform team.") && said.contains("# Minor versions only."));
    assert_eq!(r.fleet.count("PUT", FILES), 0);

    r.press("workflow-comments-confirm-save");
    r.drain();
    assert_eq!(r.fleet.bodies("PUT", FILES)[0]["draft"]["steps"][0]["title"], json!("Outline"));
    assert!(!r.has("workflow-comments-confirm-dialog"));
}

#[test]
fn edit_in_file_view_keeps_the_comments_the_file_view_saves_the_text_as_it_is() {
    let mut r = rig();
    r.mount(true);
    r.press("workflow-comments-file-view");
    r.frames();
    assert_eq!(r.rt.state.workflows_nav.editor.view, EditorView::File);
    assert_eq!(r.rt.state.workflows_nav.editor.text, "# Kept by the platform team.\nname: Build it our way\n");
    assert!(!r.has("workflow-comments-banner"), "the banner is the designer's");
    assert_eq!(r.words("workflow-problems-ok"), "No problems. The File view saves your text as it is, comments included.");

    let comments = json!([{"line": 1, "text": "Kept by the platform team."}]);
    r.fleet
        .on("POST", CHECK, move |b| (200, check_json(Some(review_draft()), b["text"].as_str().unwrap_or(""), json!([]), comments.clone())));
    r.rt.update(|s, _| s.workflows_nav.editor.edit_text("# Kept by the platform team.\nname: Build it, our way\n"));
    r.settled();
    r.press("workflow-save");
    r.drain();
    assert_eq!(r.fleet.bodies("PUT", FILES)[0]["text"], json!("# Kept by the platform team.\nname: Build it, our way\n"));
    assert!(!r.has("workflow-comments-confirm-dialog"));
}

#[test]
fn edit_in_file_view_from_the_confirmation_goes_back_to_the_text_with_its_comments_not_the_designer_s_edits() {
    let mut r = rig();
    r.mount(true);
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.steps[0].title = "Outline".into();
        e.edit_draft(next);
    });
    r.settled();
    r.press("workflow-save");
    r.wait("the question", |s| ed(s).asking_to_remove_comments);
    r.frames();

    let comments = json!([{"line": 1, "text": "Kept by the platform team."}, {"line": 9, "text": "Minor versions only."}]);
    r.fleet
        .on("POST", CHECK, move |b| (200, check_json(Some(review_draft()), b["text"].as_str().unwrap_or(""), json!([]), comments.clone())));
    r.press("workflow-comments-confirm-file");
    r.wait("the check of the text", |s| !ed(s).is_busy());
    r.frames();

    let e = r.rt.state.workflows_nav.editor.view;
    assert_eq!(e, EditorView::File);
    assert_eq!(r.rt.state.workflows_nav.editor.text, "# Kept by the platform team.\nname: Build it our way\n");
    assert!(!r.rt.state.workflows_nav.editor.is_dirty(), "the designer's edits are gone");
    assert!(!r.has("workflow-unsaved"));
    assert!(!r.has("workflow-comments-confirm-dialog"));
    assert_eq!(r.fleet.count("PUT", FILES), 0);
}

#[test]
fn stays_in_the_file_view_when_the_designer_can_t_show_the_text() {
    let mut r = rig();
    r.mount(false);
    r.press("workflow-view-file");
    r.frames();
    assert_eq!(r.rt.state.workflows_nav.editor.view, EditorView::File);

    r.rt.update(|s, _| s.workflows_nav.editor.edit_text("name: [broken\n"));
    r.press("workflow-view-designer");
    r.wait("the designer to be refused", |s| ed(s).designer_blocked.is_some());
    r.frames();

    assert_eq!(r.rt.state.workflows_nav.editor.view, EditorView::File);
    assert!(r.words("workflow-designer-blocked").contains("Fix the errors in the File view to use the designer."));
    assert!(r.words("workflow-problem").starts_with("line 1 \u{b7} This isn't valid YAML"));
}

#[test]
fn offers_reload_or_keep_mine_when_the_file_changed_on_disk() {
    let mut r = rig();
    r.mount(false);
    changed_on_disk(&mut r);
    assert!(r.has("workflow-conflict-dialog"));
    let titles: Vec<_> = r.rt.elements().into_iter().filter_map(|e| e.label).collect();
    assert!(titles.iter().any(|t| t == "build-it-our-way.yaml changed on disk"), "{titles:?}");

    r.fleet.always(
        "PUT",
        FILES,
        200,
        crate::tests_workflow_editor::file_json(check_json(Some(review_draft()), "kept", json!([]), json!([])), "h2"),
    );
    r.press("workflow-conflict-keep");
    r.drain();
    let puts = r.fleet.bodies("PUT", FILES);
    assert_eq!(puts.len(), 2);
    assert_eq!((puts[1]["force"].clone(), puts[1]["hash"].clone()), (json!(true), json!("h1")));
    assert_eq!(r.rt.state.workflows_nav.editor.file.as_ref().unwrap().hash, "h2");
    assert!(!r.has("workflow-conflict-dialog"));
}

fn changed_on_disk(r: &mut Rig) {
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.name = "Mine".into();
        e.edit_draft(next);
    });
    r.settled();
    r.fleet.always("PUT", FILES, 409, json!({"error": "This file changed on disk since you opened it."}));
    r.press("workflow-save");
    r.wait("the conflict", |s| ed(s).conflict.is_some());
    r.frames();
}

#[test]
fn reload_takes_the_file_as_it_is_on_disk_and_drops_the_edits() {
    let mut r = rig();
    r.mount(false);
    changed_on_disk(&mut r);

    r.press("workflow-conflict-reload");
    r.wait("the file to be read again", |s| ed(s).file.as_ref().is_some_and(|f| f.hash == "h3") && !ed(s).is_busy());
    r.frames();
    let last = r.fleet.workflow_requests().last().cloned().expect("a request");
    assert_eq!(last, ("POST".to_string(), "/api/workflows/files/open".to_string()));
    assert_eq!(r.fleet.bodies("POST", "/api/workflows/files/open")[0], json!({"directory": REPO, "workflowId": ID}));
    assert_eq!(r.rt.state.workflows_nav.editor.draft.as_ref().unwrap().name, "Build it our way");
    assert!(!r.has("workflow-unsaved") && !r.has("workflow-conflict-dialog"));
}

#[test]
fn try_it_asks_to_save_first_while_there_are_unsaved_edits() {
    let mut r = rig();
    r.mount(false);
    r.press("workflow-try");
    assert!(r.rt.state.workflows_nav.showing_runs, "nothing unsaved: the run view");
    assert!(!r.has("workflow-notice"));
    r.rt.update(|s, _| s.workflows_nav.showing_runs = false);
    r.frames();

    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.name = "Changed".into();
        e.edit_draft(next);
    });
    r.settled();
    r.press("workflow-try");
    assert!(!r.rt.state.workflows_nav.showing_runs);
    assert_eq!(r.words("workflow-notice"), "Save first: a run uses the file as saved.");
}

// ─── useWorkflowsNav: leaving unsaved edits ─────────────────────────────────────────────────────────────────────

fn edit_name(r: &mut Rig) {
    r.rt.update(|s, _| {
        let e = &mut s.workflows_nav.editor;
        let mut next = e.draft.clone().expect("a draft");
        next.name = "Edited".into();
        e.edit_draft(next);
    });
    r.settled();
}

#[test]
fn moves_at_once_when_the_guard_answers_yes_on_the_spot() {
    let mut r = rig();
    r.mount(false);
    r.press("workflow-row-repo:ship-it");
    assert_eq!(r.rt.state.workflows_nav.active_id(), "repo:ship-it");
    assert!(r.rt.state.ui.confirm.is_none());
}

#[test]
fn waits_for_the_user_s_answer_while_the_guard_asks_and_stays_when_they_cancel() {
    let mut r = rig();
    r.mount(false);
    edit_name(&mut r);
    r.press("workflow-row-repo:ship-it");
    assert_eq!(r.rt.state.workflows_nav.active_id(), ID, "waiting for the answer");
    let c = r.rt.state.ui.confirm.clone().expect("the question");
    assert_eq!((c.title.as_str(), c.subject.as_str()), ("Discard your unsaved changes?", "Edited"));
    assert_eq!(
        (c.body.as_str(), c.confirm_label.as_str(), c.destructive),
        ("Your edits to this workflow are lost.", "Discard changes", true)
    );

    r.press("confirm-cancel");
    assert_eq!(r.rt.state.workflows_nav.active_id(), ID);
    assert!(r.rt.state.ui.confirm.is_none());
    assert!(r.rt.state.workflows_nav.editor.is_dirty(), "the edits stay");
}

#[test]
fn moves_once_the_user_says_to_discard() {
    let mut r = rig();
    r.mount(false);
    edit_name(&mut r);
    r.press("workflow-row-repo:ship-it");
    r.press("confirm-action");
    assert_eq!(r.rt.state.workflows_nav.active_id(), "repo:ship-it");
    assert!(r.rt.state.ui.confirm.is_none());
}

#[test]
fn doesn_t_start_a_draft_over_unsaved_edits_the_user_keeps() {
    // Piece S starts drafts through the same question (`Leave`); until then the move it guards is another row, and what keeps
    // holding is that a "no" sends nothing to the server.
    let mut r = rig();
    r.mount(false);
    edit_name(&mut r);
    r.press("workflow-row-repo:ship-it");
    r.press("confirm-cancel");
    assert_eq!(r.rt.state.workflows_nav.active_id(), ID);
    assert!(r.fleet.workflow_requests().iter().all(|(_, path)| !path.starts_with("/api/workflows/drafts")));
}

// ─── What the native frame adds ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_drafted_workflow_asks_to_discard_the_draft() {
    let mut r = rig();
    r.mount(false);
    r.rt.update(|s, _| {
        let file = s.workflows_nav.editor.file.clone().unwrap();
        let drafted = crate::fleet::workflows::draft::DraftedWorkflow {
            repository: REPO.into(),
            repository_name: "repo".into(),
            check: file.check,
            asks: 1,
            ..Default::default()
        };
        s.workflows_nav.editor.adopt_draft(HOME_MACHINE_KEY, drafted);
    });
    r.frames();
    r.press("workflow-row-repo:ship-it");
    let c = r.rt.state.ui.confirm.clone().expect("the question");
    assert_eq!(
        (c.title.as_str(), c.body.as_str(), c.confirm_label.as_str()),
        ("Discard the drafted workflow?", "It hasn't been saved.", "Discard draft")
    );
}

#[test]
fn ctrl_or_cmd_s_saves_wherever_the_focus_is() {
    let mut r = rig();
    r.mount(false);
    edit_name(&mut r);
    let mods =
        if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { ctrl: true, cmd: true, ..Mods::default() } };
    press_key(&mut r.rt, Key::Command('s'), mods);
    r.drain();
    assert_eq!(r.fleet.count("PUT", FILES), 1);
}

#[test]
fn ctrl_s_with_nothing_to_save_does_nothing() {
    let mut r = rig();
    r.mount(false);
    let mods =
        if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { ctrl: true, cmd: true, ..Mods::default() } };
    press_key(&mut r.rt, Key::Command('s'), mods);
    assert_eq!(r.fleet.count("PUT", FILES), 0);
}

#[test]
fn the_list_row_says_edited_while_the_open_workflow_is_dirty() {
    let mut r = rig();
    r.mount(false);
    assert_eq!(r.rt.state.workflows_nav.unsaved, None);
    edit_name(&mut r);
    assert_eq!(r.rt.state.workflows_nav.unsaved.as_deref(), Some(ID));
    r.press("workflow-save");
    r.drain();
    assert_eq!(r.rt.state.workflows_nav.unsaved, None);
}

#[test]
fn edit_in_the_run_view_goes_back_to_the_designer() {
    let mut r = rig();
    r.mount(false);
    r.press("workflow-try");
    r.frames();
    assert!(r.has("workflow-edit") && !r.has("workflow-editor"));
    r.press("workflow-edit");
    r.frames();
    assert!(r.has("workflow-editor") && !r.rt.state.workflows_nav.showing_runs);
}

#[test]
fn a_failed_open_is_a_note_in_the_middle_and_is_not_asked_again() {
    let mut r = rig();
    r.fleet.always("POST", "/api/workflows/files/open", 404, json!({"error": "There's no such workflow file."}));
    r.rt.update(|s, _| {
        s.show_workflows(true, Scene::Library);
        let nav = &mut s.workflows_nav;
        nav.set_folder(Folder::Repository(REPO.into()), true);
        nav.library = Some(library());
        nav.library_repository = Some(REPO.into());
        nav.reload_wanted = false;
        nav.move_to(ID);
    });
    r.wait("the open to be refused", |s| ed(s).load_error.is_some());
    r.frames();
    assert_eq!(r.words("workflow-editor-error"), "There's no such workflow file.");
    r.frames();
    r.frames();
    assert_eq!(r.fleet.count("POST", "/api/workflows/files/open"), 1);
}

#[test]
fn leaving_the_page_drops_the_editor() {
    let mut r = rig();
    r.mount(false);
    edit_name(&mut r);
    r.rt.update(|s, _| s.rail = Rail::Sessions);
    r.frames();
    let s: &State = &r.rt.state;
    assert!(s.workflows_nav.editor.file.is_none() && s.workflows_nav.unsaved.is_none());
}
