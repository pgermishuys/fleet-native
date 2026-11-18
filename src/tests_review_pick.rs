//! Selecting lines in the review (component 12, piece L): `ChangesReview.test.ts` "selecting lines". A person drags across the lines'
//! text; the bar under the review says which lines, and Copy link, Quote and Add to message act on them.
use crate::fleet::diff_contents::Entry;
use crate::fleet::diffs::{FileDiff, FileStatus};
use crate::fleet::review::Layout;
use crate::mock::changes::Case;
use crate::tests::{FRAME, app_tall, click};
use crate::{State, conv};
use rune::Runtime;

type Rt = Runtime<State>;

const NOTES: &str = "src/paginate.py";

const BEFORE: &str = "export interface Note {\n  id: string;\n  title: string;\n}\nconst a = 1;\nconst b = 2;\nconst c = 3;\nconst d = 4;\n\
                      const e = 5;\nconst f = oldName(1);\nconst g = 7;\n";
const AFTER: &str = "export interface Note {\n  id: string;\n  title: string;\n  tags: string[];\n}\nconst a = 1;\nconst b = 2;\nconst c = 3;\n\
                     const d = 4;\nconst e = 5;\nconst f = newName(1);\nconst g = 7;\n";

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

/// The review of one invented file, `src/paginate.py` standing in for a TypeScript one, over a fresh draft.
fn shown(layout: Layout) -> Rt {
    let mut rt = app_tall();
    rt.update(move |s, _| {
        s.show_changes_scene(Case::Review, true);
        let file = FileDiff {
            file: NOTES.into(),
            status: FileStatus::Modified,
            additions: 2,
            deletions: 1,
            before: BEFORE.into(),
            after: AFTER.into(),
            binary: false,
            truncated: false,
        };
        let id = s.canvas_session();
        s.diff_contents.put(&id, NOTES, Entry::Ready(file));
        s.set_review_layout(layout);
        // The folded runs are opened so every line is drawn.
        let items = crate::view::review::review_items(match s.diff_contents.entry(NOTES) {
            Some(Entry::Ready(f)) => f,
            _ => unreachable!(),
        });
        for item in items.iter() {
            if let conv::review::ReviewItem::Fold(f) = item {
                s.review.open_fold(NOTES, &f.id);
            }
        }
    });
    settle(&mut rt);
    rt
}

/// The key of the text of the line that has `number` on the `new` or old side.
fn key_of(rt: &Rt, number: u32, new: bool, column: Option<bool>) -> String {
    let Some(Entry::Ready(f)) = rt.state.diff_contents.entry(NOTES) else { panic!("no contents") };
    let items = crate::view::review::review_items(f);
    let line = items
        .iter()
        .flat_map(|i| match i {
            conv::review::ReviewItem::Hunk(h) => h.lines.iter(),
            conv::review::ReviewItem::Fold(f) => f.lines.iter(),
        })
        .find(|l| if new { l.line.new == Some(number) } else { l.line.old == Some(number) })
        .unwrap_or_else(|| panic!("no line {number}"));
    crate::fleet::review_pick::text_key(
        NOTES,
        line.index,
        column.map(|new| if new { conv::line_quote::Side::New } else { conv::line_quote::Side::Old }),
    )
}

fn rect(rt: &Rt, key: &str) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key}")).rect
}

/// Drags from the start of one line's text (or its end) to the end of another's (or its start).
fn select(rt: &mut Rt, from: &str, to: &str, from_end: bool, to_start: bool) {
    let (a, b) = (rect(rt, from), rect(rt, to));
    let down = (if from_end { a[0] + a[2] - 18.0 } else { a[0] + 1.0 }, a[1] + a[3] / 2.0);
    let up = (if to_start { b[0] + 1.0 } else { b[0] + b[2] - 18.0 }, b[1] + b[3] / 2.0);
    rt.pointer_down(down.0, down.1);
    rt.frame(FRAME);
    rt.pointer_move(up.0, up.1);
    rt.frame(FRAME);
    rt.pointer_up(up.0, up.1);
    settle(rt);
}

fn text_of(rt: &Rt, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text).unwrap_or_default()
}

fn has(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

// ChangesReview.test.ts "selecting lines": "a selection of the file's lines shows the bar; Add to message puts them in the draft as a reference"
#[test]
fn a_selection_of_the_files_lines_shows_the_bar_add_to_message_puts_them_in_the_draft_as_a_reference() {
    let mut rt = shown(Layout::Unified);
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    assert!(!has(&rt, "lines-bar"));

    // From "  id: string;" through the added "  tags: string[];"
    let (from, to) = (key_of(&rt, 2, true, None), key_of(&rt, 4, true, None));
    select(&mut rt, &from, &to, false, false);

    assert_eq!(text_of(&rt, "line-bar-file"), "paginate.py");
    assert_eq!(text_of(&rt, "line-bar-label"), "L2\u{2013}4");
    // The panel is 360 wide, under Fleet's 404 px container query: the count is left off there.
    assert_eq!(rt.state.review.picked.as_ref().map(|p| p.lines.count()).as_deref(), Some("3 lines"));
    assert_eq!(text_of(&rt, "line-bar-count"), "");
    click(&mut rt, "copy-link");
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("src/paginate.py:2-4"));
    assert_eq!(rt.state.review.toast.as_ref().map(|t| t.0.as_str()), Some("Copied src/paginate.py:2-4"));
    assert!(has(&rt, "lines-bar"), "the buttons keep the selection");
    click(&mut rt, "copy-quote");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("src/paginate.py:2-4\n```py\n  id: string;\n  title: string;\n  tags: string[];\n```\n")
    );
    assert_eq!(rt.state.review.toast.as_ref().map(|t| t.0.as_str()), Some("Copied L2\u{2013}4 as a quote"));

    click(&mut rt, "add-to-message");
    settle(&mut rt);
    let session = rt.state.canvas_session();
    let draft = if rt.state.draft_of.as_deref() == Some(session.as_str()) {
        rt.state.draft.text.clone()
    } else {
        rt.state.drafts.text_of(&session).to_string()
    };
    assert_eq!(draft, "@src/paginate.py:2-4 ");
    let mut focused = None;
    rt.update(|_, tx| focused = tx.focused().map(str::to_string));
    assert_eq!(focused.as_deref(), Some("composer"));
    assert!(!has(&rt, "lines-bar"));
}

// ChangesReview.test.ts "selecting lines": "removed lines are quoted as they were, with no link"
#[test]
fn removed_lines_are_quoted_as_they_were_with_no_link() {
    let mut rt = shown(Layout::Unified);
    let key = key_of(&rt, 10, false, None);
    // The removed line alone: it has an old number and no new one.
    select(&mut rt, &key, &key, false, false);
    assert_eq!(text_of(&rt, "line-bar-label"), "Old L10");
    assert!(!has(&rt, "copy-link"));
    click(&mut rt, "add-to-message");
    settle(&mut rt);
    let session = rt.state.canvas_session();
    let draft = if rt.state.draft_of.as_deref() == Some(session.as_str()) {
        rt.state.draft.text.clone()
    } else {
        rt.state.drafts.text_of(&session).to_string()
    };
    assert_eq!(draft, "src/paginate.py:10 (before)\n```py\nconst f = oldName(1);\n```\n");
}

// ChangesReview.test.ts "selecting lines": "a selection that only touches the edge of a line leaves it out"
#[test]
fn a_selection_that_only_touches_the_edge_of_a_line_leaves_it_out() {
    let mut rt = shown(Layout::Unified);
    let (from, to) = (key_of(&rt, 1, true, None), key_of(&rt, 4, true, None));
    select(&mut rt, &from, &to, true, true);
    assert_eq!(text_of(&rt, "line-bar-label"), "L2\u{2013}3");
}

// ChangesReview.test.ts "selecting lines": "in Split, a drag down the old column counts old lines only"
#[test]
fn in_split_a_drag_down_the_old_column_counts_old_lines_only() {
    let mut rt = shown(Layout::Split);
    let (a, b) = (key_of(&rt, 9, false, Some(false)), key_of(&rt, 10, false, Some(false)));
    select(&mut rt, &a, &b, false, false);
    assert_eq!(text_of(&rt, "line-bar-label"), "Old L9\u{2013}10");

    // A press in the new column next lets the old selection go and takes the new side.
    let (c, d) = (key_of(&rt, 3, true, Some(true)), key_of(&rt, 4, true, Some(true)));
    select(&mut rt, &c, &d, false, false);
    assert_eq!(text_of(&rt, "line-bar-label"), "L3\u{2013}4");
}

// ChangesReview.test.ts "selecting lines": "a selection outside the review shows no bar"
#[test]
fn a_selection_outside_the_review_shows_no_bar() {
    let mut rt = shown(Layout::Unified);
    // Text elsewhere (the conversation) selected: the review hears nothing.
    let outside = rt.elements().into_iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("review-path-")) && !e.exiting);
    assert!(outside.is_some());
    rt.update(|_, tx| tx.select_all_text("review-path-src/paginate.py"));
    settle(&mut rt);
    assert!(!has(&rt, "lines-bar"));
    assert!(rt.state.review.picked.is_none());
}

// Esc clears the lines, and a click on Clear does too.
#[test]
fn esc_and_clear_drop_the_bar() {
    let mut rt = shown(Layout::Unified);
    let (from, to) = (key_of(&rt, 2, true, None), key_of(&rt, 3, true, None));
    select(&mut rt, &from, &to, false, false);
    assert!(has(&rt, "lines-bar"));
    click(&mut rt, "line-bar-clear");
    assert!(!has(&rt, "lines-bar"));
    assert!(rt.state.review.picked.is_none());
}

// The toast goes after 2.2 s.
#[test]
fn the_toast_goes_after_two_seconds_and_a_bit() {
    let mut rt = shown(Layout::Unified);
    let (from, to) = (key_of(&rt, 2, true, None), key_of(&rt, 3, true, None));
    select(&mut rt, &from, &to, false, false);
    click(&mut rt, "copy-link");
    assert!(has(&rt, "review-toast"));
    for _ in 0..30 {
        rt.frame(0.1);
    }
    assert!(!has(&rt, "review-toast"));
}
