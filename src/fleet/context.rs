//! How full a session's context window is, as Fleet records it from the session's harness: weave-fleet's
//! `client/src/lib/context-usage.ts`, the payload half (`toContextUsage`). The same shape comes in the
//! snapshot's `context` and live as `context.updated`. The ring, the percent, the tone and "turns
//! before compaction" are component 5's. Pure: no Rune, no I/O.
use serde_json::Value;

/// One model call's tokens, each kind counted once.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextCall {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    pub reasoning: u64,
    /// All of them: what the next call starts from.
    pub used: u64,
    /// The harness gave only the total: it's all in `input`, and the split isn't known.
    pub total_only: bool,
}

/// The context's size at the end of a turn.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextTurn {
    pub used: u64,
    pub limit: Option<u64>,
    pub at: String,
    pub after_compaction: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextUsage {
    pub session_id: String,
    /// Tokens in the context as of the last call; `None` before the first one and right after a compaction.
    pub used: Option<u64>,
    /// The model's context window; `None` when the harness hasn't said.
    pub limit: Option<u64>,
    /// How many tokens make the harness compact on its own, when it can tell.
    pub compacts_at: Option<u64>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub last_call: Option<ContextCall>,
    pub last_call_at: Option<String>,
    pub compacting: bool,
    pub compacted_at: Option<String>,
    /// Why the last compaction failed, until another one starts.
    pub compaction_error: Option<String>,
    /// Oldest first.
    pub turns: Vec<ContextTurn>,
    pub updated_at: String,
}

/// A count: a non-negative finite number, or a string holding one (the REST API sends strings).
fn count(v: Option<&Value>) -> Option<u64> {
    let n = match v? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|_| !s.trim().is_empty())?,
        _ => return None,
    };
    (n.is_finite() && n >= 0.0).then_some(n as u64)
}

fn positive(n: Option<u64>) -> Option<u64> {
    n.filter(|n| *n > 0)
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn to_call(raw: Option<&Value>) -> Option<ContextCall> {
    let raw = raw.filter(|v| v.is_object())?;
    let n = |k: &str| count(raw.get(k)).unwrap_or(0);
    let (input, cache_read, cache_write, output, reasoning) = (n("input"), n("cacheRead"), n("cacheWrite"), n("output"), n("reasoning"));
    Some(ContextCall {
        input,
        cache_read,
        cache_write,
        output,
        reasoning,
        used: count(raw.get("used")).unwrap_or(input + cache_read + cache_write + output + reasoning),
        total_only: raw.get("totalOnly") == Some(&Value::Bool(true)),
    })
}

fn to_turn(raw: &Value) -> Option<ContextTurn> {
    if !raw.is_object() {
        return None;
    }
    Some(ContextTurn {
        used: count(raw.get("used"))?,
        limit: positive(count(raw.get("limit"))),
        at: text(raw, "at")?,
        after_compaction: raw.get("afterCompaction") == Some(&Value::Bool(true)),
    })
}

/// `toContextUsage`: Fleet's payload as the client uses it, or `None` when it isn't one. The server
/// leaves out what's unknown.
pub fn to_context_usage(raw: &Value) -> Option<ContextUsage> {
    if !raw.is_object() {
        return None;
    }
    let session_id = raw.get("sessionId")?.as_str()?.to_string();
    Some(ContextUsage {
        session_id,
        used: count(raw.get("used")),
        limit: positive(count(raw.get("limit"))),
        compacts_at: positive(count(raw.get("compactsAt"))),
        model_id: text(raw, "modelId"),
        provider_id: text(raw, "providerId"),
        last_call: to_call(raw.get("lastCall")),
        last_call_at: text(raw, "lastCallAt"),
        compacting: raw.get("compacting") == Some(&Value::Bool(true)),
        compacted_at: text(raw, "compactedAt"),
        compaction_error: text(raw, "compactionError"),
        turns: raw.get("turns").and_then(Value::as_array).into_iter().flatten().filter_map(to_turn).collect(),
        updated_at: text(raw, "updatedAt").unwrap_or_default(),
    })
}
