//! Component 18, piece S: drafting. `WorkflowCreateDialog.test.ts` "Describe it asks…", "useWorkflowsNav: drafting a workflow" (5),
//! the view halves of `WorkflowEditor.test.ts` "a drafted workflow" (9; the state halves are `tests_workflow_editor`), and
//! `SessionItemSaveAsWorkflow.test.ts` (4), under their names; then what the native frame adds (the list's draft row, one ask at
//! a time, a late answer dropped, the leave question). A fake Fleet answers (`tests_workflow_editor`).
use crate::Rail;
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses, Load};
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::draft::{DraftedWorkflow, WorkflowFile};
use crate::fleet::workflows::editor::EditorView;
use crate::fleet::workflows::nav::DRAFT_ID;
use crate::fleet::workflows::{Library, MODEL_ROLES_PREFERENCE_KEY, Workflow};
use crate::mock::workflows::Scene;
use crate::tests::{app, click};
use crate::tests_workflow_editor::{FILES, MISSING_MAX, REPO, Rig, check_json, drafted_ok, ed, file_json, review_draft, rig};
use rune::{Key, KeyEvent, Mods};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const FROM_SESSION: &str = "/api/workflows/drafts/from-session";
const FROM_DESCRIPTION: &str = "/api/workflows/drafts/from-description";

fn harness(kind: &str, name: &str, off_the_record: bool) -> HarnessEntry {
    HarnessEntry {
        display_name: name.into(),
        supports_workflow_steps: true,
        supports_off_the_record: off_the_record,
        ..HarnessEntry::ready(kind)
    }
}

fn library() -> Library {
    let wf = |id: &str, name: &str| Workflow {
        id: id.into(),
        name: name.into(),
        file: Some(format!(".weave/workflows/{}.yaml", &id[5..])),
        ..Workflow::default()
    };
    Library { repository: Some(REPO.into()), repository_name: Some("repo".into()), workflows: vec![wf("repo:ship-it", "Ship it")] }
}

fn drafted_json(title: Value) -> Value {
    json!({"repository": REPO, "repositoryName": "repo", "sessionTitle": title, "asks": 1, "tokens": {"total": 3412, "fromCache": 3100},
        "check": check_json(Some(review_draft()), "name: Build it our way\nsteps: []\n", json!([]), json!([]))})
}

impl Rig {
    /// The Workflows page on a repository, two harnesses that can be asked off the record, and Standard mapped to a model.
    fn drafts_page(&mut self) {
        self.rt.update(|s, _| {
            s.show_workflows(true, Scene::Library);
            let nav = &mut s.workflows_nav;
            nav.set_folder(Folder::Repository(REPO.into()), true);
            nav.library = Some(library());
            nav.library_repository = Some(REPO.into());
            nav.reload_wanted = false;
            s.feeds.harnesses.entry(HOME_MACHINE_KEY.into()).or_default().harnesses = Harnesses {
                all: vec![
                    harness("opencode", "OpenCode", true),
                    harness("opencode2", "OpenCode 2", true),
                    harness("claude-code", "Claude Code", false),
                ],
                load: Load::Done,
                preferred: Some("opencode".into()),
            };
            s.pickers.prefs.set(MODEL_ROLES_PREFERENCE_KEY, r#"{"opencode":{"standard":{"model":"openai/gpt-5.4","effort":null}}}"#);
        });
        self.frames();
    }

    fn type_in(&mut self, words: &str) {
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.frames();
    }

    /// Answers to asks have come (the dropped ones too), `n` in all.
    fn wait_answers(&mut self, n: u64) {
        self.wait("the answers to come", |s| s.workflows_nav.draft_answers >= n);
        self.frames();
    }

    fn error(&self) -> Option<String> {
        self.rt.state.workflows_nav.drafting.as_ref().and_then(|d| d.error.clone())
    }

    fn wait_drafted(&mut self) {
        self.wait("the draft to come", |s| s.workflows_nav.drafted.is_some());
        self.frames();
    }

    /// Save as workflow… on a session, as the row's menu does it.
    fn save_as_workflow(&mut self) {
        self.rt.update(|s, _| {
            s.enter_workflows();
            s.workflow_draft_from_session("session-1", "Fix the login bug");
        });
        self.frames();
    }
}

fn disabled_answer(fleet: &crate::tests_workflow_editor::Fleet, path: &str, gate: &Arc<AtomicBool>, answer: Value) {
    let gate = gate.clone();
    fleet.on("POST", path, move |_| {
        // A model that takes its time: the answer waits until the test lets it go.
        while !gate.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        (200, answer.clone())
    });
}

// ─── WorkflowCreateDialog.test.ts ───────────────────────────────────────────────────────────────────────────────

#[test]
fn describe_it_asks_for_what_the_workflow_should_do_says_what_it_costs_and_hands_it_on_to_be_drafted() {
    let mut r = rig();
    r.fleet.always("POST", FROM_DESCRIPTION, 200, drafted_json(Value::Null));
    r.drafts_page();
    click(&mut r.rt, "workflow-new");
    click(&mut r.rt, "workflow-create-describe");
    r.frames();

    assert!(r.words("workflow-create-dialog").contains("What should it do?"));
    assert_eq!(
        r.words("workflow-describe-cost"),
        "Asks the model once. From a session it reads the conversation from the cache, so it's cheap."
    );
    assert!(!r.rt.state.workflow_can_describe(), "Draft it waits for the words");
    // Only the harnesses workflows run on that can ask off the record, on their Standard model.
    let names: Vec<String> = r.rt.state.workflow_describe_harnesses().into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["OpenCode", "OpenCode 2"]);
    assert_eq!(r.words("workflow-describe-model"), "On your Standard model: gpt-5.4.");

    click(&mut r.rt, "workflow-describe-harness");
    r.frames();
    click(&mut r.rt, "workflow-describe-harness-list-opt1");
    r.type_in("  Bump the dependencies and check nothing broke.  ");
    assert!(r.rt.state.workflow_can_describe());
    click(&mut r.rt, "workflow-describe-submit");

    assert!(r.rt.state.workflows_nav.create.is_none(), "the dialog closes");
    r.wait_drafted();
    assert_eq!(
        r.fleet.bodies("POST", FROM_DESCRIPTION),
        [json!({"directory": REPO, "description": "Bump the dependencies and check nothing broke.", "harnessType": "opencode2"})]
    );
    // Nothing is created: the draft opens unsaved.
    assert_eq!(r.fleet.count("POST", FILES), 0);
    assert!(r.rt.state.workflows_nav.editor.drafted.is_some());
}

#[test]
fn describe_it_names_the_harnesses_it_needs_when_none_can_be_asked() {
    let mut r = rig();
    r.drafts_page();
    r.rt.update(|s, _| {
        s.feeds.harnesses.entry(HOME_MACHINE_KEY.into()).or_default().harnesses =
            Harnesses { all: vec![harness("claude-code", "Claude Code", false)], load: Load::Done, preferred: None };
    });
    click(&mut r.rt, "workflow-new");
    click(&mut r.rt, "workflow-create-describe");
    r.type_in("Do something.");
    assert_eq!(r.words("workflow-describe-model"), "Describe it needs a harness workflows run on: OpenCode or OpenCode 2.");
    assert!(!r.rt.state.workflow_can_describe());
}

#[test]
fn describe_it_with_one_harness_has_nothing_to_pick() {
    let mut r = rig();
    r.drafts_page();
    r.rt.update(|s, _| {
        s.feeds.harnesses.entry(HOME_MACHINE_KEY.into()).or_default().harnesses =
            Harnesses { all: vec![harness("opencode2", "OpenCode 2", true)], load: Load::Done, preferred: None };
    });
    click(&mut r.rt, "workflow-new");
    click(&mut r.rt, "workflow-create-describe");
    click(&mut r.rt, "workflow-describe-harness");
    assert!(r.rt.find("workflow-describe-harness-list-menu").is_none(), "a select with one choice stays shut");
    assert_eq!(r.rt.state.workflow_describe_harness().as_deref(), Some("opencode2"));
}

// ─── use-workflows-nav-drafts.test.ts, "useWorkflowsNav: drafting a workflow" ───────────────────────────────────

#[test]
fn says_it_s_drafting_from_the_session_then_hands_the_draft_over_and_picks_its_repository() {
    let mut r = rig();
    let gate = Arc::new(AtomicBool::new(false));
    disabled_answer(&r.fleet, FROM_SESSION, &gate, drafted_json(json!("Fix the login bug")));
    r.drafts_page();
    r.rt.update(|s, _| s.workflows_nav.set_folder(Folder::Repository("/work/elsewhere".into()), true));

    r.save_as_workflow();
    let nav = &r.rt.state.workflows_nav;
    assert_eq!(nav.active_id(), DRAFT_ID);
    assert_eq!(nav.drafting.as_ref().and_then(|d| d.ask.session_title()), Some("Fix the login bug"));
    assert_eq!(r.error(), None);
    assert_eq!(r.words("workflow-drafting-title"), "Drafting a workflow from Fix the login bug…");
    assert_eq!(
        r.words("workflow-drafting-note"),
        "The model is reading the conversation from the cache and writing the steps. It takes up to a minute or two."
    );
    assert!(r.has("workflow-drafting-cancel"));
    assert_eq!(r.words("workflow-draft-row"), "New workflow Drafting…");
    let heard = r.fleet.heard_handle();
    r.wait("the ask to be heard", |_| heard("POST", FROM_SESSION) == 1);
    assert_eq!(r.fleet.bodies("POST", FROM_SESSION), [json!({"sessionId": "session-1"})]);

    gate.store(true, Ordering::SeqCst);
    r.wait_drafted();
    let nav = &r.rt.state.workflows_nav;
    assert!(nav.drafting.is_none());
    assert_eq!(nav.drafted.as_ref().map(|d| d.repository.as_str()), Some(REPO));
    assert_eq!(nav.repository_path(), Some(REPO), "the draft's repository is the Run box's folder");
}

#[test]
fn asks_from_a_description_with_the_chosen_harness() {
    let mut r = rig();
    r.fleet.always("POST", FROM_DESCRIPTION, 200, drafted_json(Value::Null));
    r.drafts_page();
    r.rt.update(|s, _| {
        s.workflow_start_draft(crate::fleet::workflows::drafting::Ask::Description {
            directory: REPO.into(),
            description: "Bump the dependencies.".into(),
            harness_type: Some("opencode2".into()),
        })
    });
    assert_eq!(r.rt.state.workflows_nav.drafting.as_ref().and_then(|d| d.ask.session_title()), None);
    r.wait_drafted();
    assert_eq!(
        r.fleet.bodies("POST", FROM_DESCRIPTION),
        [json!({"directory": REPO, "description": "Bump the dependencies.", "harnessType": "opencode2"})]
    );
    assert_eq!(r.rt.state.workflows_nav.drafted.as_ref().unwrap().session_title, None);
}

#[test]
fn cancel_stops_waiting_for_the_request_and_goes_back_to_the_library() {
    let mut r = rig();
    let gate = Arc::new(AtomicBool::new(false));
    disabled_answer(&r.fleet, FROM_SESSION, &gate, drafted_json(json!("Fix the login bug")));
    r.drafts_page();
    r.save_as_workflow();
    let heard = r.fleet.heard_handle();
    r.wait("the ask to be heard", |_| heard("POST", FROM_SESSION) == 1);

    click(&mut r.rt, "workflow-drafting-cancel");
    let nav = &r.rt.state.workflows_nav;
    assert!(nav.drafting.is_none() && nav.drafted.is_none());
    assert_ne!(nav.active_id(), DRAFT_ID);
    assert!(!r.has("workflow-draft-row"));

    // The answer that comes after is dropped: nothing opens.
    gate.store(true, Ordering::SeqCst);
    r.wait_answers(1);
    let nav = &r.rt.state.workflows_nav;
    assert!(nav.drafted.is_none() && nav.editor.drafted.is_none());
    assert_ne!(nav.active_id(), DRAFT_ID);
}

#[test]
fn a_failure_says_why_and_can_be_tried_again() {
    let mut r = rig();
    r.fleet.always("POST", FROM_SESSION, 400, json!({"error": "The model took too long to answer. Try again."}));
    r.drafts_page();
    r.save_as_workflow();
    r.wait("the failure", |s| s.workflows_nav.drafting.as_ref().is_some_and(|d| d.error.is_some()));
    r.frames();

    assert_eq!(r.error().as_deref(), Some("The model took too long to answer. Try again."));
    assert_eq!(r.words("workflow-drafting-error"), "The model took too long to answer. Try again.");
    assert_eq!(r.words("workflow-draft-row"), "New workflow Failed");
    assert!(r.has("workflow-drafting-close"));

    r.fleet.always("POST", FROM_SESSION, 200, drafted_json(json!("Fix the login bug")));
    click(&mut r.rt, "workflow-drafting-retry");
    assert_eq!(r.error(), None);
    r.wait_drafted();
    assert_eq!(r.fleet.count("POST", FROM_SESSION), 2);
    assert!(r.rt.state.workflows_nav.drafted.is_some());
}

#[test]
fn saving_the_draft_makes_it_the_repository_s_workflow() {
    let mut r = rig();
    r.drafts_page();
    r.rt.update(|s, _| {
        s.workflow_draft_now(crate::fleet::workflows::drafting::Ask::Session { id: "s".into(), title: "Fix the login bug".into() })
    });
    r.rt.update(|s, _| s.workflows_nav.draft_saved("repo:fix-a-bug"));
    let nav = &r.rt.state.workflows_nav;
    assert_eq!(nav.active_id(), "repo:fix-a-bug");
    assert!(nav.drafted.is_none() && nav.drafting.is_none());
}

// ─── WorkflowEditor.test.ts, "a drafted workflow": what is drawn ────────────────────────────────────────────────

impl Rig {
    fn open_draft(&mut self, drafted: DraftedWorkflow) {
        self.drafts_page();
        self.rt.update(move |s, _| {
            s.workflows_nav.drafted = Some(drafted.clone());
            s.workflows_nav.editor.adopt_draft(HOME_MACHINE_KEY, drafted);
            s.workflows_nav.show_draft();
        });
        self.frames();
    }
}

#[test]
fn opens_unsaved_in_the_designer_with_a_banner_saying_where_it_came_from_and_what_it_cost_drawn() {
    let mut r = rig();
    r.open_draft(drafted_ok());

    let banner = r.words("workflow-drafted-banner");
    assert!(banner.contains("Drafted from Fix the login bug. Review it before saving."), "{banner}");
    assert!(banner.contains("Nothing is saved until you press Save."));
    assert!(banner.contains("Asked the model once · 3,412 tokens, 3,100 from the cache."));
    assert!(!banner.contains("It still has errors"));
    assert_eq!(r.rt.state.workflows_nav.editor.view, EditorView::Designer);
    assert_eq!(r.words("workflow-unsaved"), "Unsaved");
    assert_eq!(r.words("workflow-file-name"), ".weave/workflows/build-it-our-way.yaml");
    assert!(r.save_enabled());
    assert_eq!(r.words("workflow-draft-row"), "Build it our way Unsaved");
    // Nothing is written until Save.
    assert_eq!(r.fleet.count("POST", FILES), 0);
}

#[test]
fn says_it_was_drafted_from_a_description_and_shows_no_count_when_the_harness_gives_none_drawn() {
    let mut r = rig();
    let mut d = drafted_ok();
    d.session_title = None;
    d.tokens = None;
    r.open_draft(d);
    let banner = r.words("workflow-drafted-banner");
    assert!(banner.contains("Drafted from your description. Review it before saving."), "{banner}");
    assert!(banner.contains("Asked the model once."));
    assert!(!banner.contains("tokens"));
}

#[test]
fn the_file_it_would_make_follows_the_name_as_it_s_edited_drawn() {
    let mut r = rig();
    r.open_draft(drafted_ok());
    r.edit_draft(|d| d.name = "Fix a login bug".into());
    r.frames();
    assert_eq!(r.words("workflow-file-name"), ".weave/workflows/fix-a-login-bug.yaml");
}

#[test]
fn save_creates_the_file_and_the_banner_goes_with_the_draft() {
    let mut r = rig();
    r.open_draft(drafted_ok());
    click(&mut r.rt, "workflow-save");
    r.drain();
    assert!(!r.has("workflow-drafted-banner"));
    assert!(r.rt.state.workflows_nav.editor.drafted.is_none());
    assert_eq!(r.rt.state.workflows_nav.editor.file.as_ref().map(|f| f.workflow_id.as_str()), Some("repo:build-it-our-way"));
    assert!(!r.rt.state.workflows_nav.editor.is_dirty());
    // The page follows: the draft is the repository's workflow, with no row of its own.
    assert_eq!(r.rt.state.workflows_nav.active_id(), "repo:build-it-our-way");
    assert!(r.rt.state.workflows_nav.drafted.is_none());
    assert!(!r.has("workflow-draft-row"));
}

#[test]
fn a_name_that_s_taken_is_refused_with_the_server_s_message_and_the_draft_stays_unsaved_drawn() {
    let mut r = rig();
    let taken = "There's already a workflow called Build it our way. Pick another name.";
    r.fleet.always("POST", FILES, 409, json!({"error": taken}));
    r.open_draft(drafted_ok());
    click(&mut r.rt, "workflow-save");
    r.wait("the refusal", |s| ed(s).save_error.is_some());
    r.frames();
    assert!(r.words("workflow-problems").contains(taken));
    assert!(r.has("workflow-drafted-banner"));
    assert!(!r.has("workflow-conflict"));
}

#[test]
fn a_draft_that_still_has_errors_opens_in_the_file_view_with_them_and_save_waits_drawn() {
    let mut r = rig();
    let mut d = drafted_ok();
    d.asks = 2;
    d.tokens = Some(crate::fleet::workflows::draft::DraftTokens { total: 7300, from_cache: 6800 });
    d.check = crate::tests_workflow_editor::check_of(
        Some(review_draft()),
        "name: Build it our way\n",
        json!([{"line": 38, "message": MISSING_MAX, "step": 3}]),
        json!([]),
    );
    r.open_draft(d);
    assert_eq!(r.rt.state.workflows_nav.editor.view, EditorView::File);
    assert!(r.words("workflow-problems").contains(&format!("line 38 · {MISSING_MAX}")), "{}", r.words("workflow-problems"));
    let banner = r.words("workflow-drafted-banner");
    assert!(banner.contains("It still has errors: fix them in the File view, then save."), "{banner}");
    assert!(banner.contains("Asked the model twice: its first answer had errors · 7,300 tokens, 6,800 from the cache."));
    assert!(!r.save_enabled());
}

#[test]
fn a_draft_the_parser_couldn_t_read_at_all_opens_in_the_file_view_drawn() {
    let mut r = rig();
    let mut d = drafted_ok();
    d.check = crate::tests_workflow_editor::check_of(
        None,
        "I can't do that.\n",
        json!([{"line": 1, "message": "The file isn't a mapping.", "step": null}]),
        json!([]),
    );
    r.open_draft(d);
    assert_eq!(r.rt.state.workflows_nav.editor.view, EditorView::File);
    assert!(r.has("workflow-file-editor"));
    assert_eq!(r.rt.state.workflows_nav.editor.text, "I can't do that.\n");
}

#[test]
fn try_it_asks_to_save_first() {
    let mut r = rig();
    r.open_draft(drafted_ok());
    click(&mut r.rt, "workflow-try");
    assert_eq!(r.words("workflow-notice"), "Save first: a run uses the file as saved.");
}

// ─── SessionItemSaveAsWorkflow.test.ts ──────────────────────────────────────────────────────────────────────────

/// The row menu of the first project's second session, on the machine you're on.
fn menu_of(harness_type: &str, workflows_on: bool, archived: bool) -> Vec<(String, Option<String>, bool)> {
    let mut rt = app();
    let kind = harness_type.to_string();
    rt.update(move |s, _| {
        s.pickers.prefs.set("Workflows", if workflows_on { "true" } else { "false" });
        s.feeds.harnesses.entry(HOME_MACHINE_KEY.into()).or_default().harnesses = Harnesses {
            all: vec![harness("opencode", "OpenCode", true), harness("claude-code", "Claude Code", false)],
            load: Load::Done,
            preferred: None,
        };
        s.projects[0].sessions[1].row.harness_type = Some(kind);
        s.projects[0].sessions[1].row.archived = archived;
    });
    let s = &rt.state;
    let menu = crate::fleet::row_menu::build(s, &s.projects[0].sessions[1], None);
    menu.entries
        .iter()
        .filter_map(|e| match e {
            crate::ui::menu::Entry::Item(i) => Some((i.label.clone(), i.hint.clone(), i.disabled)),
            _ => None,
        })
        .collect()
}

#[test]
fn is_under_repeat_on_a_schedule_and_says_what_it_costs_for_a_harness_that_can_ask_off_the_record() {
    let items = menu_of("opencode", true, false);
    let at = |label: &str| items.iter().position(|(l, ..)| l == label).unwrap_or_else(|| panic!("no {label}"));
    let save = at("Save as workflow\u{2026}");
    assert_eq!(
        items[save].1.as_deref(),
        Some("Asks the model once. From a session it reads the conversation from the cache, so it's cheap.")
    );
    assert_eq!(save, at("Repeat on a schedule\u{2026}") + 1);
}

#[test]
fn opens_workflows_and_asks_for_a_draft_from_the_session() {
    let mut r = rig();
    r.fleet.always("POST", FROM_SESSION, 200, drafted_json(json!("Fix the login bug")));
    r.drafts_page();
    r.rt.update(|s, _| {
        s.projects[0].sessions[1].id = "session-1".into();
        s.projects[0].sessions[1].title = "Fix the login bug".into();
        s.projects[0].sessions[1].row.harness_type = Some("opencode".into());
        s.rail = Rail::Sessions;
    });
    let press = {
        let s = &r.rt.state;
        let menu = crate::fleet::row_menu::build(s, &s.projects[0].sessions[1], None);
        menu.entries
            .into_iter()
            .find_map(|e| match e {
                crate::ui::menu::Entry::Item(i) if i.id.as_deref() == Some("session-save-as-workflow") => i.run,
                _ => None,
            })
            .expect("the item")
    };
    r.rt.update(move |s, tx| press(s, tx));
    assert_eq!(r.rt.state.rail, Rail::Workflows);
    assert_eq!(r.rt.state.workflows_nav.active_id(), DRAFT_ID);
    r.wait_drafted();
    assert_eq!(r.fleet.bodies("POST", FROM_SESSION), [json!({"sessionId": "session-1"})]);
    assert_eq!(r.rt.state.workflows_nav.drafted.as_ref().and_then(|d| d.session_title.as_deref()), Some("Fix the login bug"));
}

#[test]
fn isn_t_offered_on_a_harness_that_can_t_ask_off_the_record() {
    assert!(menu_of("claude-code", true, false).iter().all(|(l, ..)| l != "Save as workflow\u{2026}"));
}

#[test]
fn isn_t_offered_with_workflows_off_or_on_an_archived_session() {
    assert!(menu_of("opencode", true, true).iter().all(|(l, ..)| l != "Save as workflow\u{2026}"));
    assert!(menu_of("opencode", false, false).iter().all(|(l, ..)| l != "Save as workflow\u{2026}"));
}

// ─── What the native frame adds ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_second_ask_replaces_the_first_and_only_the_second_answer_opens() {
    let mut r = rig();
    let gate = Arc::new(AtomicBool::new(false));
    disabled_answer(&r.fleet, FROM_SESSION, &gate, drafted_json(json!("The first one")));
    r.fleet.always("POST", FROM_DESCRIPTION, 200, drafted_json(Value::Null));
    r.drafts_page();
    r.save_as_workflow();
    r.rt.update(|s, _| {
        s.workflow_start_draft(crate::fleet::workflows::drafting::Ask::Description {
            directory: REPO.into(),
            description: "Another.".into(),
            harness_type: None,
        })
    });
    r.wait_drafted();
    gate.store(true, Ordering::SeqCst);
    r.wait_answers(2);
    assert_eq!(r.rt.state.workflows_nav.drafted.as_ref().unwrap().session_title, None, "the first answer was dropped");
}

#[test]
fn a_draft_over_unsaved_edits_asks_first_and_a_no_sends_nothing() {
    let mut r = rig();
    r.mount(false);
    r.rt.update(|s, _| s.workflow_draft_from_session("session-1", "Fix the login bug"));
    assert!(r.rt.state.ui.confirm.is_none(), "nothing unsaved, nothing asked");
    let heard = r.fleet.heard_handle();
    r.wait("the first ask to be heard", |_| heard("POST", FROM_SESSION) == 1);
    r.rt.update(|s, _| s.workflows_nav.draft_saved("repo:build-it-our-way"));
    r.rt.update(|s, _| s.workflows_nav.editor.edit_text("name: Edited\n"));
    r.rt.update(|s, _| s.workflow_draft_from_session("session-2", "Another session"));
    r.frames();
    let c = r.rt.state.ui.confirm.clone().expect("the question");
    assert_eq!(c.title, "Discard your unsaved changes?");
    click(&mut r.rt, "confirm-cancel");
    assert!(r.rt.state.workflows_nav.drafting.is_none());
    assert_eq!(r.fleet.count("POST", FROM_SESSION), 1, "only the first ask was sent");
}

#[test]
fn leaving_a_draft_for_another_workflow_forgets_it() {
    let mut r = rig();
    r.open_draft(drafted_ok());
    click(&mut r.rt, "workflow-row-repo:ship-it");
    click(&mut r.rt, "confirm-action");
    let nav = &r.rt.state.workflows_nav;
    assert_eq!(nav.active_id(), "repo:ship-it");
    assert!(nav.drafted.is_none() && nav.editor.drafted.is_none());
    assert!(!r.has("workflow-draft-row"));
}

#[test]
fn the_draft_row_does_nothing_when_pressed_and_no_other_page_shows_it() {
    let mut r = rig();
    r.open_draft(drafted_ok());
    click(&mut r.rt, "workflow-draft-row");
    assert_eq!(r.rt.state.workflows_nav.active_id(), DRAFT_ID);
    assert!(r.has("workflow-drafted-banner"));
}

#[test]
fn a_file_made_by_new_is_not_a_draft() {
    // `adopt` (New, Duplicate) clears what a draft left; only `adopt_draft` makes an unsaved one.
    let mut r = rig();
    r.open_draft(drafted_ok());
    let file =
        WorkflowFile::from_json(&file_json(check_json(Some(review_draft()), "name: Build it our way\n", json!([]), json!([])), "h1"));
    r.rt.update(move |s, _| s.workflows_nav.editor.adopt(HOME_MACHINE_KEY, REPO, file));
    assert!(r.rt.state.workflows_nav.editor.drafted.is_none());
}
