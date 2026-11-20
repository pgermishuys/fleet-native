//! Completion in the code view (component 11, piece U; row 13.11 of docs/parity/11-files.md): the words of the file as you type, import
//! paths from the session's folder listing, and the popup's keys. Fleet's own tests are of its sources (`completion.test.ts`, ported in
//! `conv::completion`); these are ours, over the invented `src/paginate.py` and `src/ledger/index.ts`.
use crate::State;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::file_tab::{Case, PATH};
use crate::tests::FRAME;
use crate::view::code_view::CODE;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};
use serde_json::json;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
}

fn typed(rt: &mut Runtime<State>, text: &str) {
    for c in text.chars() {
        press(rt, Key::Text(c.to_string()));
    }
}

fn new_app() -> Runtime<State> {
    let mut rt = crate::terminal_hooks(App::new(crate::spec())).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

fn open(case: Case) -> Runtime<State> {
    let mut rt = new_app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(case, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    rt
}

fn lines(rt: &Runtime<State>, path: &str) -> Vec<String> {
    let s = &rt.state;
    let session = s.canvas_session();
    let machine = s.machine_key_of(&session);
    s.file_buffers.buffer(machine, &session, path).map(|b| b.lines().to_vec()).unwrap_or_default()
}

fn offered(rt: &Runtime<State>) -> Vec<String> {
    rt.state.completion.items.iter().map(|r| r.item.label.clone()).collect()
}

/// A new line at the end of the file, the caret on it.
fn new_line_at_the_end(rt: &mut Runtime<State>) {
    rt.key(KeyEvent { key: Key::End, mods: Mods { cmd: true, ctrl: true, ..Mods::default() } });
    rt.frame(FRAME);
    press(rt, Key::Enter);
}

#[test]
fn typing_a_word_offers_the_words_of_the_file_and_tab_takes_the_chosen_one() {
    let mut rt = open(Case::Code);
    new_line_at_the_end(&mut rt);
    typed(&mut rt, "DEFA");
    assert!(rt.state.completion.open, "the popup opens while a word is typed");
    assert!(rt.find("completion").is_some(), "and is drawn");
    assert_eq!(offered(&rt), ["DEFAULT_SIZE"]);
    press(&mut rt, Key::Tab);
    assert_eq!(lines(&rt, PATH).last().map(String::as_str), Some("DEFAULT_SIZE"));
    assert!(!rt.state.completion.open, "taking one shuts it");
    press(&mut rt, Key::Text("x".into()));
    assert!(!rt.state.completion.open, "nothing else begins `DEFAULT_SIZEx`");
}

#[test]
fn the_arrows_choose_enter_takes_and_escape_shuts_without_typing() {
    let mut rt = open(Case::Code);
    new_line_at_the_end(&mut rt);
    typed(&mut rt, "pag");
    let all = offered(&rt);
    assert!(all.len() >= 3 && all.iter().all(|w| w.to_lowercase().contains('p')), "the words beginning `pag`: {all:?}");
    assert_eq!(rt.state.completion.selected, 0);
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.completion.selected, 1);
    press(&mut rt, Key::Up);
    press(&mut rt, Key::Up);
    assert_eq!(rt.state.completion.selected, all.len() - 1, "Up from the first goes round to the last");
    let before = lines(&rt, PATH);
    press(&mut rt, Key::Escape);
    assert!(!rt.state.completion.open);
    assert_eq!(lines(&rt, PATH), before, "Esc leaves the text alone");
    typed(&mut rt, "e");
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Enter);
    let last = lines(&rt, PATH).last().cloned().unwrap_or_default();
    assert_eq!(last, "page_1", "Enter took the second of the words, in alphabetical order");
    assert!(lines(&rt, PATH).len() == before.len(), "and made no new line");
}

#[test]
fn enter_with_the_popup_shut_is_a_new_line() {
    let mut rt = open(Case::Code);
    new_line_at_the_end(&mut rt);
    let n = lines(&rt, PATH).len();
    press(&mut rt, Key::Enter);
    assert_eq!(lines(&rt, PATH).len(), n + 1);
}

#[test]
fn an_import_path_in_a_typescript_file_completes_from_the_folder() {
    let mut rt = open(Case::Complete);
    // The scene has typed `./t` in the second import.
    assert!(rt.state.completion.open);
    let all = offered(&rt);
    assert_eq!(
        all,
        ["totals", "totals", "totals.py", "types"],
        "the files and the word that begin with `t`, TypeScript extensions left off: {all:?}"
    );
    assert!(!all.iter().any(|l| l == "index"), "the file itself is left out");
    assert!(rt.find("completion-chosen").is_some());
    press(&mut rt, Key::Enter);
    let text = lines(&rt, crate::mock::completion::TS_PATH);
    assert_eq!(text[1], "import { totals } from \"./totals\";", "only the segment after the last slash was replaced");
}

// ─── the folder's listing, over a fake server ───────────────────────────────

#[derive(Default)]
struct Seen {
    /// Every `find/files` request: its `q`.
    finds: Vec<String>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let route = heard.path.clone();
    let body = if route.starts_with("/hubs/session-events/negotiate") {
        json!({"negotiateVersion": 1, "connectionToken": "tok"}).to_string()
    } else if let Some((_, q)) = route.split_once("/find/files?q=") {
        seen.lock().unwrap().finds.push(q.to_string());
        json!({"files": ["src/app.py", "src/util.py", "src/lib/", "src/data.json"]}).to_string()
    } else if route.contains("/files/content?path=") {
        json!({"content": "alpha\nbeta\n", "hash": "h1", "isBinary": false, "isTruncated": false}).to_string()
    } else if route.starts_with("/api/sessions") {
        std::fs::read_to_string(format!("{}/tests/fixtures/api-sessions.json", env!("CARGO_MANIFEST_DIR"))).unwrap()
    } else {
        "{}".to_string()
    };
    crate::test_http::respond_json(&mut stream, 200, &body);
}

fn on_fake_fleet() -> (Runtime<State>, Receiver<Job>, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
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
        s.open_file(&id, "src/main.py", crate::fleet::canvases::OpenFile { keep: true, view: None }, tx);
    });
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the file to be read and drawn",
        |s| {
            let id = s.canvas_session();
            let m = s.machine_key_of(&id).to_string();
            s.file_buffers.info(&m, &id, "src/main.py").is_some_and(|i| i.status == crate::fleet::file_buffers::Status::Ready)
        },
        crate::test_wait::app_state,
    );
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    (rt, jobs, seen)
}

fn wait_for_popup(rt: &mut Runtime<State>, jobs: &Receiver<Job>, seen: &Arc<Mutex<Seen>>) {
    let described = seen.clone();
    crate::test_wait::drive(
        rt,
        jobs,
        "the folder's files to come back and the popup to open",
        |s| s.completion.open,
        move |s| format!("{}; asked {:?}", crate::test_wait::app_state(s), described.lock().unwrap().finds),
    );
    rt.frame(FRAME);
}

#[test]
fn an_import_path_asks_the_server_for_the_folder_once_and_keeps_the_answer() {
    let (mut rt, jobs, seen) = on_fake_fleet();
    new_line_at_the_end(&mut rt);
    typed(&mut rt, "from \"./");
    wait_for_popup(&mut rt, &jobs, &seen);
    assert_eq!(seen.lock().unwrap().finds, ["src%2F"], "the folder of src/main.py, asked once");
    // Folders first, the file itself (src/main.py isn't in the answer) and the rest by name; `.py` stays on a Python file's names.
    assert_eq!(offered(&rt), ["lib/", "app.py", "data.json", "util.py"]);
    // More typing narrows from the kept listing: no second request.
    typed(&mut rt, "u");
    assert_eq!(offered(&rt), ["util.py"]);
    assert_eq!(seen.lock().unwrap().finds.len(), 1, "the listing is kept for ten seconds");
    press(&mut rt, Key::Tab);
    let text = lines(&rt, "src/main.py");
    assert_eq!(text.last().map(String::as_str), Some("from \"./util.py\""), "{text:?}");
}
