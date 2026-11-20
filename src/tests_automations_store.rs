//! Component 19, piece C: the automations store and its routes. Fleet's `stores/__tests__/automations.test.ts` under its
//! names, against a fake Fleet that keeps its list between calls (and a second for another machine), then the rules the native
//! store keeps that Fleet's tests don't name: every route and its verb, a refusal's words or `HTTP <status>`, the list a
//! change leaves alone when it fails, and runs, drafts and the event catalog going to the machine asked.
use crate::fleet::automations::model::{Automation, AutomationDraft, AutomationRequest, AutomationRun, RunState};
use crate::fleet::automations::store::{BASE, Done, item_path, read_error};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::machines::Case;
use crate::tests::{FRAME, app};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Heard = Arc<Mutex<Vec<(String, String, Value)>>>;

/// A stand-in for `/api/automations` that keeps its list between calls, like Fleet's `fakeServer`.
pub(crate) struct Fake {
    pub(crate) base: String,
    heard: Heard,
    refuse_next_create: Arc<Mutex<Option<String>>>,
    /// The next read of the list is refused with these words (the page's Try again).
    pub(crate) refuse_next_list: Arc<Mutex<Option<String>>>,
    /// While set, the list is held back: the request is heard but not answered.
    pub(crate) hold_list: Arc<AtomicBool>,
    /// The next call of this verb (`enable`, `disable`, `run`, `delete`) is refused with these words.
    pub(crate) refuse_verb: Arc<Mutex<Option<(String, String)>>>,
    /// What the next Run now answers with, merged into the run it makes (a skipped one).
    pub(crate) run_answer: Arc<Mutex<Option<Value>>>,
}

/// Takes the refusal waiting for `verb`, if that is the one.
fn refusal(slot: &Mutex<Option<(String, String)>>, verb: &str) -> Option<String> {
    let mut held = slot.lock().unwrap();
    if held.as_ref().is_some_and(|(v, _)| v == verb) { held.take().map(|(_, e)| e) } else { None }
}

pub(crate) fn automation(id: &str, extra: Value) -> Value {
    let mut a = json!({
        "id": id, "name": format!("Automation {id}"), "prompt": "Summarise the open PRs", "triggerType": "schedule",
        "triggerConfig": "0 9 * * 1", "maxConcurrentRuns": 1, "maxRunsPerHour": 10, "timeoutMinutes": 30, "isEnabled": true,
        "workspaceId": null, "model": null, "agent": null, "createdAt": "2026-09-14T09:00:00Z", "updatedAt": null,
        "timeZone": "Africa/Johannesburg"
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        a[k] = v.clone();
    }
    a
}

pub(crate) fn fake(initial: Vec<Value>) -> Fake {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let (heard, list) = (Heard::default(), Arc::new(Mutex::new(initial)));
    let (refuse, hold) = (Arc::new(Mutex::new(None::<String>)), Arc::new(AtomicBool::new(false)));
    let (log, items, refuse_next, gate) = (heard.clone(), list.clone(), refuse.clone(), hold.clone());
    let list_refusal = Arc::new(Mutex::new(None::<String>));
    let refuse_list = list_refusal.clone();
    let (refuse_verb, run_answer) = (Arc::new(Mutex::new(None)), Arc::new(Mutex::new(None::<Value>)));
    let (verb_gate, run_gate) = (refuse_verb.clone(), run_answer.clone());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, items, refuse_next, gate) = (log.clone(), items.clone(), refuse_next.clone(), gate.clone());
            let refuse_list = refuse_list.clone();
            let (verb_gate, run_gate) = (verb_gate.clone(), run_gate.clone());
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if !req.route().starts_with(BASE) {
                    return crate::test_http::respond_json(&mut stream, "404 X", "{}");
                }
                let body = req.json();
                log.lock().unwrap().push((req.method.clone(), req.path.clone(), body.clone()));
                let route = req.route().to_string();
                let toggle = route
                    .strip_prefix("/api/automations/")
                    .and_then(|r| r.split_once('/'))
                    .map(|(id, verb)| (id.replace("%20", " "), verb));
                let verb = if req.method == "DELETE" { Some("delete") } else { toggle.as_ref().map(|t| t.1) };
                if let Some(error) = verb.and_then(|v| refusal(&verb_gate, v)) {
                    return crate::test_http::respond_json(&mut stream, "400 X", &json!({"error": error}).to_string());
                }
                match (req.method.as_str(), route.as_str(), toggle) {
                    ("GET", BASE, _) => {
                        if let Some(error) = refuse_list.lock().unwrap().take() {
                            return crate::test_http::respond_json(&mut stream, "503 X", &json!({"error": error}).to_string());
                        }
                        while gate.load(Ordering::SeqCst) {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        let all = items.lock().unwrap().clone();
                        crate::test_http::respond_json(&mut stream, "200 X", &json!({"automations": all}).to_string());
                    }
                    ("POST", BASE, _) => match refuse_next.lock().unwrap().take() {
                        Some(error) => crate::test_http::respond_json(&mut stream, "400 X", &json!({"error": error}).to_string()),
                        None => {
                            let mut list = items.lock().unwrap();
                            let mut extra = body;
                            extra["isEnabled"] = json!(true);
                            let created = automation(&format!("a{}", list.len() + 1), extra);
                            list.push(created.clone());
                            crate::test_http::respond_json(&mut stream, "201 X", &created.to_string());
                        }
                    },
                    ("POST", _, Some((id, verb @ ("enable" | "disable")))) => {
                        let mut list = items.lock().unwrap();
                        if !list.iter().any(|a| a["id"] == id.as_str()) {
                            return crate::test_http::respond_json(
                                &mut stream,
                                "404 X",
                                &json!({"error": "No such automation"}).to_string(),
                            );
                        }
                        for a in list.iter_mut().filter(|a| a["id"] == id.as_str()) {
                            a["isEnabled"] = json!(verb == "enable");
                        }
                        crate::test_http::respond_json(&mut stream, "204 X", "");
                    }
                    ("POST", _, Some((id, "run"))) => {
                        let mut run = json!({"id": "r1", "automationId": id, "trigger": "manual", "startedAt": "2026-09-15T10:00:00Z", "state": "starting"});
                        for (k, v) in run_gate.lock().unwrap().take().as_ref().and_then(Value::as_object).into_iter().flatten() {
                            run[k] = v.clone();
                        }
                        crate::test_http::respond_json(&mut stream, "201 X", &run.to_string());
                    }
                    ("GET", _, Some((id, "runs"))) => {
                        let run = |n: u32, state: &str| json!({"id": format!("r{n}"), "automationId": id, "trigger": "schedule", "startedAt": "2026-09-15T10:00:00Z", "state": state});
                        crate::test_http::respond_json(
                            &mut stream,
                            "200 X",
                            &json!({"runs": [run(2, "running"), run(1, "done")]}).to_string(),
                        );
                    }
                    ("PUT", _, None) => crate::test_http::respond_json(&mut stream, "200 X", "{}"),
                    ("DELETE", _, None) => {
                        let id = route.rsplit('/').next().unwrap_or("").replace("%20", " ");
                        items.lock().unwrap().retain(|a| a["id"] != id.as_str());
                        crate::test_http::respond_json(&mut stream, "204 X", "");
                    }
                    ("GET", "/api/automations/event-catalog", _) => {
                        crate::test_http::respond_json(&mut stream, "200 X", &json!(["session.completed", "run.failed"]).to_string())
                    }
                    ("GET", _, _) if route.contains("/draft-from-session/") => crate::test_http::respond_json(
                        &mut stream,
                        "200 X",
                        &json!({"prompt": "Tidy the changelog", "folder": "/work/kettle", "isolation": "worktree"}).to_string(),
                    ),
                    _ => crate::test_http::respond_json(&mut stream, "404 X", &json!({"error": "not faked"}).to_string()),
                }
            });
        }
    });
    Fake { base, heard, refuse_next_create: refuse, refuse_next_list: list_refusal, hold_list: hold, refuse_verb, run_answer }
}

impl Fake {
    pub(crate) fn list_calls(&self) -> usize {
        self.calls("GET", BASE)
    }

    pub(crate) fn calls(&self, method: &str, path: &str) -> usize {
        self.heard.lock().unwrap().iter().filter(|(m, p, _)| m == method && p == path).count()
    }

    fn body_of(&self, method: &str, path: &str) -> Value {
        self.heard.lock().unwrap().iter().rev().find(|(m, p, _)| m == method && p == path).map(|c| c.2.clone()).unwrap_or(Value::Null)
    }

    /// Every automations request heard, as "METHOD path".
    fn all(&self) -> Vec<String> {
        self.heard.lock().unwrap().iter().map(|(m, p, _)| format!("{m} {p}")).collect()
    }

    pub(crate) fn handle(&self) -> impl Fn(&str, &str) -> usize + use<> {
        let heard = self.heard.clone();
        move |method, path| heard.lock().unwrap().iter().filter(|(m, p, _)| m == method && p == path).count()
    }
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
    harbor: Fake,
}

/// The app connected to a fake Fleet, with `m-harbor` answering at a second fake. Nothing has been asked yet.
fn rig(home: Vec<Value>, harbor: Vec<Value>) -> Rig {
    let (home, harbor) = (fake(home), fake(harbor));
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    let base = harbor.base.clone();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_machines_only(Case::Several);
        s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor is listed").base_url = base;
    });
    rt.frame(FRAME);
    Rig { rt, queue, home, harbor }
}

type Outcome<T> = Arc<Mutex<Option<Result<T, String>>>>;

/// A `done` that keeps its answer where the test can read it.
fn keep<T: Send + 'static>() -> (Outcome<T>, Done<T>) {
    let slot: Outcome<T> = Arc::new(Mutex::new(None));
    let put = slot.clone();
    (slot, Box::new(move |_, _, result| *put.lock().unwrap() = Some(result)))
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| {
            let a = s.automations_of(HOME_MACHINE_KEY);
            format!("home list {:?}, loading {}, error {:?}", a.list.iter().map(|x| &x.id).collect::<Vec<_>>(), a.is_loading, a.error)
        });
    }

    fn refresh(&mut self, machine: &'static str) {
        self.rt.update(move |s, tx| s.automations_refresh(machine, tx, None));
    }

    /// Reads the list and waits for it.
    fn load(&mut self, machine: &'static str) {
        self.refresh(machine);
        self.wait("the list", move |s| !s.automations_of(machine).is_loading);
    }

    fn ids(&self, machine: &str) -> Vec<String> {
        self.rt.state.automations_of(machine).list.iter().map(|a| a.id.clone()).collect()
    }

    fn names(&self, machine: &str) -> Vec<String> {
        self.rt.state.automations_of(machine).list.iter().map(|a| a.name.clone()).collect()
    }

    /// Waits for `done`'s answer.
    fn answer<T: Send + Clone + 'static>(&mut self, what: &str, slot: &Outcome<T>) -> Result<T, String> {
        let slot = slot.clone();
        let look = slot.clone();
        self.wait(what, move |_| look.lock().unwrap().is_some());
        slot.lock().unwrap().clone().expect("an answer")
    }
}

fn request(name: &str, config: &str) -> AutomationRequest {
    AutomationRequest {
        name: name.into(),
        prompt: "Summarise".into(),
        trigger_type: "schedule".into(),
        trigger_config: config.into(),
        ..Default::default()
    }
}

const HOME: &str = HOME_MACHINE_KEY;

// ─── stores/__tests__/automations.test.ts: "useAutomationsStore" ────────────────────────────────────────────────────

#[test]
fn lists_a_new_automation_as_soon_as_it_s_created() {
    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    r.load(HOME);
    let (made, done) = keep::<Automation>();
    let weekly = request("Weekly digest", "0 9 * * 1");
    r.rt.update(move |s, tx| s.automation_create(HOME, &weekly, tx, done));
    let created = r.answer("the create", &made).expect("created");

    assert_eq!(created.name, "Weekly digest");
    assert_eq!(r.names(HOME), ["Automation a1", "Weekly digest"], "the list was read again before the answer came back");
}

#[test]
fn shows_a_switch_off_everywhere_the_list_is_shown() {
    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    r.load(HOME);
    let (off, done) = keep::<()>();
    r.rt.update(move |s, tx| s.automation_disable(HOME, "a1", tx, done));
    r.answer("the switch-off", &off).expect("switched off");

    assert!(!r.rt.state.automations_of(HOME).list[0].is_enabled);
    assert!(!r.rt.state.automations_of(HOME).get("a1").unwrap().is_enabled);
}

#[test]
fn shares_one_request_between_views_that_load_at_the_same_time() {
    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    r.home.hold_list.store(true, Ordering::SeqCst);
    let (a, b) = (keep::<()>(), keep::<()>());
    let (slot_a, slot_b) = (a.0.clone(), b.0.clone());
    r.rt.update(move |s, tx| {
        for (_, done) in [a, b] {
            s.automations_refresh(HOME, tx, Some(Box::new(move |s, tx| done(s, tx, Ok(())))));
        }
        s.automations_refresh(HOME, tx, None);
    });
    // The one request is heard and held; the two later calls found it in flight.
    let heard = r.home.handle();
    r.wait("the one list request to be heard", move |_| heard("GET", BASE) == 1);
    r.home.hold_list.store(false, Ordering::SeqCst);
    r.wait("the list", |s| !s.automations_of(HOME).is_loading);
    r.wait("both waiters", move |_| slot_a.lock().unwrap().is_some() && slot_b.lock().unwrap().is_some());

    assert_eq!(r.home.list_calls(), 1);
    assert_eq!(r.ids(HOME), ["a1"]);
}

#[test]
fn throws_the_server_s_reason_when_a_create_is_refused() {
    let mut r = rig(vec![], vec![]);
    *r.home.refuse_next_create.lock().unwrap() = Some("Invalid cron expression: bad format".into());
    let (made, done) = keep::<Automation>();
    let bad = request("Bad", "every monday");
    r.rt.update(move |s, tx| s.automation_create(HOME, &bad, tx, done));

    assert_eq!(r.answer("the refusal", &made).unwrap_err(), "Invalid cron expression: bad format");
    assert_eq!(r.home.list_calls(), 0, "a refused change reads nothing again");
}

#[test]
fn stops_showing_the_spinner_after_the_first_load_and_keeps_it_off_for_reloads() {
    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    assert!(r.rt.state.automations_of(HOME).is_loading, "loading until the first load ends");
    r.load(HOME);
    assert!(!r.rt.state.automations_of(HOME).is_loading);

    r.home.hold_list.store(true, Ordering::SeqCst);
    r.refresh(HOME);
    let heard = r.home.handle();
    r.wait("the reload to be heard", move |_| heard("GET", BASE) == 2);
    assert!(!r.rt.state.automations_of(HOME).is_loading, "a reload doesn't bring the spinner back");
    assert_eq!(r.ids(HOME), ["a1"], "and keeps the list it is replacing");
    r.home.hold_list.store(false, Ordering::SeqCst);
    r.wait("the reload to end", |s| !s.automations.of(HOME).is_some_and(|m| m.error.is_some()) && s.automations_of(HOME).list.len() == 1);
}

// ─── "an automations store of another machine" ──────────────────────────────────────────────────────────────────────

#[test]
fn an_automations_store_of_another_machine_asks_that_machine_for_everything_and_keeps_its_own_list() {
    let mut r = rig(vec![automation("on-home", json!({}))], vec![automation("on-harbor", json!({}))]);
    r.load("m-harbor");
    let (on, enabled) = keep::<()>();
    r.rt.update(move |s, tx| s.automation_enable("m-harbor", "on-harbor", tx, enabled));
    r.answer("the switch-on", &on).expect("switched on");
    let (ran, done) = keep::<AutomationRun>();
    r.rt.update(move |s, tx| s.automation_run("m-harbor", "on-harbor", tx, done));
    r.answer("the run", &ran).expect("run");

    assert_eq!(r.ids("m-harbor"), ["on-harbor"]);
    assert!(r.harbor.all().contains(&"POST /api/automations/on-harbor/run".to_string()), "{:?}", r.harbor.all());
    assert!(r.home.all().is_empty(), "home heard nothing: {:?}", r.home.all());
    assert!(r.rt.state.automations_of(HOME).list.is_empty(), "home's own store is untouched");
    assert!(r.rt.state.automations_of(HOME).is_loading, "and still waiting for its first load");
}

// ─── The rules the native store keeps ───────────────────────────────────────────────────────────────────────────────

#[test]
fn every_change_is_its_own_verb_and_path_and_reloads_the_list() {
    let mut r = rig(vec![automation("a 1", json!({"isEnabled": false}))], vec![]);
    r.load(HOME);
    let (a, put) = keep::<()>();
    let (b, enable) = keep::<()>();
    let (c, delete) = keep::<()>();
    let mut update = request("Renamed", "0 8 * * *");
    update.workspace_id = Some(None);
    r.rt.update(move |s, tx| s.automation_update(HOME, "a 1", &update, tx, put));
    r.answer("the update", &a).expect("updated");
    r.rt.update(move |s, tx| s.automation_enable(HOME, "a 1", tx, enable));
    r.answer("the enable", &b).expect("enabled");
    assert!(r.rt.state.automations_of(HOME).list[0].is_enabled);
    r.rt.update(move |s, tx| s.automation_delete(HOME, "a 1", tx, delete));
    r.answer("the delete", &c).expect("deleted");

    assert!(r.ids(HOME).is_empty());
    assert_eq!(r.home.calls("PUT", &item_path("a 1", "")), 1, "the id is escaped: {:?}", r.home.all());
    assert_eq!(r.home.body_of("PUT", &item_path("a 1", ""))["workspaceId"], Value::Null);
    assert_eq!(r.home.calls("DELETE", "/api/automations/a%201"), 1);
    assert_eq!(r.home.list_calls(), 4, "the first load and one after each of the three changes");
}

#[test]
fn a_run_comes_back_still_starting_and_the_list_is_read_again() {
    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    r.load(HOME);
    let (ran, done) = keep::<AutomationRun>();
    r.rt.update(move |s, tx| s.automation_run(HOME, "a1", tx, done));
    let run = r.answer("the run", &ran).expect("run");

    assert_eq!((run.id.as_str(), run.state, run.trigger.as_str()), ("r1", RunState::Starting, "manual"));
    assert_eq!(r.home.list_calls(), 2);
}

#[test]
fn runs_the_draft_and_the_event_catalog_are_reads_that_leave_the_list_alone() {
    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    r.load(HOME);
    let (runs, got_runs) = keep::<Vec<AutomationRun>>();
    let (draft, got_draft) = keep::<AutomationDraft>();
    let (events, got_events) = keep::<Vec<String>>();
    r.rt.update(move |s, tx| {
        s.automation_runs(HOME, "a1", tx, got_runs);
        s.automation_draft_from_session(HOME, "s/1", tx, got_draft);
        s.automation_event_catalog(HOME, tx, got_events);
    });
    let runs = r.answer("the runs", &runs).expect("runs");
    let draft = r.answer("the draft", &draft).expect("a draft");
    let events = r.answer("the catalog", &events).expect("a catalog");

    assert_eq!(runs.iter().map(|x| (x.id.as_str(), x.state)).collect::<Vec<_>>(), [("r2", RunState::Running), ("r1", RunState::Done)]);
    assert_eq!(
        (draft.prompt.as_str(), draft.folder.as_deref(), draft.isolation.as_str()),
        ("Tidy the changelog", Some("/work/kettle"), "worktree")
    );
    assert_eq!(events, ["session.completed", "run.failed"]);
    assert_eq!(r.home.calls("GET", "/api/automations/draft-from-session/s%2F1"), 1, "{:?}", r.home.all());
    assert_eq!(r.home.list_calls(), 1);
}

#[test]
fn a_refusal_with_no_words_says_the_status_and_a_failed_load_says_so_and_keeps_the_old_list() {
    assert_eq!(read_error(500, ""), "HTTP 500");
    assert_eq!(read_error(404, "<html>"), "HTTP 404");
    assert_eq!(read_error(404, r#"{"error":"No such automation"}"#), "No such automation");
    assert_eq!(read_error(502, r#"{"error":7}"#), "HTTP 502");

    let mut r = rig(vec![automation("a1", json!({}))], vec![]);
    r.load(HOME);
    // An automation that isn't there: the server says so with a 404, and the list is as it was.
    let (gone, done) = keep::<()>();
    r.rt.update(move |s, tx| s.automation_enable(HOME, "ghost", tx, done));
    assert_eq!(r.answer("the refusal", &gone).unwrap_err(), "No such automation");
    assert_eq!(r.ids(HOME), ["a1"]);

    // A machine that isn't there: the load fails with words, stops loading, and a later load clears the words.
    let mut r = rig(vec![], vec![]);
    r.rt.update(|s, _| {
        s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("listed").base_url = crate::test_http::dead_address()
    });
    r.refresh("m-harbor");
    r.wait("the failure", |s| s.automations_of("m-harbor").error.is_some());
    assert!(!r.rt.state.automations_of("m-harbor").is_loading, "a failed first load ends the spinner");
    assert!(r.rt.state.automations_of("m-harbor").list.is_empty());
    let _ = &r.harbor;
}
