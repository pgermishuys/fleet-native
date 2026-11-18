//! `lib/line-quote.ts` and `lib/review-selection.ts` (component 12, piece L): the labels, the reference, the slice and the fence,
//! and which lines a selection stands for.
use crate::conv::line_quote::*;

fn row(old: Option<u32>, new: Option<u32>) -> SelectedRow {
    SelectedRow { old, new }
}

#[test]
fn selection_lines_counts_new_numbers_when_any_line_remains() {
    let rows = [row(Some(3), Some(3)), row(Some(4), None), row(None, Some(4))];
    assert_eq!(selection_lines(&rows), Some(ReviewSelection { side: Side::New, first: 3, last: 4 }));
}

#[test]
fn selection_lines_counts_old_numbers_for_removed_lines_only() {
    let rows = [row(Some(10), None), row(Some(11), None)];
    let sel = selection_lines(&rows).unwrap();
    assert_eq!((sel.side, sel.first, sel.last), (Side::Old, 10, 11));
    assert_eq!(sel.label(), "Old L10\u{2013}11");
    assert_eq!(sel.link("a.rs"), None);
    assert_eq!(selection_lines(&[]), None);
}

#[test]
fn a_row_the_selection_only_touches_at_its_edge_is_left_out() {
    let t = |n, s, e| Touched { row: row(Some(n), Some(n)), starts_at_end: s, ends_at_start: e };
    let rows = rows_of(vec![t(1, true, false), t(2, false, false), t(3, false, false), t(4, false, true)]);
    assert_eq!(rows.iter().map(|r| r.new.unwrap()).collect::<Vec<_>>(), [2, 3]);
    // A lone row stays, however little of it is taken.
    assert_eq!(rows_of(vec![t(1, true, false)]).len(), 1);
}

#[test]
fn labels_references_and_slices() {
    assert_eq!(line_range_label(7, 7), "L7");
    assert_eq!(line_range_label(18, 27), "L18\u{2013}27");
    assert_eq!(line_count_label(7, 7), "1 line");
    assert_eq!(line_count_label(18, 27), "10 lines");
    assert_eq!(line_reference("src/app.ts", 7, 7), "src/app.ts:7");
    assert_eq!(line_reference("src/app.ts", 18, 27), "src/app.ts:18-27");
    assert_eq!(slice_lines("a\r\nb\nc\nd", 2, 3), "b\nc");
}

#[test]
fn the_fence_is_longer_than_any_run_of_backticks_inside() {
    assert_eq!(line_quote("a.md:1", "docs/a.MD", "x"), "a.md:1\n```md\nx\n```\n");
    assert_eq!(line_quote("h", "Makefile", "``` and ````"), "h\n`````\n``` and ````\n`````\n");
    assert_eq!(line_quote("h", "src/notes.ts", "t"), "h\n```ts\nt\n```\n");
}

#[test]
fn quote_of_old_lines_says_before() {
    let sel = ReviewSelection { side: Side::Old, first: 11, last: 11 };
    assert_eq!(
        sel.quote("src/notes.ts", "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nconst f = oldName(1);\n", "x"),
        "src/notes.ts:11 (before)\n```ts\nconst f = oldName(1);\n```\n"
    );
}
