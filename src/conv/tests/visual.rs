//! `lib/__tests__/visual-payload.test.ts` under its names, and the live auto-open rule of `ActivityStream.vue`.
use crate::conv::visual::{LIVE_TOLERANCE_MS, is_live, parse_visual_payload};
use crate::fleet::canvases::{VisualPayload, VisualType};
use serde_json::{Value, json};

fn parse(v: Value) -> Option<VisualPayload> {
    parse_visual_payload(&v.to_string())
}

fn payload(kind: VisualType, content: &str, title: Option<&str>) -> Option<VisualPayload> {
    Some(VisualPayload { kind, title: title.map(str::to_string), source_path: None, content: content.into() })
}

// valid payloads
#[test]
fn parses_html_payload() {
    assert_eq!(parse(json!({"$type": "html", "content": "<div>Hello</div>"})), payload(VisualType::Html, "<div>Hello</div>", None));
}

#[test]
fn parses_markdown_payload() {
    assert_eq!(
        parse(json!({"$type": "markdown", "content": "# Title\n\nContent"})),
        payload(VisualType::Markdown, "# Title\n\nContent", None)
    );
}

#[test]
fn parses_payload_with_optional_title() {
    assert_eq!(
        parse(json!({"$type": "html", "content": "<p>Test</p>", "title": "My Title"})),
        payload(VisualType::Html, "<p>Test</p>", Some("My Title"))
    );
}

#[test]
fn ignores_extra_fields() {
    assert_eq!(
        parse(json!({"$type": "markdown", "content": "test", "extra": "ignored", "another": 123})),
        payload(VisualType::Markdown, "test", None)
    );
}

// invalid payloads
#[test]
fn returns_null_for_plain_text() {
    assert_eq!(parse_visual_payload("just plain text"), None);
}

#[test]
fn returns_null_for_invalid_json() {
    assert_eq!(parse_visual_payload("{ invalid json }"), None);
}

#[test]
fn returns_null_for_the_retired_diagram_and_sequence_types() {
    for kind in ["visual/flow", "visual/sequence"] {
        assert_eq!(parse(json!({"$type": kind, "content": "A -> B"})), None);
    }
}

#[test]
fn returns_null_for_unknown_type() {
    assert_eq!(parse(json!({"$type": "unknown", "content": "test"})), None);
}

#[test]
fn returns_null_for_missing_type() {
    assert_eq!(parse(json!({"content": "test"})), None);
}

#[test]
fn returns_null_for_missing_content() {
    assert_eq!(parse(json!({"$type": "html"})), None);
}

#[test]
fn returns_null_for_non_string_content() {
    assert_eq!(parse(json!({"$type": "html", "content": 123})), None);
}

#[test]
fn returns_null_for_non_string_type() {
    assert_eq!(parse(json!({"$type": 123, "content": "test"})), None);
}

#[test]
fn returns_null_for_json_array() {
    assert_eq!(parse(json!([{"$type": "html", "content": "test"}])), None);
}

#[test]
fn returns_null_for_json_primitive() {
    for raw in ["\"string\"", "123", "true", "null"] {
        assert_eq!(parse_visual_payload(raw), None, "{raw}");
    }
}

// security: prototype pollution prevention. The payload is read field by field, so nothing else is ever kept.
#[test]
fn strips_proto_key_from_top_level() {
    // `json!` would take `__proto__` as an ordinary key, as `JSON.parse` does.
    let raw = r#"{"$type":"html","content":"test","__proto__":{"polluted":true}}"#;
    assert_eq!(parse_visual_payload(raw), payload(VisualType::Html, "test", None));
}

#[test]
fn strips_constructor_key_from_top_level() {
    assert_eq!(
        parse(json!({"$type": "html", "content": "test", "constructor": {"polluted": true}})),
        payload(VisualType::Html, "test", None)
    );
}

#[test]
fn strips_proto_recursively_in_nested_objects() {
    let raw = r#"{"$type":"html","content":"test","nested":{"__proto__":{"polluted":true},"safe":"value"}}"#;
    assert_eq!(parse_visual_payload(raw), payload(VisualType::Html, "test", None));
}

#[test]
fn strips_constructor_recursively_in_nested_objects() {
    let nested = json!({"constructor": {"polluted": true}, "safe": "value"});
    assert_eq!(parse(json!({"$type": "html", "content": "test", "nested": nested})), payload(VisualType::Html, "test", None));
}

#[test]
fn strips_dangerous_keys_in_arrays() {
    let items = json!([{"constructor": {"bad": true}, "value": 2}]);
    assert_eq!(parse(json!({"$type": "html", "content": "test", "items": items})), payload(VisualType::Html, "test", None));
}

#[test]
fn handles_deeply_nested_dangerous_keys() {
    let deep = json!({"level1": {"level2": {"level3": {"constructor": {"polluted": true}, "safe": "value"}}}});
    let mut all = deep.as_object().cloned().unwrap_or_default();
    all.insert("$type".into(), json!("html"));
    all.insert("content".into(), json!("test"));
    assert_eq!(parse(Value::Object(all)), payload(VisualType::Html, "test", None));
}

// edge cases
#[test]
fn handles_empty_string_content() {
    assert_eq!(parse(json!({"$type": "html", "content": ""})), payload(VisualType::Html, "", None));
}

#[test]
fn handles_empty_string_title() {
    assert_eq!(parse(json!({"$type": "html", "content": "test", "title": ""})), payload(VisualType::Html, "test", Some("")));
}

#[test]
fn ignores_non_string_title() {
    assert_eq!(parse(json!({"$type": "html", "content": "test", "title": 123})), payload(VisualType::Html, "test", None));
}

#[test]
fn handles_whitespace_only_content() {
    assert_eq!(parse(json!({"$type": "html", "content": "   \n\t  "})), payload(VisualType::Html, "   \n\t  ", None));
}

// Ours: the start test is on the text's first character, past any white space (`/^\s*\{/`).
#[test]
fn reads_a_payload_after_leading_white_space() {
    assert!(parse_visual_payload("  \n{\"$type\":\"markdown\",\"content\":\"a\"}").is_some());
}

// ActivityStream.vue: a visual arriving within 5 s of the conversation opening is live; history is not.
#[test]
fn a_visual_made_after_the_conversation_opened_or_just_before_it_is_live_and_history_is_not() {
    assert!(is_live(Some(10_000), 10_000));
    assert!(is_live(Some(10_000 - LIVE_TOLERANCE_MS), 10_000));
    assert!(!is_live(Some(10_000 - LIVE_TOLERANCE_MS - 1), 10_000));
    assert!(!is_live(None, 10_000), "a message with no time is history");
}
