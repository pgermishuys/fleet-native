//! The HTTP calls this slice makes, blocking: run them on a thread, never in a view or a handler.
//! Loopback and no token (`docs/FLEET.md`, "Signing in"); https and bearer tokens arrive with the
//! machines lane.
use serde_json::{Value, json};
use std::time::Duration;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into()
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

/// The session list's rows (`GET /api/sessions`).
pub fn sessions(base: &str) -> Result<Value, String> {
    get(base, "/api/sessions?limit=200")
}

/// Send a message to the agent (`SendPromptApiRequest`, only the text).
pub fn prompt(base: &str, session: &str, text: &str) -> Result<(), String> {
    post(base, &format!("/api/sessions/{session}/prompt"), &json!({ "text": text })).map(drop)
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
