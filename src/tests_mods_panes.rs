//! Component 22, piece F: a mod's panes as tabs. `lib/mods/__tests__/panes.test.ts` ("mod panes", 9),
//! `composables/__tests__/use-session-mod-panes.test.ts` (5), `components/canvas/__tests__/CanvasHost.mods.test.ts` (5) and the two
//! "From mods" cases of `CanvasHost.paneMenu.test.ts`, under Fleet's names, against a fake Fleet that answers the Mods switch, the
//! session's sites and panes, renders and actions. Fleet's tests read the DOM; these read the tabs, the views and what lays out.
use crate::fleet::canvases::{CanvasKind, mod_pane_canvas_id};
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::mods::panes::empty_view;
use crate::fleet::mods::points::{ModPaneView, ModTreeView};
use crate::fleet::mods::types::Author;
use crate::fleet::state::Topic;
use crate::tests::{FRAME, app};
use crate::{State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn text(words: &str) -> Value {
    json!({ "type": "Text", "props": {}, "children": [words] })
}

fn view(title: &str, mod_name: &str) -> ModPaneView {
    ModPaneView {
        session_id: "s1".into(),
        pane_id: "runs".into(),
        title: title.into(),
        mod_id: mod_name.into(),
        view: ModTreeView::new(Value::Null, vec![Author { name: "ci-mod".into(), draft: false }]),
    }
}

fn pane(draft: bool, pane_id: &str, title: &str, mod_name: &str, words: &str) -> ModPaneView {
    ModPaneView {
        session_id: "s-1".into(),
        pane_id: pane_id.into(),
        title: title.into(),
        mod_id: mod_name.into(),
        view: ModTreeView::new(text(words), vec![Author { name: "ci-mod".into(), draft }]),
    }
}

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

fn rect(rt: &Rt, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).0
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn mod_tabs(rt: &Rt, session: &str) -> Vec<crate::fleet::canvases::CanvasInstance> {
    rt.state.canvases.session(session).canvases.into_iter().filter(|c| c.kind == CanvasKind::Mod).collect()
}

fn mod_titles(rt: &Rt) -> Vec<String> {
    rt.state.mod_panes.views.items().iter().map(|p| p.title.clone()).collect()
}

fn active(rt: &Rt, session: &str) -> String {
    rt.state.canvases.session(session).active_id
}

// ---- panes.test.ts ----

/// An app whose open session is `s1`, which is the session `State::close_tab` closes in (no server: nothing is asked).
fn model() -> Rt {
    let mut rt = app();
    rt.update(|s, _| s.route = crate::fleet::store::Route::Session("s1".into()));
    rt
}

// "registers the mod canvas kind after the core ones"
#[test]
fn registers_the_mod_canvas_kind_after_the_core_ones() {
    assert_eq!(CanvasKind::Mod.label(), "Mod");
    let at = |k: CanvasKind| CanvasKind::ALL.iter().position(|x| *x == k).unwrap();
    assert!(at(CanvasKind::Mod) > at(CanvasKind::File));
}

// "opens one canvas, activates it, and titles the tab from the pane"
#[test]
fn opens_one_canvas_activates_it_and_titles_the_tab_from_the_pane() {
    let mut rt = model();
    rt.update(|s, _| s.open_mod_pane(view("Test runs", "ci-mod")));
    let tabs = mod_tabs(&rt, "s1");
    assert_eq!(tabs.len(), 1);
    assert_eq!(active(&rt, "s1"), tabs[0].id);
    assert_eq!(tabs[0].title(), "Test runs");
    assert_eq!(rt.state.mod_panes.get("s1", "ci-mod", "runs").map(|v| v.title.clone()).as_deref(), Some("Test runs"));
}

// "re-opening replaces the view, brings the tab forward and adds no tab"
#[test]
fn re_opening_replaces_the_view_brings_the_tab_forward_and_adds_no_tab() {
    let mut rt = model();
    rt.update(|s, _| s.open_mod_pane(view("Test runs", "ci-mod")));
    rt.update(|s, _| {
        s.canvases.activate("s1", "files");
        s.open_mod_pane(view("Renamed", "ci-mod"));
    });
    let tabs = mod_tabs(&rt, "s1");
    assert_eq!(tabs.len(), 1);
    assert_eq!(active(&rt, "s1"), tabs[0].id);
    assert_eq!(tabs[0].title(), "Renamed");
    assert_eq!(rt.state.mod_panes.get("s1", "ci-mod", "runs").unwrap().title, "Renamed");
    assert_eq!(rt.state.mod_panes.views.len(), 1);
}

// "updating redraws an open pane and its tab title without pulling it forward or adding a tab"
#[test]
fn updating_redraws_an_open_pane_and_its_tab_title_without_pulling_it_forward_or_adding_a_tab() {
    let mut rt = model();
    rt.update(|s, _| s.open_mod_pane(view("Test runs", "ci-mod")));
    rt.update(|s, _| {
        s.canvases.activate("s1", "files");
        s.update_mod_pane(view("Renamed", "ci-mod"));
    });
    let tabs = mod_tabs(&rt, "s1");
    assert_eq!(tabs.len(), 1);
    assert_eq!(active(&rt, "s1"), "files");
    assert_eq!(tabs[0].title(), "Renamed");
    assert_eq!(rt.state.mod_panes.get("s1", "ci-mod", "runs").unwrap().title, "Renamed");
}

// "updating a pane whose tab was closed does nothing: no view comes back and no tab opens"
#[test]
fn updating_a_pane_whose_tab_was_closed_does_nothing_no_view_comes_back_and_no_tab_opens() {
    let mut rt = model();
    rt.update(|s, _| s.open_mod_pane(view("Test runs", "ci-mod")));
    rt.update(|s, tx| s.close_tab(&mod_pane_canvas_id("ci-mod", "runs"), tx));
    assert!(rt.state.mod_panes.views.is_empty());
    rt.update(|s, _| s.update_mod_pane(view("Late", "ci-mod")));
    assert!(rt.state.mod_panes.views.is_empty());
    assert!(mod_tabs(&rt, "s1").is_empty());
}

// "updating a pane that was never opened does nothing"
#[test]
fn updating_a_pane_that_was_never_opened_does_nothing() {
    let mut rt = model();
    rt.update(|s, _| s.update_mod_pane(view("Test runs", "ci-mod")));
    assert!(mod_tabs(&rt, "s1").is_empty());
    assert!(rt.state.mod_panes.views.is_empty());
}

// "two mods with the same pane id get two tabs and two views"
#[test]
fn two_mods_with_the_same_pane_id_get_two_tabs_and_two_views() {
    let mut rt = model();
    rt.update(|s, _| {
        s.open_mod_pane(view("One", "mod-one"));
        s.open_mod_pane(view("Two", "mod-two"));
    });
    let tabs = mod_tabs(&rt, "s1");
    assert_eq!(tabs.len(), 2);
    assert_ne!(tabs[0].id, tabs[1].id);
    assert_eq!(mod_titles(&rt), ["One", "Two"]);
    rt.update(|s, _| s.close_mod_pane("s1", "mod-one", "runs"));
    let left: Vec<_> = mod_tabs(&rt, "s1").iter().filter_map(|c| c.mod_pane.as_ref().map(|p| p.mod_name.clone())).collect();
    assert_eq!(left, ["mod-two"]);
    assert_eq!(mod_titles(&rt), ["Two"]);
}

// "closing the tab from the canvas store drops the pane's view (the phone menu stops listing it)"
#[test]
fn closing_the_tab_from_the_canvas_store_drops_the_panes_view_the_phone_menu_stops_listing_it() {
    let mut rt = model();
    rt.update(|s, _| s.open_mod_pane(view("Test runs", "ci-mod")));
    let id = mod_tabs(&rt, "s1")[0].id.clone();
    rt.update(|s, tx| s.close_tab(&id, tx));
    assert!(mod_tabs(&rt, "s1").is_empty());
    assert!(rt.state.mod_panes.get("s1", "ci-mod", "runs").is_none());
    assert!(rt.state.mod_panes.views.is_empty());
}

// "closing removes the view and the canvas"
#[test]
fn closing_removes_the_view_and_the_canvas() {
    let mut rt = model();
    rt.update(|s, _| s.open_mod_pane(view("Test runs", "ci-mod")));
    rt.update(|s, _| s.close_mod_pane("s1", "ci-mod", "runs"));
    assert!(mod_tabs(&rt, "s1").is_empty());
    assert!(rt.state.mod_panes.get("s1", "ci-mod", "runs").is_none());
}

// ---- a fake Fleet ----

#[derive(Default)]
struct Fake {
    mods_on: bool,
    panes: Value,
    /// What renders answer: the next one each time, the last one kept.
    renders: Vec<Value>,
    action: Option<Value>,
    /// A render answers with its own title as the tree: which ask an answer belongs to doesn't depend on which request got there first.
    echo: bool,
    calls: Vec<(String, String, Value)>,
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || {
                let req = test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let mut f = fake.lock().unwrap();
                f.calls.push((req.method.clone(), req.path.clone(), req.json()));
                let reply = if req.path == "/api/features/mods" {
                    json!({ "on": f.mods_on, "safeMode": false })
                } else if req.path.ends_with("/mods/sites") {
                    json!({ "sites": [], "panes": f.panes })
                } else if req.path.ends_with("/mods/action") {
                    f.action.clone().unwrap_or(json!({ "handled": true }))
                } else if req.path.ends_with("/mods/render") && f.echo {
                    drawn(req.json()["props"]["title"].as_str().unwrap_or_default(), false)
                } else if req.path.ends_with("/mods/render") {
                    let next = if f.renders.len() > 1 { Some(f.renders.remove(0)) } else { f.renders.first().cloned() };
                    next.unwrap_or(json!({ "tree": null, "mods": [] }))
                } else {
                    json!({})
                };
                drop(f);
                test_http::respond_json(&mut stream, "200 OK", &reply.to_string());
            });
        }
    });
    base
}

fn drawn(words: &str, draft: bool) -> Value {
    json!({ "tree": text(words), "mods": [{ "name": "release-board", "draft": draft }] })
}

struct Rig {
    rt: Rt,
    queue: Receiver<Job>,
    fake: Shared,
}

/// An app on the open session `s-1` of a fake Fleet, the panel open.
fn rig(panes: Value) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake { mods_on: true, panes, renders: vec![drawn("Board", false)], ..Default::default() }));
    let base = serve(fake.clone());
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
    settle(&mut rt);
    Rig { rt, queue, fake }
}

impl Rig {
    /// Lets everything on its way land: jobs applied and frames drawn until nothing is asked.
    fn settle(&mut self) {
        for _ in 0..3 {
            test_wait::drive(
                &mut self.rt,
                &self.queue,
                "every request to be answered",
                |s| s.mod_renders.idle() && s.mod_panes.idle(),
                |_| String::new(),
            );
            settle(&mut self.rt);
        }
    }

    fn with(&self, change: impl FnOnce(&mut Fake)) {
        change(&mut self.fake.lock().unwrap());
    }

    fn calls(&self, tail: &str) -> Vec<Value> {
        self.fake.lock().unwrap().calls.iter().filter(|c| c.1.ends_with(tail)).map(|c| c.2.clone()).collect()
    }

    /// Waits until the fake has heard `n` requests ending in `tail` (a used control's action is no draw `settle` could count).
    fn wait_calls(&mut self, tail: &str, n: usize) {
        let (fake, tail) = (self.fake.clone(), tail.to_string());
        test_wait::drive(
            &mut self.rt,
            &self.queue,
            &format!("{n} requests ending {tail}"),
            move |_| fake.lock().unwrap().calls.iter().filter(|c| c.1.ends_with(&tail)).count() >= n,
            |_| String::new(),
        );
        self.settle();
    }

    fn emit(&mut self, topic: Topic, kind: Kind, payload: Value) {
        self.rt.update(move |s, _| s.dispatch(&topic, &[Event::Other(kind, payload)]));
        self.settle();
    }

    fn open(&mut self, payload: Value) {
        self.emit(Topic::Session("s-1".into()), Kind::ModPane, payload);
    }

    fn tree_of(&self, mod_name: &str, pane: &str) -> Option<Value> {
        self.rt.state.mod_panes.get("s-1", mod_name, pane).map(|v| (*v.view.tree).clone())
    }

    fn tab_count(&self) -> usize {
        self.rt.state.canvases.session("s-1").canvases.len()
    }
}

// ---- use-session-mod-panes.test.ts ----

fn board() -> Value {
    json!({ "sessionId": "s-1", "mod": "release-board", "id": "board", "title": "Release board", "open": true })
}

// "opens a tab for the pane a mod opened, brings it forward, and asks that mod to draw it"
#[test]
fn use_session_mod_panes_opens_a_tab_for_the_pane_a_mod_opened_brings_it_forward_and_asks_that_mod_to_draw_it() {
    let mut r = rig(json!([]));
    r.open(board());
    let ids: Vec<_> = mod_tabs(&r.rt, "s-1").iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids, [mod_pane_canvas_id("release-board", "board")]);
    assert_eq!(active(&r.rt, "s-1"), mod_pane_canvas_id("release-board", "board"));
    assert_eq!(
        r.calls("/mods/render"),
        [json!({ "site": "Pane", "requestId": "board", "mod": "release-board", "props": { "title": "Release board" } })]
    );
    assert_eq!(r.tree_of("release-board", "board"), Some(text("Board")));
}

// "opens the content panel when it was closed"
#[test]
fn use_session_mod_panes_opens_the_content_panel_when_it_was_closed() {
    let mut r = rig(json!([]));
    r.rt.update(|s, _| s.set_content_open(false));
    assert!(!r.rt.state.content_open);
    r.open(board());
    assert!(r.rt.state.content_open);
}

// "closes the tab when the mod closes the pane"
#[test]
fn use_session_mod_panes_closes_the_tab_when_the_mod_closes_the_pane() {
    let mut r = rig(json!([]));
    r.open(board());
    r.emit(
        Topic::Session("s-1".into()),
        Kind::ModPane,
        json!({ "sessionId": "s-1", "mod": "release-board", "id": "board", "open": false }),
    );
    assert!(mod_tabs(&r.rt, "s-1").is_empty());
    assert_eq!(r.tree_of("release-board", "board"), None);
}

// "draws an open pane again on mods.changed, when the first draw came back empty during a reload"
#[test]
fn use_session_mod_panes_draws_an_open_pane_again_on_mods_changed_when_the_first_draw_came_back_empty_during_a_reload() {
    let mut r = rig(json!([]));
    r.with(|f| f.renders = vec![json!({ "tree": null, "mods": [] }), drawn("Release 2.4", false)]);
    r.open(board());
    assert_eq!(r.tree_of("release-board", "board"), Some(Value::Null));
    r.emit(Topic::Sessions, Kind::ModsChanged, json!({}));
    assert_eq!(r.calls("/mods/render").len(), 2);
    assert_eq!(r.tree_of("release-board", "board"), Some(text("Release 2.4")));
}

// "draws an open pane again when its mod redraws, but not for another mod's redraw"
#[test]
fn use_session_mod_panes_draws_an_open_pane_again_when_its_mod_redraws_but_not_for_another_mods_redraw() {
    let mut r = rig(json!([]));
    r.open(board());
    r.emit(Topic::Session("s-1".into()), Kind::ModUi, json!({ "sessionId": "s-1", "mod": "other-mod" }));
    assert_eq!(r.calls("/mods/render").len(), 1);
    r.emit(Topic::Session("s-1".into()), Kind::ModUi, json!({ "sessionId": "s-1", "mod": "release-board" }));
    assert_eq!(r.calls("/mods/render").len(), 2);
    // On `sessions` too: for this session, or with no session.
    r.emit(Topic::Sessions, Kind::ModUi, json!({ "sessionId": "s-1", "mod": "release-board" }));
    assert_eq!(r.calls("/mods/render").len(), 3);
    r.emit(Topic::Sessions, Kind::ModUi, json!({ "sessionId": "another", "mod": "release-board" }));
    assert_eq!(r.calls("/mods/render").len(), 3, "another session's redraw is not ours");
}

// ---- panes.rs: the newest ask wins (row 7.4) ----

#[test]
fn the_newest_ask_per_pane_wins_and_no_answer_leaves_the_pane_as_it_was() {
    let mut r = rig(json!([]));
    r.open(board());
    assert_eq!(r.tree_of("release-board", "board"), Some(text("Board")));
    // An answer that is a refusal changes nothing.
    r.with(|f| f.renders = vec![json!("nope")]);
    r.emit(Topic::Session("s-1".into()), Kind::ModUi, json!({ "mod": "release-board" }));
    assert_eq!(r.tree_of("release-board", "board"), Some(text("Board")));
    // Two asks at once: only the later one's answer lands.
    r.with(|f| f.echo = true);
    r.rt.update(|s, _| {
        s.draw_mod_pane("s-1", "release-board", "board", "older");
        s.draw_mod_pane("s-1", "release-board", "board", "newer");
    });
    r.settle();
    assert_eq!(r.tree_of("release-board", "board"), Some(text("newer")));
}

#[test]
fn an_answer_with_no_mods_is_credited_to_the_panes_mod() {
    let mut r = rig(json!([]));
    r.with(|f| f.renders = vec![json!({ "tree": text("x"), "mods": [] })]);
    r.open(board());
    let v = r.rt.state.mod_panes.get("s-1", "release-board", "board").unwrap();
    assert_eq!(v.view.mods, [Author { name: "release-board".into(), draft: false }]);
}

#[test]
fn a_used_control_goes_to_the_mod_and_the_pane_draws_again_when_it_was_not_handled() {
    let mut r = rig(json!([]));
    let button = json!({ "type": "Button", "props": { "key": "go", "label": "Go" }, "handles": { "onPress": "h1" } });
    r.with(|f| f.renders = vec![json!({ "tree": button, "mods": [{ "name": "release-board", "draft": false }] })]);
    r.open(board());
    settle(&mut r.rt);
    crate::tests::click(&mut r.rt, "mod-pane:s-1:release-board:board.0");
    r.wait_calls("/mods/action", 1);
    assert_eq!(r.calls("/mods/action").len(), 1);
    assert_eq!(r.calls("/mods/render").len(), 1, "handled: no redraw");
    r.with(|f| f.action = Some(json!({ "handled": false })));
    crate::tests::click(&mut r.rt, "mod-pane:s-1:release-board:board.0");
    // The action's own answer isn't counted by `settle`: wait for the draw it asks for.
    r.wait_calls("/mods/render", 2);
    assert_eq!(r.calls("/mods/action").len(), 2);
    assert_eq!(r.calls("/mods/render").len(), 2, "not handled: the pane draws again");
}

// ---- CanvasHost.mods.test.ts ----

fn tab_key(mod_name: &str, pane: &str) -> String {
    crate::fleet::canvas_host::tab_key(&mod_pane_canvas_id(mod_name, pane))
}

fn put(r: &mut Rig, v: ModPaneView) {
    r.rt.update(move |s, _| s.open_mod_pane(v));
    settle(&mut r.rt);
}

// "has no scroll padding without a draft pane (main's strip), and gains it with one"
#[test]
fn canvas_host_has_no_scroll_padding_without_a_draft_pane_main_s_strip_and_gains_it_with_one() {
    let mut r = rig(json!([]));
    put(&mut r, pane(false, "runs", "Test runs", "ci-mod", "3 passing"));
    assert!(!has(&r.rt, "canvas-tab-reach-mod:ci-mod:runs"), "no draft pane: the strip is main's");
    put(&mut r, pane(true, "d", "Draft pane", "ci-mod", "3 passing"));
    // The new tab slides in (240 ms); measure it where it comes to rest.
    for _ in 0..40 {
        r.rt.frame(FRAME);
    }
    for id in ["changes", "files", "mod:ci-mod:runs", "mod:ci-mod:d"] {
        assert!(has(&r.rt, &format!("canvas-tab-reach-{id}")), "{id}: the strip reveals a box 24 px wider on each side");
    }
    let (tab, reach) = (rect(&r.rt, "canvas-tab-mod:ci-mod:d"), rect(&r.rt, "canvas-tab-reach-mod:ci-mod:d"));
    assert!(
        (tab[0] - reach[0] - 24.0).abs() < 0.6 && ((reach[0] + reach[2]) - (tab[0] + tab[2]) - 24.0).abs() < 0.6,
        "{tab:?} in {reach:?}"
    );
}

// "shows a mod pane's title in its tab, with the Draft mark only for a draft"
#[test]
fn canvas_host_shows_a_mod_panes_title_in_its_tab_with_the_draft_mark_only_for_a_draft() {
    let mut r = rig(json!([]));
    put(&mut r, pane(false, "runs", "Test runs", "ci-mod", "3 passing"));
    assert_eq!(mod_tabs(&r.rt, "s-1")[0].title(), "Test runs");
    assert_eq!(active(&r.rt, "s-1"), "mod:ci-mod:runs");
    assert!(has(&r.rt, &tab_key("ci-mod", "runs")));
    assert!(!has(&r.rt, "mod-draft-mark"));
    assert!(has(&r.rt, "mod-pane:s-1:ci-mod:runs"), "its tree is drawn");
    put(&mut r, pane(true, "runs", "Test runs", "ci-mod", "3 passing"));
    assert_eq!(r.tab_count(), 3);
    let (mark, tab) = (rect(&r.rt, "mod-draft-mark"), rect(&r.rt, &tab_key("ci-mod", "runs")));
    assert!(mark[0] >= tab[0] && mark[0] + mark[2] <= tab[0] + tab[2], "the mark is inside the tab: {mark:?} {tab:?}");
    assert_eq!(mark[3].round(), 15.0, "compact: line 13 and a 1 px border each side");
    r.rt.update(|s, _| s.close_mod_pane("s-1", "ci-mod", "runs"));
    settle(&mut r.rt);
    assert_eq!(r.tab_count(), 2);
    assert!(mod_tabs(&r.rt, "s-1").is_empty());
}

// "closing a pane's tab with its × drops the pane's view"
#[test]
fn canvas_host_closing_a_panes_tab_with_its_x_drops_the_panes_view() {
    let mut r = rig(json!([]));
    put(&mut r, pane(false, "runs", "Test runs", "ci-mod", "3 passing"));
    crate::tests::click(&mut r.rt, &crate::fleet::canvas_host::close_key("mod:ci-mod:runs"));
    settle(&mut r.rt);
    assert_eq!(r.tab_count(), 2);
    assert!(r.rt.state.mod_panes.views.is_empty());
}

// "redrawing a pane with an update keeps the tab where the user is"
#[test]
fn canvas_host_redrawing_a_pane_with_an_update_keeps_the_tab_where_the_user_is() {
    let mut r = rig(json!([]));
    put(&mut r, pane(false, "runs", "Test runs", "ci-mod", "3 passing"));
    crate::tests::click(&mut r.rt, "canvas-tab-changes");
    r.rt.update(|s, _| s.update_mod_pane(pane(true, "runs", "Test runs again", "ci-mod", "3 passing")));
    settle(&mut r.rt);
    assert_eq!(active(&r.rt, "s-1"), "changes");
    assert_eq!(mod_tabs(&r.rt, "s-1")[0].title(), "Test runs again");
    assert_eq!(r.rt.state.canvases.session("s-1").canvases.last().unwrap().title(), "Test runs again");
    assert!(has(&r.rt, "mod-draft-mark"));
}

// "two mods with the same pane id draw their own pane in their own tab"
#[test]
fn canvas_host_two_mods_with_the_same_pane_id_draw_their_own_pane_in_their_own_tab() {
    let mut r = rig(json!([]));
    put(&mut r, pane(false, "runs", "One", "mod-one", "from one"));
    put(&mut r, pane(false, "runs", "Two", "mod-two", "from two"));
    assert_eq!(r.tab_count(), 4);
    assert!(has(&r.rt, "mod-pane:s-1:mod-two:runs.0") && !has(&r.rt, "mod-pane:s-1:mod-one:runs.0"), "the active pane's tree");
    crate::tests::click(&mut r.rt, &tab_key("mod-one", "runs"));
    settle(&mut r.rt);
    assert!(has(&r.rt, "mod-pane:s-1:mod-one:runs.0") && !has(&r.rt, "mod-pane:s-1:mod-two:runs.0"));
}

// ---- CanvasHost.paneMenu.test.ts ----

fn declared() -> Value {
    json!([
        { "mod": "release-board", "id": "board", "title": "Release board", "icon": "gauge", "draft": false, "off": false },
        { "mod": "ci-watch", "id": "runs", "title": "CI runs", "icon": "zap", "draft": false, "off": true },
        { "mod": "test-digest", "id": "summary", "title": "Test summary", "icon": null, "draft": true, "off": false },
    ])
}

fn menu(r: &mut Rig) {
    r.settle();
    crate::tests::click(&mut r.rt, "canvas-add");
    settle(&mut r.rt);
}

fn hint(r: &Rig, mod_name: &str, pane: &str) -> Option<String> {
    let key = format!("canvas-picker-mod-{mod_name}-{pane}");
    r.rt.state.mod_menu_panes().into_iter().find(|(p, _)| p.mod_name == mod_name && p.id == pane).map(|(p, open)| {
        if p.off {
            "Off".into()
        } else if open {
            "Open".into()
        } else {
            format!("{key}:{}", p.mod_name)
        }
    })
}

// "lists each declared pane with its mod's name, a Draft mark for a draft, and Off for a mod that is off"
#[test]
fn pane_menu_lists_each_declared_pane_with_its_mods_name_a_draft_mark_for_a_draft_and_off_for_a_mod_that_is_off() {
    let mut r = rig(declared());
    menu(&mut r);
    for key in ["canvas-picker-mod-release-board-board", "canvas-picker-mod-ci-watch-runs", "canvas-picker-mod-test-digest-summary"] {
        assert!(has(&r.rt, key), "{key}");
    }
    assert_eq!(hint(&r, "ci-watch", "runs").as_deref(), Some("Off"));
    assert!(hint(&r, "release-board", "board").unwrap().ends_with(":release-board"), "the mod's name is the hint");
    // One Draft mark in the menu, on the draft's row, inside it.
    let marks: Vec<_> = r.rt.state.mod_menu_panes().into_iter().filter(|(p, _)| p.draft).collect();
    assert_eq!(marks.len(), 1);
    let row = rect(&r.rt, "canvas-picker-mod-test-digest-summary");
    let mark = rect(&r.rt, "mod-draft-mark");
    assert!(mark[1] >= row[1] && mark[1] + mark[3] <= row[1] + row[3], "{mark:?} in {row:?}");
    assert_eq!(mark[3].round(), 17.0, "full size: line 15 and a 1 px border each side");
    // Off mods' panes are listed too; the group has its label.
    assert!(r.rt.state.mod_menu_panes().iter().any(|(p, _)| p.off));
}

// "opens the pane's tab and asks its mod to draw it; the row then says Open"
#[test]
fn pane_menu_opens_the_panes_tab_and_asks_its_mod_to_draw_it_the_row_then_says_open() {
    let mut r = rig(declared());
    menu(&mut r);
    crate::tests::click(&mut r.rt, "canvas-picker-mod-release-board-board");
    r.settle();
    assert!(mod_tabs(&r.rt, "s-1").iter().any(|c| c.mod_pane.as_ref().is_some_and(|p| p.pane_id == "board")));
    let render = &r.calls("/mods/render")[0];
    assert_eq!(
        (render["site"].as_str(), render["requestId"].as_str(), render["mod"].as_str()),
        (Some("Pane"), Some("board"), Some("release-board"))
    );
    assert_eq!(r.tree_of("release-board", "board"), Some(text("Board")));
    assert!(has(&r.rt, "mod-pane:s-1:release-board:board.0"), "the tree is drawn");
    assert!(!has(&r.rt, "canvas-picker-menu"), "picking closes the menu");
    menu(&mut r);
    assert_eq!(hint(&r, "release-board", "board").as_deref(), Some("Open"));
}

// "has no From mods group when no mod declares a pane" (ported by 10E; kept with the rest)
#[test]
fn pane_menu_has_no_from_mods_group_when_no_mod_declares_a_pane() {
    let mut r = rig(json!([]));
    menu(&mut r);
    assert!(r.rt.state.mod_menu_panes().is_empty());
    assert!(!has(&r.rt, "canvas-picker-mod-release-board-board"));
    let _ = empty_view("s", "m", "p", "t");
}
