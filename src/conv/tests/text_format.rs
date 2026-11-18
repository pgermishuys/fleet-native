//! `text-format.test.ts` (the round trips and the two read cases) and the four `minimalChange` cases of `file-buffer.test.ts`,
//! under Fleet's names. "editing keeps the file's line breaks" (3 cases) waits for slice 2's buffer edits (piece Q).
use crate::conv::minimal_change::minimal_change;
use crate::conv::text_format::{LineSeparator, TextFormat, join_lines, read_text_format, split_lines, to_disk_text};

/// Fleet's `decode` keeps a BOM as U+FEFF, as the server's UTF8Encoding does.
fn round_trip(bytes: &[u8]) -> Vec<u8> {
    let raw = std::str::from_utf8(bytes).unwrap();
    let (text, format) = read_text_format(raw);
    let lines = split_lines(&text, &format);
    to_disk_text(&join_lines(&lines, &format), &format).into_bytes()
}

fn open(raw: &str) -> (TextFormat, Vec<String>) {
    let (text, format) = read_text_format(raw);
    let lines = split_lines(&text, &format);
    (format, lines)
}

#[test]
fn keeps_the_bytes_of_every_file_layout() {
    let cases = [
        ("LF with a trailing newline", "one\ntwo\n"),
        ("LF without a trailing newline", "one\ntwo"),
        ("CRLF", "one\r\ntwo\r\n"),
        ("CRLF without a trailing newline", "one\r\ntwo"),
        ("old Mac CR", "one\rtwo\r"),
        ("mixed, mostly CRLF", "a\r\nb\nc\r\nd\r\n"),
        ("mixed, mostly LF", "a\nb\r\nc\nd\n"),
        ("LF with a stray CR", "a\rb\nc\n"),
        ("BOM + CRLF", "\u{FEFF}one\r\ntwo\r\n"),
        ("BOM, no newline", "\u{FEFF}x"),
        ("empty", ""),
        ("only a newline", "\n"),
        ("blank lines at the end", "a\n\n\n"),
        ("non-ASCII", "naïve — 日本 🎉\r\n"),
    ];
    for (name, raw) in cases {
        assert_eq!(round_trip(raw.as_bytes()), raw.as_bytes(), "keeps the bytes of {name}");
    }
}

#[test]
fn keeps_the_bom_out_of_the_editor_text() {
    let (format, lines) = open("\u{FEFF}hello\r\n");
    assert_eq!(format, TextFormat { line_separator: LineSeparator::Crlf, bom: true });
    assert_eq!(lines[0], "hello");
}

#[test]
fn splits_a_crlf_file_into_lines() {
    let (format, lines) = open("a\r\nb\r\nc");
    assert_eq!(lines.len(), 3);
    assert_eq!(format.line_separator.as_str(), "\r\n");
}

#[test]
fn an_untouched_mixed_file_keeps_its_odd_breaks_inside_its_lines() {
    // The lines are split on the file's own break only, so the lone LF stays a character of line 2.
    let (_, lines) = open("a\r\nb\nc\r\n");
    assert_eq!(lines, ["a", "b\nc", ""]);
}

#[test]
fn ties_and_files_without_breaks_use_lf() {
    assert_eq!(open("a\nb\r\nc").0.line_separator, LineSeparator::Lf);
    assert_eq!(open("abc").0.line_separator, LineSeparator::Lf);
    assert_eq!(open("a\rb\r").0.line_separator, LineSeparator::Cr);
}

// ─── minimalChange ──────────────────────────────────────────────────────────

fn text(s: &str) -> Vec<String> {
    s.split('\n').map(str::to_string).collect()
}

#[test]
fn minimal_change_replaces_only_what_differs() {
    let change = minimal_change(&text("a\nb\nc\nd"), &text("a\nB\nc\nd")).unwrap();
    assert_eq!((change.from, change.to), (2, 3));
    assert_eq!(change.insert, ["B"]);
    assert_eq!(change.lines, (2, 2));
}

#[test]
fn minimal_change_handles_inserted_and_removed_lines() {
    assert_eq!(minimal_change(&text("a\nc"), &text("a\nb\nc")).unwrap().lines, (2, 3));
    let removal = minimal_change(&text("a\nb\nc"), &text("a\nc")).unwrap();
    assert_eq!(removal.insert_len(), 0);
}

#[test]
fn minimal_change_is_null_for_the_same_text() {
    assert_eq!(minimal_change(&text("x\ny"), &text("x\ny")), None);
}

#[test]
fn minimal_change_keeps_the_cursor_where_it_was_when_the_change_is_elsewhere() {
    // "one\ntwo\nthree", the cursor at 11 (inside "three"), and the first line changes case.
    let change = minimal_change(&text("one\ntwo\nthree"), &text("ONE\ntwo\nthree")).unwrap();
    assert_eq!(change.map(11), 11);
    assert_eq!(change.apply(&text("one\ntwo\nthree")), text("ONE\ntwo\nthree"));
    // A change before the cursor that grows the text moves it along.
    let longer = minimal_change(&text("one\ntwo"), &text("one!!\ntwo")).unwrap();
    assert_eq!(longer.map(5), 7);
}
