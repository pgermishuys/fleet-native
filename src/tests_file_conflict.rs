//! A file that changed on disk while it was edited (component 11, piece S): the conflict's bar and notice, Use the agent's, Keep mine,
//! Compare and Done. The names are those of `FileCanvas.test.ts` "when the save loses the race (409)" where Fleet has the case.
use crate::State;
use crate::fleet::canvases::OpenFile;
use crate::fleet::file_buffers::Status;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::file_tab::Case;
use crate::tests::{FRAME, click};
use crate::view::code_view::CODE;
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};
use serde_json::json;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const PATH: &str = "src/app.py";

#[derive(Default)]
struct Seen {
    puts: Vec<serde_json::Value>,
}

/// The file reads as two lines under h1. The first save loses the race (409 with the agent's one line under h9); the next is saved.
fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let route = heard.path.clone();
    if heard.method == "PUT" && route.contains("/files/content") {
        let sent = heard.json();
        let mut seen = seen.lock().unwrap();
        let (status, body) = match (seen.puts.len(), sent["baseHash"].as_str()) {
            (0, _) => (409, json!({"saved": false, "content": "agent's\n", "hash": "h9"})),
            (_, Some("h9")) => (200, json!({"saved": true, "hash": "h10"})),
            _ => (409, json!({"content": "agent's again\n", "hash": "h11"})),
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

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
}

fn info_of(s: &State) -> Option<crate::fleet::file_buffers::Info> {
    let id = s.canvas_session();
    let m = s.machine_key_of(&id).to_string();
    s.file_buffers.info(&m, &id, PATH).cloned()
}

fn lines_of(rt: &Runtime<State>) -> Vec<String> {
    let s = &rt.state;
    let id = s.canvas_session();
    let m = s.machine_key_of(&id).to_string();
    s.file_buffers.buffer(&m, &id, PATH).map(|b| b.lines().to_vec()).unwrap_or_default()
}

fn all_text(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

fn toast(rt: &Runtime<State>) -> Option<String> {
    rt.state.file_lines.toast.as_ref().map(|(w, _)| w.clone())
}

/// A fake Fleet, `src/app.py` open and typed into ("x" at its start), the Save pressed and its answer (a 409) heard: the conflict.
fn conflicted() -> (Runtime<State>, Receiver<Job>, Arc<Mutex<Seen>>) {
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
        s.open_file(&id, PATH, OpenFile { keep: true, view: None }, tx);
    });
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the file to be read and drawn",
        |s| info_of(s).is_some_and(|i| i.status == Status::Ready),
        crate::test_wait::app_state,
    );
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.update(|_, tx| tx.focus(CODE, false));
    rt.frame(FRAME);
    press(&mut rt, Key::Text("x".into()));
    click(&mut rt, "file-save");
    let (looked, described) = (seen.clone(), seen.clone());
    crate::test_wait::drive(
        &mut rt,
        &jobs,
        "the save to be sent and answered with a conflict",
        move |s| !looked.lock().unwrap().puts.is_empty() && info_of(s).is_some_and(|i| !i.saving && i.conflict.is_some()),
        move |s| format!("{}; puts {:?}", crate::test_wait::app_state(s), described.lock().unwrap().puts),
    );
    rt.frame(FRAME);
    (rt, jobs, seen)
}

fn wait_for_puts(rt: &mut Runtime<State>, jobs: &Receiver<Job>, seen: &Arc<Mutex<Seen>>, n: usize) {
    let (looked, described) = (seen.clone(), seen.clone());
    crate::test_wait::drive(
        rt,
        jobs,
        "the second save to be sent and answered",
        move |s| looked.lock().unwrap().puts.len() >= n && info_of(s).is_some_and(|i| !i.saving),
        move |s| format!("{}; puts {:?}", crate::test_wait::app_state(s), described.lock().unwrap().puts),
    );
    rt.frame(FRAME);
}

// FV: "shows the bar and changes nothing"
#[test]
fn when_the_save_loses_the_race_it_shows_the_bar_and_changes_nothing() {
    let (rt, _jobs, seen) = conflicted();
    assert!(rt.find("file-conflict").is_some(), "the notice");
    assert!(all_text(&rt).contains("The agent changed app.py while you were editing. Nothing was overwritten."));
    assert!(all_text(&rt).contains("Changed on disk"), "the bar's state");
    assert!(rt.find("file-save").is_none(), "no Save while in conflict");
    for key in ["conflict-compare", "conflict-mine", "conflict-theirs"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
    assert_eq!(lines_of(&rt), ["xalpha", "beta", ""], "the text is what it was");
    assert_eq!(seen.lock().unwrap().puts.len(), 1, "nothing else was sent");
}

// FV: "Use the agent's replaces the buffer"
#[test]
fn use_the_agents_replaces_the_buffer() {
    let (mut rt, _jobs, _seen) = conflicted();
    click(&mut rt, "conflict-theirs");
    assert_eq!(lines_of(&rt), ["agent's", ""]);
    assert!(rt.find("file-conflict").is_none());
    assert!(!info_of(&rt.state).unwrap().dirty);
}

// FV: "Keep mine saves over the agent's version it showed"
#[test]
fn keep_mine_saves_over_the_agents_version_it_showed() {
    let (mut rt, jobs, seen) = conflicted();
    click(&mut rt, "conflict-mine");
    wait_for_puts(&mut rt, &jobs, &seen, 2);
    let puts = seen.lock().unwrap().puts.clone();
    assert_eq!(puts.len(), 2);
    assert_eq!(puts[1]["baseHash"], "h9", "over exactly the version that was shown");
    assert_eq!(puts[1]["content"], "xalpha\nbeta\n");
    assert!(rt.find("file-conflict").is_none());
    assert_eq!(toast(&rt).as_deref(), Some("Saved your version of app.py"));
}

// FV: "Compare shows the merge view; Done rebases on the agent's version and stays unsaved"
#[test]
fn compare_shows_the_merge_view_and_done_rebases_on_the_agents_version_and_stays_unsaved() {
    let (mut rt, _jobs, _seen) = conflicted();
    click(&mut rt, "conflict-compare");
    assert!(all_text(&rt).contains("Red is the agent's version, green is yours."));
    assert!(rt.find("file-comparing").is_some() && rt.find("file-conflict").is_none());
    assert!(rt.find("diff-text-0").is_some(), "the merge view draws the rows");
    assert!(rt.find("diff-take-0").is_some() && rt.find("diff-keep-0").is_some(), "each change has its two buttons");
    click(&mut rt, "compare-done");
    let id = rt.state.canvas_session();
    let m = rt.state.machine_key_of(&id).to_string();
    assert_eq!(rt.state.file_buffers.buffer(&m, &id, PATH).unwrap().base_hash(), Some("h9"));
    assert!(rt.find("file-comparing").is_none());
    assert!(rt.find("file-save").is_some(), "Unsaved, with Save");
    assert_eq!(toast(&rt).as_deref(), Some("Merged. Save to write it."));
    assert_eq!(lines_of(&rt), ["xalpha", "beta", ""], "your text stays");
}

#[test]
fn taking_the_agents_lines_hunk_by_hunk_then_done_says_you_took_its_version() {
    let (mut rt, _jobs, _seen) = conflicted();
    click(&mut rt, "conflict-compare");
    // The whole file differs from the agent's: one change.
    click(&mut rt, "diff-take-0");
    assert_eq!(lines_of(&rt), ["agent's", ""]);
    assert!(rt.find("diff-take-0").is_none(), "no change is left");
    click(&mut rt, "compare-done");
    assert_eq!(toast(&rt).as_deref(), Some("You took the agent's version"));
    assert!(rt.find("file-save").is_none(), "clean");
}

#[test]
fn keeping_mine_on_a_change_settles_it_and_done_leaves_the_file_unsaved() {
    let (mut rt, _jobs, _seen) = conflicted();
    click(&mut rt, "conflict-compare");
    click(&mut rt, "diff-keep-0");
    assert!(rt.find("diff-keep-0").is_none(), "the change is settled");
    assert_eq!(lines_of(&rt), ["xalpha", "beta", ""]);
    click(&mut rt, "compare-done");
    assert_eq!(toast(&rt).as_deref(), Some("Merged. Save to write it."));
}

#[test]
fn a_file_that_is_no_longer_text_offers_keep_mine_alone() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::ConflictGone, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(all_text(&rt).contains("paginate.py changed on disk and can't be shown here any more. Nothing was overwritten."));
    assert!(rt.find("conflict-mine").is_some());
    assert!(rt.find("conflict-compare").is_none() && rt.find("conflict-theirs").is_none());
}

#[test]
fn the_scene_of_a_conflict_and_of_compare_draw() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Compare, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("file-comparing").is_some());
    assert!(rt.find("diff-take-0").is_some());
}

// CS: "keeps a preview with unsaved changes and opens the next file beside it", with the buffer's own answer (rows 2.2 and 2.4)
#[test]
fn a_preview_with_unsaved_text_is_not_replaced_by_the_next_one() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Unsaved, true);
    });
    rt.frame(FRAME);
    rt.update(|s, tx| {
        let id = s.canvas_session();
        let machine = s.machine_key_of(&id).to_string();
        let buffers = &s.file_buffers;
        s.canvases.unpin_file(&id, crate::mock::file_tab::PATH, &|p| buffers.is_dirty(&machine, &id, p));
        s.open_file(&id, "README.md", OpenFile::default(), tx);
    });
    rt.frame(FRAME);
    let tabs = rt.state.open_canvases();
    let paths: Vec<_> = tabs.canvases.iter().filter_map(|c| c.file.as_ref().map(|f| f.path.clone())).collect();
    assert!(paths.contains(&crate::mock::file_tab::PATH.to_string()), "the edited file's tab stays: {paths:?}");
    assert!(paths.contains(&"README.md".to_string()), "and the next file opens beside it");
    let id = rt.state.canvas_session();
    let m = rt.state.machine_key_of(&id).to_string();
    assert!(rt.state.file_buffers.is_dirty(&m, &id, crate::mock::file_tab::PATH), "its unsaved text is still there");
}

#[test]
fn take_the_agents_on_one_change_leaves_the_other_changes_as_they_are() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| {
        s.sheet_w = 1400.0;
        s.show_file(Case::Compare, true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let before = lines_of_path(&rt, crate::mock::file_tab::PATH);
    assert!(rt.find("diff-take-0").is_some() && rt.find("diff-take-1").is_some(), "two changes");
    click(&mut rt, "diff-take-0");
    let after = lines_of_path(&rt, crate::mock::file_tab::PATH);
    assert_eq!(after[0], "\"\"\"Splits a list of rows into pages of a fixed size.\"\"\"", "the agent's first line");
    assert!(after.contains(&"# the page size is a count of rows".to_string()), "my added line stays");
    assert_eq!(after.len(), before.len());
}

fn lines_of_path(rt: &Runtime<State>, path: &str) -> Vec<String> {
    let s = &rt.state;
    let id = s.canvas_session();
    let m = s.machine_key_of(&id).to_string();
    s.file_buffers.buffer(&m, &id, path).map(|b| b.lines().to_vec()).unwrap_or_default()
}
