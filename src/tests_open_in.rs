//! The Open in submenu (piece N of docs/parity/06-session-list.md): rows 4.40-4.47 of `OpenToolContextSubmenu.vue` and its
//! composables (Fleet has no component test for them, so the names are the rows'). The invented machine answers a menu with
//! no server; the requests are read from a fake Fleet that opens nothing. Failures show nothing (4.47).
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::open_in::{Category, Opened, absolute, groups, parse_key_files, parse_tools};
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const DIR: &str = "/tmp/fleet-native-scratch-n/demo";

fn menu_on(rt: &mut Runtime<State>) {
    rt.update(|s, _| s.projects[0].sessions[1].listing.workspace_directory = DIR.into());
    rt.frame(FRAME);
    let (x, y) = rt.find("session-0-1").expect("the row");
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("ctx-menu").is_some(), "the menu opened");
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn open_in(rt: &mut Runtime<State>) {
    click(rt, "session-context-open-in");
    rt.frame(FRAME);
}

/// Keys of rows whose key starts with `open-in-` and that are drawn, top to bottom.
fn rows(rt: &Runtime<State>) -> Vec<String> {
    let mut r: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter_map(|e| e.key.filter(|k| k.starts_with("open-in-")).map(|k| (e.rect[1] + e.rect[0] * 0.001, k)))
        .collect();
    r.sort_by(|a, b| a.0.total_cmp(&b.0));
    r.into_iter().map(|(_, k)| k).collect()
}

#[test]
fn the_trigger_row_has_a_chevron_and_opens_a_submenu_on_a_press() {
    let mut rt = app();
    menu_on(&mut rt);
    assert!(has(&rt, "session-context-open-in") && !has(&rt, "open-in-code"));
    open_in(&mut rt);
    assert!(has(&rt, "open-in-code") && has(&rt, "open-in-term"));
    // The same press again closes it.
    open_in(&mut rt);
    assert!(!has(&rt, "open-in-code"));
}

#[test]
fn the_trigger_says_what_it_does_in_the_footer_and_the_submenu_leaves_it_alone() {
    let mut rt = app();
    menu_on(&mut rt);
    open_in(&mut rt);
    let first = rt.state.ui.hint.clone();
    assert_eq!(first.as_deref(), Some(crate::view::open_in::HINT));
    let (x, y) = rt.find("open-in-term").unwrap();
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.ui.hint, first, "rows of the submenu don't change the footer");
}

#[test]
fn says_detecting_tools_with_a_spinner_while_none_are_known() {
    let mut rt = app();
    menu_on(&mut rt);
    rt.update(|s, _| s.open_in.tools = None);
    open_in(&mut rt);
    assert_eq!(rows(&rt), vec!["open-in-loading"]);
    assert!(rt.state.open_in.tools.is_none());
}

#[test]
fn says_no_tools_detected_when_the_machine_has_none() {
    let mut rt = app();
    menu_on(&mut rt);
    rt.update(|s, _| s.open_in.tools = Some(Vec::new()));
    open_in(&mut rt);
    assert_eq!(rows(&rt), vec!["open-in-empty"]);
}

#[test]
fn an_error_with_no_tools_known_reads_as_none_detected() {
    let mut rt = app();
    menu_on(&mut rt);
    rt.update(|s, _| {
        s.open_in.tools = None;
        s.open_in.tools_error = Some("down".into());
    });
    open_in(&mut rt);
    assert_eq!(rows(&rt), vec!["open-in-empty"]);
}

#[test]
fn lists_editors_then_terminals_then_file_managers_with_a_separator_between_groups() {
    let mut rt = app();
    menu_on(&mut rt);
    rt.update(|s, _| {
        let t = |id: &str, category| crate::fleet::open_in::Tool { id: id.into(), label: id.to_uppercase(), category };
        // Sent out of order: the menu's order is Fleet's.
        s.open_in.tools =
            Some(vec![t("fm", Category::Explorer), t("sh", Category::Terminal), t("ed", Category::Editor), t("ed2", Category::Editor)]);
        s.open_in.files.clear();
    });
    open_in(&mut rt);
    assert_eq!(rows(&rt), vec!["open-in-ed", "open-in-ed2", "open-in-sh", "open-in-fm"]);
    let seps =
        rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("session-context-open-in-sub-sep"))).count();
    assert_eq!(seps, 2, "one between each pair of groups, none before the first");
}

#[test]
fn a_group_with_no_tool_leaves_no_separator() {
    let tools = parse_tools(&json!([{"id": "a", "label": "A", "category": "terminal"}, {"id": "b", "label": "B", "category": "explorer"}]));
    let g = groups(&tools);
    assert_eq!(g.iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![Category::Terminal, Category::Explorer]);
}

#[test]
fn a_terminal_and_a_file_manager_open_the_folder_in_one_click() {
    let mut rt = app();
    menu_on(&mut rt);
    open_in(&mut rt);
    click(&mut rt, "open-in-term");
    assert_eq!(rt.state.open_in.opened, vec![Opened::Directory { directory: DIR.into(), tool: "term".into() }]);
    assert!(!has(&rt, "ctx-menu"), "the menu closes on the pick");
    menu_on(&mut rt);
    open_in(&mut rt);
    click(&mut rt, "open-in-files");
    assert_eq!(rt.state.open_in.opened.len(), 2);
    assert_eq!(rt.state.open_in.opened[1], Opened::Directory { directory: DIR.into(), tool: "files".into() });
}

#[test]
fn an_editor_without_key_files_opens_the_folder_in_one_click() {
    let mut rt = app();
    menu_on(&mut rt);
    open_in(&mut rt);
    // The invented machine gives key files to "code" only.
    click(&mut rt, "open-in-notes");
    assert_eq!(rt.state.open_in.opened, vec![Opened::Directory { directory: DIR.into(), tool: "notes".into() }]);
}

#[test]
fn an_editor_with_key_files_is_a_submenu_with_open_directory_a_separator_and_the_files() {
    let mut rt = app();
    menu_on(&mut rt);
    open_in(&mut rt);
    click(&mut rt, "open-in-code");
    let r = rows(&rt);
    let at = |k: &str| r.iter().position(|x| x == k).unwrap_or_else(|| panic!("{k} missing in {r:?}"));
    assert!(at("open-in-code-dir") < at("open-in-code-file-0") && at("open-in-code-file-0") < at("open-in-code-file-1"));
    assert!(has(&rt, "open-in-code-sub-sep1"), "a separator under Open directory");
    assert!(!has(&rt, "open-in-code-loading"), "no Loading… once the files are read");
    click(&mut rt, "open-in-code-dir");
    assert_eq!(rt.state.open_in.opened, vec![Opened::Directory { directory: DIR.into(), tool: "code".into() }]);
}

#[test]
fn a_key_file_opens_at_the_folder_joined_with_its_path() {
    let mut rt = app();
    menu_on(&mut rt);
    open_in(&mut rt);
    click(&mut rt, "open-in-code");
    click(&mut rt, "open-in-code-file-1");
    assert_eq!(rt.state.open_in.opened, vec![Opened::File { path: format!("{DIR}/docs/notes.md"), tool: "code".into() }]);
    assert!(!has(&rt, "ctx-menu"));
}

#[test]
fn shows_loading_in_the_editors_submenu_while_key_files_are_being_read() {
    let mut rt = app();
    menu_on(&mut rt);
    rt.update(|s, _| s.open_in.files_loading = true);
    open_in(&mut rt);
    click(&mut rt, "open-in-code");
    assert!(has(&rt, "open-in-code-loading"));
    assert!(has(&rt, "open-in-code-file-0"), "the files already known are listed beside it");
}

#[test]
fn escape_closes_the_editors_submenu_before_open_in_and_that_before_the_menu() {
    let mut rt = app();
    menu_on(&mut rt);
    open_in(&mut rt);
    click(&mut rt, "open-in-code");
    let esc = |rt: &mut Runtime<State>| {
        rt.key(rune::KeyEvent { key: rune::Key::Escape, mods: rune::Mods::default() });
        rt.frame(FRAME);
    };
    esc(&mut rt);
    assert!(has(&rt, "open-in-code") && !has(&rt, "open-in-code-dir"));
    esc(&mut rt);
    assert!(has(&rt, "ctx-menu") && !has(&rt, "open-in-code"));
}

// ── The data (4.45) and the paths ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn reads_the_tools_from_an_object_or_a_bare_array() {
    let one = json!({"id": "e", "label": "E", "iconName": "x", "category": "editor"});
    let a = parse_tools(&json!({ "tools": [one.clone()] }));
    let b = parse_tools(&json!([one]));
    assert_eq!(a, b);
    assert_eq!(a[0].label, "E");
    assert!(parse_tools(&json!({"tools": 3})).is_empty() && parse_tools(&Value::Null).is_empty());
    assert!(parse_tools(&json!([{"id": "z", "label": "Z", "category": "mystery"}])).is_empty(), "no group to put it in");
}

#[test]
fn reads_the_key_files_by_tool() {
    let f = parse_key_files(&json!({"filesByTool": {"a": ["x.md", "y/z.md"], "b": []}}));
    assert_eq!(f["a"], vec!["x.md", "y/z.md"]);
    assert!(f["b"].is_empty());
    assert!(parse_key_files(&json!({})).is_empty());
}

#[test]
fn joins_a_folder_and_a_file_and_collapses_double_slashes() {
    assert_eq!(absolute("/srv/p", "a/b.md"), "/srv/p/a/b.md");
    assert_eq!(absolute("/srv/p/", "a.md"), "/srv/p/a.md");
    assert_eq!(absolute("/srv/p", "/a.md"), "/srv/p/a.md");
}

// ── Against a fake Fleet ───────────────────────────────────────────────────────────────────────────────────────────────

type Heard = Arc<Mutex<Vec<(String, String, Value)>>>;

/// A Fleet that has two tools and key files, notes every call, and refuses an open when `refuse` is set.
fn fake(refuse: bool) -> (String, Heard) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let heard: Heard = Default::default();
    let log = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let log = log.clone();
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let (head, body) = (req.head, req.body);
                let mut first = head.lines().next().unwrap_or("").split(' ');
                let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                let posted: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                log.lock().unwrap().push((method.clone(), path.clone(), posted));
                let (status, reply) = match (method.as_str(), path.split('?').next().unwrap_or("")) {
                    ("GET", "/api/available-tools") => (
                        200,
                        json!({"tools": [
                            {"id": "ed", "label": "Ed", "iconName": "x", "category": "editor"},
                            {"id": "sh", "label": "Sh", "iconName": "x", "category": "terminal"}
                        ]})
                        .to_string(),
                    ),
                    ("GET", "/api/key-files") => (200, json!({"filesByTool": {"ed": ["README.md"]}}).to_string()),
                    ("POST", "/api/open-directory" | "/api/open-file") if refuse => (500, "{\"error\":\"no\"}".to_string()),
                    ("POST", "/api/open-directory" | "/api/open-file") => (200, "{}".to_string()),
                    _ => (404, "{}".to_string()),
                };
                let out = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(out.as_bytes());
            });
        }
    });
    (base, heard)
}

fn rig(base: String) -> (Runtime<State>, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));
    (rt, queue)
}

fn calls(heard: &Heard, method: &str, needle: &str) -> Vec<(String, String, Value)> {
    heard.lock().unwrap().iter().filter(|c| c.0 == method && c.1.contains(needle)).cloned().collect()
}

#[test]
fn asks_the_server_for_the_tools_and_the_folders_key_files_when_the_menu_opens() {
    let (base, heard) = fake(false);
    let (mut rt, queue) = rig(base);
    menu_on(&mut rt);
    test_wait::drive(
        &mut rt,
        &queue,
        "the tools and key files",
        |s| s.open_in.tools.is_some() && !s.open_in.files.is_empty(),
        test_wait::app_state,
    );
    let tools = calls(&heard, "GET", "/api/available-tools");
    assert!(!tools.is_empty());
    let files = calls(&heard, "GET", "/api/key-files");
    assert_eq!(files[0].1, "/api/key-files?directory=%2Ftmp%2Ffleet-native-scratch-n%2Fdemo");
    open_in(&mut rt);
    // Ed has key files, Sh has none: Ed is a submenu, Sh a direct row.
    click(&mut rt, "open-in-ed");
    assert!(has(&rt, "open-in-ed-file-0") && has(&rt, "open-in-sh"));
}

#[test]
fn a_terminal_posts_the_folder_and_the_tool_and_a_key_file_posts_its_absolute_path() {
    let (base, heard) = fake(false);
    let (mut rt, queue) = rig(base);
    menu_on(&mut rt);
    test_wait::drive(&mut rt, &queue, "the lists", |s| s.open_in.tools.is_some() && !s.open_in.files.is_empty(), test_wait::app_state);
    open_in(&mut rt);
    click(&mut rt, "open-in-sh");
    test_wait::until_true("the open-directory call", || calls(&heard, "POST", "/api/open-directory").len() == 1);
    assert_eq!(calls(&heard, "POST", "/api/open-directory")[0].2, json!({"directory": DIR, "tool": "sh"}));
    menu_on(&mut rt);
    test_wait::drive(&mut rt, &queue, "the lists again", |s| !s.open_in.files.is_empty(), test_wait::app_state);
    open_in(&mut rt);
    click(&mut rt, "open-in-ed");
    click(&mut rt, "open-in-ed-file-0");
    test_wait::until_true("the open-file call", || calls(&heard, "POST", "/api/open-file").len() == 1);
    assert_eq!(calls(&heard, "POST", "/api/open-file")[0].2, json!({"filePath": format!("{DIR}/README.md"), "tool": "ed"}));
}

#[test]
fn a_refused_open_shows_nothing() {
    let (base, heard) = fake(true);
    let (mut rt, queue) = rig(base);
    menu_on(&mut rt);
    test_wait::drive(&mut rt, &queue, "the tools", |s| s.open_in.tools.is_some(), test_wait::app_state);
    open_in(&mut rt);
    click(&mut rt, "open-in-sh");
    test_wait::until_true("the call", || calls(&heard, "POST", "/api/open-directory").len() == 1);
    test_wait::drive(&mut rt, &queue, "the answer to be handled", |s| s.ui.toast.is_none(), test_wait::app_state);
    for _ in 0..30 {
        while let Ok(job) = queue.try_recv() {
            rt.update(job);
        }
        rt.frame(FRAME);
    }
    assert!(rt.state.ui.toast.is_none(), "no toast for a refused open (4.47)");
}

#[test]
fn the_trigger_row_keeps_its_fill_while_its_submenu_is_open() {
    let mut rt = app();
    menu_on(&mut rt);
    let fill =
        |rt: &Runtime<State>| rt.elements().iter().find(|e| e.key.as_deref() == Some("session-context-open-in")).map(|e| e.fill.a).unwrap();
    assert!(fill(&rt) < 0.001, "closed and not hovered: no fill");
    open_in(&mut rt);
    // Move the pointer into the submenu: the trigger is no longer hovered, and stays lit.
    let (x, y) = rt.find("open-in-term").unwrap();
    rt.pointer_move(x, y);
    for _ in 0..6 {
        rt.frame(FRAME);
    }
    assert!(fill(&rt) > 0.01, "the submenu is open: the trigger holds its fill");
}
