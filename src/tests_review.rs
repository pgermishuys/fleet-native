//! The review's header and file sections (component 12, piece E): `ChangesReview.test.ts` under its names. The scenes hold invented
//! files (`mock::changes`); Try again talks to a fake Fleet that answers one file's request and counts what it was asked.
use crate::fleet::diff_contents::Entry;
use crate::fleet::link::{Job, Link, Post};
use crate::mock::changes::Case;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn shown(case: Case) -> Rt {
    let mut rt = app();
    rt.update(move |s, _| s.show_changes_scene(case, true));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

fn all_text(rt: &Rt) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

const NOTES: &str = "src/paginate.py";

// ChangesReview.test.ts: "shows one section per file with totals, hunks, and folded unchanged runs"
// (the hunks' headers and the folds are piece F's; here the sections, the totals and the changed lines with their context)
#[test]
fn shows_one_section_per_file_with_totals_and_the_changed_lines() {
    let rt = shown(Case::Review);
    let sections = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("review-file-src/"))).count();
    assert_eq!(sections, 2, "one card per file");
    let totals = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("review-totals")).expect("the totals");
    assert_eq!(totals.label.as_deref(), Some("+20\u{2212}3"));
    let text = all_text(&rt);
    assert!(text.contains("DEFAULT_PAGE_SIZE = 20") && text.contains("def clamp_page_size(requested):"), "{text}");
    // Files are sorted by path.
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with("review-file-head-")).collect();
    assert_eq!(keys, ["review-file-head-src/page_size.py", "review-file-head-src/paginate.py"]);
}

// ChangesReview.test.ts: "collapses a file section when its header is clicked"
#[test]
fn collapses_a_file_section_when_its_header_is_clicked() {
    let mut rt = shown(Case::Review);
    let lines = format!("review-lines-{NOTES}");
    assert!(has(&rt, &lines));
    click(&mut rt, &format!("review-file-head-{NOTES}"));
    settle(&mut rt);
    assert!(!has(&rt, &lines), "a shut file draws no body");
    assert!(has(&rt, "review-lines-src/page_size.py"), "the other file stays open");
    click(&mut rt, &format!("review-file-head-{NOTES}"));
    settle(&mut rt);
    assert!(has(&rt, &lines));
}

// ChangesReview.test.ts: "always shows the whole session, with no scope to pick"
#[test]
fn always_shows_the_whole_session_with_no_scope_to_pick() {
    let rt = shown(Case::Review);
    let text = all_text(&rt);
    assert!(text.contains("Whole session") && !text.contains("This turn"), "{text}");
    assert!(has(&rt, "review-file-head-src/page_size.py") && has(&rt, &format!("review-file-head-{NOTES}")));
}

// ChangesReview.test.ts: "a lockfile starts shut, and opens on a click"
#[test]
fn a_lockfile_starts_shut_and_opens_on_a_click() {
    let mut rt = shown(Case::ReviewStates);
    assert!(has(&rt, "review-file-head-yarn.lock") || rt.state.review_files().iter().any(|f| f.file == "yarn.lock"));
    assert!(!rt.state.review.is_open("yarn.lock"));
    assert!(rt.state.review.is_open("src/alpha.rs"));
    assert!(has(&rt, "review-lines-src/alpha.rs"));
    // Opening it asks for its contents (no server here, so it stays on Loading…).
    rt.update(|s, _| s.toggle_review_file("yarn.lock"));
    assert!(rt.state.review.is_open("yarn.lock"));
}

// ChangesReview.test.ts: "offers Try again on a file whose diff failed to load, and draws it when the retry answers"
#[test]
fn offers_try_again_on_a_file_whose_diff_failed_to_load_and_draws_it_when_the_retry_answers() {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = crate::test_http::read(&mut stream).raw();
            let body = if request.starts_with("GET") && request.contains("/diffs/file") {
                counter.fetch_add(1, Ordering::SeqCst);
                r#"{"file":"src/paginate.py","status":"modified","additions":1,"deletions":0,"before":"a = 1\n","after":"a = 1\ntags = []\n"}"#
            } else {
                "{}"
            };
            let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            crate::tests::finish(&mut stream);
        }
    });
    let mut rt = shown(Case::Review);
    rt.update(|s, _| {
        let id = s.diffs.session().to_string();
        let version = crate::fleet::diff_contents::version_of(s.diffs.by_file(NOTES).expect("the file"));
        s.diff_contents.put_at(&id, NOTES, &version, Entry::Error("HTTP 502".into()));
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    rt.update(move |s, _| s.link = Some(link));
    settle(&mut rt);
    assert!(all_text(&rt).contains("Couldn't load this diff."));
    click(&mut rt, &format!("review-retry-{NOTES}"));
    test_wait::drive(
        &mut rt,
        &queue,
        "the file asked again and drawn",
        |s| matches!(s.diff_contents.entry(NOTES), Some(Entry::Ready(_))),
        |_| format!("asked {} times", asked.load(Ordering::SeqCst)),
    );
    settle(&mut rt);
    let text = all_text(&rt);
    assert!(!text.contains("Couldn't load") && text.contains("tags = []"), "{:?} {text}", rt.state.diff_contents.entry(NOTES));
    assert!(asked.load(Ordering::SeqCst) >= 1);
}

// 5.7: Collapse all and Expand all
#[test]
fn collapse_all_shuts_every_file_and_the_button_then_offers_expand_all() {
    let mut rt = shown(Case::Review);
    let tip = |rt: &Rt| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("review-collapse")).and_then(|e| e.label);
    assert_eq!(tip(&rt).as_deref(), Some("Collapse all files"));
    click(&mut rt, "review-collapse");
    settle(&mut rt);
    assert!(!has(&rt, &format!("review-lines-{NOTES}")) && !has(&rt, "review-lines-src/page_size.py"));
    assert_eq!(tip(&rt).as_deref(), Some("Expand all files"));
    click(&mut rt, "review-collapse");
    settle(&mut rt);
    assert!(has(&rt, &format!("review-lines-{NOTES}")));
}

// 5.5: the layout choice shows the pick
#[test]
fn the_layout_choice_follows_the_pick() {
    let mut rt = shown(Case::Review);
    use crate::fleet::review::Layout;
    let layout = |rt: &Rt| rt.state.review_layout();
    assert_eq!(layout(&rt), Layout::Unified);
    click(&mut rt, "review-layout-split");
    settle(&mut rt);
    assert_eq!(layout(&rt), Layout::Split);
    click(&mut rt, "review-layout-unified");
    assert_eq!(layout(&rt), Layout::Unified);
}

// 6.6: the notes of a file's body
#[test]
fn a_file_body_says_binary_too_large_and_no_line_changes() {
    let rt = shown(Case::ReviewStates);
    let text = all_text(&rt);
    assert!(text.contains("Binary file \u{2014} no diff available"), "{text}");
    assert!(text.contains("File too large to diff") && text.contains("No line changes."), "{text}");
    assert!(text.contains("Couldn't load this diff.") || has(&rt, "review-retry-assets/failed.rs"), "{text}");
}
