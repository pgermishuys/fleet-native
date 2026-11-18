//! Find in files, the logic (component 12, piece O): `use-find-text.test.ts` (7) and `find-text.test.ts` (4) under their names, and the
//! request the palette and Ctrl/Cmd+Shift+F make (`stores/find-in-files.ts`). A fake Fleet answers `GET /find/text` from a queue and logs
//! what it was asked; the box is typed into through `State`, as piece P's Files tab will.
use crate::fleet::file_browser::SearchIn;
use crate::fleet::find_text::*;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::files_tab::Case;
use crate::tests::{FRAME, app, finish};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

/// One answer the fake Fleet gives.
type Reply = (u16, Value);

fn answer() -> Value {
    json!({
        "files": [{ "path": "src/a.ts", "total": 1, "matches": [{ "line": 3, "column": 5, "text": "needle", "ranges": [{ "start": 0, "end": 6 }] }] }],
        "matches": 1,
        "stopped": null,
        "engine": "git",
    })
}

/// The `find/text` request lines the fake Fleet was asked, in order.
type Asked = Arc<Mutex<Vec<String>>>;

fn scene(mut replies: VecDeque<Reply>) -> (Rt, Receiver<Job>, Asked) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let asked: Asked = Default::default();
    let log = asked.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = crate::test_http::read(&mut stream).raw();
            let line = request.lines().next().unwrap_or_default().to_string();
            let (status, body) = if line.contains("/find/text") {
                log.lock().unwrap().push(line);
                replies.pop_front().unwrap_or((200, answer()))
            } else {
                (200, json!({}))
            };
            let body = body.to_string();
            let _ = write!(stream, "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            finish(&mut stream);
        }
    });
    let mut rt = app();
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    rt.update(move |s, _| {
        s.show_files_tab(Case::Tree, true);
        s.link = Some(link);
        s.file_browser.search_in = SearchIn::Contents;
    });
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    (rt, queue, asked)
}

fn type_query(rt: &mut Rt, query: &'static str) {
    rt.update(move |s, _| s.file_browser.field = rune::TextEdit::new(query));
    rt.frame(FRAME);
}

/// Runs frames until `seconds` of the app's clock have passed, so a debounce can be told from a pause too short.
fn run_for(rt: &mut Rt, seconds: f64) {
    let until = rt.state.clock + seconds;
    while rt.state.clock < until {
        rt.frame(FRAME);
    }
}

fn settled(rt: &mut Rt, queue: &Receiver<Job>, what: &str, done: impl FnMut(&State) -> bool) {
    test_wait::drive(rt, queue, what, done, |s| {
        format!("loading {}, result {}, failure {:?}", s.find_text.loading, s.find_text.result.is_some(), s.find_text.failure)
    });
}

fn asked(log: &Asked) -> Vec<String> {
    log.lock().unwrap().clone()
}

// use-find-text.test.ts

#[test]
fn asks_nothing_for_an_empty_or_one_letter_query() {
    let (mut rt, _queue, log) = scene(VecDeque::new());
    type_query(&mut rt, "n");
    run_for(&mut rt, DEBOUNCE * 2.0);
    assert!(asked(&log).is_empty());
    assert!(rt.state.find_text.result.is_none());
    assert!(!FindText::is_active("n"));
    assert!(!FindText::is_active("  n "));
    assert!(FindText::is_active("ne"));
}

#[test]
fn waits_for_typing_to_pause_then_asks_once_with_the_switches() {
    let (mut rt, queue, log) = scene(VecDeque::from([(200, answer())]));
    rt.update(|s, _| {
        s.find_text.regex = true;
        s.find_text.match_case = true;
    });
    type_query(&mut rt, "ne");
    type_query(&mut rt, "need");
    run_for(&mut rt, DEBOUNCE - 0.1);
    assert!(asked(&log).is_empty(), "still inside the pause");
    assert!(rt.state.find_text.loading, "Searching… shows while the debounce runs");

    settled(&mut rt, &queue, "the answer", |s| s.find_text.result.is_some());
    let lines = asked(&log);
    // Shown by `--nocapture`: the request log of typing "ne", "need" with a pause.
    println!("{}", lines.join("\n"));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("GET /api/sessions/") && lines[0].contains("/find/text?q=need&regex=true&matchCase=true"), "{lines:?}");
    assert_eq!(rt.state.find_text.result, parse_result(&answer()));
    assert!(rt.state.find_text.failure.is_none() && !rt.state.find_text.loading);
}

#[test]
fn a_pause_in_typing_asks_for_what_was_typed_so_far() {
    let (mut rt, queue, log) = scene(VecDeque::new());
    type_query(&mut rt, "pag");
    settled(&mut rt, &queue, "the answer for pag", |s| s.find_text.result.is_some() && !s.find_text.loading);
    type_query(&mut rt, "page");
    settled(&mut rt, &queue, "the second request", |_| asked(&log).len() == 2);
    settled(&mut rt, &queue, "the answer for page", |s| !s.find_text.loading);
    // Shown by `--nocapture`: the request log of typing "pag", a pause, "page".
    let lines = asked(&log);
    println!("{}", lines.join("\n"));
    assert!(lines[0].contains("q=pag&") && lines[1].contains("q=page&"), "{lines:?}");
    assert!(rt.state.find_text.took_ms >= 0.0);
}

#[test]
fn reports_a_bad_pattern_with_the_servers_reason_and_no_result() {
    let (mut rt, queue, _log) = scene(VecDeque::from([(400, json!({ "error": "Unmatched [ (git extended regex)" }))]));
    type_query(&mut rt, "a[");
    settled(&mut rt, &queue, "the failure", |s| s.find_text.failure.is_some());
    assert_eq!(
        rt.state.find_text.failure,
        Some(FindTextFailure { kind: FailureKind::Pattern, message: "Unmatched [ (git extended regex)".into() })
    );
    assert!(rt.state.find_text.result.is_none());
}

#[test]
fn says_an_older_machine_cant_search_instead_of_showing_no_matches() {
    let (mut rt, queue, _log) = scene(VecDeque::from([(404, json!("Not found"))]));
    type_query(&mut rt, "needle");
    settled(&mut rt, &queue, "the failure", |s| s.find_text.failure.is_some());
    let failure = rt.state.find_text.failure.clone().unwrap();
    assert_eq!(failure.kind, FailureKind::Unsupported);
    assert_eq!(failure.message, "This machine can't search inside files yet.");
    assert!(rt.state.find_text.result.is_none());
}

#[test]
fn reports_any_other_failure_and_a_thrown_request() {
    let (mut rt, queue, _log) = scene(VecDeque::from([(500, json!({ "error": "boom" })), (503, Value::Null)]));
    type_query(&mut rt, "needle");
    settled(&mut rt, &queue, "the first failure", |s| s.find_text.failure.is_some());
    assert_eq!(rt.state.find_text.failure, Some(FindTextFailure { kind: FailureKind::Failed, message: "boom".into() }));

    type_query(&mut rt, "needles");
    settled(&mut rt, &queue, "the second failure", |s| s.find_text.failure.as_ref().is_some_and(|f| f.message != "boom"));
    assert_eq!(rt.state.find_text.failure.as_ref().unwrap().message, "The search failed (HTTP 503).", "no reason: the status says");

    // A request that never got an answer carries its own words.
    let thrown = classify(Err("offline".into()));
    assert_eq!(thrown, Err(FindTextFailure { kind: FailureKind::Failed, message: "offline".into() }));
}

#[test]
fn clears_the_answer_when_the_query_gets_short_again() {
    let (mut rt, queue, _log) = scene(VecDeque::new());
    type_query(&mut rt, "needle");
    settled(&mut rt, &queue, "the answer", |s| s.find_text.result.is_some());

    type_query(&mut rt, "");
    assert!(rt.state.find_text.result.is_none());
    assert!(!rt.state.find_text.loading);
}

/// The query changes at `now`; the clock then runs past the debounce.
fn sent(f: &mut FindText, session: &str, query: &str, now: f64) -> Step {
    assert!(matches!(f.sync(session, query, now), Step::Wait(_)), "the debounce runs first");
    f.sync(session, query, now + DEBOUNCE)
}

#[test]
fn aborts_a_request_a_newer_query_replaces() {
    let mut f = FindText::default();
    let Step::Send { run: first, .. } = sent(&mut f, "s1", "needle", 1.0) else { panic!("the first search is sent") };
    assert_eq!(f.sync("s1", "needle", 5.0), Step::Idle, "nothing changed, nothing to do");
    // A newer query replaces it before the first answer is back.
    assert!(matches!(f.sync("s1", "needles", 2.0), Step::Wait(_)));
    f.answered(first, classify(Ok((200, answer()))), 5.0);
    assert!(f.result.is_none() && f.loading, "the older run's answer is dropped, the newer one still searches");
    let Step::Send { run: second, query, .. } = f.sync("s1", "needles", 2.0 + DEBOUNCE) else { panic!("the second search is sent") };
    assert_eq!(query, "needles");
    f.answered(second, classify(Ok((200, answer()))), 5.0);
    assert!(f.result.is_some() && !f.loading);
}

#[test]
fn another_session_drops_the_old_answer_and_a_switch_asks_again() {
    let mut f = FindText::default();
    let Step::Send { run, .. } = sent(&mut f, "s1", "needle", 1.0) else { panic!("sent") };
    f.answered(run, classify(Ok((200, answer()))), 1.0);
    assert!(f.result.is_some());
    f.regex = true;
    assert!(matches!(f.sync("s1", "needle", 3.0), Step::Wait(_)));
    assert!(f.result.is_some(), "the last answer stays in view while the next one comes");
    f.sync("s2", "needle", 3.0);
    assert!(f.result.is_none(), "an answer for another session is not shown");
}

// find-text.test.ts

fn file(path: &str, lines: &[u32]) -> TextFile {
    TextFile {
        path: path.into(),
        total: lines.len() as u32,
        matches: lines.iter().map(|&line| TextMatch { line, column: 1, text: "x".into(), ranges: vec![] }).collect(),
    }
}

fn plain(text: &str) -> TextSegment {
    TextSegment { text: text.into(), marked: false }
}

fn marked(text: &str) -> TextSegment {
    TextSegment { text: text.into(), marked: true }
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange { start, end }
}

#[test]
fn splits_a_line_into_plain_and_marked_pieces() {
    assert_eq!(
        segments_of("const MaxAttempts = MaxAttempts;", &[range(6, 17), range(20, 31)]),
        [plain("const "), marked("MaxAttempts"), plain(" = "), marked("MaxAttempts"), plain(";")]
    );
}

#[test]
fn returns_the_whole_line_when_nothing_is_marked_and_ignores_ranges_outside_it() {
    assert_eq!(segments_of("hello", &[]), [plain("hello")]);
    assert_eq!(segments_of("hello", &[range(9, 12)]), [plain("hello")]);
}

#[test]
fn keeps_overlapping_ranges_from_repeating_text() {
    let joined: String = segments_of("abcdef", &[range(0, 4), range(2, 6)]).iter().map(|p| p.text.as_str()).collect();
    assert_eq!(joined, "abcdef");
}

#[test]
fn counts_a_range_in_utf16_units_as_the_server_does() {
    // 😀 is two UTF-16 units, so "x" starts at 3.
    assert_eq!(segments_of("a😀x", &[range(3, 4)]), [plain("a😀"), marked("x")]);
}

#[test]
fn shows_the_first_matches_per_file_until_the_file_is_expanded() {
    let files = [file("a.ts", &[1, 2, 3, 4, 5, 6, 7]), file("b.ts", &[9])];
    assert_eq!(visible_hits(&files, &[]).len(), MATCHES_PER_FILE + 1);
    assert_eq!(visible_hits(&files, &["a.ts".into()]).len(), 8);
    assert_eq!(visible_hits(&files, &[]).last(), Some(&VisibleHit { path: "b.ts".into(), line: 9 }));
}

// stores/find-in-files.ts, and the palette command

#[test]
fn takes_the_find_request_once_for_the_session_that_was_asked() {
    let mut f = FindInFiles::default();
    f.ask("s1");
    assert!(!f.take("s2"), "a request for another session is left alone");
    assert!(f.take("s1"));
    assert!(!f.take("s1"));
}

#[test]
fn the_palette_command_opens_the_files_tab_and_the_tab_takes_the_request_into_contents() {
    let mut rt = app();
    rt.update(|s, _| s.show_files_tab(Case::Tree, true));
    rt.frame(FRAME);
    let command = crate::ui::commands::find(&rt.state, "find-in-files").expect("the command");
    assert!(matches!(command.run, crate::ui::commands::Run::Now(_)), "no longer waiting on a component");
    assert_eq!(command.description, "Search inside the files of this session's folder.");

    // The scene's session is invented, so ask for it the way the command does for an open one.
    rt.update(|s, _| {
        s.set_content_open(false);
        s.file_browser.search_in = SearchIn::Names;
        let id = s.files_session();
        s.find_in_files.ask(&id);
        s.edit_canvases(|c, id| c.open(id, crate::fleet::canvases::CanvasKind::Context));
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Names, "another tab is on screen: the request waits");
    rt.update(|s, _| {
        s.edit_canvases(|c, id| c.open(id, crate::fleet::canvases::CanvasKind::Files));
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Contents);
    assert!(rt.state.find_text.focus_field);
}

#[test]
fn ctrl_or_cmd_shift_f_does_what_the_palette_command_does() {
    use rune::{Key, KeyEvent, Mods};
    let mut rt = app();
    rt.update(|s, _| {
        let at = s.open_at();
        let at = if at.0 == usize::MAX { (0, 0) } else { at };
        s.set_session_id(at, "s-find-1");
        s.set_open(at);
    });
    rt.update(|s, _| {
        s.edit_canvases(|c, id| c.open(id, crate::fleet::canvases::CanvasKind::Context));
        s.set_content_open(false);
    });
    rt.frame(FRAME);
    let mods = if cfg!(target_os = "macos") {
        Mods { cmd: true, shift: true, ..Mods::default() }
    } else {
        Mods { cmd: true, ctrl: true, shift: true, ..Mods::default() }
    };
    rt.key(KeyEvent { key: Key::Command('f'), mods });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.content_open, "the right panel shows");
    assert_eq!(rt.state.open_canvases().active_id, "files");
    assert_eq!(rt.state.file_browser.search_in, SearchIn::Contents);
    assert!(rt.state.find_text.focus_field);
}
