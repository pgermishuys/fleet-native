//! `lib/__tests__/changes-find.test.ts`, under the same names (snake_case): the logic of find in Changes. `markChangesMatches` (2) is
//! piece K's, which draws the marks.
use crate::conv::changes_find::{FindFile, FindLine, find_changes_hits, find_lines, selected_query, step_shortcut};
use crate::conv::diff::parse_diff_lines;
use crate::conv::review::{ReviewItem, build_review};
use rune::{Key, KeyEvent, Mods};

fn numbered(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("line {i}")).collect()
}

fn line(index: usize, text: &str, item: &str, in_fold: bool) -> FindLine {
    FindLine { index, text: text.into(), item_id: item.into(), in_fold }
}

mod find_lines_tests {
    use super::*;

    #[test]
    fn lists_a_files_hunk_and_folded_lines_in_drawn_order_saying_which_are_folded() {
        let before = numbered(30).join("\n");
        let after = before.replace("line 3\n", "line 3!\n").replace("line 28\n", "line 28!\n");
        let items = build_review(&parse_diff_lines(&before, &after));
        let lines = find_lines(&items);

        let mut sorted: Vec<usize> = lines.iter().map(|l| l.index).collect();
        sorted.sort();
        assert_eq!(lines.iter().map(|l| l.index).collect::<Vec<_>>(), sorted);
        let folded: Vec<&FindLine> = lines.iter().filter(|l| l.in_fold).collect();
        assert_eq!(folded.len(), 18);
        assert!(folded.iter().all(|l| l.item_id == items[1].id()));
        assert!(matches!(items[1], ReviewItem::Fold(_)));
        assert!(!lines.iter().find(|l| l.text == "line 3!").expect("the changed line").in_fold);
    }
}

mod find_changes_hits_tests {
    use super::*;

    fn files() -> Vec<FindFile> {
        vec![
            FindFile { path: "a.ts".into(), lines: vec![line(0, "const Note = note(note);", "h0", false)] },
            FindFile { path: "b.ts".into(), lines: vec![line(4, "nothing", "f1", true), line(5, "NOTE", "f1", true)] },
        ]
    }

    #[test]
    fn finds_every_match_ignoring_case_file_by_file_and_left_to_right() {
        let hits = find_changes_hits(&files(), "note");
        let shape: Vec<(&str, usize, usize)> = hits.iter().map(|h| (h.path.as_str(), h.line, h.ordinal)).collect();
        assert_eq!(shape, [("a.ts", 0, 0), ("a.ts", 0, 1), ("a.ts", 0, 2), ("b.ts", 5, 0)]);
        assert_eq!((hits[3].item_id.as_str(), hits[3].in_fold), ("f1", true));
    }

    #[test]
    fn finds_nothing_for_an_empty_or_blank_query() {
        assert!(find_changes_hits(&files(), "").is_empty());
        assert!(find_changes_hits(&files(), "   ").is_empty());
    }

    #[test]
    fn does_not_count_overlapping_matches_twice() {
        let one = vec![FindFile { path: "x".into(), lines: vec![line(0, "aaaa", "h", false)] }];
        assert_eq!(find_changes_hits(&one, "aa").len(), 2);
    }
}

mod step_shortcut_tests {
    use super::*;

    fn key(key: Key, f: impl FnOnce(&mut Mods)) -> KeyEvent {
        let mut mods = Mods::default();
        f(&mut mods);
        KeyEvent { key, mods }
    }
    fn ctrl(m: &mut Mods) {
        m.ctrl = true;
        m.cmd = true;
    }

    #[test]
    fn steps_forward_on_f3_and_ctrl_g_back_with_shift() {
        assert_eq!(step_shortcut(&key(Key::Function(3), |_| {}), false), 1);
        assert_eq!(step_shortcut(&key(Key::Function(3), |m| m.shift = true), false), -1);
        assert_eq!(step_shortcut(&key(Key::Command('g'), ctrl), false), 1);
        assert_eq!(
            step_shortcut(
                &key(Key::Command('g'), |m| {
                    ctrl(m);
                    m.shift = true;
                }),
                false
            ),
            -1
        );
    }

    #[test]
    fn uses_cmd_on_a_mac_and_ignores_other_keys() {
        assert_eq!(step_shortcut(&key(Key::Command('g'), |m| m.cmd = true), true), 1);
        assert_eq!(step_shortcut(&key(Key::Command('g'), |m| m.ctrl = true), true), 0);
        assert_eq!(step_shortcut(&key(Key::Command('g'), |_| {}), false), 0);
        assert_eq!(
            step_shortcut(
                &key(Key::Command('g'), |m| {
                    ctrl(m);
                    m.alt = true;
                }),
                false
            ),
            0
        );
        assert_eq!(step_shortcut(&key(Key::Function(4), |_| {}), false), 0);
    }
}

mod selected_query_tests {
    use super::*;

    #[test]
    fn takes_a_one_line_selection_inside_the_root_and_nothing_from_outside_it() {
        // The caller says what is selected inside Changes: a selection elsewhere is `None`.
        assert_eq!(selected_query(Some("title")), "title");
        assert_eq!(selected_query(None), "");
    }

    #[test]
    fn leaves_out_a_selection_over_a_line_or_over_200_characters() {
        assert_eq!(selected_query(Some("one\ntwo")), "");
        assert_eq!(selected_query(Some(&"x".repeat(200))), "x".repeat(200));
        assert_eq!(selected_query(Some(&"x".repeat(201))), "");
    }
}
