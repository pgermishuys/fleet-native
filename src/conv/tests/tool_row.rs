//! `lib/__tests__/tool-row.test.ts`, the status table of `ToolCard.rows.test.ts` and `MessageToolList.test.ts`.
use crate::conv::tools::{
    Category, Glyph, ToolRowEntry, ToolRowItem, ToolStatus, format_call_duration, get_tool, group_reads, is_shell_row, shell_header,
    tool_duration_ms,
};
use serde_json::{Value, json};

mod format_call_duration_tests {
    use super::*;

    #[test]
    fn says_tenths_under_a_second_then_seconds_then_minutes() {
        let said = |ms: f64| format_call_duration(Some(ms));
        assert_eq!(said(40.0), "0.1s");
        assert_eq!(said(140.0), "0.1s");
        assert_eq!(said(900.0), "0.9s");
        assert_eq!(said(34_000.0), "34s");
        assert_eq!(said(125_000.0), "2m 5s");
    }

    #[test]
    fn is_empty_when_nothing_was_timed() {
        assert_eq!(format_call_duration(None), "");
        assert_eq!(format_call_duration(Some(-5.0)), "");
        assert_eq!(format_call_duration(Some(f64::NAN)), "");
    }

    #[test]
    fn a_call_is_timed_by_the_start_and_end_the_harness_kept() {
        assert_eq!(tool_duration_ms(&json!({"time": {"start": 1000, "end": 1100}})), Some(100.0));
        assert_eq!(tool_duration_ms(&json!({"time": {"start": 1000}})), None);
        assert_eq!(tool_duration_ms(&json!({"time": {"start": 2000, "end": 1000}})), None);
        assert_eq!(tool_duration_ms(&json!({})), None);
    }
}

mod shell_header_tests {
    use super::*;

    #[test]
    fn joins_the_command_exit_code_and_time() {
        assert_eq!(shell_header(&json!({"command": "bun test"}), "x", Some(0), Some(100.0)), "bun test · exit 0 · 0.1s");
    }

    #[test]
    fn leaves_out_what_the_harness_did_not_report() {
        assert_eq!(shell_header(&json!({"command": "bun test\nmore"}), "x", None, None), "bun test");
        assert_eq!(shell_header(&Value::Null, "ls", None, None), "ls");
    }
}

mod group_reads_tests {
    use super::*;

    struct Call(&'static str, &'static str, ToolStatus);
    impl ToolRowItem for Call {
        fn id(&self) -> &str {
            self.0
        }
        fn kind(&self) -> &str {
            self.1
        }
        fn status(&self) -> ToolStatus {
            self.2
        }
    }
    fn read(id: &'static str) -> Call {
        Call(id, "read", ToolStatus::Completed)
    }
    fn read_in(id: &'static str, status: ToolStatus) -> Call {
        Call(id, "read", status)
    }
    fn edit(id: &'static str) -> Call {
        Call(id, "edit", ToolStatus::Completed)
    }
    fn shapes(entries: &[ToolRowEntry<Call>]) -> Vec<&'static str> {
        entries.iter().map(|e| if matches!(e, ToolRowEntry::Reads { .. }) { "reads" } else { "tool" }).collect()
    }

    #[test]
    fn collapses_two_or_more_completed_reads_in_a_row() {
        let entries = group_reads([read("a"), read("b"), read("c"), edit("d")]);
        assert_eq!(shapes(&entries), ["reads", "tool"]);
        let ToolRowEntry::Reads { tools, id } = &entries[0] else { panic!("not a group") };
        assert_eq!(tools.iter().map(|t| t.0).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(id, "reads-a");
    }

    #[test]
    fn takes_reads_still_streaming_into_the_row_whether_or_not_their_path_has_arrived() {
        let entries = group_reads([read("a"), read_in("b", ToolStatus::Running), read_in("c", ToolStatus::Pending)]);
        assert_eq!(shapes(&entries), ["reads"]);
        let ToolRowEntry::Reads { tools, .. } = &entries[0] else { panic!("not a group") };
        assert_eq!(tools.iter().map(|t| t.0).collect::<Vec<_>>(), ["a", "b", "c"]);
    }

    #[test]
    fn leaves_a_single_read_and_a_failed_read_as_their_own_rows() {
        assert_eq!(shapes(&group_reads([read("a")])), ["tool"]);
        assert_eq!(shapes(&group_reads([read("a"), read_in("b", ToolStatus::Error), read("c")])), ["tool", "tool", "tool"]);
    }

    #[test]
    fn works_for_any_harnesss_read_tool_name() {
        let kinds =
            [Call("0", "Read", ToolStatus::Completed), Call("1", "ls", ToolStatus::Completed), Call("2", "read", ToolStatus::Completed)];
        assert_eq!(shapes(&group_reads(kinds)), ["reads"]);
    }

    /// `MessageToolList.test.ts`, "draws nothing without calls".
    #[test]
    fn draws_nothing_without_calls() {
        assert!(group_reads(Vec::<Call>::new()).is_empty());
    }
}

/// The status table of `ToolCard.rows.test.ts`: what the right-hand end of a row shows for each status word.
mod tool_card_status_indicator {
    use super::*;

    #[test]
    fn shows_a_running_error_or_background_call_with_a_glyph() {
        for (word, glyph) in [("Running", Glyph::Running), ("Error", Glyph::Error), ("Background", Glyph::Running)] {
            let status = ToolStatus::from_word(Some(word));
            assert_eq!(status.glyph(), glyph, "{word}");
            assert!(status.shows_glyph(), "{word}");
        }
    }

    #[test]
    fn shows_a_completed_call_with_no_tick_and_no_glyph() {
        assert!(!ToolStatus::from_word(Some("Completed")).shows_glyph());
    }

    #[test]
    fn shows_a_stopped_call_as_the_word_with_no_glyph() {
        let stopped = ToolStatus::from_word(Some("Stopped"));
        assert_eq!(stopped, ToolStatus::Stopped);
        assert!(!stopped.shows_glyph());
    }

    #[test]
    fn shows_a_pending_cancelled_or_unknown_call_with_no_indicator_at_all() {
        for word in ["Pending", "Cancelled", "SomethingElse"] {
            assert!(!ToolStatus::from_word(Some(word)).shows_glyph(), "{word}");
        }
    }

    #[test]
    fn defaults_to_pending_when_no_status_is_given_and_capitalises_the_harness_word() {
        assert_eq!(ToolStatus::from_word(None), ToolStatus::Pending);
        assert_eq!(ToolStatus::from_word(Some("running")), ToolStatus::Running);
        assert_eq!(ToolStatus::from_word(Some("error")), ToolStatus::Error);
        assert_eq!(ToolStatus::from_word(Some("completed")), ToolStatus::Completed);
    }

    #[test]
    fn maps_each_status_to_its_glyph() {
        let glyph = |w| ToolStatus::from_word(Some(w)).glyph();
        assert_eq!(glyph("Pending"), Glyph::Idle);
        assert_eq!(glyph("Cancelled"), Glyph::Idle);
        assert_eq!(glyph("Stopped"), Glyph::Idle);
        assert_eq!(glyph("Completed"), Glyph::Completed);
    }
}

mod tool_card_quiet_rows {
    use super::*;

    #[test]
    fn a_shell_row_shows_how_long_it_took_but_code_mode_does_not() {
        assert!(is_shell_row("bash"));
        assert!(is_shell_row("terminal"));
        assert!(!is_shell_row("execute"));
        assert!(!is_shell_row("read"));
    }

    #[test]
    fn reads_are_dimmed() {
        assert_eq!(get_tool("read").category, Category::Read);
    }
}
