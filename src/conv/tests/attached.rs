//! `lib/__tests__/attached-files.test.ts` ("attached files"; the big-paste cases belong to the composer).
use crate::conv::attached::*;

fn log() -> AttachedFile {
    AttachedFile {
        path: ".weave/attachments/3f9a2c1d/build-output.log".into(),
        name: "build-output.log".into(),
        size: 188_416,
        lines: Some(2031),
    }
}
fn pdf() -> AttachedFile {
    AttachedFile {
        path: ".weave/attachments/3f9a2c1d/incident-report.pdf".into(),
        name: "incident-report.pdf".into(),
        size: 421_888,
        lines: None,
    }
}

#[test]
fn formats_sizes() {
    assert_eq!(format_file_size(900), "900 B");
    assert_eq!(format_file_size(188_416), "184 KB");
    assert_eq!(format_file_size(1_572_864), "1.5 MB");
}

#[test]
fn labels_a_chip_by_extension() {
    assert_eq!(file_extension_label("a.log"), "LOG");
    assert_eq!(file_extension_label("Makefile"), "FILE");
    assert_eq!(file_extension_label("a.markdown"), "MARK");
    assert_eq!(file_extension_label(".gitignore"), "FILE");
}

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

#[test]
fn says_line_for_one_line_and_file_for_no_extension() {
    assert!(format_attached_note(&[AttachedFile { lines: Some(1), size: 10, ..log() }]).contains("(1 line, 10 B)"));
    assert!(format_attached_note(&[AttachedFile { name: "dump".into(), size: 3072, ..pdf() }]).contains("(File, 3 KB)"));
}

#[test]
fn puts_the_note_after_the_typed_text() {
    assert_eq!(with_attached_note("Why does it fail?  \n", &[log()]), format!("Why does it fail?\n\n{}", format_attached_note(&[log()])));
    assert_eq!(with_attached_note("", &[log()]), format_attached_note(&[log()]));
    assert_eq!(with_attached_note("hi", &[]), "hi");
}

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

#[test]
fn parses_a_message_that_is_only_the_note_and_leaves_other_text_alone() {
    assert_eq!(parse_attached_note(&format_attached_note(&[log()])).0, "");
    assert_eq!(parse_attached_note("Attached: nothing here"), ("Attached: nothing here".to_string(), vec![]));
}
