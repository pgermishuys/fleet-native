//! The list ones of `ActivityStream.messages.test.ts` and `ActivityStream.thinking.test.ts`, `merge-messages.test.ts`
//! (its seven cases are one rule: delivered, then sent, no sort) and the pure helpers of `activity-messages.ts`. The
//! tests of Vue identity caches, bubble props that a view owns, and sub-agent link / Improve (component 2) are not here.
use crate::composer::choice::ModelOption;
use crate::conv::gist::Thinking;
use crate::conv::message_list::*;
use crate::fleet::delegations::{Delegation, DelegationStatus};
use crate::fleet::model::{ModKind, ModMark, Msg, PartData};
use crate::fleet::prompts::{PromptStatus, SentPrompt};
use crate::view::conversation::Role;
use serde_json::json;

fn text_msg(id: &str, role: Role, body: &str) -> Msg {
    let mut m = Msg::new(id, "s1", role);
    m.created_at = id[1..].parse().ok();
    if role == Role::Assistant {
        m.agent = Some("build".into());
    }
    m.parts = vec![PartData::Text { id: format!("{id}-p"), text: body.into() }];
    m
}
fn tool_msg(id: &str, tool: &str, call: &str, state: serde_json::Value) -> Msg {
    let mut m = Msg::new(id, "s1", Role::Assistant);
    m.agent = Some("build".into());
    m.parts = vec![PartData::Tool { id: format!("{id}-t"), tool: tool.into(), call_id: call.into(), state }];
    m
}
fn thinking_only() -> Msg {
    let mut m = Msg::new("m2", "s1", Role::Assistant);
    m.agent = Some("build".into());
    m.parts = vec![PartData::Reasoning { id: "r".into(), text: "Weighing options".into(), summary: None }];
    m
}
fn models() -> Vec<ModelOption> {
    vec![ModelOption::new("acme", "Acme", "acme/swift", "Swift 2")]
}
fn context<'a>(models: &'a [ModelOption], delegations: &'a [Delegation], thinking: Thinking) -> ListContext<'a> {
    static NONE: std::sync::LazyLock<crate::conv::background::Finished> = std::sync::LazyLock::new(Default::default);
    ListContext { delegations, finished: &NONE, models, thinking }
}
fn list(messages: &[Msg], sent: &[SentPrompt], thinking: Thinking) -> Vec<ListMessage> {
    build_list(messages, sent, &context(&models(), &[], thinking))
}
fn ids(messages: &[ListMessage]) -> Vec<&str> {
    messages.iter().map(|m| m.id.as_str()).collect()
}
fn sent(id: &str, body: &str, status: PromptStatus) -> SentPrompt {
    SentPrompt {
        id: id.into(),
        correlation_id: id.into(),
        event_id: None,
        server_message_id: None,
        status,
        body: body.into(),
        created_at: 3,
        sent_at: 0.0,
    }
}

#[test]
fn names_the_author_the_model_and_the_role_of_each_message() {
    let mut reviewer = text_msg("m2", Role::Assistant, "Hi there");
    reviewer.agent = Some("code_reviewer".into());
    reviewer.model_id = Some("acme/swift".into());
    let out = list(&[text_msg("m1", Role::User, "Hello"), reviewer, text_msg("m3", Role::User, "Thanks")], &[], Thinking::Folded);
    assert_eq!((out[0].author.as_str(), out[0].role, out[0].body.as_str()), ("You", Role::User, "Hello"));
    assert_eq!((out[1].author.as_str(), out[1].role, out[1].body.as_str()), ("Code Reviewer", Role::Assistant, "Hi there"));
    assert_eq!(out[1].model_name.as_deref(), Some("Swift 2"));
    assert_eq!(out[2].author, "You");
}

#[test]
fn clusters_a_run_of_messages_from_one_sender_and_keeps_another_senders_apart() {
    let mut other = text_msg("m5", Role::Assistant, "Other agent");
    other.agent = Some("Explore".into());
    let messages = [
        text_msg("m1", Role::User, "Look at this"),
        text_msg("m2", Role::Assistant, "First"),
        text_msg("m3", Role::Assistant, "Second"),
        text_msg("m4", Role::Assistant, "Third"),
        other,
    ];
    let out = list(&messages, &[], Thinking::Folded);
    use ClusterPosition::*;
    assert_eq!(out.iter().map(|m| m.cluster).collect::<Vec<_>>(), [Single, First, Middle, Last, Single]);
    assert_eq!(out.iter().map(|m| m.show_identity).collect::<Vec<_>>(), [true, true, false, false, true]);
}

#[test]
fn captions_a_prompt_a_mod_sent_or_rewrote_and_no_other() {
    let mut sent_by = text_msg("m2", Role::User, "Run the tests");
    sent_by.mod_mark = Some(ModMark { kind: ModKind::Sent, name: "tests-after-edits".into() });
    let mut rewritten = text_msg("m3", Role::User, "Looks good to me.");
    rewritten.mod_mark = Some(ModMark { kind: ModKind::Rewritten, name: "lgtm-expander".into() });
    let out = list(&[text_msg("m1", Role::User, "Please ship it"), sent_by, rewritten], &[], Thinking::Folded);
    assert_eq!(
        out.iter().map(|m| m.mod_mark.as_ref().map(|k| (k.kind, k.name.as_str()))).collect::<Vec<_>>(),
        [None, Some((ModKind::Sent, "tests-after-edits")), Some((ModKind::Rewritten, "lgtm-expander")),]
    );
}

#[test]
fn shows_a_shell_command_the_user_ran_as_its_own_block_on_the_users_side() {
    let mut shell = Msg::new("m2", "s1", Role::Shell);
    shell.parts = vec![PartData::Tool {
        id: "m2-t".into(),
        tool: "shell".into(),
        call_id: "c".into(),
        state: json!({"status": "completed", "input": {"command": "ls src"}, "output": "a.ts", "metadata": {"exit": 0}}),
    }];
    let out = list(&[text_msg("m1", Role::User, "Run it"), shell], &[], Thinking::Folded);
    assert!(out[1].shell && out[1].tools.is_empty() && out[1].body.is_empty());
    assert_eq!((out[1].role, out[1].author.as_str(), out[1].sender_key.as_str()), (Role::Shell, "You", "shell"));
}

#[test]
fn puts_a_prompt_that_was_sent_but_not_echoed_yet_after_the_messages_and_drops_it_once_the_stream_has_it() {
    let mut messages = vec![text_msg("m1", Role::User, "Earlier"), text_msg("m2", Role::Assistant, "Answer")];
    let prompts = [sent("p1", "Do the next thing", PromptStatus::Pending), sent("p2", "Not sure it left", PromptStatus::NeedsRetry)];
    let out = list(&messages, &prompts, Thinking::Folded);
    assert_eq!(ids(&out), ["m1", "m2", "optimistic-p1", "optimistic-p2"]);
    assert_eq!((out[2].author.as_str(), out[2].role, out[2].body.as_str()), ("You", Role::User, "Do the next thing"));
    assert_eq!(out.iter().filter(|m| m.optimistic == Some(PromptStatus::NeedsRetry)).count(), 1);
    // The echo of the first arrives under the id the prompt had.
    messages.push(text_msg("p1", Role::User, "Do the next thing"));
    assert_eq!(ids(&list(&messages, &prompts, Thinking::Folded)), ["m1", "m2", "p1", "optimistic-p2"]);
}

#[test]
fn keeps_tool_calls_and_questions_apart_in_order() {
    let mut m = Msg::new("m1", "s1", Role::Assistant);
    let call = |id: &str, tool: &str, call_id: &str, status: &str| PartData::Tool {
        id: id.into(),
        tool: tool.into(),
        call_id: call_id.into(),
        state: json!({"status": status, "input": {}}),
    };
    m.parts = vec![
        call("a", "read", "c1", "completed"),
        call("b", "grep", "c2", "completed"),
        call("c", "question", "c3", "running"),
        PartData::Text { id: "d".into(), text: "Reading.".into() },
    ];
    let out = list(&[m], &[], Thinking::Folded);
    assert_eq!(out[0].tools.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(out[0].question_parts, ["c"]);
    assert_eq!(out[0].body, "Reading.");
}

#[test]
fn draws_a_background_notice_with_how_the_work_ended_and_no_bubble_text_of_its_own() {
    // The notice's own parse (its state, its words) is piece K's; what the list owes it is a message that stays.
    let mut notice = ListMessage::new("m2", Role::Assistant);
    notice.background = true;
    assert!(has_visible_content(&notice, Thinking::Folded));
    assert!(notice.body.is_empty());
}

#[test]
fn hides_an_assistant_message_with_nothing_to_show_and_keeps_an_empty_user_one() {
    let out = list(
        &[text_msg("m1", Role::User, ""), text_msg("m2", Role::Assistant, "   "), text_msg("m3", Role::Assistant, "Real answer")],
        &[],
        Thinking::Folded,
    );
    assert_eq!(ids(&out), ["m1", "m3"]);
}

#[test]
fn shows_thinking_alone_as_a_message() {
    let out = list(&[text_msg("m1", Role::User, "Hmm"), thinking_only()], &[], Thinking::Folded);
    assert_eq!(ids(&out), ["m1", "m2"]);
}

#[test]
fn leaves_thinking_alone_out_when_thinking_is_hidden() {
    let out = list(&[text_msg("m1", Role::User, "Hmm"), thinking_only()], &[], Thinking::Hidden);
    assert_eq!(ids(&out), ["m1"]);
}

#[test]
fn shows_only_what_came_after_the_boundary_of_a_side_conversation() {
    let messages = [
        text_msg("m1", Role::User, "Copied"),
        text_msg("m2", Role::Assistant, "Copied too"),
        text_msg("m3", Role::User, "Side question"),
        text_msg("m4", Role::Assistant, "Side answer"),
    ];
    let (after, cut) = messages_after(&messages, Some("m2"));
    assert!(cut);
    let out = list(after, &[], Thinking::Folded);
    assert_eq!(ids(&out), ["m3", "m4"]);
    assert!(out[0].show_identity);
    // No boundary: all, nothing cut. A boundary that isn't there: the ids that sort after it.
    assert_eq!(messages_after(&messages, None), (&messages[..], false));
    assert_eq!(messages_after(&messages, Some("m25")).0.len(), 2);
    assert_eq!(messages_after(&messages, Some("m0")), (&messages[..], false));
    assert!(messages_after(&messages, Some("m9")).0.is_empty());
}

// ─── thinking: the live block ───────────────────────────────────────────────

#[test]
fn shows_a_message_that_so_far_is_only_thinking_marked_live_while_the_turn_runs() {
    let messages = [text_msg("m1", Role::User, "Page the orders list."), thinking_only()];
    assert_eq!(live_reasoning_part(&messages, true), Some("r"));
}

#[test]
fn isnt_live_once_the_turn_has_ended_or_once_something_follows_the_thinking() {
    let mut messages = vec![text_msg("m1", Role::User, "Page the orders list."), thinking_only()];
    assert_eq!(live_reasoning_part(&messages, false), None);
    messages[1].parts.push(PartData::Text { id: "t1".into(), text: "Switching to a cursor.".into() });
    assert_eq!(live_reasoning_part(&messages, true), None);
    // Nor when the newest message is the person's.
    assert_eq!(live_reasoning_part(&[text_msg("m1", Role::User, "Hi")], true), None);
}

// ─── merge-messages.test.ts ─────────────────────────────────────────────────

#[test]
fn merges_delivered_then_sent_in_each_ones_own_order_whatever_the_timestamps_say() {
    let at = |id: &str, t: i64| {
        let mut m = ListMessage::new(id, Role::User);
        m.created_at = Some(t);
        m
    };
    let merged = crate::fleet::prompts::merge_by_timestamp(vec![at("d1", 300), at("d2", 100)], vec![at("o1", 100), at("o2", 50)]);
    assert_eq!(ids(&merged), ["d1", "d2", "o1", "o2"]);
    assert_eq!(ids(&crate::fleet::prompts::merge_by_timestamp(Vec::new(), vec![at("o1", 1), at("o2", 2)])), ["o1", "o2"]);
    assert_eq!(ids(&crate::fleet::prompts::merge_by_timestamp(vec![at("d1", 1)], Vec::new())), ["d1"]);
    assert!(crate::fleet::prompts::merge_by_timestamp::<ListMessage>(Vec::new(), Vec::new()).is_empty());
}

// ─── activity-messages.ts ───────────────────────────────────────────────────

#[test]
fn names_an_agent_the_way_the_bubble_does() {
    assert_eq!(format_agent_display_name("code_reviewer"), "Code Reviewer");
    assert_eq!(format_agent_display_name("  build--fast  "), "Build Fast");
    assert_eq!(format_agent_display_name("GPT-5 helper"), "GPT 5 helper");
    assert_eq!(format_agent_display_name("planner (v2)"), "Planner (V2)");
    assert_eq!(format_agent_display_name(" _ - "), "Assistant");
    assert_eq!(display_author(Role::Assistant, None), "Assistant");
    assert_eq!(sender_key(Role::Assistant, Some("Code_Reviewer")), "code reviewer");
    assert_eq!(sender_key(Role::User, Some("anything")), "user");
}

#[test]
fn joins_text_parts_with_a_blank_line_and_leaves_reasoning_and_images_out() {
    let parts = [
        PartData::Text { id: "1".into(), text: "One".into() },
        PartData::Reasoning { id: "2".into(), text: "hmm".into(), summary: None },
        PartData::File { id: "3".into(), mime: "image/png".into(), filename: None, url: "u".into() },
        PartData::File { id: "4".into(), mime: "text/plain".into(), filename: Some(" notes.txt ".into()), url: "https://x/y".into() },
        PartData::File { id: "5".into(), mime: "text/plain".into(), filename: None, url: String::new() },
        PartData::Text { id: "6".into(), text: "Two".into() },
    ];
    assert_eq!(render_message_body(&parts), "One\n\n[notes.txt](https://x/y)\n\nAttached file\n\nTwo");
}

#[test]
fn a_sub_agent_call_that_has_a_session_stays_visible_and_one_without_does_not() {
    let call = tool_msg("m1", "task", "call_a", json!({"status": "completed", "input": {"subagent_type": "explore"}}));
    let none = list(std::slice::from_ref(&call), &[], Thinking::Folded);
    assert!(!none[0].tools[0].delegation);
    let delegations = [Delegation {
        delegation_id: "d1".into(),
        parent_tool_call_id: Some("call_a".into()),
        child_session_id: Some("child1".into()),
        title: "Explore".into(),
        status: DelegationStatus::Running,
        created_at: None,
        child_activity_status: None,
        background: false,
    }];
    let with = build_list(&[call], &[], &context(&models(), &delegations, Thinking::Folded));
    assert!(with[0].tools[0].delegation);
}

#[test]
fn reads_a_screenshot_a_page_and_a_canvas_off_the_calls_metadata() {
    let state = json!({"status": "completed", "metadata": {
        "screenshot": {"sessionId": "s", "id": "i", "width": 10, "height": 8},
        "page": {"id": "p", "entry": "index.html"},
        "canvasId": "c1"
    }});
    let out = list(&[tool_msg("m1", "browser_take_screenshot", "c", state)], &[], Thinking::Folded);
    let tool = &out[0].tools[0];
    assert!(tool.screenshot && tool.page.is_some() && tool.canvas);
    let bare = list(&[tool_msg("m2", "read", "c", json!({"status": "completed"}))], &[], Thinking::Folded);
    assert!(!bare[0].tools[0].screenshot && bare[0].tools[0].page.is_none() && !bare[0].tools[0].canvas);
}
