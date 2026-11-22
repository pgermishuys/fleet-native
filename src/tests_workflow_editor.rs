//! Component 18, piece L: the editor's state and the file routes against a fake Fleet. The state halves of
//! `WorkflowEditor.test.ts`, "WorkflowEditor" and "a drafted workflow", under their names (the strip, the banners, the dialogs
//! and the buttons are piece M's, the canvas piece O's), then the rules the state keeps: the check waits 300 ms after the last
//! key and sends once, an overtaken check is ignored, a view switch or a save waits for the check, a failed open says so.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::draft::{DraftTokens, DraftedWorkflow, WorkflowCheck, WorkflowDraft, WorkflowFile, draft_cost, drafted_from};
use crate::fleet::workflows::editor::{CHECK_DELAY, DESIGNER_BLOCKED_MESSAGE, EditorView, Source};
use crate::tests::{FRAME, app};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

pub(crate) const REPO: &str = "/work/repo";
pub(crate) const FILES: &str = "/api/workflows/files";
pub(crate) const CHECK: &str = "/api/workflows/check";
pub(crate) const OPEN: &str = "/api/workflows/files/open";
pub(crate) const MISSING_MAX: &str =
    "review sends work back to an earlier step, so it needs a max: how many times it may do that in a run.";

pub(crate) type Heard = Arc<Mutex<Vec<(String, String, Value)>>>;
pub(crate) type Handler = Arc<dyn Fn(&Value) -> (u16, Value) + Send + Sync>;

/// A Fleet whose workflow routes answer from what they are sent, as the real server's would (a check echoes its text or
/// writes its draft), and whose answers a test can replace per route. Every request is recorded.
pub(crate) struct Fleet {
    pub(crate) base: String,
    /// Set when the fake is dropped: its accept loop then ends instead of holding a thread for the rest of the test run
    /// (the macOS runner has run out of threads with every test's fake still listening).
    stop: Arc<AtomicBool>,
    pub(crate) heard: Heard,
    pub(crate) routes: Arc<Mutex<HashMap<String, Handler>>>,
}

pub(crate) fn review_draft() -> Value {
    let step = |id: &str, title: &str| json!({"id": id, "title": title, "kind": "agent", "prompt": format!("Do {title}.")});
    json!({"name": "Build it our way", "description": null, "placeholder": null, "steps": [
        step("plan", "Plan"), step("ok-plan", "OK the plan"), step("implement", "Implement"),
        {"id": "review", "title": "Review", "kind": "agent", "outcomes": ["approved", "changes"], "routes": {"approved": "end", "changes": "implement"}, "maxLoops": 2, "prompt": "Review it."}
    ]})
}

pub(crate) fn check_json(draft: Option<Value>, text: &str, errors: Value, comments: Value) -> Value {
    json!({"text": text, "errors": errors, "draft": draft, "comments": comments})
}

pub(crate) fn file_json(check: Value, hash: &str) -> Value {
    json!({"workflowId": "repo:build-it-our-way", "file": ".weave/workflows/build-it-our-way.yaml", "hash": hash, "check": check})
}

pub(crate) fn default_answer(method: &str, path: &str, body: &Value) -> (u16, Value) {
    let draft = body.get("draft").filter(|d| !d.is_null()).cloned();
    let text = body.get("text").and_then(Value::as_str);
    match (method, path) {
        ("POST", CHECK) => match text {
            Some(t) if t.starts_with("name: [broken") => {
                (200, check_json(None, t, json!([{"line": 1, "message": "This isn't valid YAML: …", "step": null}]), json!([])))
            }
            Some(t) => (200, check_json(Some(review_draft()), t, json!([]), json!([]))),
            None => (200, check_json(draft, "written by the check", json!([]), json!([]))),
        },
        ("POST", OPEN) => (200, file_json(check_json(Some(review_draft()), "name: Build it our way\n", json!([]), json!([])), "h3")),
        ("PUT", FILES) | ("POST", FILES) => {
            let hash = if method == "PUT" { "h2" } else { "h4" };
            let written = text.unwrap_or("written");
            (200, file_json(check_json(draft.or_else(|| Some(review_draft())), written, json!([]), json!([])), hash))
        }
        _ => (404, json!({})),
    }
}

pub(crate) fn fleet() -> Fleet {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard: Heard = Default::default();
    let routes: Arc<Mutex<HashMap<String, Handler>>> = Default::default();
    let (log, table) = (heard.clone(), routes.clone());
    let stop = Arc::new(AtomicBool::new(false));
    let ending = stop.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if ending.load(Ordering::SeqCst) {
                return;
            }
            let (log, table) = (log.clone(), table.clone());
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
                let route = req.route().to_string();
                if route.starts_with("/api/workflows") {
                    log.lock().unwrap().push((req.method.clone(), route.clone(), body.clone()));
                }
                let custom = table.lock().unwrap().get(&format!("{} {route}", req.method)).cloned();
                // A handler may wait (a slow check): it runs with no lock held.
                let (status, answer) = match custom {
                    Some(handler) => handler(&body),
                    None => default_answer(&req.method, &route, &body),
                };
                crate::test_http::respond_json(&mut stream, format!("{status} X"), &answer.to_string());
            });
        }
    });
    Fleet { base, stop, heard, routes }
}

impl Drop for Fleet {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The accept loop is waiting for a connection: one wakes it to see the flag.
        drop(std::net::TcpStream::connect(self.base.trim_start_matches("http://")));
    }
}

impl Fleet {
    pub(crate) fn on(&self, method: &str, path: &str, handler: impl Fn(&Value) -> (u16, Value) + Send + Sync + 'static) {
        self.routes.lock().unwrap().insert(format!("{method} {path}"), Arc::new(handler));
    }

    /// The same answer every time.
    pub(crate) fn always(&self, method: &str, path: &str, status: u16, answer: Value) {
        self.on(method, path, move |_| (status, answer.clone()));
    }

    /// The requests heard for this method and path: their bodies, oldest first.
    pub(crate) fn bodies(&self, method: &str, path: &str) -> Vec<Value> {
        self.heard.lock().unwrap().iter().filter(|(m, p, _)| m == method && p == path).map(|(.., b)| b.clone()).collect()
    }

    pub(crate) fn count(&self, method: &str, path: &str) -> usize {
        self.bodies(method, path).len()
    }

    pub(crate) fn heard_handle(&self) -> impl Fn(&str, &str) -> usize + use<> {
        let heard = self.heard.clone();
        move |method, path| heard.lock().unwrap().iter().filter(|(m, p, _)| m == method && p == path).count()
    }

    pub(crate) fn workflow_requests(&self) -> Vec<(String, String)> {
        self.heard.lock().unwrap().iter().map(|(m, p, _)| (m.clone(), p.clone())).collect()
    }
}

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    pub(crate) queue: Receiver<Job>,
    pub(crate) fleet: Fleet,
}

pub(crate) fn rig() -> Rig {
    let fleet = fleet();
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), fleet.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| s.link = Some(link));
    rt.frame(FRAME);
    Rig { rt, queue, fleet }
}

pub(crate) fn check_of(draft: Option<Value>, text: &str, errors: Value, comments: Value) -> WorkflowCheck {
    WorkflowCheck::from_json(&check_json(draft, text, errors, comments))
}

pub(crate) fn ed(s: &State) -> &crate::fleet::workflows::editor::Editor {
    &s.workflows_nav.editor
}

impl Rig {
    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            let e = ed(s);
            format!(
                "view {:?}, source {:?}, checking {}, saving {}, save error {:?}, conflict {:?}, requests {:?}",
                e.view,
                e.source,
                e.is_checking,
                e.is_saving,
                e.save_error,
                e.conflict,
                self.fleet.workflow_requests()
            )
        });
    }

    /// Time passes: the clock `poll` reads moves on.
    pub(crate) fn later(&mut self, secs: f64) {
        self.rt.frame(secs);
    }

    /// The wait after the last key ends and the check it sends is answered.
    pub(crate) fn settle(&mut self) {
        self.later(FRAME);
        self.later(CHECK_DELAY + 0.01);
        self.wait("the check to be answered", |s| !ed(s).is_busy());
    }

    /// The file `reviewDraft` opens as, adopted as New would.
    pub(crate) fn adopt(&mut self, comments: bool) {
        let (text, found) = if comments {
            (
                "# Kept by the platform team.\nname: Build it our way\n",
                json!([{"line": 1, "text": "Kept by the platform team."}, {"line": 9, "text": "Minor versions only."}]),
            )
        } else {
            ("name: Build it our way\n", json!([]))
        };
        let file = WorkflowFile::from_json(&file_json(check_json(Some(review_draft()), text, json!([]), found), "h1"));
        self.rt.update(move |s, _| s.workflows_nav.editor.adopt(HOME_MACHINE_KEY, REPO, file));
    }

    pub(crate) fn adopt_draft(&mut self, drafted: DraftedWorkflow) {
        self.rt.update(move |s, _| s.workflows_nav.editor.adopt_draft(HOME_MACHINE_KEY, drafted));
    }

    pub(crate) fn edit_draft(&mut self, change: impl FnOnce(&mut WorkflowDraft) + Send + 'static) {
        self.rt.update(move |s, _| {
            let e = &mut s.workflows_nav.editor;
            let mut next = e.draft.clone().expect("a draft is open");
            change(&mut next);
            e.edit_draft(next);
        });
    }

    pub(crate) fn edit_text(&mut self, text: &'static str) {
        self.rt.update(move |s, _| s.workflows_nav.editor.edit_text(text));
    }

    pub(crate) fn save(&mut self) {
        self.rt.update(|s, _| s.workflow_editor_save(false));
    }

    pub(crate) fn view(&mut self, view: EditorView) {
        self.rt.update(move |s, _| s.workflow_editor_set_view(view));
    }

    pub(crate) fn ed(&self) -> &crate::fleet::workflows::editor::Editor {
        ed(&self.rt.state)
    }

    /// A save was answered (or refused).
    pub(crate) fn saved(&mut self) {
        self.wait("the save to be answered", |s| !ed(s).is_saving && ed(s).file.is_some());
    }
}

pub(crate) fn drafted(check: WorkflowCheck) -> DraftedWorkflow {
    DraftedWorkflow {
        repository: REPO.into(),
        repository_name: "repo".into(),
        session_title: Some("Fix the login bug".into()),
        check,
        asks: 1,
        tokens: Some(DraftTokens { total: 3412, from_cache: 3100 }),
    }
}

pub(crate) fn drafted_ok() -> DraftedWorkflow {
    drafted(check_of(Some(review_draft()), "name: Build it our way\nsteps: []\n", json!([]), json!([])))
}

// ─── WorkflowEditor.test.ts, "WorkflowEditor" ───────────────────────────────────────────────────────────────────

#[test]
fn shows_the_parser_s_error_on_the_step_and_under_the_canvas_and_save_waits_until_it_s_fixed() {
    let mut r = rig();
    r.adopt(false);
    assert!(!r.ed().can_save(), "nothing edited");
    assert!(!r.ed().is_dirty());

    let broken = json!({"line": 38, "message": MISSING_MAX, "step": 3});
    r.fleet.on("POST", CHECK, move |body| {
        (200, check_json(body.get("draft").cloned(), "written by the check", json!([broken.clone()]), json!([])))
    });
    r.edit_draft(|d| d.steps[3].max_loops = None);
    assert!(r.ed().is_dirty(), "Unsaved");
    assert!(!r.ed().can_save(), "a check is waiting");
    r.settle();

    let sent = &r.fleet.bodies("POST", CHECK)[0];
    assert!(sent["draft"]["steps"][3]["maxLoops"].is_null());
    let e = r.ed();
    assert_eq!((e.errors().len(), e.errors()[0].line, e.errors()[0].step), (1, 38, Some(3)));
    assert_eq!(e.errors()[0].message, MISSING_MAX);
    assert!(e.is_dirty() && !e.can_save(), "Save waits until it's fixed");

    r.fleet.on("POST", CHECK, |body| (200, check_json(body.get("draft").cloned(), "written by the check", json!([]), json!([]))));
    r.edit_draft(|d| d.steps[3].max_loops = Some(3));
    r.settle();
    assert!(r.ed().errors().is_empty() && r.ed().can_save());

    r.save();
    r.saved();
    let puts = r.fleet.bodies("PUT", FILES);
    assert_eq!(puts.len(), 1);
    let sent = &puts[0];
    assert_eq!(
        (sent["directory"].as_str(), sent["workflowId"].as_str(), sent["hash"].as_str()),
        (Some(REPO), Some("repo:build-it-our-way"), Some("h1"))
    );
    assert_eq!(sent["force"], json!(false));
    assert_eq!(sent["draft"]["steps"][3]["maxLoops"], json!(3));
    assert!(sent.get("text").is_none());
    let saved = r.rt.state.workflows_nav.editor.take_saved().expect("a save was announced");
    assert_eq!(saved.file, ".weave/workflows/build-it-our-way.yaml");
    assert_eq!(r.rt.state.workflows_nav.editor.take_saved(), None, "announced once");
    assert!(!r.ed().is_dirty(), "the saved file is the open one");
    assert_eq!(r.ed().file.as_ref().unwrap().hash, "h2");
}

#[test]
fn says_runs_in_progress_keep_the_version_they_started_with() {
    // The sentence is the frame's (M); the state's half is what it counts by: the save names the workflow whose runs these are.
    let mut r = rig();
    r.adopt(false);
    r.edit_draft(|d| d.name = "Build it, our way".into());
    r.settle();
    r.save();
    r.saved();
    let saved = r.rt.state.workflows_nav.editor.take_saved().expect("a save was announced");
    assert_eq!(saved.workflow_id, "repo:build-it-our-way");
    assert_eq!(r.fleet.bodies("PUT", FILES)[0]["draft"]["name"], json!("Build it, our way"));
}

#[test]
fn warns_about_comments_and_the_first_save_from_the_designer_asks_and_lists_them() {
    let mut r = rig();
    r.adopt(true);
    let comments: Vec<(u32, &str)> = r.ed().comments.iter().map(|c| (c.line, c.text.as_str())).collect();
    assert_eq!(comments, [(1, "Kept by the platform team."), (9, "Minor versions only.")]);

    r.edit_draft(|d| d.steps[0].title = "Outline".into());
    r.settle();
    r.save();
    // The ask is made from the check's answer, so wait for it to be put.
    r.wait("the question", |s| ed(s).asking_to_remove_comments);
    assert_eq!(r.fleet.count("PUT", FILES), 0);
    assert!(!r.ed().is_saving);

    r.rt.update(|s, _| s.workflow_editor_confirm_remove_comments());
    r.saved();
    let saved = &r.fleet.bodies("PUT", FILES)[0];
    assert_eq!(saved["draft"]["steps"][0]["title"], json!("Outline"));
    assert!(!r.ed().asking_to_remove_comments);

    // Once per open file: another designer save doesn't ask again.
    r.edit_draft(|d| d.steps[0].title = "Outline it".into());
    r.settle();
    r.save();
    r.saved();
    assert_eq!(r.fleet.count("PUT", FILES), 2);
}

#[test]
fn edit_in_file_view_keeps_the_comments_the_file_view_saves_the_text_as_it_is() {
    let mut r = rig();
    r.adopt(true);

    r.rt.update(|s, _| s.workflow_editor_keep_comments());
    assert_eq!(r.ed().view, EditorView::File);
    assert_eq!(r.ed().text, "# Kept by the platform team.\nname: Build it our way\n");

    let comments = json!([{"line": 1, "text": "Kept by the platform team."}]);
    r.fleet.on("POST", CHECK, move |body| {
        (200, check_json(Some(review_draft()), body["text"].as_str().unwrap_or(""), json!([]), comments.clone()))
    });
    r.edit_text("# Kept by the platform team.\nname: Build it, our way\n");
    r.settle();
    r.save();
    r.saved();

    let saved = &r.fleet.bodies("PUT", FILES)[0];
    assert_eq!(saved["text"], json!("# Kept by the platform team.\nname: Build it, our way\n"));
    assert!(saved.get("draft").is_none());
    assert!(!r.ed().asking_to_remove_comments, "a text save never asks");
}

#[test]
fn edit_in_file_view_from_the_confirmation_goes_back_to_the_text_with_its_comments_not_the_designer_s_edits() {
    let mut r = rig();
    r.adopt(true);
    r.edit_draft(|d| d.steps[0].title = "Outline".into());
    r.settle();
    r.save();
    r.wait("the question", |s| ed(s).asking_to_remove_comments);

    let comments = json!([{"line": 1, "text": "Kept by the platform team."}, {"line": 9, "text": "Minor versions only."}]);
    r.fleet.on("POST", CHECK, move |body| {
        (200, check_json(Some(review_draft()), body["text"].as_str().unwrap_or(""), json!([]), comments.clone()))
    });
    r.rt.update(|s, _| s.workflow_editor_keep_comments());
    r.wait("the check of the text", |s| !ed(s).is_busy());

    let e = r.ed();
    assert_eq!(e.view, EditorView::File);
    assert_eq!(e.text, "# Kept by the platform team.\nname: Build it our way\n");
    assert!(!e.is_dirty());
    assert!(!e.asking_to_remove_comments);
    let checks = r.fleet.bodies("POST", CHECK);
    assert_eq!(checks.last().unwrap(), &json!({"text": "# Kept by the platform team.\nname: Build it our way\n"}));
    assert_eq!(r.fleet.count("PUT", FILES), 0);
}

#[test]
fn stays_in_the_file_view_when_the_designer_can_t_show_the_text() {
    let mut r = rig();
    r.adopt(false);
    r.view(EditorView::File);
    assert_eq!(r.ed().view, EditorView::File);

    r.edit_text("name: [broken\n");
    // Asking for the designer checks what's waiting first; the answer is that it can't be shown.
    r.view(EditorView::Designer);
    r.wait("the designer to be refused", |s| ed(s).designer_blocked.is_some());

    let e = r.ed();
    assert_eq!(e.view, EditorView::File);
    assert_eq!(e.designer_blocked.as_deref(), Some(DESIGNER_BLOCKED_MESSAGE));
    assert_eq!((e.errors()[0].line, e.errors()[0].message.as_str()), (1, "This isn't valid YAML: …"));
    assert_eq!(r.fleet.bodies("POST", CHECK), [json!({"text": "name: [broken\n"})], "one check, flushed by the switch, not twice");

    // Typing again takes the message down.
    r.edit_text("name: Build it our way\n");
    assert_eq!(r.ed().designer_blocked, None);
}

#[test]
fn offers_reload_or_keep_mine_when_the_file_changed_on_disk() {
    let mut r = rig();
    r.adopt(false);
    r.edit_draft(|d| d.name = "Mine".into());
    r.settle();
    r.fleet.always("PUT", FILES, 409, json!({"error": "This file changed on disk since you opened it."}));

    r.save();
    r.wait("the conflict", |s| ed(s).conflict.is_some());
    assert_eq!(r.ed().conflict.as_deref(), Some("This file changed on disk since you opened it."));
    assert!(r.ed().is_dirty(), "the edits stay");
    assert_eq!(r.ed().save_error, None, "a conflict is not a save error");

    r.fleet.on("PUT", FILES, |body| (200, file_json(check_json(body.get("draft").cloned(), "written", json!([]), json!([])), "h2")));
    r.rt.update(|s, _| s.workflow_editor_keep_mine());
    r.wait("the forced save", |s| ed(s).file.as_ref().is_some_and(|f| f.hash == "h2"));

    let puts = r.fleet.bodies("PUT", FILES);
    assert_eq!(puts.len(), 2);
    assert_eq!((puts[1]["force"].clone(), puts[1]["hash"].clone()), (json!(true), json!("h1")));
    assert_eq!(r.ed().conflict, None);
}

#[test]
fn reload_takes_the_file_as_it_is_on_disk_and_drops_the_edits() {
    let mut r = rig();
    r.adopt(false);
    r.edit_draft(|d| d.name = "Mine".into());
    r.settle();
    r.fleet.always("PUT", FILES, 409, json!({"error": "changed"}));
    r.save();
    r.wait("the conflict", |s| ed(s).conflict.is_some());

    r.rt.update(|s, _| s.workflow_editor_reload());
    r.wait("the file to be read again", |s| ed(s).file.as_ref().is_some_and(|f| f.hash == "h3"));

    assert_eq!(r.fleet.bodies("POST", OPEN).last().unwrap(), &json!({"directory": REPO, "workflowId": "repo:build-it-our-way"}));
    let e = r.ed();
    assert_eq!(e.draft.as_ref().unwrap().name, "Build it our way");
    assert!(!e.is_dirty() && e.conflict.is_none());
}

// ─── WorkflowEditor.test.ts, "a drafted workflow" ───────────────────────────────────────────────────────────────

#[test]
fn opens_unsaved_in_the_designer_with_a_banner_saying_where_it_came_from_and_what_it_cost() {
    let mut r = rig();
    let d = drafted_ok();
    assert_eq!(drafted_from(&d), "Drafted from Fix the login bug. Review it before saving.");
    assert_eq!(draft_cost(&d), "Asked the model once · 3,412 tokens, 3,100 from the cache.");
    r.adopt_draft(d);

    let e = r.ed();
    assert_eq!(e.view, EditorView::Designer);
    assert_eq!(e.draft.as_ref().unwrap().steps.len(), 4);
    assert!(e.is_dirty(), "Unsaved");
    assert_eq!(e.file_name(), ".weave/workflows/build-it-our-way.yaml");
    assert!(e.drafted.is_some());
    // Nothing is written until Save.
    assert_eq!(r.fleet.workflow_requests(), []);
}

#[test]
fn says_it_was_drafted_from_a_description_and_shows_no_count_when_the_harness_gives_none() {
    let mut d = drafted_ok();
    d.session_title = None;
    d.tokens = None;
    assert_eq!(drafted_from(&d), "Drafted from your description. Review it before saving.");
    assert_eq!(draft_cost(&d), "Asked the model once.");
    let mut r = rig();
    r.adopt_draft(d);
    assert_eq!(r.ed().drafted.as_ref().unwrap().session_title, None);
}

#[test]
fn the_file_it_would_make_follows_the_name_as_it_s_edited() {
    let mut r = rig();
    r.adopt_draft(drafted_ok());
    r.edit_draft(|d| d.name = "Fix a login bug".into());
    assert_eq!(r.ed().file_name(), ".weave/workflows/fix-a-login-bug.yaml");

    // From the File view the name is the one the check last read out of the text.
    r.fleet.on("POST", CHECK, |body| {
        let mut draft = review_draft();
        draft["name"] = json!("Typed in the text");
        (200, check_json(Some(draft), body["text"].as_str().unwrap_or(""), json!([]), json!([])))
    });
    r.edit_text("name: Typed in the text\n");
    r.settle();
    assert_eq!(r.ed().file_name(), ".weave/workflows/typed-in-the-text.yaml");
}

#[test]
fn save_creates_the_file_the_text_as_the_model_wrote_it_and_it_s_an_ordinary_workflow_after_that() {
    let mut r = rig();
    r.adopt_draft(drafted_ok());
    assert!(r.ed().can_save(), "a draft is unsaved from the start, so Save is on");

    r.save();
    r.saved();

    assert_eq!(r.fleet.workflow_requests(), [("POST".to_string(), FILES.to_string())]);
    assert_eq!(r.fleet.bodies("POST", FILES), [json!({"directory": REPO, "text": "name: Build it our way\nsteps: []\n"})]);
    let e = r.ed();
    assert!(e.drafted.is_none());
    assert_eq!(e.file.as_ref().unwrap().workflow_id, "repo:build-it-our-way");
    assert!(!e.is_dirty());
    assert!(r.rt.state.workflows_nav.editor.take_saved().is_some());
}

#[test]
fn save_after_designer_edits_sends_the_draft() {
    let mut r = rig();
    r.adopt_draft(drafted_ok());
    r.edit_draft(|d| d.description = Some("Ours.".into()));
    r.settle();
    r.save();
    r.saved();

    let created = &r.fleet.bodies("POST", FILES)[0];
    assert_eq!(created["draft"]["description"], json!("Ours."));
    assert!(created.get("text").is_none());
}

#[test]
fn a_name_that_s_taken_is_refused_with_the_server_s_message_and_the_draft_stays_unsaved() {
    let mut r = rig();
    let taken = "There's already a workflow called Build it our way. Pick another name.";
    r.fleet.always("POST", FILES, 409, json!({"error": taken}));
    r.adopt_draft(drafted_ok());

    r.save();
    r.wait("the refusal", |s| ed(s).save_error.is_some());

    let e = r.ed();
    assert_eq!(e.save_error.as_deref(), Some(taken));
    assert!(e.drafted.is_some(), "the draft stays");
    assert_eq!(e.conflict, None, "a 409 of a new file is not the changed-on-disk conflict");
    assert!(!e.is_saving);
}

#[test]
fn a_draft_that_still_has_errors_opens_in_the_file_view_with_them_and_save_waits() {
    let mut r = rig();
    let mut d = drafted(check_of(
        Some(review_draft()),
        "name: Build it our way\n",
        json!([{"line": 38, "message": MISSING_MAX, "step": 3}]),
        json!([]),
    ));
    d.asks = 2;
    d.tokens = Some(DraftTokens { total: 7300, from_cache: 6800 });
    assert_eq!(draft_cost(&d), "Asked the model twice: its first answer had errors · 7,300 tokens, 6,800 from the cache.");
    r.adopt_draft(d);

    let e = r.ed();
    assert_eq!(e.view, EditorView::File);
    assert_eq!((e.errors()[0].line, e.errors()[0].message.as_str()), (38, MISSING_MAX));
    assert!(!e.can_save());
    r.save();
    r.later(FRAME);
    assert_eq!(r.fleet.workflow_requests(), [], "a save with errors sends nothing");
}

#[test]
fn a_draft_the_parser_couldn_t_read_at_all_opens_in_the_file_view() {
    let mut r = rig();
    r.adopt_draft(drafted(check_of(
        None,
        "I can't do that.\n",
        json!([{"line": 1, "message": "The file isn't a mapping.", "step": null}]),
        json!([]),
    )));

    assert_eq!(r.ed().view, EditorView::File);
    assert_eq!(r.ed().text, "I can't do that.\n");
    assert_eq!(r.ed().file_name(), ".weave/workflows/workflow.yaml", "a draft with no name yet");
}

// ─── The rules the state keeps ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn opens_a_repository_workflow_in_the_designer_and_one_the_designer_can_t_show_in_the_file_view() {
    let mut r = rig();
    r.rt.update(|s, _| s.workflow_editor_open(HOME_MACHINE_KEY, REPO, "repo:build-it-our-way"));
    r.wait("the file", |s| ed(s).file.is_some());
    assert_eq!(r.ed().view, EditorView::Designer);
    assert_eq!(r.fleet.bodies("POST", OPEN), [json!({"directory": REPO, "workflowId": "repo:build-it-our-way"})]);
    assert!(!r.ed().is_dirty() && r.ed().source.is_none());

    r.fleet.on("POST", OPEN, |_| {
        (200, file_json(check_json(None, "name: [broken\n", json!([{"line": 1, "message": "no", "step": null}]), json!([])), "h5"))
    });
    r.rt.update(|s, _| s.workflow_editor_open(HOME_MACHINE_KEY, REPO, "repo:broken"));
    r.wait("the broken file", |s| ed(s).file.as_ref().is_some_and(|f| f.hash == "h5"));
    assert_eq!(r.ed().view, EditorView::File, "check.draft is null");
    assert_eq!(r.ed().text, "name: [broken\n");
}

#[test]
fn a_file_that_won_t_open_says_why_and_a_newer_open_wins_over_an_older_answer() {
    let mut r = rig();
    r.fleet.always("POST", OPEN, 404, json!({"error": "That workflow file is gone."}));
    r.rt.update(|s, _| s.workflow_editor_open(HOME_MACHINE_KEY, REPO, "repo:gone"));
    r.wait("the refusal", |s| ed(s).load_error.is_some());
    assert_eq!(r.ed().load_error.as_deref(), Some("That workflow file is gone."));
    assert!(r.ed().file.is_none());

    // The first open is slow; a file adopted meanwhile is not overwritten by its late answer.
    let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let held = gate.clone();
    r.fleet.on("POST", OPEN, move |_| {
        while !held.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        (200, file_json(check_json(Some(review_draft()), "late\n", json!([]), json!([])), "late"))
    });
    r.rt.update(|s, _| s.workflow_editor_open(HOME_MACHINE_KEY, REPO, "repo:slow"));
    r.adopt(false);
    gate.store(true, std::sync::atomic::Ordering::SeqCst);
    let heard = r.fleet.heard_handle();
    r.wait("the slow open to have been asked", move |_| heard("POST", OPEN) == 2);
    r.later(FRAME);
    r.later(FRAME);
    assert_eq!(r.ed().file.as_ref().unwrap().hash, "h1", "the adopted file stays");
    assert_eq!(r.ed().load_error, None);
}

#[test]
fn the_check_waits_300_ms_after_the_last_key_and_sends_only_the_last_text() {
    let mut r = rig();
    r.adopt(false);
    r.view(EditorView::File);
    r.edit_text("name: A\n");
    r.later(FRAME);
    r.later(0.2);
    assert_eq!(r.fleet.count("POST", CHECK), 0, "not yet");
    r.edit_text("name: AB\n");
    r.later(FRAME);
    r.later(0.2);
    assert_eq!(r.fleet.count("POST", CHECK), 0, "a key restarts the wait");
    assert!(r.ed().is_checking && r.ed().is_busy(), "checking is shown at once");
    r.later(0.2);
    r.wait("the check", |s| !ed(s).is_busy());
    assert_eq!(r.fleet.bodies("POST", CHECK), [json!({"text": "name: AB\n"})]);
    assert_eq!(r.ed().source, Some(Source::Text));
    assert_eq!(r.ed().check.as_ref().unwrap().text, "name: AB\n");
}

#[test]
fn a_check_that_was_overtaken_is_ignored_when_it_lands() {
    let mut r = rig();
    r.adopt(false);
    r.view(EditorView::File);
    let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let held = gate.clone();
    r.fleet.on("POST", CHECK, move |body| {
        let text = body["text"].as_str().unwrap_or("").to_string();
        if text == "slow\n" {
            while !held.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        (200, check_json(Some(review_draft()), &text, json!([]), json!([])))
    });

    r.edit_text("slow\n");
    r.later(FRAME);
    r.later(CHECK_DELAY + 0.01);
    let heard = r.fleet.heard_handle();
    r.wait("the slow check to be asked", move |_| heard("POST", CHECK) == 1);
    r.edit_text("fast\n");
    r.settle();
    assert_eq!(r.ed().check.as_ref().unwrap().text, "fast\n");

    gate.store(true, std::sync::atomic::Ordering::SeqCst);
    // The slow answer lands after; wait for the app to have taken every job its thread posted, then look.
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        while let Ok(job) = r.queue.try_recv() {
            r.rt.update(job);
        }
    }
    r.later(FRAME);
    assert_eq!(r.ed().check.as_ref().unwrap().text, "fast\n", "the older answer didn't replace it");
    assert!(!r.ed().is_checking);
}

#[test]
fn a_check_that_fails_shows_as_a_save_error_and_a_file_that_can_t_be_checked_stays_unsaved() {
    let mut r = rig();
    r.adopt(false);
    r.view(EditorView::File);
    r.fleet.always("POST", CHECK, 500, json!({"error": "The parser fell over."}));
    r.edit_text("name: A\n");
    r.settle();
    assert_eq!(r.ed().save_error.as_deref(), Some("The parser fell over."));
    assert!(!r.ed().is_checking);

    r.fleet.always("POST", CHECK, 500, json!({}));
    r.edit_text("name: AB\n");
    r.settle();
    assert_eq!(r.ed().save_error.as_deref(), Some("Couldn't check the workflow."), "Fleet's own words when the server gave none");
}

#[test]
fn a_view_switch_checks_what_s_waiting_first_and_to_the_file_takes_the_text_the_designer_wrote() {
    let mut r = rig();
    r.adopt(false);
    r.fleet
        .on("POST", CHECK, |body| (200, check_json(body.get("draft").cloned(), "name: Written by the designer\n", json!([]), json!([]))));
    r.edit_draft(|d| d.name = "Written by the designer".into());
    // No wait for the timer: the switch itself sends the check.
    r.view(EditorView::File);
    r.wait("the switch", |s| ed(s).view == EditorView::File);
    assert_eq!(r.fleet.count("POST", CHECK), 1);
    assert_eq!(r.ed().text, "name: Written by the designer\n");

    // And back: the text wasn't edited, so the designer's own draft is still there.
    r.view(EditorView::Designer);
    assert_eq!(r.ed().view, EditorView::Designer);
    assert_eq!(r.ed().draft.as_ref().unwrap().name, "Written by the designer");
}

#[test]
fn leaving_one_file_for_another_drops_the_check_that_was_waiting() {
    let mut r = rig();
    r.adopt(false);
    r.view(EditorView::File);
    r.edit_text("name: A\n");
    r.adopt(false);
    r.later(FRAME);
    r.later(CHECK_DELAY + 0.1);
    assert_eq!(r.fleet.count("POST", CHECK), 0);
    assert!(!r.ed().is_busy() && !r.ed().is_checking);
    assert_eq!(r.ed().text, "name: Build it our way\n");
}
