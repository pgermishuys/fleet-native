//! Editing in the file tab (component 11, piece R): the bar's Unsaved and Save, Ctrl/Cmd+S in the editor, the toasts, and a preview
//! tab kept by typing. The names are those of `FileCanvas.test.ts` (FV) where Fleet has the case.
use crate::State;
use crate::fleet::canvases::OpenFile;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::file_tab::{Case, PATH};
use crate::tests::{FRAME, click};
use crate::view::code_view::CODE;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};
use serde_json::json;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

fn ctrl() -> Mods {
    if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { cmd: true, ctrl: true, ..Mods::default() } }
}

fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(FRAME);
}

fn typed(rt: &mut Runtime<State>, text: &str) {
    for c in text.chars() {
        press(rt, Key::Text(c.to_string()), Mods::default());
    }
}

fn new_app() -> Runtime<State> {
    let mut rt = crate::terminal_hooks(App::new(crate::spec())).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

/// The mock file tab on `src/paginate.py`, the code view holding the keyboard.
fn open() -> Runtime<State> {
    let mut rt = new_app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Code, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    rt
}

fn toast(rt: &Runtime<State>) -> Option<String> {
    rt.state.file_lines.toast.as_ref().map(|(w, _)| w.clone())
}

#[test]
fn shows_unsaved_after_typing_and_the_save_key_goes_when_the_text_goes_back() {
    let mut rt = open();
    assert!(rt.find("file-state").is_none(), "a clean file says nothing");
    typed(&mut rt, "x");
    assert!(rt.find("file-state").is_some(), "Unsaved");
    assert!(rt.find("file-save").is_some(), "Save");
    press(&mut rt, Key::Command('z'), ctrl());
    assert!(rt.find("file-state").is_none(), "typing back to the saved text is clean again");
}

// FileCanvas.vue `hasChanges` (rows 6.2 and 6.5): a file git sees as unchanged has changes once its text is unsaved, so Diff turns
// on and compares with the saved text; typing back to the saved text turns it off again.
#[test]
fn an_unsaved_file_has_changes_so_diff_turns_on_against_the_saved_text() {
    let mut rt = open();
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.diffs.reset(&id);
    });
    rt.frame(FRAME);
    let diff_on =
        |rt: &Runtime<State>| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("file-view-diff")).is_some_and(|e| e.focusable);
    assert!(!diff_on(&rt), "no changes: Diff is off");
    typed(&mut rt, "#");
    assert!(diff_on(&rt), "unsaved text: Diff is on");
    press(&mut rt, Key::Backspace, Mods::default());
    assert!(!diff_on(&rt), "typed back to the saved text: off again");
}

#[test]
fn typing_in_a_preview_tab_keeps_it() {
    let mut rt = open();
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.canvases.unpin_file(&id, PATH, &|_| false);
    });
    let is_preview = |rt: &Runtime<State>| rt.state.open_canvases().active().and_then(|c| c.file.as_ref()).is_some_and(|f| f.preview);
    assert!(is_preview(&rt), "the tab is a preview");
    typed(&mut rt, "x");
    assert!(!is_preview(&rt), "typing kept it");
}

#[test]
fn ctrl_s_on_a_clean_file_says_there_is_nothing_to_save() {
    let mut rt = open();
    press(&mut rt, Key::Command('s'), ctrl());
    assert_eq!(toast(&rt).as_deref(), Some("No changes to save"));
}

#[test]
fn a_file_being_saved_says_saving_and_has_no_save_button() {
    let mut rt = new_app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Saving, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("file-state").is_some(), "Saving…");
    assert!(rt.find("file-save").is_none());
}

// ─── a save over a fake server ──────────────────────────────────────────────

#[derive(Default)]
struct Seen {
    puts: Vec<serde_json::Value>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let route = heard.path.clone();
    if heard.method == "PUT" && route.contains("/files/content") {
        let sent = heard.json();
        let mut seen = seen.lock().unwrap();
        // The first save is based on h1, the next on the hash the first one answered with.
        let (status, body) = match (sent["baseHash"].as_str(), seen.puts.len()) {
            (Some("h1"), 0) => (200, json!({"saved": true, "hash": "h2"})),
            (Some("h2"), 1) => (200, json!({"saved": true, "hash": "h3"})),
            (Some(_), _) => (409, json!({"content": "other\n", "hash": "h9"})),
            _ => (400, json!({"error": "no hash"})),
        };
        seen.puts.push(sent);
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

/// The app on a fake Fleet whose `src/app.py` reads as two lines, the file open in the tab with the code holding the keyboard.
fn on_fake_fleet() -> (Runtime<State>, Receiver<Job>, Arc<Mutex<Seen>>) {
    let listener = crate::test_http::bind().unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let shared = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let seen = shared.clone();
            std::thread::spawn(move || serve(stream, seen));
        }
    });
    let (tx, jobs) = channel::<Job>();
    let tx = Mutex::new(tx);
    let post = Post::new(move |job| {
        let _ = tx.lock().unwrap().send(job);
    });
    let mut rt = new_app();
    let link = Link::start(base, post);
    rt.update(move |s, tx| {
        s.link = Some(link);
        let list = std::fs::read_to_string(format!("{}/tests/fixtures/api-sessions.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        s.on_list(crate::fleet::sessions::parse_list(&serde_json::from_str(&list).unwrap(), 0), tx);
        s.sheet_w = 1400.0;
        s.content_open = true;
        s.right.widened = true;
        let id = s.open_id().unwrap();
        s.open_file(&id, "src/app.py", OpenFile { keep: true, view: None }, tx);
    });
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the file to be read and drawn",
        |s| {
            let id = s.canvas_session();
            let m = s.machine_key_of(&id).to_string();
            s.file_buffers.info(&m, &id, "src/app.py").is_some_and(|i| i.status == crate::fleet::file_buffers::Status::Ready)
        },
        crate::test_wait::app_state,
    );
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    (rt, jobs, seen)
}

fn wait_for_puts(rt: &mut Runtime<State>, jobs: &Receiver<Job>, seen: &Arc<Mutex<Seen>>, n: usize) {
    let (looked, described) = (seen.clone(), seen.clone());
    crate::test_wait::drive(
        rt,
        jobs,
        "the save to be sent and answered",
        move |s| {
            let id = s.canvas_session();
            let m = s.machine_key_of(&id).to_string();
            looked.lock().unwrap().puts.len() >= n && s.file_buffers.info(&m, &id, "src/app.py").is_some_and(|i| !i.saving)
        },
        move |s| format!("{}; puts {:?}", crate::test_wait::app_state(s), described.lock().unwrap().puts),
    );
    rt.frame(FRAME);
}

#[test]
fn saves_with_the_hash_it_read_from_the_button_and_the_text_it_shows() {
    let (mut rt, jobs, seen) = on_fake_fleet();
    typed(&mut rt, "x");
    assert!(rt.find("file-save").is_some(), "Unsaved shows Save");
    click(&mut rt, "file-save");
    wait_for_puts(&mut rt, &jobs, &seen, 1);
    let puts = seen.lock().unwrap().puts.clone();
    assert_eq!(puts.len(), 1, "one save");
    assert_eq!(puts[0]["path"], "src/app.py");
    assert_eq!(puts[0]["content"], "xalpha\nbeta\n");
    assert_eq!(puts[0]["baseHash"], "h1");
    assert_eq!(toast(&rt).as_deref(), Some("Saved src/app.py"));
    assert!(rt.find("file-state").is_none(), "clean after the save");
}

#[test]
fn ctrl_s_saves_through_the_editors_keymap_and_the_next_save_is_based_on_the_new_hash() {
    let (mut rt, jobs, seen) = on_fake_fleet();
    typed(&mut rt, "x");
    press(&mut rt, Key::Command('s'), ctrl());
    wait_for_puts(&mut rt, &jobs, &seen, 1);
    assert_eq!(toast(&rt).as_deref(), Some("Saved src/app.py"));
    typed(&mut rt, "y");
    press(&mut rt, Key::Command('s'), ctrl());
    wait_for_puts(&mut rt, &jobs, &seen, 2);
    let puts = seen.lock().unwrap().puts.clone();
    assert_eq!(puts.len(), 2, "only the two saves were sent");
    assert_eq!(puts[1]["baseHash"], "h2");
    assert_eq!(puts[1]["content"], "xyalpha\nbeta\n");
}
