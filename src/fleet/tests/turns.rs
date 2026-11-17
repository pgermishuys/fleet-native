//! Turns, the logic (component 10, piece I): `lib/__tests__/turns.test.ts` under Fleet's names, the `turns` column of
//! `tool-classification.test.ts`, and cases from reading `TurnsCanvas.vue` for its words and the cache.
use super::super::delegations::{Delegation, DelegationStatus};
use super::super::model::{Msg, PartData, SlashCommand, Tokens};
use super::super::turns::*;
use crate::composer::choice::ModelOption;
use crate::conv::diff::{DiffLine, Kind, parse_unified_diff};
use crate::conv::fold::FileStats;
use crate::conv::message_list::{FileStat, FileStatus};
use crate::view::conversation::Role;
use serde_json::{Value, json};
use std::cell::Cell;

thread_local! { static PARTS: Cell<u32> = const { Cell::new(0) }; }

fn next_part() -> u32 {
    PARTS.with(|c| {
        c.set(c.get() + 1);
        c.get()
    })
}

fn text(body: &str) -> PartData {
    PartData::Text { id: format!("text-{}", next_part()), text: body.into() }
}

fn tool(name: &str, state: Value) -> PartData {
    let call = format!("call-{}", next_part());
    tool_call(name, state, &call)
}

fn tool_call(name: &str, state: Value, call: &str) -> PartData {
    PartData::Tool { id: format!("tool-{call}"), tool: name.into(), call_id: call.into(), state }
}

fn user(id: &str, body: &str, created: i64) -> Msg {
    Msg { parts: vec![text(body)], created_at: Some(created), ..Msg::new(id, "s1", Role::User) }
}

fn assistant(id: &str, parts: Vec<PartData>) -> Msg {
    Msg {
        parts,
        created_at: Some(2_000),
        agent: Some("build".into()),
        model_id: Some("claude-opus-5".into()),
        ..Msg::new(id, "s1", Role::Assistant)
    }
}

fn at(mut m: Msg, created: i64) -> Msg {
    m.created_at = Some(created);
    m
}

fn model(mut m: Msg, id: &str) -> Msg {
    m.model_id = Some(id.into());
    m
}

fn derive(messages: &[Msg]) -> Vec<SessionTurn> {
    derive_turns(messages, &[], TurnOptions::default())
}

fn stats(entries: &[(&str, u32, u32, Option<FileStatus>)]) -> FileStats {
    entries.iter().map(|&(p, additions, deletions, status)| (p.to_string(), FileStat { additions, deletions, status })).collect()
}

fn with_stats(messages: &[Msg], stats: Option<&FileStats>, directory: Option<&str>) -> Vec<SessionTurn> {
    derive_turns(messages, &[], TurnOptions { file_stats: stats, directory })
}

fn diff_line(kind: &str, content: &str, n: u32) -> Value {
    if kind == "add" {
        json!({"type": "add", "content": content, "newLineNumber": n})
    } else {
        json!({"type": "remove", "content": content, "oldLineNumber": n})
    }
}

fn edit_state(path: &str, lines: Vec<Value>) -> Value {
    json!({"status": "completed", "input": {"filePath": path}, "diffLines": lines})
}

// ─── lib/__tests__/turns.test.ts ────────────────────────────────────────────

#[test]
fn round_boundaries_starts_a_round_at_each_user_message_and_runs_it_to_the_next_one() {
    let turns = derive(&[
        user("u1", "First ask", 1_000),
        at(assistant("a1", vec![text("working")]), 1_100),
        at(assistant("a2", vec![text("still working")]), 1_200),
        user("u2", "Second ask", 2_000),
        at(assistant("a3", vec![text("done")]), 2_100),
    ]);
    assert_eq!(turns.len(), 2);
    // Newest first, matching the sidebar.
    assert_eq!((turns[0].prompt.as_str(), turns[0].number), ("Second ask", 2));
    assert_eq!((turns[1].prompt.as_str(), turns[1].number), ("First ask", 1));
    assert_eq!(turns[1].id, "u1");
}

#[test]
fn round_boundaries_shows_a_slash_command_as_it_was_sent_not_the_prompt_it_expanded_to() {
    let mut prompt = user("u1", "Tidy up the code in src/auth without changing what it does.", 1_000);
    prompt.command = Some(SlashCommand { name: "tidy".into(), arguments: Some("src/auth".into()) });
    let turns = derive(&[prompt, assistant("a1", vec![text("ok")])]);
    assert_eq!(turns[0].prompt, "/tidy src/auth");

    let mut bare = user("u1", "Expanded.", 1_000);
    bare.command = Some(SlashCommand { name: "tidy".into(), arguments: None });
    assert_eq!(derive(&[bare])[0].prompt, "/tidy");
}

#[test]
fn round_boundaries_shows_the_prompts_first_line_only_and_keeps_the_whole_prompt_for_the_tooltip() {
    let turns = derive(&[user("u1", "Fix the header\n\nIt wraps on phones.", 1_000), assistant("a1", vec![text("ok")])]);
    assert_eq!(turns[0].prompt, "Fix the header");
    assert!(turns[0].prompt_full.contains("It wraps on phones."));
}

#[test]
fn round_boundaries_keeps_assistant_messages_older_than_the_loaded_history_in_a_round_with_no_prompt() {
    let turns = derive(&[
        at(assistant("a0", vec![text("tail of an earlier round")]), 500),
        user("u1", "Next ask", 1_000),
        at(assistant("a1", vec![text("ok")]), 1_100),
    ]);
    assert_eq!(turns.len(), 2);
    assert!(!turns[1].has_prompt);
    assert!(turns[0].has_prompt);
    assert_eq!(prompt_line(&turns[1]), "Before the loaded history");
}

#[test]
fn round_boundaries_gives_a_prompt_sent_twice_in_a_row_its_own_round_each_time() {
    let turns = derive(&[user("u1", "One", 1_000), user("u2", "Two", 1_100), assistant("a1", vec![text("ok")])]);
    assert_eq!(turns.iter().map(|t| t.prompt.as_str()).collect::<Vec<_>>(), ["Two", "One"]);
    assert_eq!(turns[1].tool_count, 0);
}

#[test]
fn the_model_that_ran_the_round_takes_the_last_model_and_flags_a_round_that_changed_model() {
    let turns = derive(&[
        user("u1", "Drop to Sonnet for the rest", 1_000),
        model(assistant("a1", vec![text("ok")]), "claude-opus-5"),
        model(assistant("a2", vec![text("carrying on")]), "claude-sonnet-5"),
    ]);
    assert_eq!(turns[0].model_id.as_deref(), Some("claude-sonnet-5"));
    assert!(turns[0].model_changed);
}

#[test]
fn the_model_that_ran_the_round_does_not_flag_a_round_that_stayed_on_one_model() {
    let turns = derive(&[
        user("u1", "Carry on", 1_000),
        model(assistant("a1", vec![text("ok")]), "claude-opus-5"),
        model(assistant("a2", vec![text("more")]), "claude-opus-5"),
    ]);
    assert!(!turns[0].model_changed);
    assert_eq!(turns[0].model_id.as_deref(), Some("claude-opus-5"));
}

#[test]
fn the_model_that_ran_the_round_resolves_a_model_id_to_its_catalog_name_qualified_or_bare() {
    let models = [ModelOption { id: "claude-opus-5".into(), name: "Opus 5".into(), ..Default::default() }];
    assert_eq!(model_display_name(Some("claude-opus-5"), &models), "Opus 5");
    assert_eq!(model_display_name(Some("anthropic/claude-opus-5"), &models), "Opus 5");
    assert_eq!(model_display_name(Some("some-unknown-model"), &models), "some-unknown-model");
    assert_eq!(model_display_name(None, &models), "Unknown model");
}

#[test]
fn what_a_round_wrote_reads_a_file_and_its_plus_minus_from_the_diff_the_harness_attached() {
    let turns = derive(&[
        user("u1", "Fix the status", 1_000),
        assistant(
            "a1",
            vec![tool(
                "edit",
                edit_state(
                    "src/WeaveFleet.Application/SessionStatusDeriver.cs",
                    vec![
                        diff_line("remove", "return SessionStatus.Idle;", 118),
                        diff_line("add", "return pendingQuestion is not null", 118),
                        diff_line("add", "    ? SessionStatus.WaitingInput", 119),
                    ],
                ),
            )],
        ),
    ]);
    let turn = &turns[0];
    assert_eq!(turn.files.len(), 1);
    let file = &turn.files[0];
    assert_eq!(file.name, "SessionStatusDeriver.cs");
    assert_eq!(file.dir, "src/WeaveFleet.Application");
    assert_eq!((file.additions, file.deletions), (2, 1));
    assert_eq!((turn.additions, turn.deletions), (2, 1));
    assert!(!turn.read_only);
    assert_eq!(turn.edit_count, 1);
}

#[test]
fn what_a_round_wrote_parses_a_unified_diff_the_harness_left_on_the_calls_metadata() {
    let diff = ["@@ -10,2 +10,3 @@", " const a = 1;", "-const b = 2;", "+const b = 3;", "+const c = 4;"].join("\n");
    let turns = derive(&[
        user("u1", "Patch it", 1_000),
        assistant(
            "a1",
            vec![tool(
                "edit",
                json!({"status": "completed", "input": {"filePath": "client/src/lib/turns.ts"}, "metadata": {"diff": diff}}),
            )],
        ),
    ]);
    let file = &turns[0].files[0];
    assert_eq!((file.additions, file.deletions), (2, 1));
    assert_eq!(file.diff.iter().filter(|l| l.kind == Kind::Context).count(), 1);
}

#[test]
fn what_a_round_wrote_works_the_lines_out_from_the_input_when_no_diff_came_with_the_call() {
    let state = json!({"status": "completed", "input": {"file_path": "client/src/app.ts", "old_string": "const a = 1;", "new_string": "const alpha = 1;"}});
    let turns = derive(&[user("u1", "Rename it", 1_000), assistant("a1", vec![tool("Edit", state)])]);
    let file = &turns[0].files[0];
    assert_eq!(file.path, "client/src/app.ts");
    assert_eq!((file.additions, file.deletions), (1, 1));
}

#[test]
fn what_a_round_wrote_marks_a_written_file_new_and_counts_what_the_session_diff_says_it_changed() {
    let write = json!({"status": "completed", "input": {"filePath": "client/src/composables/use-turns.ts", "content": "export function useTurns() {}\n"}});
    let messages = [user("u1", "Add the composable", 1_000), assistant("a1", vec![tool("write", write)])];
    let diff = stats(&[("client/src/composables/use-turns.ts", 1, 0, Some(FileStatus::Added))]);
    let turns = with_stats(&messages, Some(&diff), None);
    let file = &turns[0].files[0];
    assert!(file.created);
    assert_eq!((file.additions, file.deletions), (1, 0));
}

#[test]
fn what_a_round_wrote_adds_up_several_edits_of_the_same_file_in_one_round() {
    let turns = derive(&[
        user("u1", "Two passes", 1_000),
        assistant(
            "a1",
            vec![
                tool("edit", edit_state("a.ts", vec![diff_line("add", "one", 1)])),
                tool("edit", edit_state("a.ts", vec![diff_line("add", "two", 2), diff_line("remove", "gone", 9)])),
            ],
        ),
    ]);
    assert_eq!(turns[0].files.len(), 1);
    assert_eq!((turns[0].files[0].additions, turns[0].files[0].deletions), (2, 1));
    assert_eq!(turns[0].files[0].diff.len(), 3);
    assert_eq!(turns[0].edit_count, 2);
}

#[test]
fn what_a_round_wrote_ignores_an_edit_that_failed_or_has_not_finished() {
    let turns = derive(&[
        user("u1", "Try it", 1_000),
        assistant(
            "a1",
            vec![
                tool("edit", json!({"status": "error", "input": {"filePath": "a.ts", "newString": "x"}})),
                tool("edit", json!({"status": "running", "input": {"filePath": "b.ts", "newString": "y"}})),
            ],
        ),
    ]);
    assert!(turns[0].files.is_empty());
    assert!(turns[0].read_only);
    assert_eq!(turns[0].tool_count, 2);
}

#[test]
fn what_a_round_wrote_gives_a_round_that_only_read_things_a_row_that_says_so() {
    let turns = derive(&[
        user("u1", "Why is the dashboard idle?", 1_000),
        assistant(
            "a1",
            vec![
                tool("read", json!({"status": "completed", "input": {"filePath": "a.ts"}})),
                tool("grep", json!({"status": "completed", "input": {"pattern": "idle"}})),
                tool("bash", json!({"status": "completed", "input": {"command": "dotnet test", "description": "Run tests"}})),
            ],
        ),
    ]);
    let turn = &turns[0];
    assert!(turn.read_only);
    assert_eq!((turn.tool_count, turn.edit_count), (3, 0));
    assert_eq!(turn.commands, [TurnCommand { label: "dotnet test".into(), ok: Some(true) }]);
}

#[test]
fn what_a_round_wrote_names_every_file_of_a_multi_file_patch() {
    let patch = [
        "--- a/src/one.ts",
        "+++ b/src/one.ts",
        "@@ -1,1 +1,1 @@",
        "-old",
        "+new",
        "--- /dev/null",
        "+++ b/src/two.ts",
        "@@ -0,0 +1,2 @@",
        "+first",
        "+second",
    ]
    .join("\n");
    let turns = derive(&[
        user("u1", "Apply the patch", 1_000),
        assistant("a1", vec![tool("apply_patch", json!({"status": "completed", "input": {"patchText": patch}}))]),
    ]);
    let turn = &turns[0];
    assert_eq!(turn.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["src/one.ts", "src/two.ts"]);
    assert!(turn.files[1].created);
    assert_eq!((turn.additions, turn.deletions), (3, 1));
}

#[test]
fn what_a_round_wrote_keeps_a_removed_sql_comment_in_its_file_instead_of_starting_a_new_one() {
    // Real `git diff` output: the removed line "-- drop" reads "--- drop".
    let patch = [
        "diff --git a/q.sql b/q.sql",
        "index 1111111..2222222 100644",
        "--- a/q.sql",
        "+++ b/q.sql",
        "@@ -1,3 +1,2 @@",
        " select 1;",
        "--- drop",
        "+-- keep",
        " select 2;",
        "diff --git a/b.sql b/b.sql",
        "--- a/b.sql",
        "+++ b/b.sql",
        "@@ -1 +1 @@",
        "-x",
        "+y",
    ]
    .join("\n");
    let turns = derive(&[
        user("u1", "Apply the patch", 1_000),
        assistant("a1", vec![tool("apply_patch", json!({"status": "completed", "input": {"patchText": patch}}))]),
    ]);
    let files = &turns[0].files;
    assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["q.sql", "b.sql"]);
    assert_eq!((files[0].deletions, files[0].additions), (1, 1));
}

#[test]
fn cost_timing_and_totals_adds_up_the_rounds_tokens_cost_and_time() {
    let mut first = at(assistant("a1", vec![text("part one")]), 11_000);
    (first.completed_at, first.cost, first.tokens) = (Some(12_000), Some(0.1), Some(Tokens { input: 1_000, output: 100, reasoning: 0 }));
    let mut second = at(assistant("a2", vec![text("part two")]), 12_000);
    (second.completed_at, second.cost, second.tokens) =
        (Some(25_000), Some(0.09), Some(Tokens { input: 2_000, output: 200, reasoning: 0 }));
    let turns = derive(&[user("u1", "Do it", 10_000), first, second]);
    let turn = &turns[0];
    assert!((turn.cost - 0.19).abs() < 1e-9);
    assert_eq!((turn.tokens_input, turn.tokens_output), (3_000, 300));
    assert_eq!(turn.duration_ms, Some(15_000));
    assert_eq!(turn.agent.as_deref(), Some("build"));
}

#[test]
fn cost_timing_and_totals_totals_the_session_across_its_rounds() {
    let turns = derive(&[
        user("u1", "One", 1_000),
        assistant("a1", vec![tool("edit", edit_state("a.ts", vec![diff_line("add", "x", 1)]))]),
        user("u2", "Two", 2_000),
        assistant("a2", vec![tool("edit", edit_state("b.ts", vec![diff_line("remove", "y", 3)]))]),
    ]);
    assert_eq!(turn_totals(&turns), TurnTotals { turns: 2, additions: 1, deletions: 1 });
}

fn delegation(parent: Option<&str>, created_ms: i64) -> Delegation {
    Delegation {
        delegation_id: "d1".into(),
        parent_tool_call_id: parent.map(str::to_string),
        child_session_id: Some("child-1".into()),
        title: "explore".into(),
        status: DelegationStatus::Completed,
        created_at: Some(iso(created_ms)),
        child_activity_status: None,
        background: false,
    }
}

/// An ISO time for a millisecond count, as the server writes it.
fn iso(ms: i64) -> String {
    format!("1970-01-01T00:00:{:02}.{:03}Z", ms / 1000, ms % 1000)
}

#[test]
fn subagents_puts_a_subagent_in_the_round_whose_tool_call_started_it() {
    let messages = [
        user("u1", "First", 1_000),
        at(
            assistant("a1", vec![tool_call("task", json!({"status": "completed", "input": {"description": "explore"}}), "call-task-1")]),
            1_100,
        ),
        user("u2", "Second", 5_000),
        at(assistant("a2", vec![text("done")]), 5_100),
    ];
    let turns = derive_turns(&messages, &[delegation(Some("call-task-1"), 1_500)], TurnOptions::default());
    assert!(turns[0].delegations.is_empty());
    assert_eq!(turns[1].delegations.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["explore"]);
}

#[test]
fn subagents_falls_back_to_when_the_subagent_started_when_its_call_id_is_unknown() {
    let messages = [
        user("u1", "First", 1_000),
        at(assistant("a1", vec![text("ok")]), 1_100),
        user("u2", "Second", 5_000),
        at(assistant("a2", vec![text("ok")]), 5_100),
    ];
    // Newest first: the sub-agent, created at 5.4 s, belongs to the round that began at 5 s, not the one at 1 s.
    let turns = derive_turns(&messages, &[delegation(None, 5_400)], TurnOptions::default());
    assert_eq!(turns[0].delegations.len(), 1);
    assert!(turns[1].delegations.is_empty());
}

#[test]
fn parse_unified_diff_numbers_lines_from_the_hunk_header_and_spots_a_created_file() {
    let parsed = parse_unified_diff(&["--- /dev/null", "+++ b/new.ts", "@@ -0,0 +1,2 @@", "+one", "+two"].join("\n"));
    assert!(parsed.created);
    assert_eq!(parsed.paths, ["new.ts"]);
    let line = |content: &str, new: u32| DiffLine { kind: Kind::Add, content: content.into(), old: None, new: Some(new) };
    assert_eq!(parsed.lines, [line("one", 1), line("two", 2)]);
}

#[test]
fn parse_unified_diff_returns_nothing_for_text_that_is_not_a_diff() {
    assert!(parse_unified_diff("just some output").lines.is_empty());
}

#[test]
fn an_empty_session_has_no_turns() {
    assert!(derive(&[]).is_empty());
    assert_eq!(turn_totals(&[]), TurnTotals::default());
}

fn whole_write(path: &str, content: &str) -> Value {
    json!({"status": "completed", "input": {"filePath": path, "content": content}})
}

#[test]
fn whole_file_writes_marks_a_write_that_sent_only_the_files_new_contents_whose_lines_arent_the_change() {
    let edits = crate::conv::diff::file_edits_of("write", &whole_write("/w/a.ts", "a\nb\n"));
    assert_eq!((edits[0].path.as_str(), edits[0].additions, edits[0].whole_file), ("/w/a.ts", 2, true));
}

#[test]
fn whole_file_writes_does_not_mark_an_edit_whose_old_and_new_text_give_the_change() {
    let state = json!({"status": "completed", "input": {"filePath": "/w/a.ts", "oldString": "a", "newString": "b"}});
    assert!(!crate::conv::diff::file_edits_of("edit", &state)[0].whole_file);
}

fn rewrite_rounds() -> Vec<Msg> {
    vec![user("u1", "Rewrite it", 1_000), at(assistant("a1", vec![tool("write", whole_write("/w/src/a.ts", "1\n2\n3\n4\n5\n"))]), 1_100)]
}

#[test]
fn whole_file_writes_in_a_turn_takes_the_files_counts_from_the_session_diff_not_every_line_written() {
    let diff = stats(&[("src/a.ts", 1, 1, None)]);
    let turns = with_stats(&rewrite_rounds(), Some(&diff), Some("/w"));
    assert_eq!((turns[0].additions, turns[0].deletions), (1, 1));
    assert_eq!((turns[0].files[0].additions, turns[0].files[0].deletions), (1, 1));
}

#[test]
fn whole_file_writes_in_a_turn_counts_nothing_until_the_session_diff_is_known_rather_than_every_line() {
    let turns = with_stats(&rewrite_rounds(), None, Some("/w"));
    assert_eq!((turns[0].additions, turns[0].deletions), (0, 0));
    assert_eq!(turns[0].files.len(), 1);
}

#[test]
fn whole_file_writes_in_a_turn_leaves_an_edits_own_counts_alone() {
    let state = json!({"status": "completed", "input": {"filePath": "/w/src/a.ts", "oldString": "a", "newString": "b"}});
    let messages = [user("u1", "x", 1_000), assistant("a1", vec![tool("edit", state)])];
    let turns = with_stats(&messages, Some(&FileStats::new()), Some("/w"));
    assert_eq!((turns[0].additions, turns[0].deletions), (1, 1));
}

#[test]
fn whole_file_writes_in_a_turn_calls_a_rewritten_existing_file_changed_not_new_a_write_is_not_always_a_create() {
    let diff = stats(&[("src/a.ts", 1, 1, Some(FileStatus::Modified))]);
    assert!(!with_stats(&rewrite_rounds(), Some(&diff), Some("/w"))[0].files[0].created);
}

#[test]
fn whole_file_writes_in_a_turn_calls_a_file_new_only_when_the_session_diff_says_it_was_added() {
    let diff = stats(&[("src/a.ts", 5, 0, Some(FileStatus::Added))]);
    assert!(with_stats(&rewrite_rounds(), Some(&diff), Some("/w"))[0].files[0].created);
}

#[test]
fn whole_file_writes_in_a_turn_does_not_call_a_file_new_before_the_session_diff_is_known() {
    assert!(!with_stats(&rewrite_rounds(), None, Some("/w"))[0].files[0].created);
}

#[test]
fn whole_file_writes_in_a_turn_calls_a_file_new_in_the_turn_that_added_it_not_in_a_later_turn_that_rewrites_it() {
    let write = |content: &str| tool("write", whole_write("/w/src/a.ts", content));
    let messages = [
        user("u1", "Add it", 1_000),
        at(assistant("a1", vec![write("1\n")]), 1_100),
        user("u2", "Rewrite it", 2_000),
        at(assistant("a2", vec![write("2\n")]), 2_100),
    ];
    let diff = stats(&[("src/a.ts", 1, 0, Some(FileStatus::Added))]);
    let turns = with_stats(&messages, Some(&diff), Some("/w"));
    // Newest first.
    assert_eq!(turns.iter().map(|t| t.files[0].created).collect::<Vec<_>>(), [false, true]);
}

// ─── lib/__tests__/tool-classification.test.ts: the `turns` column ─────────────────────────────────────────────────

/// Every name in Fleet's table, called once with its rich input; what the turns make of it: (shell, file, created).
#[test]
fn every_name_in_the_table_is_read_by_turns_as_the_client_does_today() {
    let rich = json!({
        "filePath": "src/widgets/panel.ts", "command": "dotnet test", "pattern": "*.ts", "url": "https://example.test/docs",
        "query": "reconnect backoff", "name": "fleet-run", "agent": "explore", "description": "Check the panel",
        "code": "await page.open()\nsecond line", "questions": [{"question": "Which folder?", "header": "Folder"}],
        "title": "Panel", "content": "one\ntwo",
    });
    let files =
        ["write", "edit", "patch", "apply_patch", "multiedit", "notebookedit", "strreplaceeditor", "Edit", "MultiEdit", "NotebookEdit"];
    let shells = ["bash", "terminal", "Bash", "shell", "SHELL"];
    let others = [
        "read",
        "list",
        "glob",
        "grep",
        "webfetch",
        "websearch",
        "task",
        "skill",
        "question",
        "todowrite",
        "todoread",
        "codesearch",
        "lsp",
        "Read",
        "Task",
        "TodoWrite",
        "WebFetch",
        "Agent",
        "LS",
        "execute",
        "subagent",
        "Subagent",
        "fleet_app_start",
        "fleet_browser_open",
        "fleet_browser_screenshot",
        "fleet_browser_read",
        "fleet_browser_act",
        "fleet_page_show",
        "fleet_walkthrough_show",
        "fleet_message",
        "fleet_session_read",
        "fleet_machine_list",
        "fleet_session_start",
        "fleet_memory_save",
        "fleet_memory_forget",
        "fleet_mod_check",
        "fleet_mod_reload",
        "fleet_mod_keep",
        "fleet_step_done",
        "fleet_canvas_open",
        "mcp__acme__lookup_order",
        "frobnicate",
        "",
    ];
    for name in files.iter().chain(&shells).chain(&others) {
        let messages = [user("u1", "go", 1), at(assistant("a1", vec![tool(name, json!({"status": "completed", "input": rich}))]), 2)];
        let turn = &derive(&messages)[0];
        let row = (!turn.commands.is_empty(), !turn.files.is_empty(), turn.files.first().is_some_and(|f| f.created));
        let want = (shells.contains(name), files.contains(name), false);
        assert_eq!(row, want, "{name}");
    }
}

#[test]
fn execute_is_a_code_tool_not_a_command_in_turns() {
    let state = json!({"status": "completed", "input": {"code": "await page.open()", "description": "Open it"}});
    let turns = derive(&[user("u1", "go", 1), assistant("a1", vec![tool("execute", state)])]);
    assert!(turns[0].commands.is_empty());
    assert_eq!(turns[0].tool_count, 1);
}

// ─── Cases from reading TurnsCanvas.vue and turns.ts ───────────────────────────────────────────────────────────────

#[test]
fn a_command_chip_has_the_first_four_commands_their_first_line_cut_and_how_they_ended() {
    let long = format!("{}\nsecond line", "x".repeat(60));
    let calls = [
        ("completed", "cargo test"),
        ("error", "cargo clippy"),
        ("running", "\n\n  npm run build  \nmore"),
        ("completed", long.as_str()),
        ("completed", "a fifth"),
    ];
    let parts = calls.iter().map(|(s, c)| tool("bash", json!({"status": s, "input": {"command": c}}))).collect();
    let turn = &derive(&[user("u1", "go", 1), assistant("a1", parts)])[0];
    assert_eq!(turn.tool_count, 5);
    assert_eq!(turn.commands.len(), 4);
    assert_eq!(turn.commands.iter().map(command_outcome).collect::<Vec<_>>(), [Some("passed"), Some("failed"), None, Some("passed")]);
    assert_eq!(turn.commands[2].label, "npm run build");
    assert_eq!(turn.commands[3].label, format!("{}…", "x".repeat(47)));
}

#[test]
fn a_command_chip_falls_back_to_the_description_and_a_call_with_neither_has_none() {
    let parts = vec![
        tool("bash", json!({"status": "completed", "input": {"description": "Run the tests"}})),
        tool("bash", json!({"status": "completed", "input": {}})),
    ];
    let turn = &derive(&[user("u1", "go", 1), assistant("a1", parts)])[0];
    assert_eq!(turn.commands, [TurnCommand { label: "Run the tests".into(), ok: Some(true) }]);
    assert_eq!(turn.tool_count, 2);
}

#[test]
fn a_long_prompt_line_is_cut_at_160_characters_and_a_prompt_with_no_text_says_so() {
    let long = "p".repeat(200);
    let turns = derive(&[user("u1", &long, 1), user("u2", "  \n ", 2)]);
    assert_eq!(turns[1].prompt, format!("{}…", "p".repeat(159)));
    assert_eq!(turns[1].prompt_full, long);
    assert_eq!(turns[0].prompt, "");
    assert_eq!(prompt_line(&turns[0]), "(no text in this prompt)");
    let exact = "q".repeat(160);
    assert_eq!(derive(&[user("u1", &exact, 1)])[0].prompt, exact);
}

#[test]
fn the_time_a_round_took_needs_the_end_to_be_after_the_start_and_reads_every_message_for_it() {
    // The end is the latest of completedAt ?? createdAt over every message, the user's included.
    let mut a = at(assistant("a1", vec![text("x")]), 3_000);
    a.completed_at = Some(9_000);
    let turn = &derive(&[user("u1", "go", 2_000), a, user("u0", "later", 8_000)])[1];
    assert_eq!((turn.started_at, turn.ended_at, turn.duration_ms), (Some(2_000), Some(9_000), Some(7_000)));
    // Same start and end: none, not zero.
    let none = &derive(&[user("u1", "go", 2_000)])[0];
    assert_eq!((none.ended_at, none.duration_ms), (Some(2_000), None));
    let unknown = Msg { parts: vec![text("go")], ..Msg::new("u1", "s1", Role::User) };
    assert_eq!(derive(&[unknown])[0].duration_ms, None);
}

#[test]
fn the_agent_is_the_first_one_set_and_a_message_with_no_model_keeps_the_last() {
    let mut a1 = assistant("a1", vec![]);
    a1.agent = None;
    let mut a2 = assistant("a2", vec![]);
    a2.agent = Some("plan".into());
    let mut a3 = assistant("a3", vec![]);
    (a3.agent, a3.model_id) = (Some("build".into()), None);
    let turn = &derive(&[user("u1", "go", 1), a1, a2, a3])[0];
    assert_eq!(turn.agent.as_deref(), Some("plan"));
    assert_eq!(turn.model_id.as_deref(), Some("claude-opus-5"));
    assert!(!turn.model_changed);
    // A round that goes opus, sonnet, opus changed model, and ends on opus.
    let back = derive(&[
        user("u1", "go", 1),
        model(assistant("a1", vec![]), "m1"),
        model(assistant("a2", vec![]), "m2"),
        model(assistant("a3", vec![]), "m1"),
    ]);
    assert!(back[0].model_changed);
    assert_eq!(back[0].model_id.as_deref(), Some("m1"));
}

#[test]
fn a_sub_agent_with_neither_a_known_call_nor_a_usable_time_belongs_nowhere() {
    let messages = [user("u1", "First", 5_000)];
    let early = delegation(None, 1_000);
    let mut broken = delegation(Some("no-such-call"), 0);
    broken.created_at = Some("not a time".into());
    let turns = derive_turns(&messages, &[early, broken], TurnOptions::default());
    assert!(turns[0].delegations.is_empty());
}

#[test]
fn a_sub_agent_carries_its_wire_word() {
    let words: Vec<_> = [
        DelegationStatus::Pending,
        DelegationStatus::Running,
        DelegationStatus::Completed,
        DelegationStatus::Error,
        DelegationStatus::Cancelled,
    ]
    .iter()
    .map(|s| s.wire())
    .collect();
    assert_eq!(words, ["pending", "running", "completed", "error", "cancelled"]);
}

#[test]
fn the_summary_bar_says_turn_or_turns_and_groups_the_counts() {
    assert_eq!((turns_text(1), turns_text(0), turns_text(12)), ("1 turn".into(), "0 turns".into(), "12 turns".into()));
    assert_eq!(additions_text(1_234).as_deref(), Some("+1,234"));
    assert_eq!(deletions_text(12).as_deref(), Some("\u{2212}12"));
    assert_eq!((additions_text(0), deletions_text(0)), (None, None));
}

#[test]
fn the_row_facts_say_how_long_and_how_many_files() {
    let mut a = at(assistant("a1", vec![tool("edit", edit_state("a.ts", vec![diff_line("add", "x", 1)]))]), 1_200);
    a.completed_at = Some(1_300);
    let mut turn = derive(&[user("u1", "go", 1_000), a]).remove(0);
    // 300 ms is read as a second at least.
    assert_eq!(took(&turn).as_deref(), Some("1 s"));
    assert_eq!(facts(&turn), "· 1 s · 1 file");
    turn.duration_ms = Some(200_000);
    assert_eq!(facts(&turn), "· 3 min 20 s · 1 file");
    turn.duration_ms = Some(3_900_000);
    assert_eq!(took(&turn).as_deref(), Some("1 h 05 min"));
    turn.duration_ms = None;
    turn.read_only = true;
    assert_eq!(facts(&turn), "· read only");
    turn.read_only = false;
    turn.files.push(turn.files[0].clone());
    assert_eq!(facts(&turn), "· 2 files");
}

#[test]
fn the_meta_line_lists_what_the_round_knows_and_leaves_the_rest_out() {
    let models = [ModelOption { id: "claude-opus-5".into(), name: "Opus 5".into(), ..Default::default() }];
    let mut a = at(assistant("a1", vec![text("x")]), 11_000);
    (a.completed_at, a.cost, a.tokens) = (Some(23_000), Some(0.19), Some(Tokens { input: 3_000, output: 300, reasoning: 0 }));
    let turn = &derive(&[user("u1", "go", 11_000), a])[0];
    let items = meta_items(turn, &models);
    let items: Vec<_> = items.iter().map(|(k, v)| format!("{k} {v}")).collect();
    assert_eq!(items, ["Model Opus 5", "Agent build", "Took 12 s", "Tokens 3k in · 300 out", "Est. cost $0.19"]);

    let bare = derive(&[Msg { parts: vec![text("go")], ..Msg::new("u1", "s1", Role::User) }]);
    assert_eq!(meta_items(&bare[0], &models).iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>(), ["Model Unknown model"]);
}

#[test]
fn the_nothing_written_note_counts_the_calls_and_keeps_fleets_space_before_the_period() {
    let none = &derive(&[user("u1", "go", 1)])[0];
    assert_eq!(nothing_written(none), "Nothing was written this turn.");
    let one = &derive(&[user("u1", "go", 1), assistant("a1", vec![tool("read", json!({"status": "completed", "input": {}}))])])[0];
    assert_eq!(nothing_written(one), "Nothing was written this turn \u{2014} 1 tool call, all reads .");
    let two = &derive(&[
        user("u1", "go", 1),
        assistant(
            "a1",
            vec![tool("read", json!({"status": "completed", "input": {}})), tool("grep", json!({"status": "error", "input": {}}))],
        ),
    ])[0];
    assert_eq!(nothing_written(two), "Nothing was written this turn \u{2014} 2 tool calls, all reads .");
}

#[test]
fn the_edit_cache_keeps_a_completed_call_and_drops_the_oldest_past_its_bound() {
    let mut cache = EditCache::default();
    let state = edit_state("a.ts", vec![diff_line("add", "x", 1)]);
    let first = cache.edits_of("p1", "c1", "edit", &state);
    assert_eq!(first.len(), 1);
    // A second read is the cached one, whatever the state says now.
    let other = edit_state("b.ts", vec![diff_line("add", "y", 1)]);
    assert_eq!(cache.edits_of("p1", "c1", "edit", &other)[0].path, "a.ts");
    // A call that hasn't finished, or isn't a write, is never kept.
    let running = json!({"status": "running", "input": {"filePath": "c.ts", "newString": "z"}});
    assert!(cache.edits_of("p2", "c2", "edit", &running).is_empty());
    assert!(cache.edits_of("p3", "c3", "read", &state).is_empty());
    assert_eq!(cache.len(), 1);
    for n in 0..MAX_CACHED_CALLS {
        cache.edits_of(&format!("p-{n}"), "c", "edit", &state);
    }
    assert_eq!(cache.len(), MAX_CACHED_CALLS);
    // The first call went out to make room, so it is read afresh.
    assert_eq!(cache.edits_of("p1", "c1", "edit", &other)[0].path, "b.ts");
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn deriving_again_with_a_shared_cache_gives_the_same_turns() {
    let messages = [user("u1", "Edit", 1_000), assistant("a1", vec![tool("edit", edit_state("a.ts", vec![diff_line("add", "x", 1)]))])];
    let mut cache = EditCache::default();
    let first = derive_turns_cached(&messages, &[], TurnOptions::default(), &mut cache);
    let second = derive_turns_cached(&messages, &[], TurnOptions::default(), &mut cache);
    assert_eq!(first, second);
    assert_eq!(cache.len(), 1);
}
