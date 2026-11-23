//! Component 22, piece G: the pane's body. `components/canvas/__tests__/ModPaneCanvas.test.ts` (6) and `ModPaneCanvas.off.test.ts` (4),
//! under Fleet's names, against a fake Fleet that answers the Mods switch, the kept mods, the session's panes, renders, actions and
//! both Turn on calls. Fleet's tests read the DOM; these read what the pane draws.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::mods::points::{ModPaneView, ModTreeView};
use crate::fleet::mods::types::Author;
use crate::fleet::state::Topic;
use crate::tests::{FRAME, app};
use crate::{State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn text(words: &str) -> Value {
    json!({ "type": "Text", "props": {}, "children": [words] })
}

fn view(mod_name: &str, pane: &str, tree: Value) -> ModPaneView {
    ModPaneView {
        session_id: "s-1".into(),
        pane_id: pane.into(),
        title: "Runs".into(),
        mod_id: mod_name.into(),
        view: ModTreeView::new(tree, vec![Author { name: mod_name.into(), draft: false }]),
    }
}

#[derive(Default)]
struct Fake {
    panes: Value,
    kept: Value,
    /// Turn on answers 500.
    refuse: bool,
    renders: Value,
    /// The rig is gone: the accept loop ends, so a long run of tests doesn't pile up threads (a macOS runner ran out).
    stop: bool,
    calls: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> (String, std::net::SocketAddr) {
    let listener = crate::test_http::bind().expect("a port");
    let addr = listener.local_addr().expect("an address");
    let base = format!("http://{addr}");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if fake.lock().unwrap().stop {
                break;
            }
            let fake = fake.clone();
            std::thread::spawn(move || {
                let req = test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let mut f = fake.lock().unwrap();
                f.calls.push((req.method.clone(), req.path.clone(), req.json()));
                let (status, reply) = if req.path == "/api/features/mods" {
                    ("200 OK", json!({ "on": true, "safeMode": false }))
                } else if req.path == "/api/mods" {
                    ("200 OK", json!({ "safeMode": false, "mods": f.kept }))
                } else if req.path.ends_with("/on") && f.refuse {
                    ("500 Internal Server Error", json!({ "error": "no" }))
                } else if req.path.ends_with("/on") {
                    ("200 OK", json!({ "name": "ci-watch", "versions": [] }))
                } else if req.path.ends_with("/mods/sites") {
                    ("200 OK", json!({ "sites": [], "panes": f.panes }))
                } else if req.path.ends_with("/mods/action") {
                    ("200 OK", json!({ "handled": true }))
                } else if req.path.ends_with("/mods/render") {
                    ("200 OK", f.renders.clone())
                } else {
                    ("200 OK", json!({}))
                };
                drop(f);
                test_http::respond_json(&mut stream, status, &reply.to_string());
            });
        }
    });
    (base, addr)
}

struct Rig {
    rt: Rt,
    queue: Receiver<Job>,
    fake: Shared,
    addr: std::net::SocketAddr,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.fake.lock().unwrap().stop = true;
        // One more connection wakes the accept loop so it sees the flag.
        let _ = std::net::TcpStream::connect(self.addr);
    }
}

fn declared(extra: Value) -> Value {
    let mut p = json!({ "mod": "ci-watch", "id": "runs", "title": "CI runs", "icon": null, "draft": false, "off": true });
    for (k, v) in extra.as_object().into_iter().flatten() {
        p[k] = v.clone();
    }
    p
}

/// An app on the open session `s-1` of a fake Fleet whose mods declare `panes` and keep `kept`, the panel open.
fn rig(panes: Value, kept: Value) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake {
        panes,
        kept,
        renders: json!({ "tree": text("3 passing"), "mods": [{ "name": "ci-watch", "draft": false }] }),
        ..Default::default()
    }));
    let (base, addr) = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.projects[0].sessions[0].id = "s-1".into();
        s.route = crate::fleet::store::Route::Session("s-1".into());
        s.content_open = true;
        s.sidebar_open = false;
        s.right.widened = true;
    });
    let mut r = Rig { rt, queue, fake, addr };
    r.settle();
    r
}

impl Rig {
    fn settle(&mut self) {
        for _ in 0..3 {
            test_wait::drive(
                &mut self.rt,
                &self.queue,
                "every request to be answered",
                |s| s.mod_renders.idle() && s.mod_panes.idle(),
                |_| String::new(),
            );
            for _ in 0..4 {
                self.rt.frame(FRAME);
            }
        }
    }

    fn put(&mut self, v: ModPaneView) {
        self.rt.update(move |s, _| s.open_mod_pane(v));
        self.settle();
    }

    fn words(&self) -> String {
        self.rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn calls(&self, method: &str, path: &str) -> Vec<Value> {
        self.fake.lock().unwrap().calls.iter().filter(|c| c.0 == method && c.1 == path).map(|c| c.2.clone()).collect()
    }

    fn with(&self, change: impl FnOnce(&mut Fake)) {
        change(&mut self.fake.lock().unwrap());
    }

    fn press(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.settle();
    }
}

fn mine(mod_name: &str) -> String {
    format!("mod-pane:s-1:{mod_name}:runs.0")
}

// ---- ModPaneCanvas.test.ts ----

// "draws the tree"
#[test]
fn mod_pane_canvas_draws_the_tree() {
    let mut r = rig(json!([]), json!([]));
    r.put(view("ci-mod", "runs", text("3 passing")));
    assert!(r.has("mod-pane") && r.has(&mine("ci-mod")));
    assert!(r.words().contains("3 passing"));
}

// "waits for the mod when it has no view, and draws once it does"
#[test]
fn mod_pane_canvas_waits_for_the_mod_when_it_has_no_view_and_draws_once_it_does() {
    let mut r = rig(json!([]), json!([]));
    r.rt.update(|s, _| {
        s.canvases.open_mod_pane("s-1", crate::fleet::canvases::ModPaneRef { mod_name: "ci-mod".into(), pane_id: "runs".into() }, "Runs");
    });
    r.settle();
    assert_eq!(r.rt.find("mod-pane-empty").map(|_| r.words()).map(|w| w.contains("Waiting for ci-mod to draw this pane")), Some(true));
    r.put(view("ci-mod", "runs", text("back")));
    assert!(!r.has("mod-pane-empty"));
    assert!(r.words().contains("back"));
}

// "is empty for a null tree and for an invalid one"
#[test]
fn mod_pane_canvas_is_empty_for_a_null_tree_and_for_an_invalid_one() {
    let mut r = rig(json!([]), json!([]));
    r.put(view("ci-mod", "runs", Value::Null));
    assert!(!r.has(&mine("ci-mod")) && r.has("mod-pane-empty"));
    assert!(r.words().contains("Nothing to show here yet"));
    r.put(view("ci-mod", "runs", json!({ "type": "Nope" })));
    assert!(!r.has(&mine("ci-mod")) && r.has("mod-pane-empty"));
    assert!(!r.words().contains("Waiting for"), "a view with nothing to draw is not a missing view");
}

// "sends actions as desktop"
#[test]
fn mod_pane_canvas_sends_actions_as_desktop() {
    let mut r = rig(json!([]), json!([]));
    r.put(view("ci-mod", "runs", json!({ "type": "Button", "props": { "key": "go", "label": "Go" }, "handles": { "onPress": "h1" } })));
    r.press(&mine("ci-mod"));
    let (fake, _) = (r.fake.clone(), ());
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the action to reach the server",
        move |_| fake.lock().unwrap().calls.iter().any(|c| c.1.ends_with("/mods/action")),
        |_| String::new(),
    );
    let sent = r.calls("POST", "/api/sessions/s-1/mods/action");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["surface"], json!("desktop"));
    assert_eq!(sent[0]["handle"], json!("h1"));
}

// "draws its own mod's pane when two mods use the same pane id"
#[test]
fn mod_pane_canvas_draws_its_own_mods_pane_when_two_mods_use_the_same_pane_id() {
    let mut r = rig(json!([]), json!([]));
    r.put(view("mod-one", "runs", text("from one")));
    r.put(view("mod-two", "runs", text("from two")));
    assert!(r.words().contains("from two") && !r.words().contains("from one"));
    // The new tab slides in (240 ms); click it where it comes to rest.
    for _ in 0..40 {
        r.rt.frame(FRAME);
    }
    r.press(&crate::fleet::canvas_host::tab_key(&crate::fleet::canvases::mod_pane_canvas_id("mod-one", "runs")));
    assert!(r.words().contains("from one") && !r.words().contains("from two"));
    // A mod that drew nothing is waited for, not shown another's.
    r.rt.update(|s, _| {
        s.canvases.open_mod_pane(
            "s-1",
            crate::fleet::canvases::ModPaneRef { mod_name: "mod-three".into(), pane_id: "runs".into() },
            "Runs",
        );
    });
    r.settle();
    assert!(r.words().contains("Waiting for mod-three to draw this pane"));
}

// "does not redraw for another pane's view"
#[test]
fn mod_pane_canvas_does_not_redraw_for_another_panes_view() {
    let mut r = rig(json!([]), json!([]));
    r.put(view("ci-mod", "runs", text("mine")));
    // Another pane's view changes while its tab is closed to the eye: an update of a pane that isn't open does nothing, and the
    // front pane keeps what it drew.
    r.rt.update(|s, _| s.update_mod_pane(view("ci-mod", "other-pane", text("other"))));
    r.settle();
    assert!(r.words().contains("mine") && !r.words().contains("other"));
    r.put(view("ci-mod", "runs", text("mine, again")));
    assert!(r.words().contains("mine, again"));
}

// ---- ModPaneCanvas.off.test.ts ----

fn open_ci_watch(r: &mut Rig) {
    r.put(view("ci-watch", "runs", text("3 passing")));
}

// "says the mod is off and turns it on with the Settings switch"
#[test]
fn mod_pane_canvas_off_says_the_mod_is_off_and_turns_it_on_with_the_settings_switch() {
    let mut r = rig(json!([declared(json!({}))]), json!([]));
    open_ci_watch(&mut r);
    assert!(r.has("mod-pane-off") && !r.has(&mine("ci-watch")));
    assert!(r.words().contains("ci-watch is off"));
    assert!(r.words().contains("This pane is drawn by the ci-watch mod. Turn it on to see it here."));
    r.press("mod-pane-turn-on");
    assert_eq!(r.calls("POST", "/api/mods/ci-watch/on").len(), 1);
    assert!(r.calls("POST", "/api/sessions/s-1/mods/drafts/ci-watch/on").is_empty());
}

// "turns a draft on with the draft's own switch"
#[test]
fn mod_pane_canvas_off_turns_a_draft_on_with_the_drafts_own_switch() {
    let mut r = rig(json!([declared(json!({ "draft": true }))]), json!([]));
    open_ci_watch(&mut r);
    r.press("mod-pane-turn-on");
    assert_eq!(r.calls("POST", "/api/sessions/s-1/mods/drafts/ci-watch/on").len(), 1);
    assert!(r.calls("POST", "/api/mods/ci-watch/on").is_empty());
}

// "draws the pane once the server says the mod is on"
#[test]
fn mod_pane_canvas_off_draws_the_pane_once_the_server_says_the_mod_is_on() {
    let mut r = rig(json!([declared(json!({}))]), json!([]));
    open_ci_watch(&mut r);
    assert!(r.has("mod-pane-off") && !r.has(&mine("ci-watch")));
    r.with(|f| f.panes = json!([declared(json!({ "off": false }))]));
    r.rt.update(|s, _| {
        s.dispatch(&Topic::Sessions, &[crate::fleet::events::Event::Other(crate::fleet::events::Kind::ModsChanged, json!({}))])
    });
    r.settle();
    assert!(!r.has("mod-pane-off"));
    assert!(r.words().contains("3 passing"));
}

// "judges a pane the mod never declared by the kept mods"
#[test]
fn mod_pane_canvas_off_judges_a_pane_the_mod_never_declared_by_the_kept_mods() {
    let mut r = rig(json!([]), json!([]));
    open_ci_watch(&mut r);
    assert!(!r.has("mod-pane-off"), "no kept mod of that name: not off");
    assert!(!r.calls("GET", "/api/mods").is_empty(), "the pane read the kept mods when it was shown");
    // The kept mods say it is off, and a kept mod's Turn on is the Mods tab's switch.
    let mut r = rig(json!([]), json!([{ "name": "ci-watch", "versions": [], "off": { "by": "user" } }]));
    open_ci_watch(&mut r);
    assert!(r.has("mod-pane-off"));
    r.press("mod-pane-turn-on");
    assert_eq!(r.calls("POST", "/api/mods/ci-watch/on").len(), 1);
    assert!(!r.has("mod-pane-off"), "the answer says it is on");
}

// ---- piece G's own rows (8.4) ----

#[test]
fn a_refused_turn_on_says_so_and_can_be_tried_again() {
    let mut r = rig(json!([declared(json!({}))]), json!([]));
    r.with(|f| f.refuse = true);
    open_ci_watch(&mut r);
    assert!(!r.words().contains("Couldn't turn"));
    r.press("mod-pane-turn-on");
    assert!(r.words().contains("Couldn't turn ci-watch on. Try again, or use Settings → Skills & tools → Mods."));
    r.press("mod-pane-turn-on");
    assert_eq!(r.calls("POST", "/api/mods/ci-watch/on").len(), 2);
}
