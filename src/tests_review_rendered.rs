//! Rendered, Open file, Copy path and the focus request in the review (component 12, piece M): `ChangesReview.test.ts` "rendered
//! markdown…", "offers Rendered for a markdown file only", "opens a changed file in a file tab…", and the focus request (`reviewFocus`).
use crate::State;
use crate::mock::changes::Case;
use crate::tests::{FRAME, app_tall, click};
use rune::Runtime;

type Rt = Runtime<State>;

const MD: &str = "docs/paging.md";
const HTML: &str = "docs/preview.html";

fn settle(rt: &mut Rt) {
    for _ in 0..6 {
        rt.frame(FRAME);
    }
}

fn shown(case: Case) -> Rt {
    let mut rt = app_tall();
    rt.update(move |s, _| s.show_changes_scene(case, true));
    settle(&mut rt);
    rt
}

/// The header's actions show while it has the pointer, so the pointer goes there first.
fn hover_head(rt: &mut Rt, path: &str) {
    let (x, y) = rt.find(&format!("review-file-head-{path}")).unwrap_or_else(|| panic!("no header for {path}"));
    rt.pointer_move(x, y);
    rt.frame(FRAME);
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

// ChangesReview.test.ts: "offers Rendered for a markdown file only"
#[test]
fn offers_rendered_for_a_markdown_file_only() {
    let rt = shown(Case::ReviewDocs);
    assert!(has(&rt, &format!("review-view-{MD}-rendered")));
    assert!(has(&rt, &format!("review-view-{HTML}-rendered")));
    let rt = shown(Case::Review);
    assert!(!has(&rt, "review-view-src/paginate.py-rendered") && !has(&rt, "review-view-src/paginate.py"));
    assert!(crate::fleet::review::has_rendered_view("README.MD") && crate::fleet::review::has_rendered_view("a/b.mdx"));
    assert!(crate::fleet::review::has_rendered_view("x.htm") && !crate::fleet::review::has_rendered_view("x.txt"));
}

// ChangesReview.test.ts: "rendered markdown can be asked about, like a file tab's" (the drawing half; asking is piece N)
#[test]
fn rendered_markdown_draws_the_new_text_and_diff_brings_the_lines_back() {
    let mut rt = shown(Case::ReviewDocs);
    assert!(!has(&rt, &format!("review-rendered-{MD}")) && !rt.state.review.is_rendered(MD));
    hover_head(&mut rt, MD);
    click(&mut rt, &format!("review-view-{MD}-rendered"));
    settle(&mut rt);
    assert!(rt.state.review.is_rendered(MD));
    assert!(has(&rt, &format!("review-rendered-{MD}-markdown")), "the Markdown is drawn");
    assert!(!rt.state.review.is_rendered(HTML), "each file has its own choice");
    hover_head(&mut rt, MD);
    click(&mut rt, &format!("review-view-{MD}-diff"));
    settle(&mut rt);
    assert!(!has(&rt, &format!("review-rendered-{MD}-markdown")));
}

#[test]
fn rendered_html_is_a_card_whose_expand_opens_it_as_a_tab() {
    let mut rt = shown(Case::ReviewDocs);
    rt.update(|s, _| s.set_review_rendered(HTML, true));
    settle(&mut rt);
    let key = format!("review-rendered-{HTML}-html");
    assert!(has(&rt, &key));
    click(&mut rt, &format!("{key}-expand"));
    settle(&mut rt);
    let open = rt.state.open_canvases();
    let tab = open.active().expect("a tab is active");
    assert_eq!(tab.payload.as_ref().and_then(|p| p.source_path.as_deref()), Some(HTML));
    assert!(tab.payload.as_ref().is_some_and(|p| p.content.contains("Page 2 of 5")));
}

// ChangesReview.test.ts: "opens a changed file in a file tab on Diff from its header" (its disabled form until component 11)
#[test]
fn open_file_is_off_with_its_reason_until_the_file_viewer_exists() {
    let rt = shown(Case::ReviewDocs);
    let e = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("review-open-file-{MD}"))).expect("button");
    assert!(!e.focusable, "drawn off");
    assert_eq!(e.help.as_deref(), Some("Opening files comes with the file viewer"));
}

#[test]
fn copy_path_puts_the_path_on_the_clipboard() {
    let mut rt = shown(Case::ReviewDocs);
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    hover_head(&mut rt, MD);
    click(&mut rt, &format!("review-copy-path-{MD}"));
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some(MD));
}

// ChangesReview.test.ts "find in changes": "shows the diff of a file viewed as Rendered when a match is in it"
#[test]
fn a_find_match_in_a_rendered_file_shows_its_diff() {
    let mut rt = app_tall();
    rt.update(|s, _| {
        s.show_review_docs(true, true);
        s.show_changes_find("size", 0);
        s.changes_find_reveal();
    });
    settle(&mut rt);
    assert!(rt.state.changes_find.hits().iter().any(|h| h.path == MD), "the match is in the Markdown file");
    assert!(!rt.state.review.is_rendered(MD), "it shows its diff");
    assert!(rt.state.review.is_rendered(HTML), "another file stays as it was");
}

// The focus request: set by `open_changes(path)`, answered once per `seq`.
#[test]
fn a_focus_request_opens_the_file_marks_it_near_and_is_answered_once() {
    let mut rt = shown(Case::ReviewStates);
    let path = "src/beta_loading.rs";
    rt.update(move |s, _| {
        s.review.toggle(path);
        assert!(!s.review.is_open(path));
        let id = s.canvas_session();
        s.canvases.open_changes(&id, Some(path));
    });
    settle(&mut rt);
    assert!(rt.state.review.is_open(path), "its section opens");
    assert!(rt.state.review.is_near(path), "and is drawn");
    // Once per request: shut it again and nothing reopens it.
    rt.update(move |s, _| s.review.toggle(path));
    settle(&mut rt);
    assert!(!rt.state.review.is_open(path));
    // A new request (a new seq) is answered again.
    rt.update(move |s, _| {
        let id = s.canvas_session();
        s.canvases.open_changes(&id, Some(path));
    });
    settle(&mut rt);
    assert!(rt.state.review.is_open(path));
}

#[test]
fn a_focus_request_for_a_file_not_in_the_list_waits_for_it() {
    let mut rt = shown(Case::ReviewStates);
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.canvases.open_changes(&id, Some("src/later.rs"));
    });
    settle(&mut rt);
    assert!(!rt.state.review.is_near("src/later.rs"));
    // It comes with the list's next answer, and the request is answered then.
    rt.update(|s, _| {
        let mut list: Vec<serde_json::Value> =
            s.diffs.files().iter().map(|f| serde_json::json!({ "file": f.file, "status": "modified" })).collect();
        list.push(serde_json::json!({ "file": "src/later.rs", "status": "added", "additions": 1, "deletions": 0 }));
        s.diffs.apply(&serde_json::json!({ "diffs": list, "available": true }));
    });
    settle(&mut rt);
    assert!(rt.state.review.is_near("src/later.rs"));
}
