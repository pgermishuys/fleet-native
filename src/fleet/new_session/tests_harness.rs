//! Fleet's `harness-profile.test.ts` under the same names (test names are the describe path, snake_cased; rows B6.7). The
//! profile texts are invented.
use super::harness::{parse_jsonc, profile_summary};
use serde_json::json;

const NEW_PROFILE_CONTENT: &str =
    "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n  // Anything here layers over your own opencode.json\n}\n";

#[test]
fn profile_summary_names_the_model_then_what_the_profile_adds() {
    let content = r#"{
      // Work: Claude through Bedrock
      "model": "amazon-bedrock/anthropic.claude-sonnet-5",
      "provider": { "amazon-bedrock": { "options": { "region": "eu-west-1" } } },
      "mcp": { "linear": { "type": "remote", "url": "https://mcp.example.dev/mcp" } },
    }"#;
    assert_eq!(profile_summary(content), "amazon-bedrock/anthropic.claude-sonnet-5 · 1 provider · 1 MCP server");
}

#[test]
fn profile_summary_counts_open_code_2s_own_names_for_providers_agents_and_mcp_servers_too() {
    let content = r#"{
      "providers": { "work": {}, "local": {} },
      "agents": { "review": {} },
      "mcp": { "servers": { "linear": { "type": "remote", "url": "https://mcp.example.dev/mcp" } } },
    }"#;
    assert_eq!(profile_summary(content), "2 providers · 1 MCP server · 1 agent");
}

#[test]
fn profile_summary_says_when_a_profile_limits_the_providers() {
    assert_eq!(profile_summary(r#"{ "enabled_providers": ["ollama"] }"#), "only ollama");
    assert_eq!(profile_summary(r#"{ "disabled_providers": ["openai", "groq"] }"#), "turns off openai, groq");
}

#[test]
fn profile_summary_describes_agents_and_a_default_agent() {
    assert_eq!(profile_summary(r#"{ "default_agent": "review", "agent": { "review": {} } }"#), "default agent review · 1 agent");
}

#[test]
fn profile_summary_says_a_new_profile_has_no_settings_yet_and_counts_other_settings() {
    assert_eq!(profile_summary(NEW_PROFILE_CONTENT), "No settings yet");
    assert_eq!(profile_summary(r#"{ "share": "disabled", "autoupdate": false }"#), "2 settings");
}

#[test]
fn profile_summary_doesnt_pretend_to_read_broken_json() {
    assert_eq!(profile_summary(r#"{ "model": "#), "Can't read this profile");
}

#[test]
fn parse_jsonc_reads_comments_and_trailing_commas_but_keeps_double_slash_inside_strings() {
    let read = parse_jsonc("{ /* a */ \"url\": \"https://x.dev/mcp\", // b\n      \"list\": [1, 2,], }").expect("an object");
    assert_eq!(serde_json::Value::Object(read), json!({ "url": "https://x.dev/mcp", "list": [1, 2] }));
}

#[test]
fn parse_jsonc_returns_none_for_anything_that_isnt_an_object() {
    assert!(parse_jsonc("[1]").is_none());
    assert!(parse_jsonc("nope").is_none());
}
