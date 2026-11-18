//! The two page cases of `activity-stream-utils.test.ts` (piece F of component 14).
use crate::conv::message_list::*;
use crate::conv::tools::ToolStatus;
use crate::view::conversation::Role;
use serde_json::json;

fn call(id: &str, source: &str) -> ListTool {
    let mut tool = ListTool::new(id, "fleet_page_show", ToolStatus::Completed);
    tool.page = Some(ToolPage { path: format!("/pages/{id}/p.html"), id: id.into(), source: source.into() });
    tool
}

fn message(role: Role, tools: Vec<ListTool>) -> ListMessage {
    let mut m = ListMessage::new(format!("m{}", tools.len()), role);
    m.tools = tools;
    m
}

fn drawn(messages: &[ListMessage]) -> Vec<String> {
    messages.iter().flat_map(|m| &m.tools).filter(|t| t.page.is_some()).map(|t| t.id.clone()).collect()
}

#[test]
fn points_a_page_shown_in_the_conversation_at_fleets_copy_and_gives_other_calls_none() {
    let shown = json!({
        "status": "completed",
        "input": {"path": "/tmp/ci/times.html", "placement": "conversation", "title": "CI test times"},
        "metadata": {"canvasId": null, "version": null, "page": {"id": "pg_0123456789abcdef0123456789abcdef", "entry": "charts/times.html"}},
    });
    assert_eq!(
        tool_page(&shown),
        Some(ToolPage {
            path: "/pages/pg_0123456789abcdef0123456789abcdef/charts/times.html".into(),
            id: "pg_0123456789abcdef0123456789abcdef".into(),
            source: "/tmp/ci/times.html".into(),
        })
    );
    assert_eq!(tool_page(&json!({"status": "completed", "metadata": {"canvasId": "cv_1", "version": 1}})), None);
    assert_eq!(tool_page(&json!({"status": "completed", "metadata": {"page": {"id": "pg_1"}}})), None);
}

#[test]
fn draws_a_page_shown_again_in_the_same_turn_once_at_its_last_show_and_keeps_earlier_turns_pages() {
    let mut messages = vec![
        message(Role::User, vec![]),
        message(Role::Assistant, vec![call("first", "/tmp/a.html")]),
        message(Role::User, vec![]),
        message(Role::Assistant, vec![call("broken", "/tmp/a.html"), call("other", "/tmp/b.html")]),
        message(Role::Assistant, vec![call("fixed", "/tmp/a.html")]),
    ];
    let hidden = without_redrawn_pages(&mut messages);
    assert_eq!(drawn(&messages), ["first", "other", "fixed"]);
    // The calls stay; only the page goes.
    assert_eq!(messages[3].tools.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["broken", "other"]);
    assert_eq!(hidden, ["broken".to_string()].into_iter().collect());
    // Nothing shown twice changes nothing.
    let mut once = messages[..2].to_vec();
    assert!(without_redrawn_pages(&mut once).is_empty());
    assert_eq!(once, messages[..2]);
}
