//! `lib/__tests__/turn-fold.test.ts` and `turn-fold-row-stats.test.ts`, under the same names. The test that a turn left
//! alone "is" the same object (`toBe`) compares by value here: messages are owned, there is no identity to keep.
use crate::conv::fold::{
    FileStats, FoldOptions, TurnStatsSnapshot, WatchedTurn, fold_turns, relative_path, summary_parts, toggle_turn, turn_ids_of,
    with_file_stats,
};
use crate::conv::message_list::{FileStat, ListMessage, ListTool, Outcome, TurnError};
use crate::conv::tools::ToolStatus;
use crate::view::conversation::Role;
use std::collections::HashSet;

fn msg(id: &str, role: Role) -> ListMessage {
    ListMessage::new(id, role)
}
fn user(id: &str) -> ListMessage {
    msg(id, Role::User)
}
fn agent(id: &str) -> ListMessage {
    msg(id, Role::Assistant)
}
fn at(mut m: ListMessage, created: i64, completed: Option<i64>) -> ListMessage {
    m.created_at = Some(created);
    m.completed_at = completed;
    m
}
fn said(mut m: ListMessage, body: &str) -> ListMessage {
    m.body = body.into();
    m
}
fn with_tools(mut m: ListMessage, tools: Vec<ListTool>) -> ListMessage {
    m.tools = tools;
    m
}
fn thinking(mut m: ListMessage) -> ListMessage {
    m.reasoning = vec![crate::conv::message_list::ListReasoning { text: "hmm".into(), summary: None }];
    m
}
fn tool(id: &str, kind: &str) -> ListTool {
    ListTool::new(id, kind, ToolStatus::Completed)
}
fn edit(id: &str, path: &str) -> ListTool {
    ListTool { file_path: Some(path.into()), ..tool(id, "edit") }
}
fn write(id: &str, path: &str) -> ListTool {
    ListTool { file_path: Some(path.into()), diff_from_content: true, ..tool(id, "write") }
}
fn ids(messages: &[ListMessage]) -> Vec<&str> {
    messages.iter().map(|m| m.id.as_str()).collect()
}
fn tool_ids(m: &ListMessage) -> Vec<&str> {
    m.tools.iter().map(|t| t.id.as_str()).collect()
}
fn none() -> HashSet<String> {
    HashSet::new()
}
fn fold(messages: &[ListMessage], running: bool, keep_last: bool, directory: Option<&str>, open: &HashSet<String>) -> Vec<ListMessage> {
    fold_turns(messages, &FoldOptions { running, keep_last, directory, open })
}
/// A finished turn, as a run of tests then an edit and a command then an answer: 35 s from prompt to last message.
fn turn() -> Vec<ListMessage> {
    vec![
        said(at(user("u1"), 1000, None), "do it"),
        with_tools(
            thinking(at(agent("a1"), 2000, Some(5000))),
            vec![tool("t1", "read"), tool("t2", "read"), edit("t3", "/work/demo/src/notes.ts")],
        ),
        with_tools(at(agent("a2"), 6000, Some(35_000)), vec![tool("t4", "bash")]),
        said(at(agent("a3"), 36_000, Some(36_000)), "All done."),
    ]
}

// ─── foldTurns ──────────────────────────────────────────────────────────────

#[test]
fn keeps_the_latest_turn_open_after_it_ends_when_you_watched_it_keep_last_and_folds_it_once_a_newer_turn_exists() {
    let kept = fold(&turn(), false, true, None, &none());
    assert_eq!(ids(&kept), ["u1", "a1", "a2", "a3"]);
    assert!(kept.iter().all(|m| m.fold.is_none()));
    let mut next = turn();
    next.push(said(at(user("u2"), 40_000, None), "next"));
    next.push(said(at(agent("b1"), 41_000, None), "ok"));
    let out = fold(&next, false, true, None, &none());
    assert!(out.iter().find(|m| m.id == "a1").unwrap().fold.is_some());
}

#[test]
fn folds_a_finished_turn_tool_rows_and_thinking_go_the_answer_stays_the_fold_row_appears() {
    let out = fold(&turn(), false, false, Some("/work/demo"), &none());
    assert_eq!(ids(&out), ["u1", "a1", "a3"]);
    let host = &out[1];
    assert!(host.fold_only);
    assert!(host.tools.is_empty());
    assert!(host.reasoning.is_empty());
    let summary = &host.fold.as_ref().unwrap().summary;
    assert_eq!((summary.reads, summary.edits, summary.commands, summary.duration_ms), (2, 1, 1, Some(35_000)));
    assert_eq!(summary_parts(summary), ["read 2 files", "edited 1", "ran 1 command"]);
}

#[test]
fn never_hides_the_agents_text_narration_between_calls_stays_in_order_and_only_tool_rows_fold() {
    let mut m = turn();
    m[1].body = "I'll start by looking.".into();
    m[2].body = "Now the test.".into();
    let out = fold(&m, false, false, Some("/work/demo"), &none());
    assert_eq!(ids(&out), ["u1", "a1", "a2", "a3"]);
    let bodies: Vec<&str> = out.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, ["do it", "I'll start by looking.", "Now the test.", "All done."]);
    assert!(!out[1].fold_only);
    assert!(out[1].fold.is_some());
    assert!(out[1].tools.is_empty());
}

#[test]
fn keeps_the_reply_to_a_steered_message_visible_in_its_own_turn() {
    let m = vec![
        at(user("u1"), 1, None),
        with_tools(agent("a1"), vec![tool("t", "bash")]),
        said(at(user("u2"), 5, None), "also this"),
        said(with_tools(agent("a2"), vec![tool("t2", "read")]), "Sure, done."),
    ];
    let out = fold(&m, false, false, None, &none());
    assert_eq!(out.iter().find(|x| x.id == "a2").unwrap().body, "Sure, done.");
}

#[test]
fn keeps_the_last_text_as_the_answer_when_all_of_it_came_before_the_last_call() {
    let m = vec![user("u"), said(with_tools(agent("a"), vec![tool("t", "bash")]), "Here is the result.")];
    assert_eq!(fold(&m, false, false, None, &none())[1].body, "Here is the result.");
}

#[test]
fn leaves_the_running_turn_open() {
    let out = fold(&turn(), true, false, None, &none());
    assert!(out.iter().all(|m| m.fold.is_none() && !m.fold_only));
    assert_eq!(out.len(), 4);
}

#[test]
fn folds_every_finished_turn_the_latest_one_included_but_only_the_running_one_stays_open() {
    let mut two = turn();
    two.push(at(user("u2"), 40_000, None));
    two.push(with_tools(agent("b1"), vec![tool("t9", "read")]));
    let out = fold(&two, true, false, None, &none());
    assert!(out.iter().find(|m| m.id == "a1").unwrap().fold_only);
    assert_eq!(out.iter().find(|m| m.id == "b1").unwrap().tools.len(), 1);
}

#[test]
fn keeps_failed_rows_visible_and_counts_them() {
    let mut m = turn();
    m[2] = with_tools(agent("a2"), vec![ListTool::new("t4", "bash", ToolStatus::Error)]);
    let out = fold(&m, false, false, None, &none());
    assert_eq!(out.iter().find(|x| x.id == "a2").unwrap().tools.len(), 1);
    assert_eq!(out[1].fold.as_ref().unwrap().summary.failed, 1);
}

#[test]
fn keeps_sub_agent_rows_pages_and_screenshots_visible() {
    let delegated = ListTool { delegation: true, ..tool("d", "task") };
    let page = ListTool { page: true, ..tool("p", "fleet_page_show") };
    let shot = ListTool { screenshot: true, ..tool("s", "browser_take_screenshot") };
    let m = vec![user("u"), said(with_tools(agent("a"), vec![delegated, page, shot, tool("r", "read")]), "x")];
    assert_eq!(tool_ids(&fold(&m, false, false, None, &none())[1]), ["d", "p", "s"]);
}

#[test]
fn shows_everything_again_when_the_turn_is_opened_with_the_fold_row_kept() {
    let open: HashSet<String> = ["u1".to_string()].into();
    let out = fold(&turn(), false, false, Some("/work/demo"), &open);
    assert_eq!(ids(&out), ["u1", "a1", "a2", "a3"]);
    assert!(out[1].fold.is_some());
    assert!(!out[1].fold_only);
}

#[test]
fn leaves_a_turn_with_nothing_to_fold_as_it_is() {
    let m = vec![user("u"), said(agent("a"), "hi")];
    assert_eq!(fold(&m, false, false, None, &none())[1], m[1]);
}

#[test]
fn counts_several_edits_of_one_file_as_one_file_edited() {
    let m = vec![user("u"), said(with_tools(agent("a"), vec![edit("e1", "a.ts"), edit("e2", "/work/demo/a.ts")]), "ok")];
    assert_eq!(fold(&m, false, false, Some("/work/demo"), &none())[1].fold.as_ref().unwrap().summary.edits, 1);
}

#[test]
fn puts_nothing_after_the_answer_of_a_turn_that_changed_files_no_changed_files_card() {
    let open: HashSet<String> = ["u1".to_string()].into();
    let out = fold(&turn(), false, false, Some("/work/demo"), &open);
    // There is no card field on a message at all: folding only adds the fold row to the turn's first agent message.
    assert_eq!(out.iter().filter(|m| m.fold.is_some()).count(), 1);
    assert_eq!(out.last().unwrap().id, "a3");
}

// ─── whole-file writes ──────────────────────────────────────────────────────

fn write_turn() -> Vec<ListMessage> {
    vec![
        said(at(user("u1"), 1000, None), "do it"),
        with_tools(
            at(agent("a1"), 2000, Some(5000)),
            vec![tool("t1", "read"), write("t2", "/work/demo/src/notes.ts"), write("t3", "/work/demo/test/notes.test.ts")],
        ),
        said(at(agent("a2"), 6000, Some(6000)), "Done."),
    ]
}
fn stats() -> FileStats {
    [
        ("src/notes.ts".to_string(), FileStat { additions: 11, deletions: 2 }),
        ("test/notes.test.ts".to_string(), FileStat { additions: 14, deletions: 1 }),
    ]
    .into()
}

#[test]
fn counts_the_files_real_change_from_the_session_diff_not_every_line_written() {
    let out = with_file_stats(&write_turn(), Some(&stats()), Some("/work/demo"), None);
    assert_eq!(out[1].tools[1].diff_stat, Some(FileStat { additions: 11, deletions: 2 }));
    assert_eq!(out[1].tools[2].diff_stat, Some(FileStat { additions: 14, deletions: 1 }));
}

#[test]
fn draws_no_counts_while_the_session_diff_isnt_loaded_rather_than_a_wrong_plus_28() {
    let out = with_file_stats(&write_turn(), None, Some("/work/demo"), None);
    assert_eq!(out[1].tools[1..].iter().map(|t| t.diff_stat).collect::<Vec<_>>(), [None, None]);
}

#[test]
fn counts_a_file_the_session_diff_doesnt_list_as_unchanged() {
    let out = with_file_stats(&write_turn(), Some(&FileStats::new()), Some("/work/demo"), None);
    assert_eq!(out[1].tools[1].diff_stat, Some(FileStat { additions: 0, deletions: 0 }));
}

#[test]
fn counts_a_file_written_twice_as_one_file_edited() {
    let mut m = write_turn();
    m[1].tools = vec![write("t2", "/work/demo/src/notes.ts"), write("t3", "/work/demo/src/notes.ts")];
    let stated = with_file_stats(&m, Some(&stats()), Some("/work/demo"), None);
    assert_eq!(fold(&stated, false, false, Some("/work/demo"), &none())[1].fold.as_ref().unwrap().summary.edits, 1);
}

// ─── regressions in the folded turn ─────────────────────────────────────────

#[test]
fn keeps_a_show_page_row_out_of_the_fold_even_when_the_call_carries_no_page_and_doesnt_count_it() {
    let m = vec![user("u"), with_tools(agent("a"), vec![tool("p", "mcp__fleet__fleet_page_show"), tool("r", "read")])];
    let out = fold(&m, false, false, None, &none());
    assert_eq!(tool_ids(&out[1]), ["p"]);
    let summary = &out[1].fold.as_ref().unwrap().summary;
    assert_eq!((summary.other, summary.reads), (0, 1));
    assert_eq!(summary_parts(summary), ["read 1 file"]);
}

#[test]
fn keeps_every_row_that_opens_something_app_run_browser_walkthrough_canvas_out_of_the_fold_and_doesnt_count_it() {
    for name in ["fleet_app_start", "mcp__fleet__fleet_app_start", "fleet_browser_open", "fleet_walkthrough_show"] {
        let m = vec![user("u"), with_tools(agent("a"), vec![tool("p", name), tool("r", "read")])];
        let out = fold(&m, false, false, None, &none());
        assert_eq!(tool_ids(&out[1]), ["p"], "{name}");
        let summary = &out[1].fold.as_ref().unwrap().summary;
        assert_eq!((summary.other, summary.reads), (0, 1), "{name}");
    }
    let canvas = ListTool { canvas: true, ..tool("c", "unknown_tool") };
    let m = vec![user("u"), with_tools(agent("a"), vec![canvas, tool("r", "read")])];
    assert_eq!(tool_ids(&fold(&m, false, false, None, &none())[1]), ["c"]);
}

#[test]
fn doesnt_count_exit_plan_mode_as_a_read() {
    let m = vec![user("u"), with_tools(agent("a"), vec![tool("x", "ExitPlanMode")])];
    let summary = fold(&m, false, false, None, &none())[1].fold.clone().unwrap().summary;
    assert_eq!((summary.reads, summary.edits, summary.other), (0, 0, 1));
}

#[test]
fn marks_a_turn_that_failed_or_was_stopped_so_no_tick_is_drawn_for_it() {
    let mut failing = agent("b");
    failing.turn_error = Some(TurnError { name: "x".into(), message: "boom".into(), is_retryable: false });
    let failed = vec![user("u"), with_tools(agent("a"), vec![tool("t", "read")]), failing];
    assert_eq!(fold(&failed, false, false, None, &none())[1].fold.as_ref().unwrap().summary.outcome, Outcome::Failed);
    let stopped = vec![user("u"), with_tools(agent("a"), vec![tool("r", "read"), ListTool::new("t", "bash", ToolStatus::Stopped)])];
    assert_eq!(fold(&stopped, false, false, None, &none())[1].fold.as_ref().unwrap().summary.outcome, Outcome::Stopped);
    assert_eq!(fold(&turn(), false, false, None, &none())[1].fold.as_ref().unwrap().summary.outcome, Outcome::Done);
}

#[test]
fn ends_worked_for_with_the_turns_own_work_not_a_later_background_notice_or_retry() {
    let mut notice = at(agent("n"), 21_000, Some(21_000));
    notice.background = true;
    let m = vec![
        at(user("u"), 1000, None),
        with_tools(at(agent("a"), 1500, Some(2000)), vec![tool("t", "bash")]),
        notice,
        said(at(agent("r"), 22_000, Some(23_000)), "Picked it up."),
    ];
    assert_eq!(fold(&m, false, false, None, &none())[1].fold.as_ref().unwrap().summary.duration_ms, Some(1000));
    let mut retried = with_tools(at(agent("a"), 1500, Some(2000)), vec![tool("t", "read")]);
    retried.turn_error = Some(TurnError { name: "x".into(), message: "m".into(), is_retryable: true });
    let retry = vec![at(user("u"), 1000, None), retried, said(at(agent("b"), 90_000, Some(95_000)), "later")];
    assert_eq!(fold(&retry, false, false, None, &none())[1].fold.as_ref().unwrap().summary.duration_ms, Some(1000));
}

#[test]
fn relativises_windows_paths_either_slash_any_drive_case_so_a_write_row_finds_its_stats() {
    assert_eq!(relative_path("C:\\work\\demo\\src\\a.ts", Some("c:\\work\\demo")), "src/a.ts");
    assert_eq!(relative_path("/work/demo/src/a.ts", Some("/work/demo/")), "src/a.ts");
    assert_eq!(relative_path("/elsewhere/a.ts", Some("/work/demo")), "/elsewhere/a.ts");
}

mod past_turns_keep_what_they_changed {
    use super::*;

    fn two() -> Vec<ListMessage> {
        vec![
            user("u1"),
            said(with_tools(agent("a1"), vec![write("w1", "/work/demo/README.md")]), "ok"),
            user("u2"),
            said(agent("a2"), "ok"),
        ]
    }
    fn stat_of(messages: &[ListMessage]) -> Option<FileStat> {
        messages.iter().find(|x| x.id == "a1").unwrap().tools[0].diff_stat
    }
    fn timed(mut m: Vec<ListMessage>) -> Vec<ListMessage> {
        m[0].created_at = Some(1000);
        m[1].created_at = Some(2000);
        m[1].completed_at = Some(3000);
        m[2].created_at = Some(10_000);
        m
    }
    fn readme(additions: u32, deletions: u32) -> FileStats {
        [("README.md".to_string(), FileStat { additions, deletions })].into()
    }

    #[test]
    fn doesnt_recount_an_earlier_turn_from_todays_diff_unknown_draws_no_counts() {
        assert_eq!(stat_of(&with_file_stats(&two(), Some(&readme(0, 3)), Some("/work/demo"), None)), None);
    }

    #[test]
    fn uses_the_snapshot_the_server_took_as_the_turn_ended_whatever_todays_diff_says() {
        let snapshots = [TurnStatsSnapshot { ended_at: 3500, files: readme(3, 1) }];
        let out = with_file_stats(&timed(two()), Some(&readme(0, 3)), Some("/work/demo"), Some(&snapshots));
        assert_eq!(stat_of(&out), Some(FileStat { additions: 3, deletions: 1 }));
    }

    #[test]
    fn never_takes_a_later_turns_snapshot_for_an_earlier_turn() {
        let later = [TurnStatsSnapshot { ended_at: 20_000, files: readme(9, 9) }];
        let out = with_file_stats(&timed(two()), Some(&FileStats::new()), Some("/work/demo"), Some(&later));
        assert_eq!(stat_of(&out), None);
    }

    #[test]
    fn a_snapshot_that_lists_no_such_file_means_the_turn_left_it_unchanged() {
        let m = timed(two())[..2].to_vec();
        let out = with_file_stats(&m, None, Some("/work/demo"), Some(&[TurnStatsSnapshot { ended_at: 3100, files: FileStats::new() }]));
        assert_eq!(stat_of(&out), Some(FileStat { additions: 0, deletions: 0 }));
    }

    #[test]
    fn a_file_outside_the_sessions_folder_is_unknown_not_plus_0_minus_0() {
        let mut m = two()[..2].to_vec();
        m[1].tools = vec![write("w9", "/other/place/x.md")];
        assert_eq!(stat_of(&with_file_stats(&m, Some(&FileStats::new()), Some("/work/demo"), None)), None);
    }
}

// ─── turn-fold-row-stats.test.ts ────────────────────────────────────────────

mod a_write_rows_counts_come_from_its_own_turns_snapshot {
    use super::*;

    // Two quick turns, as a fast model runs them: the second's prompt comes under two seconds after the first turn's
    // snapshot. The second writes a new file; only its own snapshot lists it.
    fn turns() -> Vec<ListMessage> {
        vec![
            at(user("u1"), 30_000, None),
            with_tools(at(agent("a1"), 31_000, Some(34_500)), vec![write("w1", "/work/demo/src/notes.ts")]),
            at(user("u2"), 35_500, None),
            with_tools(at(agent("a2"), 36_000, Some(36_600)), vec![write("w2", "/work/demo/src/archive.js")]),
        ]
    }
    fn snap(ended_at: i64, files: &[(&str, u32, u32)]) -> TurnStatsSnapshot {
        TurnStatsSnapshot {
            ended_at,
            files: files.iter().map(|&(p, additions, deletions)| (p.to_string(), FileStat { additions, deletions })).collect(),
        }
    }
    fn snapshots() -> Vec<TurnStatsSnapshot> {
        vec![snap(34_860, &[("src/notes.ts", 1, 0)]), snap(37_000, &[("src/notes.ts", 1, 0), ("src/archive.js", 3, 0)])]
    }

    #[test]
    fn counts_a_new_file_written_in_a_turn_that_follows_close_behind_the_last_one_not_plus_0_minus_0_from_the_turn_before() {
        let out = with_file_stats(&turns(), None, Some("/work/demo"), Some(&snapshots()));
        assert_eq!(out.iter().find(|m| m.id == "a2").unwrap().tools[0].diff_stat, Some(FileStat { additions: 3, deletions: 0 }));
    }

    #[test]
    fn keeps_the_earlier_turn_on_its_own_snapshot() {
        let out = with_file_stats(&turns(), None, Some("/work/demo"), Some(&snapshots()));
        assert_eq!(out.iter().find(|m| m.id == "a1").unwrap().tools[0].diff_stat, Some(FileStat { additions: 1, deletions: 0 }));
    }
}

// ─── what the stream adds around folding (ActivityStream.vue) ────────────────

#[test]
fn a_turn_you_watched_stays_open_until_the_next_prompt_and_a_new_session_starts_unwatched() {
    let mut watched = WatchedTurn::default();
    assert!(!watched.watched());
    watched.on_working(true);
    watched.on_working(false);
    assert!(watched.watched());
    watched.on_session_change();
    assert!(!watched.watched());
}

#[test]
fn the_fold_row_opens_and_closes_its_turn_and_turn_ids_name_each_message_turn() {
    let mut open = none();
    toggle_turn(&mut open, "u1");
    assert!(open.contains("u1"));
    toggle_turn(&mut open, "u1");
    assert!(open.is_empty());
    let mut m = turn();
    m.insert(0, agent("orphan"));
    m.push(shell("sh"));
    m.push(agent("after-shell"));
    let ids = turn_ids_of(&m);
    assert_eq!(ids["orphan"], "orphan");
    assert_eq!((ids["u1"].as_str(), ids["a3"].as_str()), ("u1", "u1"));
    // A command of yours ends the turn: what follows it is a turn of its own, named by its first agent message.
    assert_eq!(ids["after-shell"], "after-shell");
    assert!(!ids.contains_key("sh"));
}

fn shell(id: &str) -> ListMessage {
    msg(id, Role::Shell)
}
