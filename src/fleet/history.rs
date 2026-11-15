//! Older history: the page `LoadHistoryAsync` answers with, and how it joins the conversation
//! (`lib/history-merge.ts`, `use-session-stream.ts`'s `loadOlder`, `use-signalr-socket.ts`'s `loadSessionHistory`).
//! Pure data: the link thread owns a `Pending`, the state applies a `Page`.
use super::model::Msg;
use super::reducer::{committed_part, read_mod, str_of, to_role, tokens_of};
use serde_json::Value;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// How long an older page may take before the conversation calls it failed and offers a retry
/// (`HISTORY_PAGE_TIMEOUT_MS`).
pub const PAGE_TIMEOUT: Duration = Duration::from_secs(30);

/// A page of older messages (`SessionHistoryPage`).
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// The wire's `{info, parts}` payloads, oldest first.
    pub messages: Vec<Value>,
    pub cursor: Option<String>,
    pub has_more: bool,
}

impl Page {
    pub fn from_wire(v: &Value) -> Page {
        Page {
            messages: v.get("messages").and_then(Value::as_array).cloned().unwrap_or_default(),
            cursor: v.get("cursor").and_then(Value::as_str).map(str::to_string),
            has_more: v.get("hasMore").and_then(Value::as_bool).unwrap_or(false),
        }
    }
}

/// `prependHistoryPage`: the page's messages go before the current ones, in the page's order. A message
/// the conversation already holds keeps its current copy (it has fresher parts from live events), and
/// nothing already there moves.
pub fn prepend_page(current: Vec<Msg>, page: &[Value]) -> Vec<Msg> {
    let have: HashSet<&str> = current.iter().map(|m| m.id.as_str()).collect();
    let mut older: Vec<Msg> = page
        .iter()
        .filter(|m| m.get("info").and_then(|i| i.get("id")).and_then(Value::as_str).is_some_and(|id| !have.contains(id)))
        .filter_map(convert)
        .collect();
    older.extend(current);
    older
}

/// `convertToAccumulatedMessage`. Unlike a message made from live events, one from a page doesn't read the
/// `command` mark, the failure or the finish reason: Fleet's conversion leaves them out.
fn convert(message: &Value) -> Option<Msg> {
    let info = message.get("info")?;
    let id = str_of(info, "id")?;
    let mut msg = Msg::new(id.clone(), str_of(info, "sessionID").unwrap_or_default(), to_role(info.get("role").unwrap_or(&Value::Null)));
    msg.parts = message
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, p)| committed_part(&id, p, i))
        .collect();
    msg.created_at = info.pointer("/time/created").and_then(Value::as_i64);
    msg.completed_at = info.pointer("/time/completed").and_then(Value::as_i64);
    msg.agent = str_of(info, "agent");
    msg.model_id = str_of(info, "modelID");
    msg.parent_id = str_of(info, "parentID");
    msg.cost = info.get("cost").and_then(Value::as_f64);
    msg.tokens = info.get("tokens").and_then(tokens_of);
    msg.steered = info.get("steered") == Some(&Value::Bool(true));
    msg.compaction_summary = info.get("compactionSummary") == Some(&Value::Bool(true));
    msg.mod_mark = read_mod(info);
    Some(msg)
}

/// The page requests in flight on one connection: which invocation asked for which session's page.
pub struct Pending {
    requests: Vec<Request>,
    timeout: Duration,
}

impl Default for Pending {
    fn default() -> Self {
        Pending::with_timeout(PAGE_TIMEOUT)
    }
}

struct Request {
    invocation: String,
    session: String,
    cursor: String,
    since: Instant,
}

impl Pending {
    /// A shorter wait than Fleet's, for a test that can't sit for thirty seconds.
    pub fn with_timeout(timeout: Duration) -> Pending {
        Pending { requests: Vec::new(), timeout }
    }

    pub fn add(&mut self, invocation: String, session: String, cursor: String) {
        self.requests.push(Request { invocation, session, cursor, since: Instant::now() });
    }

    /// The request an answer belongs to, if it is one of these.
    pub fn complete(&mut self, invocation: &str) -> Option<(String, String)> {
        let at = self.requests.iter().position(|r| r.invocation == invocation)?;
        let r = self.requests.remove(at);
        Some((r.session, r.cursor))
    }

    /// The requests the server hasn't answered in time, taken off the list.
    pub fn expired(&mut self) -> Vec<(String, String)> {
        let timeout = self.timeout;
        self.take_where(|r| r.since.elapsed() >= timeout)
    }

    /// Every request: the connection that carried them is gone.
    pub fn drain(&mut self) -> Vec<(String, String)> {
        self.take_where(|_| true)
    }

    fn take_where(&mut self, mut pick: impl FnMut(&Request) -> bool) -> Vec<(String, String)> {
        let (taken, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.requests).into_iter().partition(|r| pick(r));
        self.requests = kept;
        taken.into_iter().map(|r| (r.session, r.cursor)).collect()
    }
}
