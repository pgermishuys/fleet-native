//! The HTTP calls this slice makes, blocking: run them on a thread, never in a view or a handler.
//! Loopback and no token (`docs/FLEET.md`, "Signing in"); https and bearer tokens arrive with the
//! machines lane.
use serde_json::{Value, json};
use std::time::Duration;

fn agent() -> ureq::Agent {
    agent_for(15)
}

fn agent_for(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(seconds))).build().into()
}

fn url(base: &str, path: &str) -> String {
    format!("{}{path}", base.trim_end_matches('/'))
}

fn read(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, String> {
    let mut response = response.map_err(|e| e.to_string())?;
    let body = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    // 204 and plain "ok" bodies have nothing to parse.
    Ok(serde_json::from_str(&body).unwrap_or(Value::Null))
}

pub fn get(base: &str, path: &str) -> Result<Value, String> {
    read(agent().get(url(base, path)).call())
}

/// A picture Fleet serves at `path` (a call's screenshot): the bytes of a 2xx answer, at most 25 MB. Any other answer,
/// or none, is an error, which the caller turns into "show nothing".
pub fn get_bytes(base: &str, path: &str) -> Result<Vec<u8>, String> {
    let mut response = agent_for(30).get(url(base, path)).call().map_err(|e| e.to_string())?;
    response.body_mut().with_config().limit(25 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())
}

pub fn post(base: &str, path: &str, body: &Value) -> Result<Value, String> {
    read(agent().post(url(base, path)).header("content-type", "application/json").send(body.to_string()))
}

/// `GET /readyz` within `timeout`: whether the Fleet at `base` is up (any 2xx).
pub fn ready(base: &str, timeout: Duration) -> bool {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(timeout)).build().into();
    agent.get(url(base, "/readyz")).call().is_ok()
}

/// The session list's rows (`GET /api/sessions`).
pub fn sessions(base: &str) -> Result<Value, String> {
    get(base, "/api/sessions?limit=200")
}

/// What a prompt carries besides its text: the agent, model and effort picked in the composer, and the id the
/// message will have (`SendPromptApiRequest`). Each is left out of the body when nothing is picked, so the server
/// uses the session's own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pick {
    pub agent: Option<String>,
    /// Provider id and model id.
    pub model: Option<(String, String)>,
    pub effort: Option<String>,
    pub user_message_id: Option<String>,
    /// Into the running turn now (`delivery: "steer"`); left out otherwise, which the server reads as a plain send.
    pub steer: bool,
    /// The sessions picked from the `@` list whose tokens the text still holds, as (token, session id), and the skills
    /// its `$` tokens name: both go with a prompt for the server to expand (`sessionReferences`, `skills`).
    pub session_references: Vec<(String, String)>,
    pub skills: Vec<String>,
}

impl Pick {
    /// Adds `sessionReferences` and `skills` to a request body when there are any.
    pub fn add_references(&self, body: &mut Value) {
        if !self.session_references.is_empty() {
            body["sessionReferences"] =
                self.session_references.iter().map(|(token, id)| json!({ "token": token, "sessionId": id })).collect();
        }
        if !self.skills.is_empty() {
            body["skills"] = json!(self.skills);
        }
    }
}

/// The body of `POST /api/sessions/{id}/prompt`.
pub fn prompt_body(text: &str, correlation_id: &str, pick: &Pick) -> Value {
    let mut body = json!({ "text": text, "correlationId": correlation_id });
    if let Some(agent) = &pick.agent {
        body["agent"] = json!(agent);
    }
    if let Some((provider, model)) = &pick.model {
        body["model"] = json!({ "providerID": provider, "modelID": model });
    }
    if let Some(effort) = &pick.effort {
        body["effort"] = json!(effort);
    }
    if let Some(id) = &pick.user_message_id {
        body["userMessageId"] = json!(id);
    }
    if pick.steer {
        body["delivery"] = json!("steer");
    }
    pick.add_references(&mut body);
    body
}

/// Send a message to the agent (`SendPromptApiRequest`: the text, the pick and the correlation id the
/// optimistic prompt waits on). The receipt is `{eventId, correlationId}`.
///
/// It waits a minute, where every other call waits 15 s: Fleet's client waits as long as the server
/// takes, and the prompt is shown as needing a retry at 15 s (`prompts::CONFIRMATION_TIMEOUT`), which
/// a request that gave up first would turn into a refusal.
pub fn prompt(base: &str, session: &str, correlation_id: &str, text: &str, pick: &Pick) -> Result<Value, String> {
    let body = prompt_body(text, correlation_id, pick);
    read(
        agent_for(60)
            .post(url(base, &format!("/api/sessions/{session}/prompt")))
            .header("content-type", "application/json")
            .send(body.to_string()),
    )
}

/// `GET /api/sessions/{id}/agents`.
pub fn agents(base: &str, session: &str) -> Result<Value, String> {
    get(base, &format!("/api/sessions/{session}/agents"))
}

/// `GET /api/sessions/{id}/models`.
pub fn models(base: &str, session: &str) -> Result<Value, String> {
    get(base, &format!("/api/sessions/{session}/models"))
}

/// `GET /api/harnesses`.
pub fn harnesses(base: &str) -> Result<Value, String> {
    get(base, "/api/harnesses")
}

/// `GET /api/usage/limits`.
pub fn usage_limits(base: &str) -> Result<Value, String> {
    get(base, "/api/usage/limits")
}

/// `GET /api/harnesses/usage`.
pub fn harness_usage(base: &str) -> Result<Value, String> {
    get(base, "/api/harnesses/usage")
}

/// `GET /api/preferences`.
pub fn preferences(base: &str) -> Result<Value, String> {
    get(base, "/api/preferences")
}

/// `PUT /api/preferences/{key}` with `{value}`. The key is one URL segment, so it is escaped.
pub fn put_preference(base: &str, key: &str, value: &str) -> Result<(), String> {
    let escaped: String = key
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let body = json!({ "value": value }).to_string();
    read(agent().put(url(base, &format!("/api/preferences/{escaped}"))).header("content-type", "application/json").send(body)).map(drop)
}

pub fn abort(base: &str, session: &str) -> Result<(), String> {
    post(base, &format!("/api/sessions/{session}/abort"), &json!({})).map(drop)
}

/// `negotiate` for the hub: the token the WebSocket's URL carries.
pub fn negotiate(base: &str) -> Result<String, String> {
    let reply = post(base, "/hubs/session-events/negotiate?negotiateVersion=1", &json!({}))?;
    reply
        .get("connectionToken")
        .or_else(|| reply.get("connectionId"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "the hub gave no connection token".to_string())
}

/// A call whose status the caller reads itself: a 404 or a 500 is an answer here, not an error. Only a request that
/// never got an answer is `Err`. `body` is sent as JSON (`{}` when a POST has none).
fn status_with(method: &str, base: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value), String> {
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).build().into();
    let target = url(base, path);
    let response = match method {
        "GET" => agent.get(target).call(),
        "DELETE" => agent.delete(target).call(),
        _ => agent.post(target).header("content-type", "application/json").send(body.map_or("{}".to_string(), Value::to_string)),
    };
    let mut response = response.map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn status_of(method: &str, base: &str, path: &str) -> Result<(u16, Value), String> {
    status_with(method, base, path, None)
}

/// `GET /api/sessions/{id}/retry`: the scheduled retry, or 204 for none (`use-session-retry.ts` `load`).
pub fn retry_load(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/retry"))
}

/// `POST /api/sessions/{id}/retry/send`: Try now.
pub fn retry_send(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("POST", base, &format!("/api/sessions/{session}/retry/send"))
}

/// `DELETE /api/sessions/{id}/retry`: Don't retry.
pub fn retry_cancel(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("DELETE", base, &format!("/api/sessions/{session}/retry"))
}

/// `GET /api/sessions/{id}/interruption`: the turn a restart cut off, or 204 for none (`use-session-interruption.ts`).
pub fn interruption_load(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/interruption"))
}

/// `POST /api/sessions/{id}/interruption/continue`: Continue (or Ask again).
pub fn interruption_continue(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("POST", base, &format!("/api/sessions/{session}/interruption/continue"))
}

/// `DELETE /api/sessions/{id}/interruption`: Dismiss.
pub fn interruption_dismiss(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("DELETE", base, &format!("/api/sessions/{session}/interruption"))
}

/// `POST /api/sessions/{id}/work/{item}/stop`: Fleet's own item id, not the harness's `workId`. The answer is the ended item.
pub fn work_stop(base: &str, session: &str, item: &str) -> Result<(u16, Value), String> {
    status_of("POST", base, &format!("/api/sessions/{session}/work/{item}/stop"))
}

/// `GET /api/sessions/{id}/work/{item}/output?offset=N`: a page of the output from byte `offset`.
pub fn work_output(base: &str, session: &str, item: &str, offset: u64) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/work/{item}/output?offset={offset}"))
}

/// `POST /api/sessions/{id}/compact`: Compact now. Fleet answers once the harness has taken the request; a refusal
/// (409 during a turn, a harness that can't) is an answer with a status, not an error.
pub fn compact(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("POST", base, &format!("/api/sessions/{session}/compact"))
}

/// `GET /api/sessions/{id}/queue`: what Fleet holds for the turn's end (`use-session-queue.ts` `load`).
pub fn queue_load(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/queue"))
}

/// `POST /api/sessions/{id}/queue`: queue an item (201 with the item).
pub fn queue_add(base: &str, session: &str, body: &Value) -> Result<(u16, Value), String> {
    status_with("POST", base, &format!("/api/sessions/{session}/queue"), Some(body))
}

/// `DELETE /api/sessions/{id}/queue/{item}`: take an item back (204; a 404 is as good).
pub fn queue_remove(base: &str, session: &str, item: &str) -> Result<(u16, Value), String> {
    status_of("DELETE", base, &format!("/api/sessions/{session}/queue/{item}"))
}

/// `POST /api/sessions/{id}/queue/{item}/send`: Send now (202).
pub fn queue_send(base: &str, session: &str, item: &str) -> Result<(u16, Value), String> {
    status_of("POST", base, &format!("/api/sessions/{session}/queue/{item}/send"))
}

/// `GET /api/sessions/{id}/work?all=true`: all of the session's work, including what ended long ago.
pub fn work_all(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/work?all=true"))
}

/// `GET /api/sessions/{id}/commands`: the harness's slash commands and skills (the `/` and `$` lists). The status is
/// read by the caller: the error text is the body's `error`, or "HTTP n".
pub fn commands(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/commands"))
}

/// A query-string value, escaped (the file search's `q` holds `/`, spaces and anything typed after the `@`).
fn query_value(value: &str) -> String {
    value
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// `GET /api/sessions/{id}/find/files?q=`: the session's files and folders matching `q`; folders end in "/".
pub fn find_files(base: &str, session: &str, q: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/find/files?q={}", query_value(q)))
}

/// `GET /api/directories/find?path=&q=`: the same search in a folder, for a draft with no session yet (component 7).
pub fn find_in_directory(base: &str, path: &str, q: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/directories/find?path={}&q={}", query_value(path), query_value(q)))
}

/// `GET /api/sessions?retentionStatus=all&limit=200`: every session, archived ones too, for the `@` list.
pub fn sessions_for_mentions(base: &str) -> Result<Value, String> {
    get(base, "/api/sessions?retentionStatus=all&limit=200")
}

/// `POST /api/sessions/{id}/command` `{command, arguments?, agent?, model?}`: run a slash command (202). A refusal is an
/// answer with a status.
pub fn command(base: &str, session: &str, body: &Value) -> Result<(u16, Value), String> {
    status_with("POST", base, &format!("/api/sessions/{session}/command"), Some(body))
}

/// `POST /api/sessions/{id}/shell` `{command}`: run a shell command in the session's folder (202). A refusal is an
/// answer with a status.
pub fn shell(base: &str, session: &str, command: &str) -> Result<(u16, Value), String> {
    status_with("POST", base, &format!("/api/sessions/{session}/shell"), Some(&json!({ "command": command })))
}
