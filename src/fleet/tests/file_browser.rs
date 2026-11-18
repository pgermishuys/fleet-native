//! The Files tab's tree (component 11, piece D): `use-file-browser.test.ts` (2) and the Tree block of `FileBrowserPanel.test.ts`
//! (3) under Fleet's names where the model can say them (the rest are in `src/tests_files_tab.rs`, which clicks), and the rules
//! `use-file-browser.ts` has no test for: the session switch, the cached folders, the refresh, the reload on `files.changed`.
//! The rules are checked on `FileBrowser`; what a request does is checked over a fake server that answers `/files/browse` from
//! a folder map and records every path it was asked for.
use super::super::canvases::{FileView, OpenFile};
use super::super::events::{Event, Kind};
use super::super::file_browser::{DOUBLE_CLICK, Entry, FileBrowser, Row, SearchIn, browse_result, parse_entries};
use super::super::finder::Step;
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

fn root() -> Vec<Entry> {
    vec![Entry::dir("src", "src"), Entry::dir("docs", "docs"), Entry::file("README.md", "README.md")]
}

fn src() -> Vec<Entry> {
    vec![Entry::file("app.ts", "src/app.ts")]
}

fn names(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| match r {
            Row::Dir { entry, depth, .. } | Row::File { entry, depth, .. } => format!("{}{}", " ".repeat(*depth), entry.name),
            Row::Loading { depth } => format!("{}…", " ".repeat(*depth)),
            Row::Empty { depth } => format!("{}(none)", " ".repeat(*depth)),
        })
        .collect()
}

fn loaded() -> FileBrowser {
    let mut b = FileBrowser::default();
    assert!(b.follow("s1"));
    assert!(b.start_root());
    b.root_done("s1", Ok(root()));
    b
}

// ─── the rules ──────────────────────────────────────────────────────────────

#[test]
fn shows_every_root_entry_even_when_some_files_changed() {
    // The tree reads the server's list; a changed file adds its marks to the row and removes nothing.
    let b = loaded();
    assert_eq!(names(&b.rows()), ["src", "docs", "README.md"]);
}

#[test]
fn the_entries_are_the_servers_in_its_order() {
    let body = r#"{"entries":[{"name":"b","relativePath":"b","isDirectory":true},{"name":"a.txt","relativePath":"a.txt","isDirectory":false}],"currentPath":""}"#;
    assert_eq!(parse_entries(body), [Entry::dir("b", "b"), Entry::file("a.txt", "a.txt")]);
    assert_eq!(parse_entries("{}"), [], "no entries is an empty folder");
    assert_eq!(parse_entries("not json"), []);
}

#[test]
fn a_failed_read_says_the_status_and_its_words() {
    assert_eq!(browse_result(Ok((404, String::new()))), Err("Failed to browse directory: 404 Not Found".into()));
    assert_eq!(browse_result(Ok((500, String::new()))), Err("Failed to browse directory: 500 Internal Server Error".into()));
    assert_eq!(browse_result(Err("connection refused".into())), Err("connection refused".into()));
    assert_eq!(browse_result(Ok((200, "{}".into()))), Ok(vec![]));
}

#[test]
fn a_missing_session_is_an_error() {
    let mut b = FileBrowser::default();
    assert!(!b.start_root());
    assert_eq!(b.error(), Some("No session ID provided"));
    let mut b = FileBrowser::default();
    assert!(!b.expand("src"));
    assert_eq!(b.error(), Some("No session ID provided"));
}

#[test]
fn a_folder_opens_under_its_row_and_a_closed_one_is_read_again() {
    let mut b = loaded();
    assert!(b.expand("src"));
    assert!(b.is_loading("src"));
    b.dir_done("s1", "src", Ok(src()));
    assert_eq!(names(&b.rows()), ["src", " app.ts", "docs", "README.md"]);
    assert!(!b.expand("src"), "an open folder is cached");
    b.collapse("src");
    assert!(b.expand("src"), "closing forgot its entries, so opening reads again");
}

#[test]
fn an_empty_folder_says_so_and_a_loading_one_says_loading() {
    let mut b = loaded();
    b.expand("docs");
    b.dir_done("s1", "docs", Ok(vec![]));
    assert_eq!(names(&b.rows()), ["src", "docs", " (none)", "README.md"]);
    // Open and being read at once (a refresh does it): the folder holds a "Loading…" line.
    b.expand("src");
    b.dir_done("s1", "src", Ok(src()));
    b.loading_for_test("src");
    assert_eq!(names(&b.rows())[..3], ["src", " …", "docs"]);
}

#[test]
fn a_failed_folder_sets_the_one_error_and_keeps_the_tree() {
    let mut b = loaded();
    b.expand("src");
    b.dir_done("s1", "src", Err("Failed to browse directory: 500 Internal Server Error".into()));
    assert_eq!(b.error(), Some("Failed to browse directory: 500 Internal Server Error"));
    assert!(!b.is_loading("src") && !b.is_open("src"));
    assert_eq!(b.root().len(), 3);
}

#[test]
fn another_session_clears_everything_and_drops_late_answers() {
    let mut b = loaded();
    b.expand("src");
    b.dir_done("s1", "src", Ok(src()));
    b.selected = Some("README.md".into());
    b.click("README.md", 1.0);
    assert!(!b.follow("s1"), "the same session starts nothing");
    assert!(b.follow("s2"));
    assert!(b.root().is_empty() && b.open_paths().is_empty() && b.error().is_none() && b.selected.is_none());
    assert_eq!(b.due_click(5.0), None, "the wait for a click goes with the session");
    b.start_root();
    b.root_done("s1", Ok(root()));
    assert!(b.root().is_empty(), "an answer for the session that was left is dropped");
    assert!(b.root_loading(), "and the new session is still loading");
}

#[test]
fn a_refresh_clears_then_opens_the_folders_again_in_the_order_they_were_opened() {
    let mut b = loaded();
    for dir in ["docs", "src"] {
        b.expand(dir);
        b.dir_done("s1", dir, Ok(vec![]));
    }
    assert!(b.refresh());
    assert!(b.root().is_empty() && b.open_paths().is_empty() && b.root_loading());
    assert_eq!(b.next_refill().as_deref(), Some("docs"));
    assert_eq!(b.next_refill().as_deref(), Some("src"));
    assert_eq!(b.next_refill(), None);
}

#[test]
fn a_click_waits_one_double_click_interval_and_a_click_elsewhere_replaces_it() {
    let mut b = loaded();
    assert_eq!(DOUBLE_CLICK, 0.25);
    b.click("README.md", 10.0);
    assert_eq!(b.selected.as_deref(), Some("README.md"), "the file is selected at once");
    assert_eq!(b.due_click(10.2), None);
    b.click("src/app.ts", 10.1);
    assert_eq!(b.due_click(10.3), None, "the second click restarted the wait");
    assert_eq!(b.due_click(10.35).as_deref(), Some("src/app.ts"), "only the last file opens");
    assert_eq!(b.due_click(11.0), None, "once");
    b.click("README.md", 12.0);
    b.double_click("README.md");
    assert_eq!(b.due_click(13.0), None, "a double-click drops the wait");
}

#[test]
fn a_burst_of_file_events_reloads_the_root_once() {
    let mut b = loaded();
    let first = b.queue_reload();
    let second = b.queue_reload();
    assert!(!b.reload_due("s1", first));
    assert!(b.reload_due("s1", second));
    assert!(!b.reload_due("s2", second), "and not for another session");
    b.follow("s2");
    assert!(!b.reload_due("s2", second), "a session switch drops the wait");
}

// ─── use-file-browser.test.ts ───────────────────────────────────────────────

fn tabs_of(s: &State, session: &str) -> Vec<(String, bool, FileView)> {
    s.canvases.session(session).canvases.iter().filter_map(|c| c.file.as_ref().map(|f| (f.path.clone(), f.preview, f.view))).collect()
}

#[test]
fn select_file_opens_the_file_as_a_preview_tab() {
    let mut rt = app();
    rt.update(|s, tx| s.open_file("session-1", "src/main.ts", OpenFile::default(), tx));
    assert_eq!(tabs_of(&rt.state, "session-1"), [("src/main.ts".to_string(), true, FileView::Edit)]);
}

#[test]
fn select_file_with_keep_opens_a_kept_tab() {
    let mut rt = app();
    rt.update(|s, tx| s.open_file("session-1", "README.md", OpenFile { keep: true, view: None }, tx));
    assert_eq!(tabs_of(&rt.state, "session-1"), [("README.md".to_string(), false, FileView::Rendered)]);
}

#[test]
fn a_second_preview_takes_the_place_of_the_first() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.open_file("session-1", "a.ts", OpenFile::default(), tx);
        s.open_file("session-1", "b.ts", OpenFile::default(), tx);
    });
    assert_eq!(tabs_of(&rt.state, "session-1"), [("b.ts".to_string(), true, FileView::Edit)]);
}

// ─── over a fake server ─────────────────────────────────────────────────────

#[derive(Default)]
struct Seen {
    /// Every `/files/browse` request: the folder asked for, "" for the root.
    browsed: Vec<String>,
    folders: HashMap<String, Vec<Entry>>,
    /// The status the server answers browse with.
    status: u16,
    /// Every `/find/files` request: the `q` asked for.
    searched: Vec<String>,
    /// What the file search answers, and with which status.
    found: Vec<String>,
    search_status: u16,
}

fn entries_json(list: &[Entry]) -> String {
    let list: Vec<Value> = list.iter().map(|e| json!({"name": e.name, "relativePath": e.path, "isDirectory": e.is_directory})).collect();
    json!({"entries": list, "currentPath": ""}).to_string()
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if request.is_empty() {
            request = line.clone();
        }
        if line == "\r\n" {
            break;
        }
    }
    let path = request.split(' ').nth(1).unwrap_or("").to_string();
    let mut status = "200 OK";
    let body = if path.starts_with("/hubs/session-events/negotiate") {
        "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string()
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        fixture("api-sessions.json")
    } else if path.ends_with("/diffs") {
        json!({"diffs": [{"file": "README.md", "status": "added", "additions": 5, "deletions": 0}], "available": true,
            "base": {"kind": "branch", "branch": "main", "commit": "aaa1111"}})
        .to_string()
    } else if let Some((_, q)) = path.split_once("/find/files?q=") {
        let mut seen = seen.lock().unwrap();
        seen.searched.push(q.to_string());
        if seen.search_status != 0 && seen.search_status != 200 {
            status = "500 Internal Server Error";
            json!({"error": "search is down"}).to_string()
        } else {
            json!({"files": seen.found}).to_string()
        }
    } else if let Some((_, rest)) = path.split_once("/files/browse") {
        let asked = rest.strip_prefix("?path=").unwrap_or("").replace("%2F", "/");
        let mut seen = seen.lock().unwrap();
        seen.browsed.push(asked.clone());
        if seen.status != 0 && seen.status != 200 {
            status = "500 Internal Server Error";
            "{}".to_string()
        } else {
            entries_json(seen.folders.get(&asked).map(Vec::as_slice).unwrap_or_default())
        }
    } else {
        "{}".to_string()
    };
    let _ = stream.write_all(
        format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
            .as_bytes(),
    );
}

struct Fake {
    rt: rune::Runtime<State>,
    id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

impl Fake {
    /// A Fleet whose session has `src/` and `docs/` folders and a README, with the Files tab in front and its root read.
    fn start() -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut folders = HashMap::new();
        folders.insert(String::new(), root());
        folders.insert("src".to_string(), src());
        folders.insert("docs".to_string(), vec![Entry::file("guide.md", "docs/guide.md")]);
        let seen = Arc::new(Mutex::new(Seen { folders, ..Default::default() }));
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
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
        });
        let id = rt.state.open_id().unwrap();
        let mut fake = Fake { rt, id, jobs, seen };
        fake.rt.update(|s, tx| {
            s.content_open = true;
            s.open_built_in(crate::fleet::canvases::CanvasKind::Files, tx);
        });
        fake.rt.frame(crate::tests::FRAME);
        fake.wait("the root to be read", |s, _| !s.file_browser.root().is_empty());
        fake
    }

    fn wait(&mut self, what: &str, done: impl Fn(&State, &[String]) -> bool) {
        let (seen, described) = (self.seen.clone(), self.seen.clone());
        crate::test_wait::drive(
            &mut self.rt,
            &self.jobs,
            what,
            |s| done(s, &seen.lock().unwrap().browsed),
            |s| format!("{}; browsed {:?}", crate::test_wait::app_state(s), described.lock().unwrap().browsed),
        );
    }

    /// Like `wait`, for what the file search has seen (`wait` holds the lock on the browse list while it looks).
    fn wait_search(&mut self, what: &str, done: impl Fn(&State, &[String]) -> bool) {
        let (seen, described) = (self.seen.clone(), self.seen.clone());
        crate::test_wait::drive(
            &mut self.rt,
            &self.jobs,
            what,
            |s| done(s, &seen.lock().unwrap().searched),
            |s| format!("{}; searched {:?}", crate::test_wait::app_state(s), described.lock().unwrap().searched),
        );
    }

    fn browsed(&self) -> Vec<String> {
        self.seen.lock().unwrap().browsed.clone()
    }

    fn files_changed(&mut self) {
        let id = self.id.clone();
        let event =
            Event::Other(Kind::FilesChanged, json!({"sessionId": id, "files": [{"path": "/repo/src/new.ts", "changeType": "add"}]}));
        self.rt.update(move |s, _| s.dispatch(&Topic::Session(id), &[event]));
    }
}

#[test]
fn the_root_loads_when_the_tab_shows_for_a_session() {
    let f = Fake::start();
    assert_eq!(f.browsed(), [""], "the root, once");
    assert_eq!(f.rt.state.file_browser.session(), Some(f.id.as_str()));
    assert_eq!(names(&f.rt.state.file_browser.rows()), ["src", "docs", "README.md"]);
}

#[test]
fn opening_a_folder_reads_it_and_an_open_one_isnt_read_again() {
    let mut f = Fake::start();
    f.rt.update(|s, _| s.toggle_files_dir("src"));
    assert!(f.rt.state.file_browser.is_loading("src"));
    f.wait("src to be read", |s, _| s.file_browser.is_open("src"));
    assert_eq!(f.browsed(), ["", "src"]);
    assert_eq!(names(&f.rt.state.file_browser.rows()), ["src", " app.ts", "docs", "README.md"]);
    // Closing forgets it; opening again reads it again.
    f.rt.update(|s, _| s.toggle_files_dir("src"));
    assert!(!f.rt.state.file_browser.is_open("src"));
    f.rt.update(|s, _| s.toggle_files_dir("src"));
    f.wait("src to be read again", |_, b| b.iter().filter(|p| *p == "src").count() == 2);
}

#[test]
fn a_refresh_keeps_which_folders_were_open() {
    let mut f = Fake::start();
    for dir in ["docs", "src"] {
        f.rt.update(move |s, _| s.toggle_files_dir(dir));
        f.wait("a folder to open", move |s, _| s.file_browser.is_open(dir));
    }
    f.rt.update(|s, _| s.refresh_files());
    assert!(f.rt.state.file_browser.root().is_empty() && f.rt.state.file_browser.root_loading(), "everything is cleared first");
    f.wait("both folders to open again", |s, _| s.file_browser.is_open("docs") && s.file_browser.is_open("src"));
    assert_eq!(f.browsed(), ["", "docs", "src", "", "docs", "src"], "the root, then each folder in turn");
    assert_eq!(names(&f.rt.state.file_browser.rows()), ["src", " app.ts", "docs", " guide.md", "README.md"]);
}

#[test]
fn a_burst_of_file_changes_reads_only_the_root_once_after_it_settles() {
    let mut f = Fake::start();
    f.rt.update(|s, _| s.toggle_files_dir("src"));
    f.wait("src to open", |s, _| s.file_browser.is_open("src"));
    f.seen.lock().unwrap().folders.insert(String::new(), vec![Entry::file("new.ts", "new.ts")]);
    for _ in 0..3 {
        f.files_changed();
    }
    assert_eq!(f.browsed(), ["", "src"], "nothing is read until the burst has settled");
    f.wait("the root to be read again", |s, _| s.file_browser.root().first().is_some_and(|e| e.name == "new.ts"));
    assert_eq!(f.browsed(), ["", "src", ""], "once, and only the root");
    assert!(f.rt.state.file_browser.is_open("src"), "open folders keep what they had");
}

#[test]
fn a_server_that_refuses_the_root_shows_its_status() {
    let f = Fake::start();
    let mut f = f;
    f.seen.lock().unwrap().status = 500;
    f.rt.update(|s, _| s.refresh_files());
    f.wait("the error", |s, _| s.file_browser.error().is_some());
    assert_eq!(f.rt.state.file_browser.error(), Some("Failed to browse directory: 500 Internal Server Error"));
}

// Searching by name (piece E): the rules under the box.

#[test]
fn a_query_of_two_characters_trimmed_is_a_search_and_folders_are_left_out() {
    let mut b = FileBrowser::default();
    b.field = rune::TextEdit::new(" a ");
    assert!(!b.is_searching(), "one character once trimmed");
    b.field = rune::TextEdit::new(" ab ");
    assert_eq!(b.wanted(), Some(" ab "));
    b.search_in = SearchIn::Contents;
    assert_eq!(b.wanted(), None, "File contents is another search");
    b.search_in = SearchIn::Names;
    let scope = crate::fleet::finder::Scope::Session("s".into());
    let typed = b.wanted().map(str::to_string);
    let Step::Wait(_) = b.search.want(Some(&scope), typed.as_deref(), 0.0) else { panic!("waits the 80 ms") };
    let Step::Send { generation, query } = b.search.want(Some(&scope), typed.as_deref(), 0.1) else { panic!("then sends") };
    b.search.answered(generation, &query, Ok(vec!["src/".into(), "src/ab.rs".into()]));
    assert_eq!(b.found(), ["src/ab.rs"]);
}

fn type_query(f: &mut Fake, words: &str) {
    let words = words.to_string();
    f.rt.update(move |s, _| s.file_browser.field = rune::TextEdit::new(&words));
    f.rt.frame(crate::tests::FRAME);
}

#[test]
fn typing_asks_the_server_once_after_the_wait_and_lists_files_without_folders() {
    let mut f = Fake::start();
    f.seen.lock().unwrap().found = vec!["src/".into(), "src/app.ts".into(), "docs/app.md".into()];
    type_query(&mut f, "app");
    f.wait_search("the search to be answered", |s, asked| !s.file_browser.search.loading && !asked.is_empty());
    assert_eq!(f.seen.lock().unwrap().searched, ["app"], "one request for the whole word");
    assert_eq!(f.rt.state.file_browser.found(), ["src/app.ts", "docs/app.md"]);
}

#[test]
fn a_refused_search_shows_the_servers_words_and_one_character_asks_nothing() {
    let mut f = Fake::start();
    f.seen.lock().unwrap().search_status = 500;
    type_query(&mut f, "a");
    for _ in 0..12 {
        f.rt.frame(crate::tests::FRAME);
    }
    assert!(f.seen.lock().unwrap().searched.is_empty());
    type_query(&mut f, "ap");
    f.wait_search("the error", |s, _| s.file_browser.search.error.is_some());
    assert_eq!(f.rt.state.file_browser.search.error.as_deref(), Some("search is down"));
}
