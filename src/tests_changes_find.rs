//! Find in Changes, the logic (component 12, piece J): `ChangesReview.test.ts` "find in changes", the cases that are about which files
//! are searched and what stepping to a match opens. A fake Fleet serves a few changed files and the machine's search, and logs
//! what it was asked; the bar is driven through `State`'s methods, as piece K's bar and keys will drive them.
use crate::fleet::diff_contents::Entry;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::changes::Case;
use crate::tests::{FRAME, app, finish};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

pub(crate) type Rt = Runtime<State>;

/// How the fake Fleet answers `GET /diffs/search`.
#[derive(Clone)]
pub(crate) enum Search {
    /// The files that hold the text.
    Found(Vec<&'static str>),
    /// An older Fleet: not found.
    Missing,
}

/// A changed file: its path, and its text before and after.
pub(crate) type Changed = (&'static str, String, String);

pub(crate) fn one(path: &'static str, before: &str, after: &str) -> Changed {
    (path, before.to_string(), after.to_string())
}

/// What the fake Fleet was asked, in order: `/diffs/file` paths and `/diffs/search` queries.
#[derive(Default)]
pub(crate) struct Asked {
    files: Vec<String>,
    searches: Vec<String>,
}

fn decode(text: &str) -> String {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok())) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn scene(changed: Vec<Changed>, search: Search) -> (Rt, Receiver<Job>, Arc<Mutex<Asked>>) {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let asked = Arc::new(Mutex::new(Asked::default()));
    let log = asked.clone();
    let texts: HashMap<String, (String, String)> = changed.iter().map(|(p, b, a)| (p.to_string(), (b.clone(), a.clone()))).collect();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = crate::test_http::read(&mut stream).raw();
            let (status, body) = if request.contains("/diffs/search") {
                let query = decode(request.split_once("q=").map_or("", |(_, r)| r.split([' ', '&']).next().unwrap_or("")));
                log.lock().unwrap().searches.push(query);
                match &search {
                    Search::Found(files) => {
                        (200, json!({ "files": files.iter().map(|f| json!({ "file": f, "count": 1 })).collect::<Vec<_>>() }).to_string())
                    }
                    Search::Missing => (404, "{}".to_string()),
                }
            } else if let Some((_, rest)) = request.split_once("path=").filter(|_| request.contains("/diffs/file")) {
                let file = decode(rest.split([' ', '&']).next().unwrap_or(""));
                log.lock().unwrap().files.push(file.clone());
                let (before, after) = texts.get(&file).cloned().unwrap_or_default();
                (
                    200,
                    json!({ "file": file, "status": "modified", "additions": 1, "deletions": 1, "before": before, "after": after })
                        .to_string(),
                )
            } else {
                (200, "{}".to_string())
            };
            let reason = if status == 200 { "OK" } else { "Not Found" };
            let _ = write!(stream, "HTTP/1.1 {status} {reason}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            finish(&mut stream);
        }
    });
    let mut rt = app();
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    rt.update(move |s, _| {
        s.show_changes_scene(Case::Loading, true);
        s.link = Some(link);
        let id = s.diffs.session().to_string();
        let list: Vec<_> =
            changed.iter().map(|(p, _, _)| json!({ "file": p, "status": "modified", "additions": 1, "deletions": 1 })).collect();
        s.diffs.apply(&json!({ "diffs": list, "available": true, "base": { "kind": "session" } }));
        s.diffs.has_loaded = true;
        s.review.open(&id);
        s.review_sync();
    });
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    (rt, queue, asked)
}

pub(crate) fn settled(rt: &mut Rt, queue: &Receiver<Job>, what: &str, done: impl FnMut(&State) -> bool) {
    test_wait::drive(rt, queue, what, done, test_wait::app_state);
}

/// Everything the open files asked for has come, so what follows counts only what find asks.
pub(crate) fn quiet(rt: &mut Rt, queue: &Receiver<Job>, asked: &Arc<Mutex<Asked>>) {
    settled(rt, queue, "the open files' contents", |s| {
        s.diffs.files().iter().all(|f| !matches!(s.diff_contents.entry(&f.file), Some(Entry::Loading)))
    });
    asked.lock().unwrap().files.clear();
}

pub(crate) fn type_query(rt: &mut Rt, query: &'static str) {
    rt.update(move |s, _| {
        s.changes_find_open(None);
        s.changes_find_set_query(query);
    });
}

pub(crate) fn found(rt: &mut Rt, queue: &Receiver<Job>, what: &str, total: usize) {
    settled(rt, queue, what, move |s| s.changes_find.hits().len() == total && s.changes_find_count_label() != "Searching…");
}

fn ten(late: &str) -> Vec<Changed> {
    (0..10)
        .map(|i| {
            let path: &'static str = Box::leak(format!("f{i}.ts").into_boxed_str());
            one(path, "a\n", if i == 9 { late } else { "b\n" })
        })
        .collect()
}

// ChangesReview.test.ts: "asks the machine which files hold the text and loads only those, not every collapsed file"
#[test]
fn asks_the_machine_which_files_hold_the_text_and_loads_only_those_not_every_collapsed_file() {
    let (mut rt, queue, asked) = scene(ten("late needle\n"), Search::Found(vec!["f9.ts"]));
    quiet(&mut rt, &queue, &asked);

    type_query(&mut rt, "needle");
    found(&mut rt, &queue, "the one match", 1);

    let log = asked.lock().unwrap();
    // Shown by `--nocapture`: what the machine was asked after the bar opened.
    println!("GET /diffs/search?q={}\nGET /diffs/file?path={}", log.searches.join(""), log.files.join("\nGET /diffs/file?path="));
    assert_eq!(log.searches, ["needle"], "one search, 120 ms after the typing");
    assert!(log.files.contains(&"f9.ts".to_string()), "{:?}", log.files);
    assert!(!log.files.contains(&"f8.ts".to_string()), "{:?}", log.files);
    drop(log);
    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");
}

// ChangesReview.test.ts: "searches every file itself when the machine can't say (an older Fleet), and for text git can't fold"
#[test]
fn searches_every_file_itself_when_the_machine_cant_say_an_older_fleet_and_for_text_git_cant_fold() {
    let (mut rt, queue, asked) = scene(ten("late Ünïcode\n"), Search::Found(vec![]));
    quiet(&mut rt, &queue, &asked);

    type_query(&mut rt, "ünïcode");
    found(&mut rt, &queue, "the match in the last file", 1);
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert!(asked.lock().unwrap().searches.is_empty(), "git folds ASCII letters only: the machine is not asked");
    asked_for(&mut rt, &queue, &asked, "f8.ts", "every file loads instead");
    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");

    // An older Fleet answers 404: the same, for an ASCII query.
    let (mut rt, queue, asked) = scene(ten("late needle\n"), Search::Missing);
    quiet(&mut rt, &queue, &asked);
    type_query(&mut rt, "needle");
    found(&mut rt, &queue, "the match in the last file", 1);
    assert_eq!(asked.lock().unwrap().searches, ["needle"]);
    asked_for(&mut rt, &queue, &asked, "f8.ts", "every file loads instead");
}

/// Waits for find to ask for `file`. The count says "1 of 1" as soon as one file holds the match, while the others can still be
/// on their way, so a file that was asked for has to be waited for, not looked for once.
fn asked_for(rt: &mut Rt, queue: &Receiver<Job>, asked: &Arc<Mutex<Asked>>, file: &str, what: &str) {
    settled(rt, queue, what, |_| asked.lock().unwrap().files.iter().any(|f| f == file));
}

// ChangesReview.test.ts: "searches a collapsed file, opens it on the match, and says which collapsed files it can't search"
#[test]
fn searches_a_collapsed_file_opens_it_on_the_match_and_says_which_collapsed_files_it_cant_search() {
    let files = vec![one("package-lock.json", "needle\n", "other\n"), one("src/a.ts", "a\n", "b\n"), one("src/b.ts", "x\n", "needle\n")];
    let (mut rt, queue, asked) = scene(files, Search::Found(vec!["package-lock.json", "src/b.ts"]));
    rt.update(|s, _| s.toggle_review_file("src/b.ts"));
    assert!(!rt.state.review.is_open("src/b.ts"));
    quiet(&mut rt, &queue, &asked);

    type_query(&mut rt, "needle");
    found(&mut rt, &queue, "the match in src/b.ts", 1);

    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");
    assert!(rt.state.review.is_open("src/b.ts"), "stepping to the match opens its file");
    assert_eq!(rt.state.changes_find_unsearched(), 1, "1 collapsed file not searched");
    assert!(!rt.state.review.is_open("package-lock.json"));
    assert!(!asked.lock().unwrap().files.contains(&"package-lock.json".to_string()), "the lockfile is never fetched");
}

// ChangesReview.test.ts: "loads files that start collapsed past the first few when the bar opens"
#[test]
fn loads_files_that_start_collapsed_past_the_first_few_when_the_bar_opens() {
    let (mut rt, queue, asked) = scene(ten("late needle\n"), Search::Missing);
    assert!(!rt.state.review.is_open("f9.ts"));
    quiet(&mut rt, &queue, &asked);

    type_query(&mut rt, "needle");
    found(&mut rt, &queue, "the match in the ninth file", 1);

    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");
    assert!(rt.state.review.is_open("f9.ts"));
    assert_eq!(rt.state.changes_find_unsearched(), 0);
}

// ChangesReview.test.ts: "finds a match in a folded unchanged run and opens the run when it steps there"
#[test]
fn finds_a_match_in_a_folded_unchanged_run_and_opens_the_run_when_it_steps_there() {
    let before: String = (0..30).map(|i| format!("row {i}\n")).collect();
    let after = before.replace("row 1\n", "row 1!\n").replace("row 28\n", "row 28!\n");
    let (mut rt, queue, asked) = scene(vec![one("rows.txt", &before, &after)], Search::Missing);
    settled(&mut rt, &queue, "rows.txt", |s| matches!(s.diff_contents.entry("rows.txt"), Some(Entry::Ready(_))));
    asked.lock().unwrap().files.clear();

    type_query(&mut rt, "row 15");
    found(&mut rt, &queue, "the match in the fold", 1);

    let hit = rt.state.changes_find.current_hit().cloned().expect("a match");
    assert!(hit.in_fold);
    assert!(rt.state.review.fold_open("rows.txt", &hit.item_id), "the run is open");
    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");
}

// 11.5: Enter and Shift+Enter, F3 and Ctrl+G step through the matches and wrap at both ends
#[test]
fn stepping_wraps_at_both_ends_and_a_new_query_goes_back_to_the_first() {
    let lines: String = (0..6).map(|i| format!("const v{i} = {i};\n")).collect();
    let (mut rt, queue, _asked) = scene(vec![one("notes.ts", "const f = 1;\n", &lines)], Search::Missing);
    type_query(&mut rt, "Const");
    found(&mut rt, &queue, "seven matches (six added lines and the removed one)", 7);
    assert_eq!(rt.state.changes_find_count_label(), "1 of 7");
    rt.update(|s, _| s.changes_find_step(1));
    assert_eq!(rt.state.changes_find_count_label(), "2 of 7");
    rt.update(|s, _| {
        s.changes_find_step(-1);
        s.changes_find_step(-1);
    });
    assert_eq!(rt.state.changes_find_count_label(), "7 of 7");
    rt.update(|s, _| s.changes_find_step(1));
    assert_eq!(rt.state.changes_find_count_label(), "1 of 7");
    rt.update(|s, _| s.changes_find_step(1));
    rt.update(|s, _| s.changes_find_set_query("v3"));
    found(&mut rt, &queue, "one match", 1);
    assert_eq!(rt.state.changes_find_count_label(), "1 of 1");
    rt.update(|s, _| s.changes_find_set_query("zzz"));
    found(&mut rt, &queue, "no matches", 0);
    assert_eq!(rt.state.changes_find_count_label(), "No matches");
}

// 11.9: a newer query replaces the one in flight; closing the bar drops the search
#[test]
fn a_newer_query_replaces_the_one_in_flight_and_a_closed_bar_asks_nothing() {
    let (mut rt, queue, asked) = scene(ten("late needle\n"), Search::Found(vec!["f9.ts"]));
    quiet(&mut rt, &queue, &asked);
    type_query(&mut rt, "nee");
    // The next keystroke comes inside the debounce: the first query is never sent.
    rt.frame(FRAME);
    rt.update(|s, _| s.changes_find_set_query("needle"));
    found(&mut rt, &queue, "the match", 1);
    assert_eq!(asked.lock().unwrap().searches, ["needle"]);

    rt.update(|s, _| s.changes_find_close());
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    assert!(rt.state.changes_find.hits().is_empty());
    assert_eq!(asked.lock().unwrap().searches, ["needle"], "closing asks nothing more");
}
