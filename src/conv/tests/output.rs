//! `lib/__tests__/tool-output.test.ts` and the logic of `ToolOutputBody.test.ts`. Fleet's DOM tests (mounting, classes in
//! HTML) become the same checks on the parsed runs and the line window; `linesToHtml` is HTML-only and skipped, its
//! "never markup" half is the test "never turns the output into markup".
use crate::conv::output::{
    COLLAPSED_LINES, MAX_EXPANDED_LINES, ParsedOutput, Window, count, line_text, parse_tool_output, parse_tool_output_cached, thousands,
};
use std::rc::Rc;

const ESC: &str = "\x1b";

/// A run as Fleet's tests write it: (text, className, style), the last two empty for none.
fn runs(parsed: &ParsedOutput, line: usize) -> Vec<(String, String, String)> {
    parsed.lines[line].iter().map(|s| (s.text.clone(), s.class_name(), s.style())).collect()
}

fn run(text: &str, class: &str, style: &str) -> (String, String, String) {
    (text.to_string(), class.to_string(), style.to_string())
}

fn texts(parsed: &ParsedOutput) -> Vec<String> {
    parsed.lines.iter().map(line_text).collect()
}

mod parse_tool_output_tests {
    use super::*;

    #[test]
    fn keeps_plain_text_as_it_is_one_line_per_line() {
        let parsed = parse_tool_output("a\nb\n\n");
        assert_eq!(texts(&parsed), ["a", "b"]);
        assert_eq!(runs(&parsed, 0), [run("a", "", "")]);
        assert_eq!(parsed.plain, "a\nb");
    }

    #[test]
    fn has_no_lines_for_empty_or_blank_output() {
        assert!(parse_tool_output("").lines.is_empty());
        assert!(parse_tool_output("\n\n").lines.is_empty());
    }

    #[test]
    fn turns_colour_codes_into_classes_in_fleets_palette_and_resets_them() {
        let parsed = parse_tool_output(&format!("{ESC}[32m✓{ESC}[0m ok {ESC}[1;31mFAIL{ESC}[22m x{ESC}[39m done"));
        assert_eq!(
            runs(&parsed, 0),
            [
                run("✓", "ansi-fg-green", ""),
                run(" ok ", "", ""),
                run("FAIL", "ansi-fg-red ansi-bold", ""),
                run(" x", "ansi-fg-red", ""),
                run(" done", "", ""),
            ]
        );
        assert_eq!(parsed.plain, "✓ ok FAIL x done");
    }

    #[test]
    fn carries_a_colour_across_lines_until_its_reset() {
        let parsed = parse_tool_output(&format!("{ESC}[33mone\ntwo{ESC}[m\nthree"));
        assert_eq!(runs(&parsed, 0), [run("one", "ansi-fg-yellow", "")]);
        assert_eq!(runs(&parsed, 1), [run("two", "ansi-fg-yellow", "")]);
        assert_eq!(runs(&parsed, 2), [run("three", "", "")]);
    }

    #[test]
    fn reads_bright_background_dim_italic_and_underline_codes() {
        let parsed = parse_tool_output(&format!("{ESC}[92;41;2;3;4mx{ESC}[23;24;49my"));
        assert_eq!(
            runs(&parsed, 0),
            [
                run("x", "ansi-fg-bright-green ansi-bg-red ansi-dim ansi-italic ansi-underline", ""),
                run("y", "ansi-fg-bright-green ansi-dim", ""),
            ]
        );
    }

    #[test]
    fn draws_256_colour_and_true_colour_codes_palette_indexes_as_classes_and_the_rest_as_a_colour() {
        let parsed = parse_tool_output(&format!("{ESC}[38;5;9ma{ESC}[38;5;208mb{ESC}[38;2;10;20;300mc{ESC}[48;5;244md"));
        assert_eq!(
            runs(&parsed, 0),
            [
                run("a", "ansi-fg-bright-red", ""),
                run("b", "", "color:rgb(255,135,0)"),
                run("c", "", "color:rgb(10,20,255)"),
                run("d", "", "color:rgb(10,20,255);background-color:rgb(128,128,128)"),
            ]
        );
    }

    #[test]
    fn drops_cursor_moves_erases_private_modes_titles_and_charset_picks() {
        let raw = format!(
            "{ESC}]0;my title{ESC}\\{ESC}[?25l{ESC}(Bstart{ESC}[2K{ESC}[1A{ESC}[10Gend{ESC}]8;;https://x.test{ESC}\\link{ESC}]8;;{ESC}\\"
        );
        assert_eq!(parse_tool_output(&raw).plain, "startendlink");
    }

    #[test]
    fn keeps_only_the_last_frame_of_a_line_rewritten_with_cr() {
        let raw = "Installing\n[#   ] 25%\r[##  ] 50%\r[####] 100%\nDone\n";
        assert_eq!(texts(&parse_tool_output(raw)), ["Installing", "[####] 100%", "Done"]);
    }

    #[test]
    fn keeps_a_frame_a_trailing_cr_would_otherwise_wipe_and_the_colour_set_in_a_dropped_frame() {
        let parsed = parse_tool_output(&format!("{ESC}[32m10%\r50%\r{ESC}[2K\r"));
        assert_eq!(parsed.lines.len(), 1);
        assert_eq!(runs(&parsed, 0), [run("50%", "ansi-fg-green", "")]);
    }

    #[test]
    fn treats_crlf_as_a_plain_line_end() {
        assert_eq!(texts(&parse_tool_output("one\r\ntwo\r\n")), ["one", "two"]);
    }

    #[test]
    fn drops_a_broken_escape_instead_of_showing_it_and_keeps_the_text_around_it() {
        assert_eq!(parse_tool_output(&format!("before{ESC}")).plain, "before");
        assert!(!parse_tool_output(&format!("a{ESC}[31")).plain.contains(ESC));
        assert_eq!(parse_tool_output(&format!("a{ESC}]0;title with no end\nnext")).plain, "a\nnext");
        assert_eq!(parse_tool_output(&format!("{ESC}[38;5mx{ESC}[38;2;1mz")).plain, "xz");
        assert_eq!(runs(&parse_tool_output(&format!("x{ESC}[1;>my")), 0), [run("xy", "", "")]);
        assert_eq!(runs(&parse_tool_output(&format!("x{ESC}[999my")), 0), [run("xy", "", "")]);
    }

    #[test]
    fn drops_bells_backspaces_and_the_other_control_characters_but_keeps_tabs() {
        assert_eq!(parse_tool_output("a\x07b\x08c\td\x00").plain, "abc\td");
    }

    #[test]
    fn never_turns_the_output_into_markup() {
        let parsed = parse_tool_output(&format!("{ESC}[31m<img src=x onerror=alert(1)>"));
        assert_eq!(runs(&parsed, 0), [run("<img src=x onerror=alert(1)>", "ansi-fg-red", "")]);
    }

    #[test]
    fn parses_5000_coloured_lines_quickly() {
        let raw = (0..5000)
            .map(|i| {
                let retry = if i % 50 == 0 { format!("\r{ESC}[2K{ESC}[33mretry {i}{ESC}[0m") } else { String::new() };
                format!("{ESC}[32m✓{ESC}[0m ok  suite/case_{i} {ESC}[2m({}ms){ESC}[0m{retry}", i % 97)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let started = std::time::Instant::now();
        let parsed = parse_tool_output(&raw);
        let elapsed = started.elapsed();
        assert_eq!(parsed.lines.len(), 5000);
        assert_eq!(line_text(&parsed.lines[4999]), "✓ ok  suite/case_4999 (52ms)");
        assert_eq!(line_text(&parsed.lines[50]), "retry 50");
        assert!(!parsed.plain.contains(ESC));
        // Linear work takes a few milliseconds; the bound is loose for busy CI machines and catches a quadratic slip.
        assert!(elapsed.as_millis() < 5_000, "took {elapsed:?}");
    }
}

mod parse_tool_output_cached_tests {
    use super::*;

    #[test]
    fn parses_the_same_output_once() {
        let raw = format!("{ESC}[31mcached {}", line!());
        assert!(Rc::ptr_eq(&parse_tool_output_cached(&raw), &parse_tool_output_cached(&raw)));
        assert!(!Rc::ptr_eq(&parse_tool_output_cached(&format!("{raw}!")), &parse_tool_output_cached(&raw)));
    }
}

/// `ToolOutputBody.test.ts`: the lines the box draws and what its toggles say.
mod tool_output_body_tests {
    use super::*;

    fn numbered(n: usize) -> String {
        (1..=n).map(|i| format!("{ESC}[32m✓{ESC}[0m case {i}")).collect::<Vec<_>>().join("\n")
    }

    /// The text the box shows for `output`, as `shown(wrapper)` reads it.
    fn shown(output: &str, expanded: bool) -> Vec<String> {
        let parsed = parse_tool_output_cached(output);
        let w = Window::of(parsed.lines.len(), expanded);
        texts(&parsed)[w.total - w.shown..].to_vec()
    }

    #[test]
    fn draws_short_output_whole_coloured_with_its_line_count() {
        let output = format!("{ESC}[31mFAIL{ESC}[0m limiter\nok\n");
        assert_eq!(shown(&output, false), ["FAIL limiter", "ok"]);
        assert_eq!(runs(&parse_tool_output(&output), 0)[0], run("FAIL", "ansi-fg-red", ""));
        let w = Window::of(2, false);
        assert!(!w.is_long, "no toggle for a short output");
        assert!(!parse_tool_output(&output).plain.contains(ESC));
    }

    #[test]
    fn shows_the_last_12_lines_until_asked_for_the_rest_then_only_the_last_12_again() {
        let output = numbered(200);
        let closed = shown(&output, false);
        assert_eq!(closed.len(), 12);
        assert_eq!(closed[0], "✓ case 189");
        assert_eq!(closed.last().unwrap(), "✓ case 200");
        let w = Window::of(200, false);
        assert_eq!(format!("Show {}", count(w.hidden(), "earlier line")), "Show 188 earlier lines");

        let open = shown(&output, true);
        assert_eq!(open.len(), 200);
        assert!(!Window::of(200, true).capped);
        assert_eq!(shown(&output, false).len(), COLLAPSED_LINES);
    }

    #[test]
    fn draws_at_most_the_last_2000_lines_when_expanded_and_says_so() {
        let output = numbered(5000);
        let w = Window::of(5000, false);
        assert_eq!(format!("Show {}", count(w.hidden(), "earlier line")), "Show 4,988 earlier lines");
        let open = shown(&output, true);
        assert_eq!(open.len(), MAX_EXPANDED_LINES);
        assert_eq!(open[0], "✓ case 3001");
        let w = Window::of(5000, true);
        assert!(w.capped);
        assert_eq!(
            format!("Showing the last {} of {}. Copy takes all of them.", count(w.shown, "line"), thousands(w.total)),
            "Showing the last 2,000 lines of 5,000. Copy takes all of them."
        );
    }

    #[test]
    fn copies_the_whole_output_as_plain_text_with_no_codes_and_only_the_last_frame_of_a_progress_line() {
        // The button waits on Rune's clipboard; what it would copy is `plain`.
        let output = format!("{}\n{ESC}[33m[#  ] 10%\r[###] 100%{ESC}[0m\n", numbered(30));
        let copied = parse_tool_output(&output).plain;
        assert_eq!(copied.split('\n').count(), 31);
        assert!(copied.starts_with("✓ case 1\n"));
        assert!(copied.ends_with("✓ case 30\n[###] 100%"));
        assert!(!copied.contains(ESC));
    }

    #[test]
    fn says_one_line_in_the_singular_and_groups_thousands() {
        assert_eq!(count(1, "earlier line"), "1 earlier line");
        assert_eq!(count(2, "earlier line"), "2 earlier lines");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(999), "999");
    }
}
