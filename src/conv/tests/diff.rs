//! `lib/__tests__/diff-parser.test.ts` and the row logic of `components/session/__tests__/DiffView.test.ts`, under the
//! same names (snake_case), plus the diff sources of `cardDiffLines` and `fileEditsOf`.
use crate::conv::diff::{DiffLine, Kind, Row, card_diff_lines, expand_label, file_edits_of, parse_diff_lines, parse_unified_diff, rows};
use serde_json::json;
use std::collections::HashSet;

fn ctx(c: &str, o: u32, n: u32) -> DiffLine {
    DiffLine { kind: Kind::Context, content: c.into(), old: Some(o), new: Some(n) }
}
fn rem(c: &str, o: u32) -> DiffLine {
    DiffLine { kind: Kind::Remove, content: c.into(), old: Some(o), new: None }
}
fn add(c: &str, n: u32) -> DiffLine {
    DiffLine { kind: Kind::Add, content: c.into(), old: None, new: Some(n) }
}
fn shape(lines: &[DiffLine]) -> Vec<(Kind, String, Option<u32>, Option<u32>)> {
    lines.iter().map(|l| (l.kind, l.content.clone(), l.old, l.new)).collect()
}

mod parse_diff_lines_tests {
    use super::*;

    #[test]
    fn returns_all_context_lines_for_identical_content() {
        let lines = parse_diff_lines("first\nsecond\nthird", "first\nsecond\nthird");
        assert_eq!(lines, vec![ctx("first", 1, 1), ctx("second", 2, 2), ctx("third", 3, 3)]);
    }

    #[test]
    fn returns_added_lines_for_added_file() {
        assert_eq!(parse_diff_lines("", "first\nsecond"), vec![add("first", 1), add("second", 2)]);
    }

    #[test]
    fn returns_removed_lines_for_deleted_file() {
        assert_eq!(parse_diff_lines("first\nsecond", ""), vec![rem("first", 1), rem("second", 2)]);
    }

    #[test]
    fn returns_mixed_adds_removes_and_context_for_modified_file() {
        let lines = parse_diff_lines("alpha\nbeta\ngamma\nepsilon", "alpha\nbeta changed\ngamma\ndelta\nepsilon");
        assert_eq!(
            lines,
            vec![ctx("alpha", 1, 1), rem("beta", 2), add("beta changed", 2), ctx("gamma", 3, 3), add("delta", 4), ctx("epsilon", 4, 5)]
        );
    }
}

mod parse_unified_diff_real_git_patches {
    use super::*;

    #[test]
    fn reads_removed_and_added_lines_that_look_like_file_headers_as_lines() {
        let patch = [
            "diff --git a/f.txt b/f.txt",
            "index e683d13..38f6880 100644",
            "--- a/f.txt",
            "+++ b/f.txt",
            "@@ -2,6 +2,6 @@ a",
            " ",
            " b",
            " - bullet",
            "--- sql comment",
            "-++ x",
            "+new",
            "+++ x2",
            " c",
            "",
        ]
        .join("\n");
        let parsed = parse_unified_diff(&patch);
        assert_eq!(parsed.paths, vec!["f.txt"]);
        assert!(!parsed.created);
        assert_eq!(
            shape(&parsed.lines),
            vec![
                (Kind::Context, "".into(), Some(2), Some(2)),
                (Kind::Context, "b".into(), Some(3), Some(3)),
                (Kind::Context, "- bullet".into(), Some(4), Some(4)),
                (Kind::Remove, "-- sql comment".into(), Some(5), None),
                (Kind::Remove, "++ x".into(), Some(6), None),
                (Kind::Add, "new".into(), None, Some(5)),
                (Kind::Add, "++ x2".into(), None, Some(6)),
                (Kind::Context, "c".into(), Some(7), Some(7)),
            ]
        );
    }

    #[test]
    fn keeps_line_numbers_when_a_blank_context_line_is_an_empty_line() {
        // git -c diff.suppressBlankEmpty=true prints a blank context line as "" instead of " ".
        let patch = ["--- a/f.txt", "+++ b/f.txt", "@@ -2,4 +2,4 @@ a", "", " b", "-old", "+new", " c", ""].join("\n");
        assert_eq!(
            shape(&parse_unified_diff(&patch).lines),
            vec![
                (Kind::Context, "".into(), Some(2), Some(2)),
                (Kind::Context, "b".into(), Some(3), Some(3)),
                (Kind::Remove, "old".into(), Some(4), None),
                (Kind::Add, "new".into(), None, Some(4)),
                (Kind::Context, "c".into(), Some(5), Some(5)),
            ]
        );
    }

    #[test]
    fn keeps_the_lines_of_a_deleted_file_that_start_with_a_dash_or_a_space() {
        let patch = ["--- a/g.txt", "+++ /dev/null", "@@ -1,3 +0,0 @@", "-  indented", "--- item", "-- item", ""].join("\n");
        let contents: Vec<String> = parse_unified_diff(&patch).lines.into_iter().map(|l| l.content).collect();
        assert_eq!(contents, vec!["  indented", "-- item", "- item"]);
    }
}

mod parse_unified_diff_paths {
    use super::*;

    #[test]
    fn names_the_file_without_a_prefix_for_each_way_git_writes_one() {
        for (name, old, new) in [
            ("default", "a/src/f.ts", "b/src/f.ts"),
            ("diff.mnemonicPrefix (commit to worktree)", "c/src/f.ts", "w/src/f.ts"),
            ("diff.mnemonicPrefix (index to worktree)", "i/src/f.ts", "w/src/f.ts"),
            ("diff.mnemonicPrefix (commit to index)", "c/src/f.ts", "i/src/f.ts"),
            ("diff.noprefix", "src/f.ts", "src/f.ts"),
        ] {
            let parsed = parse_unified_diff(&format!("--- {old}\n+++ {new}\n@@ -1 +1 @@\n-a\n+b\n"));
            assert_eq!(parsed.paths, vec!["src/f.ts"], "{name}");
        }
    }

    #[test]
    fn names_a_new_file_under_mnemonic_prefixes() {
        let parsed = parse_unified_diff("--- /dev/null\n+++ w/h.txt\n@@ -0,0 +1 @@\n+x\n");
        assert!(parsed.created);
        assert_eq!(parsed.paths, vec!["h.txt"]);
    }

    #[test]
    fn keeps_a_folder_named_like_a_prefix_when_git_writes_no_prefix() {
        assert_eq!(parse_unified_diff("--- w/f.ts\n+++ w/f.ts\n@@ -1 +1 @@\n-a\n+b\n").paths, vec!["w/f.ts"]);
    }

    #[test]
    fn reads_opencodes_apply_patch_headers() {
        let parsed = parse_unified_diff("*** Add File: notes/todo.md\n+one\n*** Update File: src/a.ts\n");
        assert!(parsed.created);
        assert_eq!(parsed.paths, vec!["notes/todo.md", "src/a.ts"]);
    }
}

mod parse_diff_lines_on_big_and_random_inputs {
    use super::*;

    fn rebuild(lines: &[DiffLine]) -> (Vec<String>, Vec<String>) {
        let side = |skip: Kind| lines.iter().filter(|l| l.kind != skip).map(|l| l.content.clone()).collect();
        (side(Kind::Add), side(Kind::Remove))
    }

    #[test]
    fn always_rebuilds_both_sides_from_the_lines_for_random_edits() {
        let mut seed: i64 = 7;
        let mut rnd = move || {
            seed = (seed * 1_103_515_245 + 12_345) & 0x7fff_ffff;
            seed as f64 / 2_147_483_647.0
        };
        for _ in 0..200 {
            let len = (rnd() * 30.0) as usize;
            let before: Vec<String> = (0..len).map(|_| format!("l{}", (rnd() * 8.0) as u32)).collect();
            let mut after: Vec<String> = before.iter().filter(|_| rnd() > 0.3).cloned().collect();
            let inserts = (rnd() * 6.0) as usize;
            for _ in 0..inserts {
                let at = (rnd() * (after.len() + 1) as f64) as usize;
                after.insert(at, format!("n{}", (rnd() * 5.0) as u32));
            }
            let lines = parse_diff_lines(&before.join("\n"), &after.join("\n"));
            assert_eq!(rebuild(&lines), (before, after));
        }
    }

    #[test]
    fn diffs_an_8000_line_file_with_over_a_thousand_changed_lines_quickly() {
        let before: Vec<String> = (0..8000).map(|i| format!("    \"pkg-{i}\": {{ \"version\": \"1.0.{i}\" }},")).collect();
        let after: Vec<String> =
            before.iter().enumerate().map(|(i, l)| if i % 7 == 0 { l.replace("1.0.", "2.1.") } else { l.clone() }).collect();
        let started = std::time::Instant::now();
        let lines = parse_diff_lines(&before.join("\n"), &after.join("\n"));
        // Fleet's bound is 500 ms in a browser; a debug build here gets four times that.
        assert!(started.elapsed().as_millis() < 10_000, "took {:?}", started.elapsed());
        assert_eq!(lines.iter().filter(|l| l.kind == Kind::Add).count(), 1143);
        assert_eq!(rebuild(&lines), (before, after));
    }
}

mod diff_view {
    use super::*;

    #[test]
    fn renders_line_number_gutters_markers_and_line_type_metadata() {
        let lines = vec![ctx("const value = 1;", 1, 1), rem("const removed = true;", 2), add("const added = true;", 2)];
        let made = rows(&lines, &HashSet::new());
        assert_eq!(made, vec![Row::Line { index: 0 }, Row::Line { index: 1 }, Row::Line { index: 2 }]);
        // The gutter cells: blank on the side a line isn't on.
        assert_eq!((lines[1].old, lines[1].new), (Some(2), None));
        assert_eq!((lines[2].old, lines[2].new), (None, Some(2)));
        assert_eq!(crate::view::diff::marker(Kind::Remove), "-");
        assert_eq!(crate::view::diff::marker(Kind::Add), "+");
        assert_eq!(crate::view::diff::marker(Kind::Context), "");
    }

    #[test]
    fn collapses_large_context_runs_and_expands_them_on_request() {
        let lines: Vec<DiffLine> = (1..=12).map(|n| ctx(&format!("unchanged {n}"), n, n)).collect();
        let made = rows(&lines, &HashSet::new());
        // Three lines, the expander, three lines: six line rows.
        assert_eq!(made.iter().filter(|r| matches!(r, Row::Line { .. })).count(), 6);
        assert_eq!(made[3], Row::Expand { id: "0-12".into(), hidden: 6 });
        assert_eq!(expand_label(6), "Show 6 unchanged lines");
        let shown: Vec<usize> = made.iter().filter_map(|r| if let Row::Line { index } = r { Some(*index) } else { None }).collect();
        assert!(!shown.contains(&3)); // "unchanged 4"

        let open: HashSet<String> = ["0-12".to_string()].into();
        let made = rows(&lines, &open);
        assert_eq!(made.len(), 12);
        assert!(made.iter().all(|r| matches!(r, Row::Line { .. })));
    }

    #[test]
    fn never_collapses_a_run_of_ten_or_fewer_and_says_line_for_one() {
        let ten: Vec<DiffLine> = (1..=10).map(|n| ctx("x", n, n)).collect();
        assert_eq!(rows(&ten, &HashSet::new()).len(), 10);
        let eleven: Vec<DiffLine> = (1..=11).map(|n| ctx("x", n, n)).collect();
        assert_eq!(rows(&eleven, &HashSet::new())[3], Row::Expand { id: "0-11".into(), hidden: 5 });
        assert_eq!(expand_label(1), "Show 1 unchanged line");
    }
}

mod diff_sources {
    use super::*;

    #[test]
    fn reads_an_array_on_the_state_and_infers_the_type_from_a_leading_sign() {
        let state = json!({"diffLines": [{"content": "+new"}, {"content": "-old"}, {"content": " same"}, {"type": "context", "text": "kept", "oldLineNumber": 3}]});
        let kinds: Vec<Kind> = card_diff_lines(&state).iter().map(|l| l.kind).collect();
        assert_eq!(kinds, vec![Kind::Add, Kind::Remove, Kind::Context, Kind::Context]);
    }

    #[test]
    fn falls_back_to_the_unified_diff_kept_in_metadata() {
        let state = json!({"metadata": {"diff": "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+b\n"}});
        assert_eq!(card_diff_lines(&state), vec![rem("a", 1), add("b", 1)]);
    }

    #[test]
    fn a_write_that_failed_or_has_not_finished_wrote_nothing() {
        let input = json!({"filePath": "a.ts", "content": "x"});
        assert!(file_edits_of("write", &json!({"status": "error", "input": input})).is_empty());
        assert!(file_edits_of("write", &json!({"status": "running", "input": input})).is_empty());
    }

    #[test]
    fn a_write_with_only_content_is_all_added_lines_of_the_whole_file() {
        let state = json!({"status": "completed", "input": {"filePath": "a.ts", "content": "one\ntwo\n"}});
        let [edit] = &file_edits_of("write", &state)[..] else { panic!("one edit") };
        assert_eq!((edit.additions, edit.deletions, edit.created, edit.whole_file), (2, 0, true, true));
        assert_eq!(edit.diff, vec![add("one", 1), add("two", 2)]);
    }

    #[test]
    fn an_edit_with_old_and_new_strings_shows_only_what_changed() {
        let state = json!({"status": "completed", "input": {"filePath": "a.ts", "oldString": "a\nb\nc", "newString": "a\nB\nc"}});
        let [edit] = &file_edits_of("edit", &state)[..] else { panic!("one edit") };
        assert_eq!(edit.diff, vec![rem("b", 2), add("B", 2)]);
    }

    #[test]
    fn one_patch_with_several_files_is_one_edit_each() {
        let patch = "--- a/x.ts\n+++ b/x.ts\n@@ -1 +1 @@\n-a\n+b\n--- /dev/null\n+++ b/y.ts\n@@ -0,0 +1 @@\n+c\n";
        let state = json!({"status": "completed", "input": {"filePath": "x.ts"}, "metadata": {"diff": patch}});
        let edits = file_edits_of("edit", &state);
        assert_eq!(edits.iter().map(|e| (e.path.as_str(), e.created)).collect::<Vec<_>>(), vec![("x.ts", false), ("y.ts", true)]);
    }
}
