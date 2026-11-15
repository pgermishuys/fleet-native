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
