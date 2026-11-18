//! Drawing what's near (component 12, piece H): `ChangesReview.test.ts` "fetches and draws an open file only once it comes near the
//! visible part, and keeps it drawn", and the 40-file change that asks for only the files near the top. A fake Fleet logs which
//! files it was asked for; the review is scrolled with the wheel, as a reader does.
use crate::fleet::diff_contents::Entry;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::changes::Case;
use crate::tests::{FRAME, app, finish, read_request};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::json;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn path(i: usize) -> String {
    format!("src/part_{i:02}.py")
}

/// `n` changed files of 5 added lines each, the session served by a fake Fleet that logs every `/diffs/file` path it is asked for.
fn forty(n: usize) -> (Rt, Receiver<Job>, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = read_request(&mut stream);
            let body = match request.split_once("path=") {
                Some((_, rest)) if request.contains("/diffs/file") => {
                    let file = rest.split([' ', '&']).next().unwrap_or("").replace("%2F", "/");
                    log.lock().unwrap().push(file.clone());
                    json!({ "file": file, "status": "modified", "additions": 5, "deletions": 0, "before": "a = 1\n", "after": "a = 1\nb = 2\nc = 3\nd = 4\ne = 5\nf = 6\n" })
                        .to_string()
                }
                _ => "{}".to_string(),
            };
            let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
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
        let list: Vec<_> = (0..n).map(|i| json!({ "file": path(i), "status": "modified", "additions": 5, "deletions": 0 })).collect();
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

fn asked(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    let mut v = log.lock().unwrap().clone();
    v.sort();
    v
}

fn settled(rt: &mut Rt, queue: &Receiver<Job>, what: &str, done: impl FnMut(&State) -> bool) {
    test_wait::drive(rt, queue, what, done, test_wait::app_state);
}

// ChangesReview.test.ts: "fetches and draws an open file only once it comes near the visible part, and keeps it drawn"
#[test]
fn fetches_and_draws_an_open_file_only_once_it_comes_near_the_visible_part_and_keeps_it_drawn() {
    let (mut rt, queue, log) = forty(40);
    // The eighth file is open and far down: nothing has asked for it, and it holds its room instead of drawing.
    let far = path(7);
    assert!(rt.state.review.is_open(&far) && !rt.state.review.is_near(&far));
    assert!(rt.find(&format!("review-pending-{far}")).is_some(), "an empty room stands in for it");
    assert!(rt.find(&format!("review-lines-{far}")).is_none());
    let before = asked(&log);
    assert!(!before.contains(&far), "{before:?}");
    // Scroll down until it is near: it is asked for, drawn, and (scrolled back up) stays drawn.
    let (x, y) = rt.find("review-scroll").expect("the review");
    let mut turns = 0;
    while !rt.state.review.is_near(&far) {
        rt.wheel(x, y, -300.0); // a negative turn reads further down
        rt.frame(FRAME);
        turns += 1;
        assert!(turns < 60, "the review never came near {far}");
    }
    settled(&mut rt, &queue, "the near file's contents", |s| matches!(s.diff_contents.entry(&far), Some(Entry::Ready(_))));
    assert!(asked(&log).contains(&far));
    for _ in 0..turns + 5 {
        rt.wheel(x, y, 300.0);
        rt.frame(FRAME);
    }
    assert!(rt.state.review.is_near(&far), "scrolled away again, it stays near");
    assert!(matches!(rt.state.diff_contents.entry(&far), Some(Entry::Ready(_))));
}

// 8.1, 8.2: opening a long change asks for the files near the top only
#[test]
fn a_forty_file_change_asks_for_only_the_files_near_the_top() {
    let (mut rt, queue, log) = forty(40);
    settled(&mut rt, &queue, "the first file's contents", |s| matches!(s.diff_contents.entry(&path(0)), Some(Entry::Ready(_))));
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    let got = asked(&log);
    // Open files are the first eight; each holds (5 + 8) × 20 = 260 px, so the 900 px box and 600 px margin reach the first six.
    assert!(!got.is_empty() && got.len() < 8, "{got:?}");
    assert!(got.iter().all(|p| *p < path(8)), "only open files: {got:?}");
    assert!(!got.contains(&path(7)), "{got:?}");
    // Every file past the near ones holds its 260 px; none of the 40 drew lines it wasn't asked for.
    let pending = (0..40).filter(|i| rt.find(&format!("review-pending-{}", path(*i))).is_some()).count();
    assert!(pending >= 2, "{pending}");
    // A request per file at most.
    let mut once = got.clone();
    once.dedup();
    assert_eq!(once, got);
}

// 8.3: a file's lines are worked out once per loaded entry
#[test]
fn a_files_lines_are_worked_out_once_and_another_file_loading_doesnt_redo_them() {
    let (mut rt, queue, _log) = forty(3);
    settled(&mut rt, &queue, "all three files", |s| (0..3).all(|i| matches!(s.diff_contents.entry(&path(i)), Some(Entry::Ready(_)))));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    let worked = crate::view::review::lines_worked();
    assert!(worked >= 3, "{worked}");
    // Another file's answer comes (a redraw with new state): none of the three is worked out again.
    rt.update(|s, _| {
        let id = s.diffs.session().to_string();
        s.diff_contents.put(&id, "src/elsewhere.py", Entry::Loading);
    });
    for _ in 0..6 {
        rt.frame(FRAME);
    }
    assert_eq!(crate::view::review::lines_worked(), worked);
}
