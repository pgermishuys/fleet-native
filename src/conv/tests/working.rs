//! `components/session/__tests__/WorkingIndicator.test.ts`, `InterruptedCard.test.ts` and the two `lib` tests they lean on
//! (`working-on.test.ts`, `retry-status.test.ts`), under the same names. The indicator's text is a function here, so
//! each case reads it at a given time instead of mounting and advancing timers; what the view draws from it is tested
//! in `src/tests.rs`.
use crate::conv::working::*;
use crate::fleet::model::{Msg, PartData};
use crate::fleet::rows::{Interruption, Retrying};
use crate::fleet::work::parse_iso_ms;
use crate::view::conversation::Role;
use serde_json::json;

fn iso(s: &str) -> i64 {
    parse_iso_ms(s).unwrap()
}

fn busy(elapsed: Option<&str>, tool: Option<&str>) -> Working {
    Working::Busy { elapsed: elapsed.map(str::to_string), tool: tool.map(str::to_string) }
}

mod working_indicator {
    use super::*;

    #[test]
    fn shows_the_working_glyph_and_word() {
        assert_eq!(working_line(&Inputs::default(), 0), busy(None, None));
    }

    #[test]
    fn counts_up_from_the_prompt() {
        let since = iso("2026-09-19T10:00:00Z");
        let at = |secs: i64| working_line(&Inputs { since: Some(since), ..Default::default() }, since + secs * 1000);
        assert_eq!(at(14), busy(Some("14 s"), None));
        assert_eq!(at(60), busy(Some("1 min"), None));
        // After the first minute only the minutes change, once a minute.
        assert_eq!(at(90), busy(Some("1 min"), None));
        assert_eq!(at(120), busy(Some("2 min"), None));
        let long = Inputs { since: Some(iso("2026-09-19T08:30:00Z")), ..Default::default() };
        assert_eq!(working_line(&long, since), busy(Some("1 h 30 min"), None));
        assert_eq!(working_line(&Inputs { since: Some(since - 92 * 60_000), ..Default::default() }, since), busy(Some("1 h 32 min"), None));
        // The clock ticks each second in the first minute, then each minute.
        let first = Inputs { since: Some(since), ..Default::default() };
        assert_eq!(working_tick_ms(&first, since + 14_000), 1_000);
        assert_eq!(working_tick_ms(&first, since + 61_000), 60_000);
    }

    #[test]
    fn says_it_needs_input_while_a_question_waits() {
        let i = Inputs { since: Some(1), waiting: true, ..Default::default() };
        assert_eq!(working_line(&i, 5_000), Working::Waiting);
    }

    #[test]
    fn never_shows_a_negative_time_when_clocks_disagree() {
        let i = Inputs { since: Some(10_000), ..Default::default() };
        assert_eq!(working_line(&i, 5_000), busy(Some("0 s"), None));
    }

    #[test]
    fn says_its_retrying_which_attempt_out_of_how_many_when_and_why_counting_down() {
        let now = iso("2026-10-07T10:00:00Z");
        let retry = Retrying {
            attempt: Some(3),
            max_attempts: Some(10),
            message: Some("API overloaded (529)".into()),
            next: Some("2026-10-07T10:00:12Z".into()),
        };
        let i = Inputs { since: Some(now - 60_000), retry: Some(&retry), ..Default::default() };
        assert_eq!(
            working_line(&i, now),
            Working::Retry("Retrying \u{b7} attempt 3 of 10 \u{b7} in 12 s \u{b7} API overloaded (529)".into())
        );
        let Working::Retry(later) = working_line(&i, now + 5_000) else { panic!("still retrying") };
        assert!(later.contains("in 7 s"));
        // It ticks each second while it counts down.
        assert_eq!(working_tick_ms(&i, now), 1_000);
        // Working again once the harness answers.
        let answered = Inputs { since: Some(now - 60_000), ..Default::default() };
        assert_eq!(working_line(&answered, now), busy(Some("1 min"), None));
    }

    #[test]
    fn says_so_instead_of_working_on_no_ticking_dots_no_growing_time() {
        let i = Inputs { since: Some(1), not_answering: Some("mini"), ..Default::default() };
        assert_eq!(working_line(&i, 60_000), Working::NotAnswering("mini".into()));
    }

    #[test]
    fn waiting_outranks_not_answering_and_retry() {
        let retry = Retrying { attempt: Some(1), ..Default::default() };
        let i = Inputs { waiting: true, retry: Some(&retry), not_answering: Some("mini"), ..Default::default() };
        assert_eq!(working_line(&i, 0), Working::Waiting);
        let i = Inputs { retry: Some(&retry), not_answering: Some("mini"), ..Default::default() };
        assert_eq!(working_line(&i, 0), Working::NotAnswering("mini".into()));
    }

    mod names_the_running_call {
        use super::*;
        const SINCE: &str = "2026-11-13T10:02:00Z";
        const NOW: &str = "2026-11-13T10:12:00Z";

        fn work(held: bool) -> WorkingOn {
            WorkingOn { name: "Bash".into(), since: Some(iso(SINCE)), more: 0, held }
        }

        #[test]
        fn adds_the_call_and_its_own_time_after_the_turns() {
            let now = iso(NOW);
            let w = work(false);
            let i = Inputs { since: Some(now - 12 * 60_000), work: Some(&w), ..Default::default() };
            assert_eq!(working_line(&i, now), busy(Some("12 min"), Some("Bash running 10 min")));
        }

        #[test]
        fn says_the_message_waits_when_its_held() {
            let now = iso(NOW);
            let w = work(true);
            let i = Inputs { since: Some(now - 12 * 60_000), work: Some(&w), ..Default::default() };
            let Working::Busy { tool: Some(tool), .. } = working_line(&i, now) else { panic!("busy") };
            assert!(tool.contains("Your message goes to the agent when it finishes."));
        }

        #[test]
        fn shows_only_working_with_no_call() {
            let i = Inputs { work: None, ..Default::default() };
            assert_eq!(working_line(&i, iso(NOW)), busy(None, None));
        }
    }
}

mod interrupted_card {
    use super::*;

    fn interruption() -> Interruption {
        Interruption {
            at: "2026-10-17T14:02:00Z".into(),
            waiting: false,
            last_call: Some("bash: bun run build".into()),
            changed_files: Some(3),
            auto: false,
        }
    }

    fn texts(i: &Interruption) -> Vec<String> {
        interruption_lines(i, "2:02 PM").into_iter().map(|p| p.text).collect()
    }

    #[test]
    fn says_what_was_working_the_last_call_and_the_changed_files() {
        let lines = texts(&interruption());
        assert_eq!(lines[0], "Fleet restarted while this was working (since 2:02 PM).");
        assert_eq!(lines[1], "Last call that didn't finish: bash: bun run build");
        assert_eq!(lines[2], "3 files changed in this worktree, still on disk.");
        // The clock and the call are the bold parts.
        let paras = interruption_lines(&interruption(), "2:02 PM");
        assert_eq!(&paras[0].text[paras[0].bold[0].clone()], "2:02 PM");
        assert_eq!(&paras[1].text[paras[1].bold[0].clone()], "bash: bun run build");
    }

    #[test]
    fn leaves_out_the_call_and_the_files_when_fleet_doesnt_know_them() {
        let i = Interruption { last_call: None, changed_files: None, ..interruption() };
        assert_eq!(texts(&i).len(), 1);
        // Nothing changed counts as nothing to say, and one file is singular.
        assert_eq!(texts(&Interruption { changed_files: Some(0), last_call: None, ..interruption() }).len(), 1);
        assert_eq!(changed_files_text(1), "1 file changed in this worktree");
    }

    #[test]
    fn says_the_question_was_lost_and_offers_ask_again() {
        let i = Interruption { waiting: true, ..interruption() };
        let paras = interruption_lines(&i, "2:02 PM");
        assert_eq!(paras.len(), 1);
        assert!(paras[0].text.contains("The question was lost."));
        assert!(!paras[0].text.contains("Last call"));
    }
}

mod working_on {
    use super::*;
    const NOW: i64 = 1_794_564_720_000;

    fn tool(id: &str, name: &str, status: &str, started_minutes_ago: i64) -> Msg {
        let mut m = Msg::new(format!("m-{id}"), "s", Role::Assistant);
        m.parts.push(PartData::Tool {
            id: id.into(),
            tool: name.into(),
            call_id: id.into(),
            state: json!({"status": status, "input": {}, "time": {"start": NOW - started_minutes_ago * 60_000}}),
        });
        m
    }

    fn user(id: &str, steered: bool) -> Msg {
        let mut m = Msg::new(id, "s", Role::User);
        m.created_at = Some(NOW);
        m.steered = steered;
        m
    }

    #[test]
    fn is_none_with_nothing_running() {
        assert_eq!(working_on(&[user("u1", false), tool("a", "bash", "completed", 3)], true), None);
    }

    #[test]
    fn names_the_one_running_tool_and_how_long_it_has_run() {
        let result = working_on(&[user("u1", false), tool("a", "bash", "running", 10)], true).unwrap();
        assert_eq!(result, WorkingOn { name: "Bash".into(), since: Some(NOW - 600_000), more: 0, held: false });
        assert_eq!(working_on_text(&result, NOW), "Bash running 10 min");
    }

    #[test]
    fn names_the_longest_running_one_and_counts_the_rest() {
        let result =
            working_on(&[tool("a", "bash", "running", 2), tool("b", "webfetch", "running", 8), tool("c", "grep", "running", 1)], false)
                .unwrap();
        assert_eq!((result.name.as_str(), result.more), ("Web Fetch", 2));
        assert_eq!(working_on_text(&result, NOW), "Web Fetch running 8 min +2 more");
    }

    #[test]
    fn says_the_steered_message_waits_when_it_sits_below_the_running_call_on_a_next_step_harness() {
        let messages = [user("u1", false), tool("a", "bash", "running", 10), user("u2", true)];
        let result = working_on(&messages, true).unwrap();
        assert!(result.held);
        assert_eq!(working_on_text(&result, NOW), "Bash running 10 min. Your message goes to the agent when it finishes.");
    }

    #[test]
    fn says_nothing_extra_on_a_harness_that_reads_a_steer_at_once_or_one_that_doesnt_say() {
        let messages = [tool("a", "bash", "running", 10), user("u2", true)];
        assert!(!working_on(&messages, false).unwrap().held);
    }

    #[test]
    fn doesnt_hold_a_steered_message_that_was_sent_before_the_call_started() {
        assert!(!working_on(&[user("u2", true), tool("a", "bash", "running", 10)], true).unwrap().held);
    }

    #[test]
    fn doesnt_hold_a_message_that_isnt_steered() {
        assert!(!working_on(&[tool("a", "bash", "running", 10), user("u2", false)], true).unwrap().held);
    }

    #[test]
    fn falls_back_to_the_messages_start_when_the_call_has_none() {
        let mut m = tool("a", "bash", "running", 0);
        if let PartData::Tool { state, .. } = &mut m.parts[0] {
            *state = json!({"status": "running", "input": {}});
        }
        m.created_at = Some(NOW - 4 * 60_000);
        assert_eq!(working_on(&[m], false).unwrap().since, Some(NOW - 4 * 60_000));
    }

    #[test]
    fn turn_started_at_skips_steered_prompts() {
        let mut first = user("u1", false);
        first.created_at = Some(1_000);
        let mut steered = user("u2", true);
        steered.created_at = Some(2_000);
        assert_eq!(turn_started_at(&[first, tool("a", "bash", "running", 1), steered]), Some(1_000));
        assert_eq!(turn_started_at(&[]), None);
    }
}

mod retry_status {
    use super::*;

    #[test]
    fn leaves_out_what_the_harness_didnt_say() {
        let now = iso("2026-10-07T10:00:00Z");
        assert_eq!(describe_retry(&Retrying::default(), now), "Retrying");
        assert_eq!(describe_retry(&Retrying { attempt: Some(3), ..Default::default() }, now), "Retrying \u{b7} attempt 3");
        let r = Retrying { message: Some("  slow down  ".into()), next: Some("2026-10-07T10:03:00Z".into()), ..Default::default() };
        assert_eq!(describe_retry(&r, now), "Retrying \u{b7} in 3 min \u{b7} slow down");
        let due = Retrying { next: Some("2026-10-07T09:59:00Z".into()), ..Default::default() };
        assert_eq!(describe_retry(&due, now), "Retrying \u{b7} now");
    }
}
