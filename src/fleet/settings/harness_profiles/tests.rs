//! `lib/__tests__/harness-profile.test.ts` under its names, and the plain rules around the store.
use super::*;
use serde_json::json;

#[test]
fn names_the_model_then_what_the_profile_adds() {
    let content = r#"{
      // Work: Claude through a cloud
      "model": "cloud/claude-sonnet",
      "provider": { "cloud": { "options": { "region": "eu-west-1" } } },
      "mcp": { "docs": { "type": "remote", "url": "https://mcp.example.test/mcp" } },
    }"#;
    assert_eq!(profile_summary(content), "cloud/claude-sonnet \u{b7} 1 provider \u{b7} 1 MCP server");
}

#[test]
fn counts_open_code_2_s_own_names_for_providers_agents_and_mcp_servers_too() {
    let content = r#"{
      "providers": { "work": {}, "local": {} },
      "agents": { "review": {} },
      "mcp": { "servers": { "docs": { "type": "remote", "url": "https://mcp.example.test/mcp" } } },
    }"#;
    assert_eq!(profile_summary(content), "2 providers \u{b7} 1 MCP server \u{b7} 1 agent");
}

#[test]
fn says_when_a_profile_limits_the_providers() {
    assert_eq!(profile_summary(r#"{ "enabled_providers": ["ollama"] }"#), "only ollama");
    assert_eq!(profile_summary(r#"{ "disabled_providers": ["alpha", "beta"] }"#), "turns off alpha, beta");
}

#[test]
fn describes_agents_and_a_default_agent() {
    assert_eq!(profile_summary(r#"{ "default_agent": "review", "agent": { "review": {} } }"#), "default agent review \u{b7} 1 agent");
}

#[test]
fn says_a_new_profile_has_no_settings_yet_and_counts_other_settings() {
    assert_eq!(profile_summary(NEW_PROFILE_CONTENT), "No settings yet");
    assert_eq!(profile_summary(r#"{ "share": "disabled", "autoupdate": false }"#), "2 settings");
    assert_eq!(profile_summary(r#"{ "share": "disabled" }"#), "1 setting");
}

#[test]
fn doesn_t_pretend_to_read_broken_json() {
    assert_eq!(profile_summary(r#"{ "model": "#), "Can't read this profile");
}

#[test]
fn reads_comments_and_trailing_commas_but_keeps_slashes_inside_strings() {
    let read = parse_jsonc("{ /* a */ \"url\": \"https://x.dev/mcp\", // b\n      \"list\": [1, 2,], }").expect("an object");
    assert_eq!(serde_json::Value::Object(read), json!({ "url": "https://x.dev/mcp", "list": [1, 2] }));
}

#[test]
fn returns_none_for_anything_that_isn_t_an_object() {
    assert!(parse_jsonc("[1]").is_none());
    assert!(parse_jsonc("nope").is_none());
}

#[test]
fn a_new_profile_starts_with_the_schema_and_a_comment() {
    assert_eq!(
        NEW_PROFILE_CONTENT,
        "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n  // Anything here layers over your own opencode.json\n}\n"
    );
}

#[test]
fn counts_open_sessions_in_words() {
    assert_eq!(usage(0), "No open sessions");
    assert_eq!(usage(1), "Used by 1 open session");
    assert_eq!(usage(2), "Used by 2 open sessions");
}

#[test]
fn a_refusal_says_its_error_message_or_detail_else_the_status() {
    assert_eq!(error_words(400, r#"{"error":"Bad JSON"}"#), "Bad JSON");
    assert_eq!(error_words(400, r#"{"message":"Too long"}"#), "Too long");
    assert_eq!(error_words(422, r#"{"detail":"Unknown key"}"#), "Unknown key");
    assert_eq!(error_words(500, r#"{"error":""}"#), "HTTP 500");
    assert_eq!(error_words(502, "<html>"), "HTTP 502");
}

fn profile(id: &str, default: bool) -> Profile {
    Profile { id: id.into(), name: id.to_uppercase(), content: "{}".into(), is_default: default, open_sessions: 0 }
}

#[test]
fn the_list_follows_a_default_a_create_an_update_and_a_delete() {
    let list = vec![profile("a", true), profile("b", false)];
    let moved = with_default(&list, Some("b"));
    assert_eq!(moved.iter().map(|p| p.is_default).collect::<Vec<_>>(), [false, true]);
    assert!(with_default(&list, None).iter().all(|p| !p.is_default));
    assert_eq!(with_created(&list, profile("c", false)).len(), 3);
    let mut changed = profile("b", false);
    changed.open_sessions = 2;
    assert_eq!(with_updated(&list, "b", changed)[1].open_sessions, 2);
    assert_eq!(without(&list, "a").iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["b"]);
}

#[test]
fn a_check_the_harness_refused_is_an_answer_with_its_words() {
    let refused = Check::parse(200, r#"{"ok":false,"error":"Unknown key","details":"line 3"}"#);
    assert_eq!((refused.ok, refused.error.as_deref(), refused.details.as_deref()), (false, Some("Unknown key"), Some("line 3")));
    assert!(Check::parse(200, r#"{"ok":true}"#).ok);
    assert_eq!(Check::parse(500, "{}").error.as_deref(), Some("HTTP 500"));
}
