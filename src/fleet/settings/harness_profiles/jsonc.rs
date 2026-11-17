//! What a profile says, in one line (`lib/harness-profile.ts`): `opencode.json` is JSON with comments and trailing commas, so
//! it is read by stripping those first. Unreadable text falls back to a plain note; the harness is what checks a profile for
//! real, when it is saved.
use serde_json::{Map, Value};

/// What a new profile starts with.
pub const NEW_PROFILE_CONTENT: &str =
    "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n  // Anything here layers over your own opencode.json\n}\n";

/// `parseJsonc`: the object the text holds, or none for anything else (a list, a number, broken text).
pub fn parse_jsonc(text: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(&strip_jsonc(text)).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// `stripJsonc`: `//` and `/* */` outside strings go, and so does a comma before a closing bracket (the way Fleet does it: on
/// the whole text, strings included).
fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = next {
                    out.push(n);
                }
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            out.push('\n');
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 1;
        } else {
            out.push(c);
        }
        i += 1;
    }
    drop_trailing_commas(&out)
}

/// `/,(\s*[}\]])/` → `$1`.
fn drop_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (at, &c) in chars.iter().enumerate() {
        if c == ',' {
            let after = chars[at + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(after, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn count(value: Option<&Value>) -> usize {
    value.and_then(Value::as_object).map_or(0, Map::len)
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// JavaScript's `join` of a list's items.
fn joined(list: &[Value]) -> String {
    let word = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Array(_) => v.to_string().trim_matches(['[', ']']).to_string(),
        Value::Object(_) => "[object Object]".into(),
        other => other.to_string(),
    };
    list.iter().map(word).collect::<Vec<_>>().join(", ")
}

/// `profileSummary`: the default model, the default agent, and how many providers, MCP servers and agents it adds.
/// OpenCode 2 reads the same file with its own names (`providers`, `agents`, `mcp.servers`), so both count.
pub fn profile_summary(content: &str) -> String {
    let Some(config) = parse_jsonc(content) else { return "Can't read this profile".into() };
    let mut parts: Vec<String> = Vec::new();
    let mcp = match config.get("mcp") {
        Some(Value::Object(m)) if m.contains_key("servers") => m.get("servers"),
        other => other,
    };
    let providers = count(config.get("provider")) + count(config.get("providers"));
    let agents = count(config.get("agent")) + count(config.get("agents"));
    if let Some(Value::String(model)) = config.get("model") {
        parts.push(model.clone());
    }
    if let Some(Value::String(agent)) = config.get("default_agent") {
        parts.push(format!("default agent {agent}"));
    }
    if providers > 0 {
        parts.push(plural(providers, "provider"));
    }
    if count(mcp) > 0 {
        parts.push(plural(count(mcp), "MCP server"));
    }
    if agents > 0 {
        parts.push(plural(agents, "agent"));
    }
    if let Some(Value::Array(list)) = config.get("enabled_providers") {
        parts.push(format!("only {}", joined(list)));
    }
    if let Some(Value::Array(list)) = config.get("disabled_providers") {
        parts.push(format!("turns off {}", joined(list)));
    }
    if parts.is_empty() {
        let settings = config.keys().filter(|k| *k != "$schema").count();
        parts.push(if settings == 0 { "No settings yet".into() } else { plural(settings, "setting") });
    }
    parts.join(" \u{b7} ")
}
