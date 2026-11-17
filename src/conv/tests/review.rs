//! `lib/__tests__/diff-review.test.ts`, under the same names (snake_case).
use crate::conv::diff::{DiffLine, Kind, parse_diff_lines};
use crate::conv::review::{ReviewHunk, ReviewItem, build_review, marked_segments, split_rows, text_with_range_reverted, word_marks};

fn lines(n: usize, from: usize) -> Vec<String> {
    (0..n).map(|i| format!("line {}", i + from)).collect()
}
fn hunks(items: Vec<ReviewItem>) -> Vec<ReviewHunk> {
    items.into_iter().filter_map(|i| if let ReviewItem::Hunk(h) = i { Some(h) } else { None }).collect()
}
fn first_hunk(before: &str, after: &str) -> ReviewHunk {
    hunks(build_review(&parse_diff_lines(before, after))).remove(0)
}

mod build_review_tests {
    use super::*;

    #[test]
    fn has_no_items_when_nothing_changed() {
        assert!(build_review(&parse_diff_lines("a\nb\n", "a\nb\n")).is_empty());
    }

    #[test]
    fn keeps_three_lines_of_context_and_folds_the_unchanged_run_between_two_hunks() {
        let before = lines(30, 1).join("\n");
        let after = lines(30, 1)
            .into_iter()
            .map(|l| if l == "line 3" || l == "line 28" { format!("{l}!") } else { l })
            .collect::<Vec<_>>()
            .join("\n");
        let items = build_review(&parse_diff_lines(&before, &after));
        let kinds: Vec<_> = items.iter().map(|i| matches!(i, ReviewItem::Hunk(_))).collect();
        assert_eq!(kinds, [true, false, true]);
        let ReviewItem::Hunk(h) = &items[0] else { panic!() };
        let got: Vec<_> = h.lines.iter().map(|l| l.line.content.as_str()).collect();
        assert_eq!(got, ["line 1", "line 2", "line 3", "line 3!", "line 4", "line 5", "line 6"]);
        let ReviewItem::Fold(f) = &items[1] else { panic!() };
        assert_eq!(f.hidden, 18);
        assert_eq!(f.id, "fold-7");
    }

    #[test]
    fn folds_the_unchanged_tail_after_the_last_hunk() {
        let mut after = vec!["x".to_string()];
        after.extend(lines(19, 2));
        let items = build_review(&parse_diff_lines(&lines(20, 1).join("\n"), &after.join("\n")));
        let kinds: Vec<_> = items.iter().map(|i| matches!(i, ReviewItem::Hunk(_))).collect();
        assert_eq!(kinds, [true, false]);
    }

    #[test]
    fn names_a_hunk_after_the_nearest_line_at_the_margin_above_it() {
        let before = ["export interface Note {", "  id: string;", "  a: 1;", "  b: 2;", "  c: 3;", "  d: 4;", "  e: 5;", "}"].join("\n");
        let after = before.replace("  e: 5;", "  e: 6;");
        assert_eq!(first_hunk(&before, &after).name, "export interface Note {");
    }

    #[test]
    fn marks_only_the_changed_words_of_a_changed_line() {
        let hunk = first_hunk("const a = oldName(1);\n", "const a = newName(1);\n");
        let marked = |kind: Kind| {
            let l = hunk.lines.iter().find(|l| l.line.kind == kind).unwrap();
            marked_segments(&l.line.content, &l.marks).into_iter().filter(|s| s.1).map(|s| s.0.to_string()).collect::<Vec<_>>()
        };
        assert_eq!(marked(Kind::Remove), ["oldName"]);
        assert_eq!(marked(Kind::Add), ["newName"]);
    }

    #[test]
    fn a_hunk_starts_at_its_first_line_that_is_not_a_removal() {
        let hunk = first_hunk("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!((hunk.id.as_str(), hunk.start_line, hunk.from, hunk.to), ("hunk-0", 1, 0, 3));
        let top = first_hunk("gone\nx\n", "x\n");
        assert_eq!(top.start_line, 1);
    }

    #[test]
    fn a_long_name_is_cut_with_an_ellipsis() {
        let long = "a".repeat(80);
        let before = [long.clone(), "  1".into(), "  2".into(), "  3".into(), "  4".into(), "  5".into(), "  6".into()].join("\n");
        let after = before.replace("  6", "  7");
        let name = first_hunk(&before, &after).name;
        assert_eq!(name, format!("{}…", "a".repeat(63)));
    }

    #[test]
    fn a_rewrite_over_600_lines_gets_no_marks() {
        let before = (0..400).map(|i| format!("value {i} old")).collect::<Vec<_>>().join("\n");
        let after = (0..400).map(|i| format!("value {i} new")).collect::<Vec<_>>().join("\n");
        let hunk = first_hunk(&before, &after);
        assert!(hunk.lines.len() > 600);
        assert!(hunk.lines.iter().all(|l| l.marks.is_empty()));
    }
}

mod word_marks_tests {
    use super::*;

    #[test]
    fn marks_nothing_when_the_lines_share_nothing() {
        assert_eq!(word_marks("abc", "xyz"), (None, None));
    }

    #[test]
    fn marks_only_the_added_words_on_an_appended_line() {
        assert_eq!(word_marks("run(a)", "run(a, b)"), (None, Some((5, 8))));
    }

    #[test]
    fn marks_nothing_when_the_change_covers_most_of_the_line() {
        assert_eq!(word_marks("x = alpha beta gamma", "x = one two three"), (None, None));
    }

    #[test]
    fn marks_in_bytes_past_a_wide_character() {
        assert_eq!(word_marks("é old end", "é new end"), (Some((3, 6)), Some((3, 6))));
    }
}

mod split_rows_tests {
    use super::*;

    #[test]
    fn pairs_a_removal_with_the_addition_after_it_and_pads_the_shorter_side() {
        let hunk = first_hunk("a\nb\nc\n", "a\nB\nB2\nc\n");
        let rows = split_rows(&hunk.lines);
        let got: Vec<_> =
            rows.iter().map(|r| (r.left.map(|l| l.line.content.as_str()), r.right.map(|l| l.line.content.as_str()))).collect();
        assert_eq!(got, [(Some("a"), Some("a")), (Some("b"), Some("B")), (None, Some("B2")), (Some("c"), Some("c"))]);
    }
}

mod text_with_range_reverted_tests {
    use super::*;

    const BEFORE: &str = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n";
    const AFTER: &str = "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nk\nL\n";

    #[test]
    fn puts_one_hunk_back_and_leaves_the_others_as_they_are() {
        let diff = parse_diff_lines(BEFORE, AFTER);
        let hs = hunks(build_review(&diff));
        assert_eq!(hs.len(), 2);
        assert_eq!(text_with_range_reverted(&diff, hs[0].from, hs[0].to, true, None), "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nL\n");
    }

    #[test]
    fn reverting_every_hunk_gives_back_the_base() {
        let diff = parse_diff_lines(BEFORE, AFTER);
        assert_eq!(text_with_range_reverted(&diff, 0, diff.len(), true, None), BEFORE);
    }

    mod keeps_the_files_bytes_outside_the_reverted_lines {
        use super::*;

        fn revert_first(before: &str, after: &str) -> String {
            let d: Vec<DiffLine> = parse_diff_lines(before, after);
            let hs = hunks(build_review(&d));
            text_with_range_reverted(&d, hs[0].from, hs[0].to, after.ends_with('\n'), Some((before, after)))
        }
        fn body(eol: &str, mid: &str) -> String {
            ["a", mid, "c", "d", "e", "f", "g", "h", "i", "j"].join(eol)
        }

        #[test]
        fn crlf_stays_crlf() {
            let before = body("\r\n", "b") + "\r\n";
            let after = body("\r\n", "B") + "\r\n";
            assert_eq!(revert_first(&before, &after), before);
        }

        #[test]
        fn no_final_newline_stays_without_one() {
            assert_eq!(revert_first(&body("\n", "b"), &body("\n", "B")), body("\n", "b"));
        }

        #[test]
        fn a_bom_stays() {
            let before = format!("\u{FEFF}{}\r\n", body("\r\n", "b"));
            let after = format!("\u{FEFF}{}\r\n", body("\r\n", "B"));
            assert_eq!(revert_first(&before, &after), before);
        }

        #[test]
        fn mixed_endings_stay_as_they_were_line_by_line() {
            let before = "a\r\nb\nc\r\nd\ne\nf\ng\nh\ni\nj\r\n";
            let after = "a\r\nB\nc\r\nd\ne\nf\ng\nh\ni\nj\r\n";
            assert_eq!(revert_first(before, after), before);
        }

        #[test]
        fn keeps_other_hunks_lines_untouched_and_a_reverted_hunk_takes_its_own_old_ending() {
            let before = "a\r\nb\r\nc\r\nd\r\ne\r\nf\r\ng\r\nh\r\ni\r\nj\r\nk\r\nl\r\n";
            let after = "a\r\nB\r\nc\r\nd\r\ne\r\nf\r\ng\r\nh\r\ni\r\nj\r\nk\r\nL\r\n";
            assert_eq!(revert_first(before, after), after.replace('B', "b"));
        }

        #[test]
        fn a_lone_cr_ends_a_line_and_the_old_last_line_takes_the_usual_ending() {
            let before = "a\rb\rc\rd\re\rf\rg\rh\ri\rj";
            let after = "a\rb\rc\rd\re\rf\rg\rh\ri\rj\rk\r";
            let d = parse_diff_lines(before, after);
            let hs = hunks(build_review(&d));
            assert_eq!(text_with_range_reverted(&d, hs[0].from, hs[0].to, true, Some((before, after))), "a\rb\rc\rd\re\rf\rg\rh\ri\rj\r");
        }
    }
}

/// `cargo test print_review -- --ignored --nocapture`: the hunks, folds and marks of an invented two-edit file, for the
/// pull request's picture. Not a check; the tests above are.
#[test]
#[ignore]
fn print_review_of_an_edited_file() {
    let before: String = (1..=24)
        .map(|n| match n {
            1 => "def page_count(total, size):".to_string(),
            2 => "    return -(-total // size)".to_string(),
            9 => "DEFAULT_SIZE = 20".to_string(),
            _ => format!("# filler {n}"),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let after = before.replace("DEFAULT_SIZE = 20", "DEFAULT_SIZE = 50").replace("-(-total // size)", "(total + size - 1) // size");
    let items = build_review(&parse_diff_lines(&before, &after));
    for item in &items {
        match item {
            ReviewItem::Fold(f) => println!("{}  fold, {} unchanged lines hidden", f.id, f.hidden),
            ReviewItem::Hunk(h) => {
                println!("{}  hunk \"{}\"  starts at line {}  (diff lines {}..={})", h.id, h.name, h.start_line, h.from, h.to);
                for l in &h.lines {
                    let sign = match l.line.kind {
                        Kind::Add => '+',
                        Kind::Remove => '-',
                        Kind::Context => ' ',
                    };
                    let shown: Vec<String> = marked_segments(&l.line.content, &l.marks)
                        .into_iter()
                        .map(|(t, m)| if m { format!("[{t}]") } else { t.to_string() })
                        .collect();
                    println!("  {sign} {}", shown.concat());
                }
            }
        }
    }
}
