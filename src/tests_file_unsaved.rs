//! Closing a file tab, and quitting, with unsaved changes (component 11, piece T). The names are those of `CanvasHost.test.ts` and
//! `use-unsaved-files-guard.test.ts` where Fleet has the case.
use crate::State;
use crate::fleet::canvases::OpenFile;
use crate::fleet::file_buffers::Status;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::file_tab::{Case, PATH};
use crate::tests::{FRAME, app, click};
use rune::{App, Env, Runtime, Shell};
use serde_json::json;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const TAB: &str = "file:src/paginate.py";

fn close_key() -> String {
    crate::fleet::canvas_host::close_key(TAB)
}

fn open(case: Case) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.sheet_w = 1400.0;
        s.show_file(case, true)
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn label(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label)
}

fn tab_open(rt: &Runtime<State>) -> bool {
    rt.state.open_canvases().canvases.iter().any(|c| c.id == TAB)
}

fn all_text(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

fn buffer_kept(rt: &Runtime<State>) -> bool {
    let s = &rt.state;
    let id = s.canvas_session();
    s.file_buffers.info(s.machine_key_of(&id), &id, PATH).is_some()
}

// CanvasHost.test.ts
#[test]
fn shows_a_dot_instead_of_the_close_button_while_the_file_has_unsaved_changes() {
    let rt = open(Case::Unsaved);
    assert!(rt.find("canvas-tab-unsaved-dot").is_some(), "the dot where the close button is");
    assert_eq!(label(&rt, &close_key()).as_deref(), Some("Close paginate.py (unsaved changes)"));
    // The same file, clean: the plain button and its label.
    let rt = open(Case::Code);
    assert!(rt.find("canvas-tab-unsaved-dot").is_none());
    assert_eq!(label(&rt, &close_key()).as_deref(), Some("Close paginate.py"));
}

// CanvasHost.test.ts
#[test]
fn closes_a_clean_file_straight_away() {
    let mut rt = open(Case::Code);
    click(&mut rt, &close_key());
    assert!(rt.find("unsaved-dialog").is_none(), "no question");
    assert!(!tab_open(&rt) && !buffer_kept(&rt), "the tab and its buffer are gone");
}

// CanvasHost.test.ts
#[test]
fn asks_before_closing_a_file_with_unsaved_changes_and_can_close_without_saving() {
    let mut rt = open(Case::Unsaved);
    click(&mut rt, &close_key());
    assert!(tab_open(&rt), "the tab stays while it asks");
    assert!(rt.find("unsaved-dialog").is_some());
    let said = all_text(&rt);
    assert!(said.contains("Save changes before closing?"));
    assert!(said.contains("src/paginate.py"), "the path");
    assert!(said.contains("has changes that aren't saved. If you close it without saving, they're lost."));
    for words in ["Cancel", "Close without saving", "Save and close"] {
        assert!(said.contains(words), "{words}");
    }
    click(&mut rt, "unsaved-discard");
    assert!(rt.find("unsaved-dialog").is_none());
    assert!(!tab_open(&rt) && !buffer_kept(&rt), "closed, the changes with it");
}

#[test]
fn cancel_and_escape_keep_the_tab_and_its_text() {
    let mut rt = open(Case::Unsaved);
    click(&mut rt, &close_key());
    click(&mut rt, "unsaved-cancel");
    assert!(rt.find("unsaved-dialog").is_none() && tab_open(&rt) && buffer_kept(&rt));
    assert!(rt.find("canvas-tab-unsaved-dot").is_some(), "still unsaved");
    click(&mut rt, &close_key());
    rt.key(rune::KeyEvent { key: rune::Key::Escape, mods: Default::default() });
    rt.frame(FRAME);
    assert!(rt.find("unsaved-dialog").is_none() && tab_open(&rt));
}

#[test]
fn a_middle_click_and_delete_ask_too() {
    let mut rt = open(Case::Unsaved);
    rt.update(|s, tx| s.request_close_tab(TAB, tx));
    rt.frame(FRAME);
    assert!(rt.find("unsaved-dialog").is_some());
    // The question is a state, so a second ask doesn't stack a second dialog or lose the first.
    rt.update(|s, tx| s.request_close_tab(TAB, tx));
    assert!(rt.state.unsaved.closing.is_some() && tab_open(&rt));
}

#[derive(Default)]
struct Seen {
    puts: Vec<serde_json::Value>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>, conflict: bool) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let route = heard.path.clone();
    if heard.method == "PUT" && route.contains("/files/content") {
        seen.lock().unwrap().puts.push(heard.json());
        let (status, body) = if conflict {
            (409, json!({"saved": false, "content": "agent's\n", "hash": "h9"}))
        } else {
            (200, json!({"saved": true, "hash": "h2"}))
        };
        return crate::test_http::respond_json(&mut stream, status, &body.to_string());
    }
    let body = if route.starts_with("/hubs/session-events/negotiate") {
        json!({"negotiateVersion": 1, "connectionToken": "tok"}).to_string()
    } else if route.contains("/files/content?path=") {
        json!({"content": "alpha\nbeta\n", "hash": "h1", "isBinary": false, "isTruncated": false}).to_string()
    } else if route.starts_with("/api/sessions") {
        std::fs::read_to_string(format!("{}/tests/fixtures/api-sessions.json", env!("CARGO_MANIFEST_DIR"))).unwrap()
    } else {
        "{}".to_string()
    };
    crate::test_http::respond_json(&mut stream, 200, &body);
}

const SERVED: &str = "src/app.py";
const SERVED_TAB: &str = "file:src/app.py";

/// A fake Fleet (its save answers 200, or 409 when `conflict`), `src/app.py` open and typed into: unsaved, with the Save not yet pressed.
fn typed(conflict: bool) -> (Runtime<State>, Receiver<Job>, Arc<Mutex<Seen>>) {
    let listener = crate::test_http::bind().unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let shared = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = shared.clone();
            std::thread::spawn(move || serve(stream, seen, conflict));
        }
    });
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let post = Post::new(move |job| {
        let _ = tx.lock().unwrap().send(job);
    });
    let mut rt = crate::terminal_hooks(App::new(crate::spec())).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    let link = Link::start(base, post);
    rt.update(move |s, tx| {
        s.link = Some(link);
        let list = std::fs::read_to_string(format!("{}/tests/fixtures/api-sessions.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        s.on_list(crate::fleet::sessions::parse_list(&serde_json::from_str(&list).unwrap(), 0), tx);
        s.sheet_w = 1400.0;
        s.content_open = true;
        s.right.widened = true;
        let id = s.open_id().unwrap();
        s.open_file(&id, SERVED, OpenFile { keep: true, view: None }, tx);
    });
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the file to be read and drawn",
        |s| {
            let id = s.canvas_session();
            s.file_buffers.info(s.machine_key_of(&id), &id, SERVED).is_some_and(|i| i.status == Status::Ready)
        },
        crate::test_wait::app_state,
    );
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(crate::view::code_view::CODE, false));
    rt.frame(FRAME);
    rt.key(rune::KeyEvent { key: rune::Key::Text("x".into()), mods: Default::default() });
    rt.frame(FRAME);
    (rt, jobs, seen)
}

fn wait_for_the_answer(rt: &mut Runtime<State>, jobs: &Receiver<Job>, seen: &Arc<Mutex<Seen>>, what: &str) {
    let (looked, described) = (seen.clone(), seen.clone());
    crate::test_wait::drive(
        rt,
        jobs,
        what,
        move |s| !looked.lock().unwrap().puts.is_empty() && s.unsaved.closing.is_none(),
        move |s| format!("{}; puts {:?}; closing {:?}", crate::test_wait::app_state(s), described.lock().unwrap().puts, s.unsaved.closing),
    );
    rt.frame(FRAME);
}

fn served_tab_open(rt: &Runtime<State>) -> bool {
    rt.state.open_canvases().canvases.iter().any(|c| c.id == SERVED_TAB)
}

// CanvasHost.test.ts: "saves and closes"
#[test]
fn save_and_close_saves_with_the_hash_it_read_and_closes_the_tab() {
    let (mut rt, jobs, seen) = typed(false);
    click(&mut rt, &crate::fleet::canvas_host::close_key(SERVED_TAB));
    assert!(rt.find("unsaved-dialog").is_some());
    click(&mut rt, "unsaved-save");
    assert!(all_text(&rt).contains("Saving\u{2026}"), "the button says so while it saves");
    wait_for_the_answer(&mut rt, &jobs, &seen, "the save to be sent and answered");
    let puts = seen.lock().unwrap().puts.clone();
    assert_eq!(puts.len(), 1, "one request");
    assert_eq!((puts[0]["baseHash"].as_str(), puts[0]["content"].as_str()), (Some("h1"), Some("xalpha\nbeta\n")));
    assert!(!served_tab_open(&rt), "saved, so it closes");
    assert!(rt.find("unsaved-dialog").is_none());
}

#[test]
fn all_three_buttons_are_off_while_it_saves() {
    let (mut rt, _jobs, _seen) = typed(false);
    click(&mut rt, &crate::fleet::canvas_host::close_key(SERVED_TAB));
    click(&mut rt, "unsaved-save");
    // A press on any of them does nothing while the save is out.
    for key in ["unsaved-cancel", "unsaved-discard", "unsaved-save"] {
        click(&mut rt, key);
        assert!(rt.state.unsaved.closing.as_ref().is_some_and(|c| c.saving) && served_tab_open(&rt), "{key}");
    }
    // Cancel does nothing while the save is out.
    rt.update(|s, _| s.close_cancel());
    assert!(rt.state.unsaved.closing.is_some());
}

#[test]
fn a_save_that_loses_the_race_leaves_the_tab_open_and_shows_its_bar() {
    let (mut rt, jobs, seen) = typed(true);
    click(&mut rt, &crate::fleet::canvas_host::close_key(SERVED_TAB));
    click(&mut rt, "unsaved-save");
    wait_for_the_answer(&mut rt, &jobs, &seen, "the save to be sent and answered with a conflict");
    assert!(served_tab_open(&rt), "nothing was written, so the tab stays");
    assert!(rt.find("unsaved-dialog").is_none());
    assert!(rt.find("file-conflict").is_some(), "the tab is showing, with the conflict's notice");
    assert_eq!(rt.state.active_canvas(), SERVED_TAB);
}

// use-unsaved-files-guard.test.ts: "asks before quitting only while a file has unsaved changes"
#[test]
fn asks_before_quitting_only_while_a_file_has_unsaved_changes() {
    // Nothing unsaved: the quit goes on, with no question.
    let mut rt = open(Case::Code);
    let asked = {
        let mut asked = false;
        rt.update(|s, _| asked = s.ask_before_quitting());
        asked
    };
    assert!(!asked && rt.state.unsaved.quitting.is_none());

    // A file with unsaved changes: it asks, naming how many, and nothing quits yet.
    let mut rt = open(Case::Unsaved);
    let mut asked = false;
    rt.update(|s, _| asked = s.ask_before_quitting());
    rt.frame(FRAME);
    assert!(asked);
    assert!(rt.find("unsaved-quit-dialog").is_some());
    assert!(all_text(&rt).contains("1 file has changes that aren't saved. If you quit, they're lost."));
    assert!(rt.shell_request.take() != Some(Shell::Quit));

    // Cancel: the app stays, the text is kept.
    click(&mut rt, "unsaved-quit-cancel");
    assert!(rt.find("unsaved-quit-dialog").is_none() && buffer_kept(&rt));
    assert!(rt.shell_request.take() != Some(Shell::Quit));

    // Quit without saving: the app quits.
    rt.update(|s, _| {
        s.ask_before_quitting();
    });
    rt.frame(FRAME);
    click(&mut rt, "unsaved-quit-confirm");
    assert!(rt.shell_request.take() == Some(Shell::Quit));
}

#[test]
fn a_file_saved_or_closed_no_longer_holds_the_quit() {
    let mut rt = open(Case::Unsaved);
    rt.update(|s, tx| s.close_tab(TAB, tx));
    let mut asked = true;
    rt.update(|s, _| asked = s.ask_before_quitting());
    assert!(!asked, "its buffer went with the tab");
}

/// A window the test can ask whether it was shown.
#[derive(Default)]
struct Window {
    shown: std::sync::atomic::AtomicUsize,
}

impl crate::desktop::close::Surface for Window {
    fn show_window(&self) {
        self.shown.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn show_tray(&self, _: usize) {}
    fn hide_tray(&self) {}
}

// Component 24's quit paths end in `quit_app`: with unsaved files the window comes back and asks, with none the app quits.
#[test]
fn a_quit_the_app_starts_asks_in_the_window_while_a_file_is_unsaved_and_quits_otherwise() {
    let window = Arc::new(Window::default());
    let mut rt = open(Case::Code);
    let surface = window.clone();
    rt.update(move |s, _| s.close.surface = Some(surface));
    rt.update(|s, tx| s.quit_requested(tx));
    assert!(rt.shell_request.take() == Some(Shell::Quit), "nothing unsaved: it quits");
    assert_eq!(window.shown.load(std::sync::atomic::Ordering::SeqCst), 0);

    let mut rt = open(Case::Unsaved);
    let surface = window.clone();
    rt.update(move |s, _| s.close.surface = Some(surface));
    rt.update(|s, tx| s.quit_requested(tx));
    rt.frame(FRAME);
    assert!(rt.shell_request.take() != Some(Shell::Quit), "it asks first");
    assert_eq!(window.shown.load(std::sync::atomic::Ordering::SeqCst), 1, "the window is brought back for the question");
    assert!(rt.find("unsaved-quit-dialog").is_some());
    click(&mut rt, "unsaved-quit-confirm");
    assert!(rt.shell_request.take() == Some(Shell::Quit));
}

fn wait_for_quit_saves(rt: &mut Runtime<State>, jobs: &Receiver<Job>, seen: &Arc<Mutex<Seen>>) {
    let (looked, described) = (seen.clone(), seen.clone());
    crate::test_wait::drive(
        rt,
        jobs,
        "the save to be sent and answered",
        move |s| !looked.lock().unwrap().puts.is_empty() && s.unsaved.quit_saving.is_none(),
        move |s| {
            format!(
                "{}; puts {:?}; quit_saving {:?}",
                crate::test_wait::app_state(s),
                described.lock().unwrap().puts,
                s.unsaved.quit_saving
            )
        },
    );
    rt.frame(FRAME);
}

// Row 13.10's gap (ours: Fleet's browser asks, and has no such button).
#[test]
fn save_all_and_quit_saves_every_unsaved_file_and_then_quits() {
    let (mut rt, jobs, seen) = typed(false);
    rt.update(|s, _| {
        s.ask_before_quitting();
    });
    rt.frame(FRAME);
    click(&mut rt, "unsaved-quit-save");
    assert!(all_text(&rt).contains("Saving\u{2026}"), "the button says so while it saves");
    wait_for_quit_saves(&mut rt, &jobs, &seen);
    assert_eq!(seen.lock().unwrap().puts.len(), 1);
    assert!(rt.shell_request.take() == Some(Shell::Quit), "saved, so the app quits");
}

#[test]
fn save_all_and_quit_stays_when_a_file_could_not_be_saved() {
    let (mut rt, jobs, seen) = typed(true);
    rt.update(|s, _| {
        s.ask_before_quitting();
    });
    rt.frame(FRAME);
    click(&mut rt, "unsaved-quit-save");
    wait_for_quit_saves(&mut rt, &jobs, &seen);
    assert!(rt.shell_request.take() != Some(Shell::Quit), "a conflict keeps the app open, the file unsaved");
    assert!(rt.state.unsaved.quitting.is_none());
    assert!(!rt.state.unsaved_files().is_empty());
}
