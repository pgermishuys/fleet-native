//! `lib/__tests__/export-markdown.test.ts`, under the same names. Fleet builds its times in the machine's zone; these
//! name one (two hours east of UTC) so the file reads the same wherever the tests run.
use crate::conv::export::{
    DEFAULT_EXPORT_OPTIONS, ExportContext, ExportOptions, ExportToolMode, build_markdown, export_file_name, find_key_lines, has_reasoning,
};
use crate::fleet::model::{Msg, PartData, SlashCommand};
use crate::view::conversation::Role;
use serde_json::{Value, json};

const OFFSET: i64 = 2 * 3600;
const DAY_2026_10_20: i64 = 1_792_454_400;

/// 20 October 2026, `hour`:`minute` in the zone the tests name.
fn at(hour: i64, minute: i64) -> i64 {
    (DAY_2026_10_20 + hour * 3600 + minute * 60 - OFFSET) * 1000
}

fn context() -> ExportContext {
    ExportContext {
        title: "Add upload retries".into(),
        harness: Some("opencode2".into()),
        model: Some("claude-sonnet-5-5".into()),
        directory: Some("/home/sam/work/harbor-api".into()),
        exported_at: at(10, 14),
        offset_secs: OFFSET,
    }
}

fn msg(id: &str, role: Role, created: Option<i64>, model: Option<&str>, parts: Vec<PartData>) -> Msg {
    let mut m = Msg::new(id, "s", role);
    m.created_at = created;
    m.model_id = model.map(str::to_string);
    m.parts = parts;
    m
}

fn text(id: &str, text: &str) -> PartData {
    PartData::Text { id: id.into(), text: text.into() }
}

fn tool(id: &str, name: &str, state: Value) -> PartData {
    PartData::Tool { id: id.into(), tool: name.into(), call_id: format!("c-{id}"), state }
}

fn assistant_with(parts: Vec<PartData>) -> Vec<Msg> {
    vec![msg("x", Role::Assistant, None, None, parts)]
}

fn messages() -> Vec<Msg> {
    let model = Some("claude-sonnet-5-5");
    let mut summary = msg("m5", Role::Assistant, None, None, vec![text("p12", "SUMMARY BODY")]);
    summary.compaction_summary = true;
    vec![
        msg("m1", Role::User, Some(at(10, 2)), None, vec![text("p0", "Uploads fail when the network blips. Add retries.")]),
        msg(
            "m2",
            Role::Assistant,
            Some(at(10, 2)),
            model,
            vec![
                PartData::Reasoning { id: "p1".into(), text: "upload.ts calls put() once.\nWrap it.".into(), summary: None },
                text("p2", "I'll read the upload code first."),
                tool(
                    "p3",
                    "read",
                    json!({"status": "completed", "input": {"filePath": "/home/sam/work/harbor-api/src/upload.ts"}, "output": "export async function upload() {}"}),
                ),
                tool(
                    "p4",
                    "edit",
                    json!({"status": "completed", "input": {"filePath": "/home/sam/work/harbor-api/src/upload.ts"}, "output": "Edit applied.",
                           "metadata": {"diff": "@@ -1 +1,2 @@\n-old\n+new\n+more"}}),
                ),
                tool(
                    "p5",
                    "bash",
                    json!({"status": "completed", "input": {"command": "bun test upload", "description": "Run the upload tests"},
                           "output": "2 pass\nS3_TOKEN=sk_live_9f3a7c51d0e84b22a6\n\u{1b}[32mok\u{1b}[0m"}),
                ),
                tool("p6", "bash", json!({"status": "error", "input": {"command": "false"}, "output": "boom"})),
                tool(
                    "p7",
                    "fleet_page_show",
                    json!({"status": "completed", "input": {"title": "Retry timeline", "path": "/home/sam/work/harbor-api/t.html"}}),
                ),
            ],
        ),
        msg(
            "m3",
            Role::User,
            Some(at(10, 9)),
            None,
            vec![
                text("p8", "Show me the waits."),
                PartData::File {
                    id: "p9".into(),
                    mime: "image/png".into(),
                    filename: Some("shot.png".into()),
                    url: "data:image/png;base64,AAAA".into(),
                },
            ],
        ),
        msg(
            "m4",
            Role::Assistant,
            Some(at(10, 9)),
            model,
            vec![
                PartData::Compaction {
                    id: "p10".into(),
                    trigger: None,
                    tokens_before: Some(181_000),
                    tokens_after: Some(34_000),
                    summary: None,
                },
                text("p11", "Waits are 200 ms then 400 ms."),
            ],
        ),
        summary,
    ]
}

fn build_with(options: ExportOptions) -> crate::conv::export::ExportResult {
    build_markdown(&messages(), &options, &context())
}

fn build(change: impl FnOnce(&mut ExportOptions)) -> crate::conv::export::ExportResult {
    let mut options = DEFAULT_EXPORT_OPTIONS;
    change(&mut options);
    build_with(options)
}

fn output_mode() -> ExportOptions {
    ExportOptions { tools: ExportToolMode::Output, ..DEFAULT_EXPORT_OPTIONS }
}

#[test]
fn writes_the_default_export_tools_as_one_line_no_thinking_short_paths_times_and_model() {
    let expected = "# Add upload retries

_opencode2 \u{b7} claude-sonnet-5-5 \u{b7} exported 2026-10-20 10:14_

## You \u{b7} 10:02

Uploads fail when the network blips. Add retries.

## Assistant \u{b7} 10:02 \u{b7} claude-sonnet-5-5

I'll read the upload code first.

- Read `src/upload.ts`
- Edit `src/upload.ts`
- Bash `bun test upload`
- Bash `false` (failed)

> Showed page: \"Retry timeline\"

## You \u{b7} 10:09

Show me the waits.

[image: shot.png]

---

_Context compacted \u{b7} 181k \u{2192} 34k tokens_

## Assistant \u{b7} 10:09 \u{b7} claude-sonnet-5-5

Waits are 200 ms then 400 ms.
";
    assert_eq!(build(|_| {}).markdown, expected);
}

#[test]
fn never_carries_file_bytes_and_leaves_out_a_compactions_summary_message() {
    let markdown = build_with(ExportOptions { thinking: true, ..output_mode() }).markdown;
    assert!(!markdown.contains("base64"));
    assert!(!markdown.contains("SUMMARY BODY"));
}

#[test]
fn leaves_tool_calls_out_but_keeps_the_page_the_agent_showed() {
    let markdown = build(|o| o.tools = ExportToolMode::Off).markdown;
    assert!(!markdown.contains("Read `"));
    assert!(!markdown.contains("(failed)"));
    assert!(markdown.contains("> Showed page: \"Retry timeline\""));
}

#[test]
fn adds_each_calls_output_a_diff_for_an_edit_and_a_text_block_for_a_command_with_tool_output_on() {
    let markdown = build_with(output_mode()).markdown;
    assert!(markdown.contains("- Read `src/upload.ts`\n\n```\nexport async function upload() {}\n```"));
    assert!(markdown.contains("```diff\n@@ -1 +1,2 @@\n-old\n+new\n+more\n```"));
    assert!(markdown.contains("```text\n2 pass\nS3_TOKEN=sk_live_9f3a7c51d0e84b22a6\nok\n```"));
}

#[test]
fn quotes_thinking_when_asked() {
    assert!(!build(|_| {}).markdown.contains("Thinking"));
    assert!(build(|o| o.thinking = true).markdown.contains("> **Thinking**\n>\n> upload.ts calls put() once.\n> Wrap it."));
}

#[test]
fn keeps_full_paths_and_drops_times_and_model_when_those_are_off() {
    let markdown = build(|o| {
        o.shorten_paths = false;
        o.meta = false;
    })
    .markdown;
    assert!(markdown.contains("- Read `/home/sam/work/harbor-api/src/upload.ts`"));
    assert!(markdown.contains("## You\n"));
    assert!(!markdown.contains("exported"));
    assert!(!markdown.contains("10:02"));
}

#[test]
fn counts_the_lines_that_look_like_keys_and_rewrites_nothing() {
    assert_eq!(build(|_| {}).key_lines, 0);
    let with_output = build_with(output_mode());
    assert_eq!(with_output.key_lines, 1);
    assert!(with_output.markdown.contains("sk_live_9f3a7c51d0e84b22a6"));
}

#[test]
fn counts_the_turns_that_were_written() {
    assert_eq!(build(|_| {}).turns, 4);
}

#[test]
fn opens_a_fence_longer_than_any_backticks_in_the_output() {
    let tricky =
        assistant_with(vec![tool("p", "bash", json!({"status": "completed", "input": {"command": "echo"}, "output": "```\ninner\n```"}))]);
    let markdown = build_markdown(&tricky, &output_mode(), &context()).markdown;
    assert!(markdown.contains("````text\n```\ninner\n```\n````"));
}

#[test]
fn cuts_a_very_long_output_and_says_so() {
    let long = assistant_with(vec![tool(
        "p",
        "bash",
        json!({"status": "completed", "input": {"command": "build"}, "output": "a".repeat(25_000)}),
    )]);
    let markdown = build_markdown(&long, &output_mode(), &context()).markdown;
    assert!(markdown.contains("\u{2026} 5000 more characters left out"));
}

#[test]
fn finds_an_edits_diff_in_opencode_2s_per_file_patches_or_builds_one_from_the_swapped_strings() {
    let edit = |extra: Value| {
        let mut state = json!({"status": "completed", "output": "Edit applied."});
        state.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        assistant_with(vec![tool("p", "edit", state)])
    };
    let patched = build_markdown(
        &edit(json!({"metadata": {"files": [{"file": "a.ts", "patch": "@@ -1 +1 @@\n-a\n+b\n"}]}})),
        &output_mode(),
        &context(),
    );
    assert!(patched.markdown.contains("```diff\n@@ -1 +1 @@\n-a\n+b\n```"));
    let swapped =
        build_markdown(&edit(json!({"input": {"filePath": "a.ts", "oldString": "a", "newString": "b\nc"}})), &output_mode(), &context());
    assert!(swapped.markdown.contains("```diff\n-a\n+b\n+c\n```"));
}

#[test]
fn writes_an_agents_steps_which_arrive_as_messages_of_their_own_as_one_turn() {
    let step = |id: &str, t: &str| msg(id, Role::Assistant, Some(at(10, 2)), None, vec![text(id, t)]);
    let options = ExportOptions { meta: false, ..DEFAULT_EXPORT_OPTIONS };
    let result = build_markdown(&[step("a", "One."), step("b", "Two.")], &options, &context());
    assert!(result.markdown.contains("## Assistant\n\nOne.\n\nTwo.\n"));
    assert_eq!(result.turns, 1);
}

#[test]
fn keeps_a_long_path_whole() {
    let long = "/home/sam/work/harbor-api/packages/uploads/src/retry/backoff-policy-with-a-long-name.ts";
    let read = assistant_with(vec![tool("p", "read", json!({"status": "completed", "input": {"path": long}}))]);
    let markdown = build_markdown(&read, &DEFAULT_EXPORT_OPTIONS, &context()).markdown;
    assert!(markdown.contains("`packages/uploads/src/retry/backoff-policy-with-a-long-name.ts`"));
}

#[test]
fn shows_a_slash_command_as_the_command_not_its_expanded_template() {
    let mut m = msg("x", Role::User, None, None, vec![text("p", "Long expanded template\u{2026}")]);
    m.command = Some(SlashCommand { name: "review".into(), arguments: Some("main".into()) });
    let markdown = build_markdown(&[m], &DEFAULT_EXPORT_OPTIONS, &context()).markdown;
    assert!(markdown.contains("`/review main`"));
    assert!(!markdown.contains("template"));
}

#[test]
fn is_true_only_when_a_message_has_reasoning_with_text() {
    let all = messages();
    assert!(has_reasoning(&all));
    assert!(!has_reasoning(&all[..1]));
}

#[test]
fn recognises_common_key_shapes_and_ignores_plain_text() {
    let lines = format!("hello\nAKIAABCDEFGHIJKLMNOP\nghp_{}\nPASSWORD=hunter2hunter2\nall fine", "a".repeat(24));
    assert_eq!(find_key_lines(&lines), 3);
}

#[test]
fn recognises_the_other_shapes_and_their_edges() {
    let key = |l: &str| find_key_lines(l) == 1;
    assert!(key("sk-abcdefghijklmnopqrstuvwxyz"));
    assert!(key("use xoxb-1234567890-abc here"));
    assert!(key("-----BEGIN RSA PRIVATE KEY-----"));
    assert!(key("Authorization: Bearer abcdefghijklmnopqrstu.vwx"));
    assert!(key("api_key: \"abcdefgh12345\""));
    assert!(key("MY_SECRET_VALUE = abcdefgh"));
    // Too short, glued to a word on the left, or no value.
    assert!(!key("sk_live_short"));
    assert!(!key("task_sk_live_9f3a7c51d0e84b22a6"));
    assert!(!key("TOKEN=short"));
    assert!(!key("the token is somewhere"));
    assert!(!key("-----BEGIN CERTIFICATE-----"));
}

#[test]
fn names_the_file_after_the_title() {
    assert_eq!(export_file_name("Add upload retries!", "ses_1"), "add-upload-retries.md");
    assert_eq!(export_file_name("???", "ses_1"), "ses_1.md");
    assert_eq!(export_file_name(&"ab ".repeat(60), "x").len(), 80 + 3);
}
