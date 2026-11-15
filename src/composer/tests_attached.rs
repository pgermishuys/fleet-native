//! `attached-files.test.ts`, `format-terminal-context.test.ts` and `use-draft-terminal-context.test.ts`, under Fleet's names.
use super::attached::*;

fn log() -> AttachedFile {
    AttachedFile {
        path: ".weave/attachments/3f9a2c1d/build-output.log".into(),
        name: "build-output.log".into(),
        size: 188416,
        lines: Some(2031),
        mime: "text/plain".into(),
    }
}

fn pdf() -> AttachedFile {
    AttachedFile {
        path: ".weave/attachments/3f9a2c1d/incident-report.pdf".into(),
        name: "incident-report.pdf".into(),
        size: 421888,
        lines: None,
        mime: "application/pdf".into(),
    }
}

// attached files: "formats sizes"
#[test]
fn formats_sizes() {
    assert_eq!(format_file_size(900), "900 B");
    assert_eq!(format_file_size(188416), "184 KB");
    assert_eq!(format_file_size(1_572_864), "1.5 MB");
}

// "labels a chip by extension"
#[test]
fn labels_a_chip_by_extension() {
    assert_eq!(file_extension_label("a.log"), "LOG");
    assert_eq!(file_extension_label("Makefile"), "FILE");
    assert_eq!(file_extension_label("archive.tar.gzip"), "GZIP");
    assert_eq!(file_extension_label("notes.markdown"), "MARK", "at most four characters");
}

// "writes the note for one and for many files"
#[test]
fn writes_the_note_for_one_and_for_many_files() {
    assert_eq!(
        format_attached_note(&[log()]),
        "Attached: .weave/attachments/3f9a2c1d/build-output.log (2,031 lines, 184 KB)\nRead it as needed."
    );
    assert_eq!(
        format_attached_note(&[log(), pdf()]),
        "Attached: .weave/attachments/3f9a2c1d/build-output.log (2,031 lines, 184 KB)\n\
         Attached: .weave/attachments/3f9a2c1d/incident-report.pdf (PDF, 412 KB)\nRead them as needed."
    );
}

// "says line for one line and File for no extension"
#[test]
fn says_line_for_one_line_and_file_for_no_extension() {
    assert!(format_attached_note(&[AttachedFile { lines: Some(1), size: 10, ..log() }]).contains("(1 line, 10 B)"));
    assert!(format_attached_note(&[AttachedFile { name: "dump".into(), size: 3072, ..pdf() }]).contains("(File, 3 KB)"));
}

// "puts the note after the typed text"
#[test]
fn puts_the_note_after_the_typed_text() {
    assert_eq!(with_attached_note("Why does it fail?  \n", &[log()]), format!("Why does it fail?\n\n{}", format_attached_note(&[log()])));
    assert_eq!(with_attached_note("", &[log()]), format_attached_note(&[log()]));
    assert_eq!(with_attached_note("hi", &[]), "hi");
}

// "parses what it wrote"
#[test]
fn parses_what_it_wrote() {
    let (text, files) = parse_attached_note(&with_attached_note("Why does it fail?", &[log(), pdf()]));
    assert_eq!(text, "Why does it fail?");
    assert_eq!(
        files,
        [
            AttachedFileRef { path: log().path, name: "build-output.log".into(), details: "2,031 lines, 184 KB".into() },
            AttachedFileRef { path: pdf().path, name: "incident-report.pdf".into(), details: "PDF, 412 KB".into() },
        ]
    );
}

// "parses a message that is only the note, and leaves other text alone"
#[test]
fn parses_a_message_that_is_only_the_note_and_leaves_other_text_alone() {
    assert_eq!(parse_attached_note(&format_attached_note(&[log()])).0, "");
    assert_eq!(parse_attached_note("Attached: nothing here"), ("Attached: nothing here".to_string(), vec![]));
    // A note's line with a space in the path isn't one, and a "Read it" with no file before it is only text.
    assert_eq!(parse_attached_note("Read it as needed.").1, vec![]);
    assert_eq!(parse_attached_note("a\nAttached: two words (x)\nRead it as needed.").1, vec![]);
}

// big paste helpers: "calls 2,000 characters or 25 lines big"
#[test]
fn calls_2000_characters_or_25_lines_big() {
    assert!(!is_big_paste(&"x".repeat(1999)));
    assert!(is_big_paste(&"x".repeat(2000)));
    assert!(!is_big_paste(&format!("{}a", "a\n".repeat(23))));
    assert!(is_big_paste(&format!("{}a", "a\n".repeat(24))));
}

// "counts lines the way the server does"
#[test]
fn counts_lines_the_way_the_server_does() {
    assert_eq!(count_lines(""), 0);
    assert_eq!(count_lines("a\nb"), 2);
    assert_eq!(count_lines("a\nb\n"), 2);
}

// "fences a file longer than any backtick run inside"
#[test]
fn fences_a_file_longer_than_any_backtick_run_inside() {
    assert_eq!(inline_file_block("a.txt", "hi\n"), "a.txt:\n```\nhi\n```");
    assert_eq!(inline_file_block("a.txt", "```x```"), "a.txt:\n````\n```x```\n````");
}

// "names pasted files without clashes and describes a draft file"
#[test]
fn names_pasted_files_without_clashes_and_describes_a_draft_file() {
    assert_eq!(pasted_file_name(&["pasted-text.txt"]), "pasted-text-2.txt");
    assert_eq!(pasted_file_name(&[]), "pasted-text.txt");
    assert_eq!(describe_draft_file(190464, Some(2031)), "2,031 lines · 186 KB");
    assert_eq!(describe_draft_file(512, None), "512 B");
}

#[test]
fn describes_a_paste_by_lines_or_by_characters() {
    assert_eq!(describe_paste(&"line\n".repeat(30)), "30 lines pasted");
    assert_eq!(describe_paste(&"x".repeat(2500)), "2,500 characters pasted");
}

#[test]
fn a_text_file_is_known_by_its_type_or_one_of_nineteen_extensions() {
    assert!(is_text_file("anything", "text/plain") && is_text_file("a", "application/json") && is_text_file("a", "application/xml"));
    assert!(is_text_file("Build.LOG", "") && is_text_file("x.sh", "") && !is_text_file("x.pdf", "application/pdf"));
    assert!(!is_text_file(".log", ""), "a dot first is no extension");
}

fn draft(name: &str, size: u64, pasted: bool) -> DraftFile {
    DraftFile {
        id: "f".into(),
        name: name.into(),
        mime: if name.ends_with(".txt") { "text/plain".into() } else { String::new() },
        size,
        lines: None,
        pasted,
        source: FileSource::Text("".into()),
    }
}

#[test]
fn a_small_text_file_goes_inline_and_a_pasted_one_or_a_big_one_uploads() {
    assert!(draft("notes.txt", 4095, false).goes_inline());
    assert!(!draft("notes.txt", 4096, false).goes_inline());
    assert!(!draft("pasted-text.txt", 10, true).goes_inline());
    assert!(!draft("photo.pdf", 10, false).goes_inline());
}

fn context(from: usize, to: usize, label: &str, text: &str) -> TerminalDraft {
    TerminalDraft { id: format!("c{from}-{to}"), terminal_id: "t1".into(), label: label.into(), from, to, text: text.into() }
}

// format-terminal-context.test.ts: "names one line or a range"
#[test]
fn names_one_line_or_a_range() {
    assert_eq!(terminal_line_range(3, 3), "line 3");
    assert_eq!(terminal_line_range(9, 11), "lines 9–11");
}

// "leaves a message with nothing attached alone"
#[test]
fn leaves_a_message_with_nothing_attached_alone() {
    assert_eq!(format_terminal_context(&[], "hello"), "hello");
}

// "puts each attachment in a labelled block before the message"
#[test]
fn puts_each_attachment_in_a_labelled_block_before_the_message() {
    let text = format_terminal_context(&[context(9, 11, "zsh", "one\ntwo"), context(1, 1, "zsh 2", "ready")], "  Why?  ");
    assert_eq!(text, "Terminal zsh, lines 9–11:\n```text\none\ntwo\n```\n\nTerminal zsh 2, line 1:\n```text\nready\n```\n\nWhy?");
}

// "sends just the blocks when nothing was typed"
#[test]
fn sends_just_the_blocks_when_nothing_was_typed() {
    assert_eq!(format_terminal_context(&[context(9, 11, "zsh", "one\ntwo")], "   "), "Terminal zsh, lines 9–11:\n```text\none\ntwo\n```");
}

// "uses a longer fence when the output has one of its own"
#[test]
fn uses_a_longer_fence_when_the_output_has_one_of_its_own() {
    assert_eq!(
        format_terminal_context(&[context(9, 11, "zsh", "```js\nx\n```")], ""),
        "Terminal zsh, lines 9–11:\n````text\n```js\nx\n```\n````"
    );
}

// use-draft-terminal-context.test.ts: "keeps each session's lines apart, and survives a new composable"
#[test]
fn keeps_each_sessions_lines_apart() {
    let mut held = Held::default();
    held.add_terminal("s1", context(9, 11, "zsh", "FAIL"));
    assert_eq!(held.terminals("s1").iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["zsh"]);
    assert!(held.terminals("s2").is_empty());
}

// "adds the same lines of the same terminal once"
#[test]
fn adds_the_same_lines_of_the_same_terminal_once() {
    let mut held = Held::default();
    held.add_terminal("s1", context(9, 11, "zsh", "FAIL"));
    held.add_terminal("s1", context(9, 11, "zsh", "FAIL again"));
    held.add_terminal("s1", context(12, 12, "zsh", "FAIL"));
    assert_eq!(held.terminals("s1").len(), 2);
}

// "removes one, or clears them all"
#[test]
fn removes_one_or_clears_them_all() {
    let mut held = Held::default();
    held.add_terminal("s1", context(9, 11, "zsh", "FAIL"));
    held.add_terminal("s1", context(12, 12, "zsh", "FAIL"));
    let first = held.terminals("s1")[0].id.clone();
    held.remove_terminal("s1", &first);
    assert_eq!(held.terminals("s1").iter().map(|c| c.from).collect::<Vec<_>>(), [12]);
    held.clear_terminals("s1");
    assert!(held.terminals("s1").is_empty());
}

#[test]
fn a_refused_prompts_lines_come_back_once_and_a_delivered_ones_do_not() {
    let mut held = Held::default();
    held.add_terminal("s1", context(3, 3, "zsh", "x"));
    assert_eq!(held.take_terminals_for_send("s1", "p1", "why?").len(), 1);
    assert!(held.terminals("s1").is_empty());
    assert_eq!(held.refused("p1"), Some(("s1".to_string(), "why?".to_string())));
    assert_eq!(held.terminals("s1").len(), 1);
    assert_eq!(held.refused("p1"), None);
    held.take_terminals_for_send("s1", "p2", "again");
    held.delivered("p2");
    assert_eq!(held.refused("p2"), None);
}
