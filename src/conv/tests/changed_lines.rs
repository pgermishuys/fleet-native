//! The `changedLines` cases of `file-buffer.test.ts`, under Fleet's names, and the edges of the line diff.
use crate::conv::changed_lines::{base_lines, changed_lines};

fn doc(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

#[test]
fn marks_lines_that_differ_from_the_git_base() {
    let text = doc(&["a", "B", "c", "new", "d", ""]);
    assert_eq!(changed_lines(&base_lines("a\nb\nc\nd\n"), &text), [2, 4]);
}

#[test]
fn ignores_line_breaks_and_a_bom_in_the_base() {
    assert_eq!(changed_lines(&base_lines("\u{feff}a\r\nb\r\n"), &doc(&["a", "b", ""])), Vec::<usize>::new());
    assert_eq!(changed_lines(&base_lines("a\rb\r"), &doc(&["a", "b", ""])), Vec::<usize>::new());
}

#[test]
fn marks_the_line_after_a_deletion() {
    assert_eq!(changed_lines(&base_lines("a\nb\nc"), &doc(&["a", "c"])), [2]);
}

#[test]
fn a_deletion_at_the_end_marks_the_last_line() {
    assert_eq!(changed_lines(&base_lines("a\nb\nc"), &doc(&["a", "b"])), [2]);
    assert_eq!(changed_lines(&base_lines("a\nb"), &doc(&[""])), [1]);
}

#[test]
fn a_replaced_run_marks_each_new_line_once() {
    assert_eq!(changed_lines(&base_lines("a\nb\nc\nd"), &doc(&["a", "X", "Y", "Z", "d"])), [2, 3, 4]);
}

#[test]
fn the_same_text_marks_nothing_and_a_new_file_marks_every_line() {
    assert!(changed_lines(&base_lines("a\nb"), &doc(&["a", "b"])).is_empty());
    assert_eq!(changed_lines(&base_lines(""), &doc(&["a", "b"])), [1, 2]);
}

#[test]
fn base_lines_split_on_any_break() {
    assert_eq!(base_lines("a\r\nb\rc\nd"), ["a", "b", "c", "d"]);
    assert_eq!(base_lines("a\n"), ["a", ""]);
    assert_eq!(base_lines(""), [""]);
}
