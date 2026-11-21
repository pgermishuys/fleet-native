//! What mission control sends to a session's own machine (`lib/mission-control-requests.ts`, `lib/push/answer.ts`): the
//! nudge, the answers to an ask, and the words for how one went. Each request is plain data; `send` runs it against a
//! machine's address (the machine's token is held by `auth`, by address) and the caller does that on a thread.
use super::Tile;
use crate::fleet::asks::Reply;
use crate::fleet::http;
use crate::fleet::machines::Machines;
use serde_json::{Value, json};

/// One call to a machine, before an address is chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
}

impl Request {
    /// Runs it against `base`: the status and the body as JSON (null when it wasn't), or why it didn't get there.
    pub fn send(&self, base: &str) -> Result<(u16, Value), String> {
        let (status, text) = http::text_with(self.method, base, &self.path, self.body.as_ref())?;
        Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
    }
}

fn session_path(session: &str, rest: &str) -> String {
    format!("/api/sessions/{}/{rest}", crate::conv::export_history::encode_component(session))
}

/// `nudgeRequest`: into the running turn when the harness steers (`delivery: "steer"`), otherwise Fleet's own queue, which
/// sends it when the turn ends; a session that isn't working just gets the prompt. `message_id` is the new `userMessageId`.
pub fn nudge_request(session: &str, text: &str, working: bool, steers: bool, message_id: &str) -> Request {
    if working && !steers {
        return Request { method: "POST", path: session_path(session, "queue"), body: Some(json!({ "text": text })) };
    }
    let mut body = json!({ "text": text, "userMessageId": message_id });
    if working {
        body["delivery"] = json!("steer");
    }
    Request { method: "POST", path: session_path(session, "prompt"), body: Some(body) }
}

/// `permissionAnswerRequest`: once, always or reject, with words for the agent on a reject. Sent on the **ask's** session id,
/// which may be a sub-agent's.
pub fn permission_answer_request(session: &str, request_id: &str, reply: Reply, message: Option<&str>) -> Request {
    let id = crate::conv::export_history::encode_component(request_id);
    let body = match message.filter(|m| !m.is_empty()) {
        Some(words) => json!({ "reply": reply.word(), "message": words }),
        None => json!({ "reply": reply.word() }),
    };
    Request { method: "POST", path: session_path(session, &format!("permissions/{id}")), body: Some(body) }
}

/// `questionAnswerRequest`: the picked labels, one list per question.
pub fn question_answer_request(session: &str, request_id: &str, answers: &[Vec<String>]) -> Request {
    let id = crate::conv::export_history::encode_component(request_id);
    Request { method: "POST", path: session_path(session, &format!("questions/{id}/answer")), body: Some(json!({ "answers": answers })) }
}

/// How an answer or a nudge went (`AnswerOutcome`); `gone` when the ask was already answered or isn't there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerOutcome {
    pub ok: bool,
    pub gone: bool,
    pub error: Option<String>,
}

/// `sendAnswer`'s words: 404 or 409 is "Already answered."; another refusal says what Fleet said (its `error`, `detail` or
/// `title`) or "It answered <status>."; no answer at all is "Couldn't reach the machine."
pub fn outcome(result: &Result<(u16, Value), String>) -> AnswerOutcome {
    let (ok, gone, error) = match result {
        Err(_) => (false, false, Some("Couldn't reach the machine.".to_string())),
        Ok((status, _)) if (200..300).contains(status) => (true, false, None),
        Ok((404 | 409, _)) => (false, true, Some("Already answered.".to_string())),
        Ok((status, body)) => {
            let said = ["error", "detail", "title"].iter().find_map(|k| body.get(k).and_then(Value::as_str));
            (false, false, Some(said.map_or_else(|| format!("It answered {status}."), str::to_string)))
        }
    };
    AnswerOutcome { ok, gone, error }
}

/// What a tile's call says when its machine left the list (`send` in `useMissionControl`).
pub const MACHINE_GONE: &str = "That machine isn't in the list any more.";

/// The key of the machine a tile belongs to, or the words for its having left the list.
pub fn machine_key(machines: &Machines, tile: &Tile) -> Result<String, &'static str> {
    machines.key_of_machine(&tile.machine_id).ok_or(MACHINE_GONE)
}

/// `steers`: whether the harness of the tile takes a message into the running turn, from the machine's
/// `GET /api/harnesses` answer. A failed read (`None`) or a harness that isn't listed says no.
pub fn steers(harnesses: Option<&Value>, harness_type: &str) -> bool {
    let Some(body) = harnesses else { return false };
    crate::fleet::catalog::parse_harnesses(body).iter().find(|h| h.kind == harness_type).is_some_and(|h| h.supports_steering)
}
