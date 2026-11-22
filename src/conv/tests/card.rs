//! `ToolCard.rows.test.ts` (open and closed, quiet rows, the shell header), `toToolCardItem`'s output and preview, and
//! the status table of `ToolCard status indicator` that piece A did not reach.
use crate::conv::card::{Toggle, build_preview, get_tool_output, is_open, shows_diff, tool_body};
use serde_json::json;

mod tool_card_open_and_closed {
    use super::*;

    fn body(state: serde_json::Value) -> crate::conv::card::ToolBody {
        tool_body("bash", "ls", &state)
    }

    #[test]
    fn starts_every_call_collapsed_except_one_that_failed() {
        assert!(body(json!({"status": "completed", "input": {}})).initially_collapsed);
        assert!(body(json!({"status": "running", "input": {}})).initially_collapsed);
        assert!(!body(json!({"status": "error", "input": {}, "error": "boom"})).initially_collapsed);
    }

    #[test]
    fn follows_the_native_toggle_opening_and_closing_flips_it() {
        let b = body(json!({"status": "completed"}));
        assert!(!is_open("bash", &b, false, None));
        let opened = Some(Toggle { open: true, under: true });
        assert!(is_open("bash", &b, false, opened));
        assert!(!is_open("bash", &b, false, Some(Toggle { open: false, under: true })));
    }

    #[test]
    fn follows_initially_collapsed_when_it_changes() {
        // A call that failed after the user opened it: the toggle was made under "collapsed", so it no longer holds.
        let failed = body(json!({"status": "error", "error": "boom"}));
        assert!(is_open("bash", &failed, false, Some(Toggle { open: false, under: true })));
        let running = body(json!({"status": "running"}));
        assert!(!is_open("bash", &running, false, Some(Toggle { open: true, under: false })));
    }

    #[test]
    fn stays_open_for_a_diff_when_inline_diffs_are_on_even_if_told_to_start_collapsed() {
        let b = tool_body("Tool", "x", &json!({"status": "completed", "diffLines": [{"type": "add", "content": "+x"}]}));
        assert!(b.has_diff && b.initially_collapsed);
        assert!(is_open("Tool", &b, true, None));
    }

    #[test]
    fn starts_collapsed_for_a_diff_when_inline_diffs_are_off() {
        let b = tool_body("Tool", "x", &json!({"status": "completed", "diffLines": [{"type": "add", "content": "+x"}]}));
        assert!(!is_open("Tool", &b, false, None));
        // Opened, the diff is hidden: the body has no output or summary, so it says so (the view draws the empty state).
        assert!(!shows_diff("Tool", &b, false));
        assert!(b.output.is_empty() && b.summary.is_empty());
    }

    #[test]
    fn a_write_opens_on_what_it_wrote_even_with_inline_diffs_off() {
        let b = tool_body("write", "a.ts", &json!({"status": "completed", "diffLines": [{"type": "add", "content": "+x"}]}));
        assert!(!is_open("write", &b, false, None), "collapsed until clicked");
        assert!(shows_diff("write", &b, false), "and then it shows what it wrote");
    }
}

mod tool_card_quiet_rows {
    use super::*;

    #[test]
    fn a_shell_row_shows_how_long_it_took_and_its_open_body_has_one_header() {
        let state = json!({"status": "completed", "input": {"command": "bun test"}, "metadata": {"exit": 0}, "output": "ok\n1 pass",
            "time": {"start": 0, "end": 100}});
        let b = tool_body("bash", "bun test", &state);
        assert_eq!(b.header.as_deref(), Some("bun test · exit 0 · 0.1 s"));
        assert!(!b.preview.as_deref().unwrap_or("").is_empty());
    }

    #[test]
    fn a_failed_call_keeps_its_preview_line() {
        let b = tool_body("bash", "x", &json!({"status": "error", "output": "boom"}));
        assert_eq!(b.preview.as_deref(), Some("└ boom"));
    }

    #[test]
    fn other_kinds_have_no_header() {
        assert_eq!(tool_body("read", "a.ts", &json!({"status": "completed", "output": "x"})).header, None);
    }
}

mod to_tool_card_item {
    use super::*;

    #[test]
    fn keeps_the_fields_the_card_shows() {
        let b = tool_body(
            "read",
            "src/a.ts",
            &json!({"status": "completed", "input": {"filePath": "src/a.ts", "offset": 5}, "output": "one\ntwo"}),
        );
        assert_eq!(b.output, "one\ntwo");
        assert_eq!(b.preview.as_deref(), Some("└ one (2 lines)"));
    }

    #[test]
    fn picks_the_output_from_the_first_key_present() {
        assert_eq!(get_tool_output(&json!({"result": "r", "stderr": "e"})).as_deref(), Some("r"));
        assert_eq!(get_tool_output(&json!({"message": "m", "stdout": "o"})).as_deref(), Some("m"));
        assert_eq!(get_tool_output(&json!({"output": null, "content": "c"})).as_deref(), Some("c"));
        assert_eq!(get_tool_output(&json!({"output": ""})).as_deref(), Some(""));
    }

    #[test]
    fn pretty_prints_a_value_that_is_not_text() {
        assert_eq!(get_tool_output(&json!({"output": {"a": [1, 2]}})).as_deref(), Some("{\n  \"a\": [\n    1,\n    2\n  ]\n}"));
    }

    #[test]
    fn falls_back_to_the_state_less_what_is_shown_elsewhere() {
        let state = json!({"status": "completed", "input": {"a": 1}, "title": "t", "summary": "s", "metadata": {"k": 1}});
        assert_eq!(get_tool_output(&state).as_deref(), Some("{\n  \"metadata\": {\n    \"k\": 1\n  }\n}"));
        assert_eq!(get_tool_output(&json!({"status": "completed", "input": {}})), None);
        assert_eq!(get_tool_output(&json!({})), None);
    }
}

mod build_preview_tests {
    use super::*;

    #[test]
    fn takes_the_first_line_with_text_once_codes_are_gone_and_counts_lines() {
        let out = "\x1b[31m\x1b[0m\n  \n\x1b[32mfirst\x1b[0m\nsecond\n\n";
        assert_eq!(build_preview(Some(out), None).as_deref(), Some("└ first (4 lines)"));
    }

    #[test]
    fn cuts_to_80_characters_as_79_and_an_ellipsis() {
        let long = "x".repeat(100);
        let want = format!("└ {}…", "x".repeat(79));
        assert_eq!(build_preview(Some(&long), None), Some(want));
        assert_eq!(build_preview(Some(&"y".repeat(80)), None), Some(format!("└ {}", "y".repeat(80))));
    }

    #[test]
    fn falls_back_to_the_summary_with_no_usable_line() {
        assert_eq!(build_preview(Some("\x1b[2K\n"), Some("did it")).as_deref(), Some("└ did it"));
        assert_eq!(build_preview(None, Some("did it")).as_deref(), Some("└ did it"));
        assert_eq!(build_preview(None, None), None);
        assert_eq!(build_preview(Some(""), Some("")), None);
    }
}
