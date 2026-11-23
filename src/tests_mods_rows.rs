//! Component 22, piece E: `components/session/__tests__/ToolCard.mods.test.ts` (22 cases) under its names. A mod's trees on a tool
//! row's line and in its opened body. Fleet's tests read the DOM and its CSS; these read what the row lays out and draws. A view is
//! contributed as the server's answer would be (`Renders::views`); asking the server is piece D's (`tests_mods_renders`).
use crate::fleet::mods::points::{ModTreeView, ToolRowSite, ToolRowView};
use crate::fleet::mods::types::{Action, ActionKind, Author};
use crate::mock::tools::{call, took};
use crate::tests::{FRAME, app};
use crate::{State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

/// The calls a finished turn holds, open so each row is on screen.
fn rows(calls: Vec<Value>) -> Rt {
    let mut rt = app();
    rt.update(|s, _| s.show_calls(calls));
    settle(&mut rt);
    rt
}

/// `BASH`: a shell call with output, a summary, and (when it failed) a preview.
fn bash(id: &str, status: &str) -> Value {
    let mut c = call(id, "bash", status, json!({"command": "ls -la", "description": "ls -la"}), took(900));
    c["state"]["output"] = json!("a\nb");
    c["state"]["summary"] = json!("Listed");
    if status == "error" {
        c["state"]["error"] = json!("a (2 lines)");
    }
    c
}

/// `EDIT`: a diff of one removed line and two added.
fn edit(id: &str) -> Value {
    call(id, "edit", "completed", json!({"filePath": "src/a.ts"}), json!({"metadata": {"filediff": {"additions": 2, "deletions": 1}}}))
}

fn empty(id: &str) -> Value {
    call(id, "bash", "completed", json!({"command": "true"}), json!({}))
}

fn text(words: &str) -> Value {
    json!({"type": "Text", "props": {}, "children": [words]})
}
fn button(label: &str) -> Value {
    json!({"type": "Button", "props": {"key": "go", "label": label}, "handles": {"onPress": "h1"}})
}
fn row(children: Vec<Value>) -> Value {
    json!({"type": "Box", "props": {"flexDirection": "row"}, "children": children})
}
fn fleet() -> Value {
    json!({"type": "Fleet"})
}

fn author(draft: bool) -> Vec<Author> {
    vec![Author { name: "M".into(), draft }]
}

/// `contribute`: the view for a call at a site, in the open session. Returns the token to take it away with.
fn contribute_for(rt: &mut Rt, site: ToolRowSite, tree: Value, call: &str, session: Option<&str>, mods: Vec<Author>) -> u64 {
    let (call, session) = (format!("call-{call}"), session.map(str::to_string));
    let token = Arc::new(Mutex::new(0));
    let out = token.clone();
    rt.update(move |s, _| {
        let session_id = session.unwrap_or_else(|| s.mod_row_session());
        let view = ToolRowView { site, session_id, call_id: call, tool: "bash".into(), view: ModTreeView::new(tree, mods) };
        *out.lock().unwrap() = s.mod_renders.views.contribute("test", vec![view]);
    });
    settle(rt);
    *token.lock().unwrap()
}

fn contribute(rt: &mut Rt, site: ToolRowSite, tree: Value, call: &str) -> u64 {
    contribute_for(rt, site, tree, call, None, author(false))
}

fn dispose(rt: &mut Rt, token: u64) {
    rt.update(move |s, _| s.mod_renders.views.dispose(token));
    settle(rt);
}

const LINE: ToolRowSite = ToolRowSite::ToolUse;
const BODY: ToolRowSite = ToolRowSite::ToolResult;

fn info(rt: &Rt, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn words(rt: &Rt) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn says(rt: &Rt, key: &str) -> String {
    info(rt, key).text.unwrap_or_default()
}

/// What the row draws, to see that two states of it are the same: every element under it, its key and its rect.
fn drawing(rt: &Rt, id: &str) -> Vec<(String, [f32; 4])> {
    let prefix = format!("tool-{id}");
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| Some((e.key.filter(|k| k.starts_with(&prefix))?, e.rect))).collect()
}

/// The key a call's line tree is drawn under, and its body's: `<key>.0` is the tree's first element.
fn lk(rt: &Rt, call: &str) -> String {
    crate::fleet::mods::rows::tree_key(LINE, &rt.state.mod_row_session(), &format!("call-{call}"))
}
fn bk(rt: &Rt, call: &str) -> String {
    crate::fleet::mods::rows::tree_key(BODY, &rt.state.mod_row_session(), &format!("call-{call}"))
}

fn open_row(rt: &mut Rt, id: &str, open: bool, under: bool) {
    let id = id.to_string();
    rt.update(move |s, _| {
        s.tool_open.insert(id, crate::conv::card::Toggle { open, under });
    });
    settle(rt);
}

/// A long line: the cap and the floor both bite.
fn long_line() -> Value {
    json!({"type": "Box", "props": {"gap": 1}, "children": [
        {"type": "Pill", "props": {"tone": "warn", "label": "a pill whose words go on for a long time and a long way past what a line holds"}},
        {"type": "Pill", "props": {"tone": "neutral", "label": "and another one beside it"}},
    ]})
}

fn wide_title(id: &str) -> Value {
    call(
        id,
        "bash",
        "completed",
        json!({"command": "bun run a-very-long-command --with --many --flags --and-more-words-after-them"}),
        took(900),
    )
}

// ---- ToolCard keeps the command when a mod draws the line --------------------------------------------------------------------

// "caps the mod slot at 60% and floors the title at min(30%, 12ch)"
#[test]
fn caps_the_mod_slot_at_60_percent_and_floors_the_title_at_min_30_percent_12ch() {
    let mut rt = rows(vec![wide_title("c1")]);
    contribute(&mut rt, LINE, long_line(), "c1");
    let header = info(&rt, "tool-c1").rect;
    let inner = header[2] - 16.0;
    let slot = info(&rt, "tool-c1-mod").rect;
    assert!(slot[2] <= inner * 0.6 + 1.0, "the slot is {} of {inner}", slot[2]);
    assert!(slot[2] > inner * 0.4, "and it takes what it can: {}", slot[2]);
    // The title ellipsizes inside its floor: 12 characters of the mono 12 px, or 30% of a narrow row.
    let title = info(&rt, "tool-c1-detail").rect;
    assert!(title[2] >= (inner * 0.3).min(86.4) - 1.0, "the title keeps {}", title[2]);
    // The pills wrap inside the cap instead of widening the row.
    assert!(slot[1] + slot[3] <= header[1] + header[3] + 1.0 || slot[3] > 26.0, "wrapped onto lines inside the row");
    // A narrow row: the floor is 30% of it.
    assert!((crate::view::mods::row::title_floor(&rt.state) - (inner * 0.3).min(86.4)).abs() < 0.5);
    rt.update(|s, _| s.mod_line_w = 160.0);
    assert!((crate::view::mods::row::title_floor(&rt.state) - 43.2).abs() < 0.01);
}

// "marks the header only when a mod draws, and leaves today's rules for plain titles alone"
#[test]
fn marks_the_header_only_when_a_mod_draws_and_leaves_todays_rules_for_plain_titles_alone() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    let before = drawing(&rt, "c1");
    assert_eq!(rt.state.mod_line_w, 0.0, "a row no mod draws on isn't measured for a cap");
    assert!(!has(&rt, "tool-c1-mod"));
    contribute(&mut rt, LINE, row(vec![text("ok")]), "c1");
    assert!(has(&rt, "tool-c1-mod"));
    assert!(rt.state.mod_line_w > 0.0, "a row a mod draws on is");
    // A row of another call keeps today's drawing, to the pixel, while this one has a mod.
    let mut rt2 = rows(vec![bash("c1", "completed"), bash("c2", "completed")]);
    let plain = drawing(&rt2, "c2");
    contribute(&mut rt2, LINE, row(vec![text("ok")]), "c1");
    assert_eq!(drawing(&rt2, "c2"), plain);
    assert_eq!(before.len(), drawing(&rows(vec![bash("c1", "completed")]), "c1").len());
}

// ---- ToolCard with a ToolUse tree ---------------------------------------------------------------------------------------------

// "draws the tree in the header and drops the check and the preview"
#[test]
fn draws_the_tree_in_the_header_and_drops_the_check_and_the_preview() {
    // A failed call keeps what it printed under its closed row...
    let mut rt = rows(vec![bash("c1", "error")]);
    open_row(&mut rt, "c1", false, false);
    assert!(has(&rt, "tool-c1-preview"));
    // ...and with a mod on the line that preview is dropped. (Fleet's Completed check isn't drawn here at all: the turn's fold says it.)
    contribute(&mut rt, LINE, text("3 files"), "c1");
    assert_eq!(says(&rt, &format!("{}.0", lk(&rt, "c1"))), "3 files");
    assert!(!has(&rt, "tool-c1-preview"));
    assert!(!has(&rt, "tool-c1-check"));
    // The body is still Fleet's own.
    open_row(&mut rt, "c1", true, false);
    assert!(has(&rt, "tool-c1-output"));
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute(&mut rt, LINE, text("3 files"), "c1");
    open_row(&mut rt, "c1", true, true);
    assert!(has(&rt, "tool-c1-output"), "a finished call's body is Fleet's own too");
}

// "keeps Fleet's +N -N after the tree, and drops only the check and the preview"
#[test]
fn keeps_fleets_plus_n_minus_n_after_the_tree_and_drops_only_the_check_and_the_preview() {
    let mut rt = rows(vec![edit("c1"), bash("c2", "completed")]);
    contribute(&mut rt, LINE, text("hi"), "c1");
    assert_eq!(says(&rt, &format!("{}.0", lk(&rt, "c1"))), "hi");
    let stats = info(&rt, "tool-c1-diff-stats");
    let kids: Vec<String> = rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect();
    assert!(kids.contains(&"+2".to_string()) && kids.contains(&"\u{2212}1".to_string()), "{kids:?}");
    assert!(info(&rt, "tool-c1-mod").rect[0] < stats.rect[0], "the tree comes before the counts");
    // No diff stats: nothing after the tree but the call's time, and no check.
    contribute(&mut rt, LINE, text("hi"), "c2");
    assert!(!has(&rt, "tool-c2-diff-stats"));
    assert!(!has(&rt, "tool-c2-check"));
    assert!(has(&rt, "tool-c2-took"), "the time a shell call took stays");
}

// "keeps Running, Error, Stopped, Background and the buttons"
#[test]
fn keeps_running_error_stopped_background_and_the_buttons() {
    let mut calls: Vec<Value> =
        ["running", "error", "cancelled", "background"].iter().enumerate().map(|(i, st)| bash(&format!("c{i}"), st)).collect();
    // Background is a call that returned and runs on; Stopped is what Fleet made of work it stopped.
    calls[2]["state"]["status"] = json!("error");
    calls[2]["state"]["metadata"] = json!({"fleetStopped": true});
    calls[3]["state"]["status"] = json!("running");
    calls[3]["state"]["background"] = json!(true);
    let mut rt = rows(calls);
    for i in 0..4 {
        contribute(&mut rt, LINE, text("hi"), &format!("c{i}"));
    }
    let seen: Vec<bool> = (0..4).map(|i| has(&rt, &format!("tool-c{i}-status"))).collect();
    assert_eq!(seen, [true; 4], "a status shows on each row: {}", words(&rt));
    for i in 0..4 {
        assert!(has(&rt, &format!("tool-c{i}-mod")));
        // The status comes after the tree.
        assert!(info(&rt, &format!("tool-c{i}-mod")).rect[0] < info(&rt, &format!("tool-c{i}-status")).rect[0], "row {i}");
    }
    // Show and Improve stay, before the tree.
    let mut shown = bash("k1", "completed");
    shown["state"]["metadata"] = json!({"canvasId": "cv_1"});
    let skill = call("k2", "skill", "completed", json!({"name": "tidy-docs"}), json!({}));
    let mut rt = rows(vec![shown, skill]);
    rt.update(|s, _| s.skills.list = Some(crate::fleet::skills::mock_skills()));
    contribute(&mut rt, LINE, text("hi"), "k1");
    contribute(&mut rt, LINE, text("hi"), "k2");
    assert!(has(&rt, "tool-k1-show"), "Show stays");
    assert!(has(&rt, "tool-k2-improve"), "Improve stays");
    assert!(info(&rt, "tool-k1-show").rect[0] < info(&rt, "tool-k1-mod").rect[0]);
    assert!(info(&rt, "tool-k2-improve").rect[0] < info(&rt, "tool-k2-mod").rect[0]);
}

// "a null tree draws nothing but still drops the result and the preview"
#[test]
fn a_null_tree_draws_nothing_but_still_drops_the_result_and_the_preview() {
    let mut rt = rows(vec![bash("c1", "error")]);
    open_row(&mut rt, "c1", false, false);
    assert!(has(&rt, "tool-c1-preview"));
    contribute(&mut rt, LINE, Value::Null, "c1");
    assert!(has(&rt, "tool-c1-mod"), "the slot is there");
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("tool-c1-mod.")) && !e.exiting), "and empty");
    assert!(!has(&rt, "tool-c1-preview"));
    assert!(!has(&rt, "tool-c1-check"));
}

// "is keyed by session and call id, not by the tool"
#[test]
fn is_keyed_by_session_and_call_id_not_by_the_tool() {
    let mut rt = rows(vec![bash("c1", "completed"), call("c2", "read", "completed", json!({"filePath": "a.txt"}), json!({}))]);
    // The view says "something else" for its tool; both a Bash and a Read row of the call find it by the call's id.
    let tree = text("yes");
    contribute_for(&mut rt, LINE, tree.clone(), "c1", None, author(false));
    contribute_for(&mut rt, LINE, tree.clone(), "c2", None, author(false));
    rt.update(|s, _| {
        let items = s.mod_renders.views.items();
        assert!(items.iter().all(|v| v.tool == "bash"));
    });
    assert!(has(&rt, "tool-c1-mod") && has(&rt, "tool-c2-mod"), "found by the call, whatever the tool");
    // Another call, another session or no call at all: nothing.
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute_for(&mut rt, LINE, text("yes"), "c9", None, author(false));
    contribute_for(&mut rt, LINE, text("yes"), "c1", Some("another-session"), author(false));
    assert!(!has(&rt, "tool-c1-mod"), "a view of another call or session isn't this row's");
    assert!(rt.state.mod_row_tree(LINE, "").is_none(), "a row with no call id has no view");
}

// "passes its session id to the tree"
#[test]
fn passes_its_session_id_to_the_tree() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute(&mut rt, LINE, text("x"), "c1");
    let session = rt.state.mod_row_session();
    // The tree is keyed by the session and the call, which is what its controls are known by.
    assert_eq!(lk(&rt, "c1"), format!("mod-line:{session}:call-c1"));
    assert!(has(&rt, &lk(&rt, "c1")), "the root draws under it");
}

// "draws Fleet's own for an invalid tree"
#[test]
fn draws_fleets_own_for_an_invalid_tree() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    let failures = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = failures.clone();
    rt.state.mod_resolver.failures().set_warn(|_| {});
    rt.state.mod_resolver.failures().on_invalid(move |_| {
        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    });
    contribute(&mut rt, LINE, json!({"type": "Nope"}), "c1");
    assert!(!has(&rt, "tool-c1-mod"));
    assert!(!has(&rt, "tool-c1-check"));
    assert_eq!(failures.load(std::sync::atomic::Ordering::Relaxed), 1, "reported once, not on every draw");
}

// "redraws when contributed and when removed, and removal restores Fleet's drawing"
#[test]
fn redraws_when_contributed_and_when_removed_and_removal_restores_fleets_drawing() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    let before = drawing(&rt, "c1");
    let token = contribute(&mut rt, LINE, text("live"), "c1");
    assert_eq!(says(&rt, &format!("{}.0", lk(&rt, "c1"))), "live");
    dispose(&mut rt, token);
    assert!(!has(&rt, "tool-c1-mod"));
    assert_eq!(drawing(&rt, "c1"), before);
}

// ---- a press ------------------------------------------------------------------------------------------------------------------

type Heard = Arc<Mutex<Vec<Value>>>;

/// A Fleet that takes a used control (`POST …/mods/action`) and says it was handled.
fn fake_fleet() -> (String, Heard) {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard: Heard = Default::default();
    let seen = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let req = test_http::read(&mut stream);
            if req.path.ends_with("/mods/action") {
                seen.lock().unwrap().push(serde_json::from_slice(&req.body).unwrap_or(Value::Null));
            }
            test_http::respond_json(&mut stream, "200 OK", &json!({ "handled": true }).to_string());
        }
    });
    (base, heard)
}

/// A server's session, so the row's actions have a machine to go to.
fn served(calls: Vec<Value>) -> (Rt, Heard) {
    use crate::fleet::link::{Link, Post};
    let (base, heard) = fake_fleet();
    let (jobs, queue) = std::sync::mpsc::channel();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = rows(calls);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.projects[0].sessions[0].id = "s-1".into();
        s.route = crate::fleet::store::Route::Session("s-1".into());
    });
    settle(&mut rt);
    QUEUES.with(|q| *q.borrow_mut() = Some(queue));
    (rt, heard)
}

thread_local! {
    static QUEUES: std::cell::RefCell<Option<std::sync::mpsc::Receiver<crate::fleet::link::Job>>> = const { std::cell::RefCell::new(None) };
}

fn click(rt: &mut Rt, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

fn heard_action(rt: &mut Rt, heard: &Heard) -> Value {
    QUEUES.with(|q| {
        let q = q.borrow();
        let q = q.as_ref().expect("a served row");
        test_wait::drive(rt, q, "the action to reach Fleet", |_| !heard.lock().unwrap().is_empty(), |_| String::new());
    });
    heard.lock().unwrap()[0].clone()
}

// "sends a press to onAction as desktop, and does not toggle the row"
#[test]
fn sends_a_press_to_on_action_as_desktop_and_does_not_toggle_the_row() {
    let (mut rt, heard) = served(vec![bash("c1", "completed")]);
    contribute(&mut rt, LINE, row(vec![button("Go")]), "c1");
    open_row(&mut rt, "c1", false, true);
    assert!(!has(&rt, "tool-c1-body"), "the row is closed");
    click(&mut rt, "mod-line:s-1:call-c1.0.0");
    let sent = heard_action(&mut rt, &heard);
    assert_eq!((sent["handle"].as_str(), sent["kind"].as_str(), sent["surface"].as_str()), (Some("h1"), Some("press"), Some("desktop")));
    assert_eq!(rt.state.mods_ui.sent, [Action { handle: "h1".into(), kind: ActionKind::Press, value: None }]);
    settle(&mut rt);
    assert!(!has(&rt, "tool-c1-body"), "the press didn't open the row");
    // A press on the tree's words, which are no control, stops there too.
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute(&mut rt, LINE, text("words"), "c1");
    open_row(&mut rt, "c1", false, true);
    let key = format!("{}.0", lk(&rt, "c1"));
    click(&mut rt, &key);
    assert!(!has(&rt, "tool-c1-body"), "nor did a press on the mod's words");
    // And the row itself still toggles on a press elsewhere in it.
    click(&mut rt, "tool-c1-detail");
    assert!(has(&rt, "tool-c1-body"));
}

// "shows no draft mark on the row"
#[test]
fn shows_no_draft_mark_on_the_row() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute_for(&mut rt, LINE, text("d"), "c1", None, author(true));
    assert!(has(&rt, "tool-c1-mod"));
    assert!(rt.state.mod_renders.views.items().iter().any(|v| v.view.mods.iter().any(|m| m.draft)));
    assert!(!words(&rt).to_lowercase().contains("draft"), "no Draft mark: {}", words(&rt));
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("draft-mark"))));
}

// ---- ToolCard with a ToolResult tree ------------------------------------------------------------------------------------------

// "replaces the body, keeping the line, the result and the preview"
#[test]
fn replaces_the_body_keeping_the_line_the_result_and_the_preview() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute(&mut rt, BODY, text("custom body"), "c1");
    open_row(&mut rt, "c1", true, true);
    assert!(has(&rt, &format!("{}.0", bk(&rt, "c1"))));
    assert!(words(&rt).contains("custom body"));
    assert!(!has(&rt, "tool-c1-output"), "Fleet's output is replaced");
    assert!(!has(&rt, "tool-c1-mod"), "a body tree doesn't draw on the line");
    assert!(!has(&rt, "tool-c1-check"));
    assert!(has(&rt, "tool-c1"), "the line stays, with its call's time: {}", has(&rt, "tool-c1-took"));
    // A failed call still keeps its preview under the closed row: only a line tree drops it.
    let mut rt = rows(vec![bash("c1", "error")]);
    contribute(&mut rt, BODY, text("custom body"), "c1");
    open_row(&mut rt, "c1", false, false);
    assert!(has(&rt, "tool-c1-preview"));
}

// "draws Fleet's own body where the tree says Fleet"
#[test]
fn draws_fleets_own_body_where_the_tree_says_fleet() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute(&mut rt, BODY, json!({"type": "Box", "props": {"flexDirection": "column"}, "children": [text("above"), fleet()]}), "c1");
    open_row(&mut rt, "c1", true, true);
    assert!(words(&rt).contains("above"));
    // The summary and the output are Fleet's, below the tree's words.
    assert_eq!(says(&rt, "tool-c1-summary"), "Listed");
    assert!(has(&rt, "tool-c1-output") && words(&rt).contains("a\nb"));
    assert!(info(&rt, &format!("{}.0.0", bk(&rt, "c1"))).rect[1] < info(&rt, "tool-c1-summary").rect[1]);
}

// "a null tree is an empty body, not No output captured"
#[test]
fn a_null_tree_is_an_empty_body_not_no_output_captured() {
    let mut rt = rows(vec![empty("c1")]);
    open_row(&mut rt, "c1", true, true);
    assert!(has(&rt, "tool-c1-empty"), "with no mod: Fleet's words");
    contribute(&mut rt, BODY, Value::Null, "c1");
    assert!(!has(&rt, "tool-c1-empty"));
    assert!(has(&rt, "tool-c1-body"));
    assert!(!words(&rt).contains("No output captured"));
}

// "draws Fleet's own for an invalid tree, and after removal"
#[test]
fn draws_fleets_own_for_an_invalid_tree_and_after_removal() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    open_row(&mut rt, "c1", true, true);
    let before = drawing(&rt, "c1");
    let token = contribute(&mut rt, BODY, json!({"type": "Nope"}), "c1");
    assert_eq!(drawing(&rt, "c1"), before);
    dispose(&mut rt, token);
    let token = contribute(&mut rt, BODY, text("x"), "c1");
    assert!(!has(&rt, "tool-c1-output"));
    dispose(&mut rt, token);
    assert_eq!(drawing(&rt, "c1"), before);
}

// "sends a press to onAction as desktop"
#[test]
fn sends_a_press_to_on_action_as_desktop_for_a_body() {
    let (mut rt, heard) = served(vec![bash("c1", "completed")]);
    contribute(&mut rt, BODY, button("Go"), "c1");
    open_row(&mut rt, "c1", true, true);
    click(&mut rt, "mod-body:s-1:call-c1.0");
    let sent = heard_action(&mut rt, &heard);
    assert_eq!((sent["handle"].as_str(), sent["kind"].as_str(), sent["surface"].as_str()), (Some("h1"), Some("press"), Some("desktop")));
}

// "both sites can draw at once"
#[test]
fn both_sites_can_draw_at_once() {
    let mut rt = rows(vec![bash("c1", "completed")]);
    contribute(&mut rt, LINE, text("line"), "c1");
    contribute(&mut rt, BODY, text("body"), "c1");
    open_row(&mut rt, "c1", true, true);
    assert_eq!(says(&rt, &format!("{}.0", lk(&rt, "c1"))), "line");
    assert!(words(&rt).contains("body"));
    assert!(!has(&rt, "tool-c1-output"));
}

// ---- MessageToolList ----------------------------------------------------------------------------------------------------------

// "passes the session and call ids to the row"
#[test]
fn passes_the_session_and_call_ids_to_the_row() {
    // A call inside a message's tool list (the rows of a turn) finds its view by the open session and its call's id.
    let mut rt = rows(vec![bash("c9", "completed")]);
    contribute(&mut rt, LINE, text("via list"), "c9");
    assert_eq!(says(&rt, &format!("{}.0", lk(&rt, "c9"))), "via list");
}

// ---- ToolCard rows at scale ---------------------------------------------------------------------------------------------------

/// A turn of many calls, in as many messages: a message's memo key is what decides whether its row draws again.
fn many(n: usize) -> Rt {
    let mut rt = app();
    rt.update(move |s, _| s.show_turn((0..n).map(|i| vec![bash(&format!("c{i}"), "completed")]).collect()));
    settle(&mut rt);
    rt
}

fn keys(rt: &Rt) -> Vec<u64> {
    rt.state.session().messages.iter().map(|m| crate::view::mods::row::key(&rt.state, m)).collect()
}

fn moved(before: &[u64], after: &[u64]) -> usize {
    assert_eq!(before.len(), after.len());
    before.iter().zip(after).filter(|(a, b)| a != b).count()
}

// "contributing one row's view redraws that row only"
#[test]
fn contributing_one_rows_view_redraws_that_row_only() {
    let mut rt = many(120);
    let before = keys(&rt);
    contribute(&mut rt, LINE, text("ok"), "c7");
    assert_eq!(moved(&before, &keys(&rt)), 1);
}

// "contributing a view for a call that is not on screen redraws nothing"
#[test]
fn contributing_a_view_for_a_call_that_is_not_on_screen_redraws_nothing() {
    let mut rt = many(120);
    let before = keys(&rt);
    contribute(&mut rt, LINE, text("ok"), "not-here");
    contribute_for(&mut rt, BODY, Value::Null, "c7", Some("other-session"), author(false));
    assert_eq!(moved(&before, &keys(&rt)), 0);
}

// "removing one row's view redraws that row only"
#[test]
fn removing_one_rows_view_redraws_that_row_only() {
    let mut rt = many(100);
    let token = contribute(&mut rt, LINE, text("ok"), "c3");
    let before = keys(&rt);
    dispose(&mut rt, token);
    assert_eq!(moved(&before, &keys(&rt)), 1);
    assert!(!rt.elements().iter().any(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.ends_with("-mod"))));
}
