//! The rows of a file's Diff view (`merge.ts`: `unifiedMergeView` with `collapseUnchanged: { margin: 3, minSize: 8 }`): where the
//! removed lines go, which words are marked, and which runs fold.
use crate::conv::changed_lines::base_lines;
use crate::conv::file_diff::{Item, Kind, MARGIN, MIN_FOLD, deleted_rows, items, rows};

fn doc(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

fn shown(base: &str, text: &[&str]) -> Vec<(Kind, String, Option<usize>)> {
    rows(&base_lines(base), &doc(text)).into_iter().map(|r| (r.kind, r.text, r.no)).collect()
}

#[test]
fn removed_lines_come_above_the_lines_that_replaced_them() {
    let got = shown("a\nb\nc\n", &["a", "B", "c", ""]);
    assert_eq!(
        got,
        [
            (Kind::Same, "a".into(), Some(1)),
            (Kind::Removed, "b".into(), None),
            (Kind::Added, "B".into(), Some(2)),
            (Kind::Same, "c".into(), Some(3)),
            (Kind::Same, "".into(), Some(4)),
        ]
    );
}

#[test]
fn a_removed_line_has_no_number_and_the_numbers_keep_counting_the_new_text() {
    let got = shown("a\nb\nc\nd", &["a", "d"]);
    assert_eq!(got.iter().map(|r| r.2).collect::<Vec<_>>(), [Some(1), None, None, Some(2)]);
    assert_eq!(got.iter().filter(|r| r.0 == Kind::Removed).count(), 2);
}

#[test]
fn a_file_with_no_base_is_all_added_and_an_empty_base_leaves_no_removed_line() {
    let got = shown("", &["a", "b"]);
    assert_eq!(got.iter().map(|r| r.0).collect::<Vec<_>>(), [Kind::Added, Kind::Added]);
}

#[test]
fn the_changed_words_are_marked_on_both_lines_of_a_pair() {
    let r = rows(&base_lines("    if total_rows <= 0:\n"), &doc(&["    if total_rows < 1:", ""]));
    let removed = r.iter().find(|r| r.kind == Kind::Removed).unwrap();
    let added = r.iter().find(|r| r.kind == Kind::Added).unwrap();
    assert_eq!(&removed.text[removed.marks[0].0..removed.marks[0].1], "= 0");
    assert_eq!(&added.text[added.marks[0].0..added.marks[0].1], " 1");
}

#[test]
fn a_deleted_file_is_all_removed_lines_with_their_numbers() {
    let r = deleted_rows("one\ntwo\n");
    assert!(r.iter().all(|r| r.kind == Kind::Removed));
    assert_eq!(r.iter().map(|r| r.no).collect::<Vec<_>>(), [Some(1), Some(2), Some(3)]);
    assert!(deleted_rows("").is_empty());
}

fn numbered(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("line {i}")).collect()
}

fn folds(items: &[Item]) -> Vec<(usize, usize)> {
    items.iter().filter_map(|i| if let Item::Fold { first, hidden } = i { Some((*first, *hidden)) } else { None }).collect()
}

#[test]
fn unchanged_runs_of_eight_or_more_fold_with_three_around_each_change() {
    let base = numbered(40);
    let mut text = base.clone();
    text[29] = "changed".into();
    let r = rows(&base, &text);
    let it = items(&r, MARGIN, MIN_FOLD, &|_| false);
    // 29 rows come before the change: all but the 3 beside it fold. The 10 after it leave 7 to fold, under the minimum.
    assert_eq!(folds(&it), [(0, 26)]);
}

#[test]
fn a_run_that_folds_less_than_eight_lines_stays_open() {
    let base = numbered(30);
    let mut text = base.clone();
    text[2] = "changed".into();
    text[17] = "changed".into();
    let r = rows(&base, &text);
    // Between the two changes lie 14 same rows (rows 4.. in the removed-and-added order): 14 - 6 = 8 fold.
    assert_eq!(folds(&items(&r, MARGIN, MIN_FOLD, &|_| false)).len(), 2);
    text[16] = "changed".into();
    let r = rows(&base, &text);
    // One fewer in between: 13 - 6 = 7, under the minimum, so only the run after the last change folds.
    let f = folds(&items(&r, MARGIN, MIN_FOLD, &|_| false));
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].1, 30 - 17 - 1 - MARGIN);
}

#[test]
fn a_file_with_nothing_changed_shows_whole_and_an_opened_fold_shows_its_rows() {
    let base = numbered(30);
    let r = rows(&base, &base);
    assert!(items(&r, MARGIN, MIN_FOLD, &|_| false).iter().all(|i| matches!(i, Item::Row(_))));
    let mut text = base.clone();
    text[25] = "x".into();
    let r = rows(&base, &text);
    let closed = items(&r, MARGIN, MIN_FOLD, &|_| false);
    let (first, hidden) = folds(&closed)[0];
    let open = items(&r, MARGIN, MIN_FOLD, &|at| at == first);
    assert!(folds(&open).is_empty());
    assert_eq!(open.len(), closed.len() - 1 + hidden);
}
