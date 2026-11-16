//! The HTTP calls this slice makes, blocking: run them on a thread, never in a view or a handler.
//! `http://` and `https://` (rustls, the same stack as the hub's `wss://`, `tls.rs`); a bearer token goes with every call when one was given (`auth`).
use serde_json::{Value, json};
use std::time::Duration;

/// Every agent starts here, so the access token (if one was given) goes with every call.
fn builder() -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope> {
    plain_builder().middleware(super::auth::middleware)
}

/// An agent that sends no token: only for asking who answers at an address (`get_plain`).
fn plain_builder() -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope> {
    let b = ureq::Agent::config_builder();
    // A test trusts its own certificate; nothing outside tests can add a root.
    #[cfg(test)]
    let b = {
        let extra: Vec<_> = super::tls::test_roots::all().iter().map(|d| ureq::tls::Certificate::from_der(d).to_owned()).collect();
        if extra.is_empty() {
            b
        } else {
            b.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::new_with_certs(&extra)).build())
        }
    };
    b
}

fn agent() -> ureq::Agent {
    agent_for(15)
}

fn agent_for(seconds: u64) -> ureq::Agent {
    builder().timeout_global(Some(Duration::from_secs(seconds))).build().into()
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

/// A page of the conversation for an export (`GET /api/sessions/{id}/messages`): the status and the body of whatever
/// answered, so a refusal can say its code. `Err` only when nothing answered.
pub fn get_page(base: &str, path: &str) -> Result<(u16, String), String> {
    let mut response = agent_for(30).get(url(base, path)).config().http_status_as_error(false).build().call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body = response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())?;
    Ok((status, body))
}

/// A GET that carries no token, within `timeout`: the status, the content type and the first 64 KB of the body. `Err`
/// only when nothing answered (`probeAddress`: `credentials: "omit"`). The token must never reach an address that
/// hasn't said whose it is.
pub fn get_plain(base: &str, path: &str, timeout: Duration) -> Result<(u16, String, String), String> {
    let agent: ureq::Agent = plain_builder().timeout_global(Some(timeout)).http_status_as_error(false).build().into();
    let mut response = agent.get(url(base, path)).call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let kind = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or_default().to_ascii_lowercase();
    let body = response.body_mut().with_config().limit(64 * 1024).read_to_string().map_err(|e| e.to_string())?;
    Ok((status, kind, body))
}

pub fn post(base: &str, path: &str, body: &Value) -> Result<Value, String> {
    read(agent().post(url(base, path)).header("content-type", "application/json").send(body.to_string()))
}

pub fn patch(base: &str, path: &str, body: &Value) -> Result<Value, String> {
    read(agent().patch(url(base, path)).header("content-type", "application/json").send(body.to_string()))
}

pub fn delete(base: &str, path: &str) -> Result<Value, String> {
    read(agent().delete(url(base, path)).call())
}

/// `GET /readyz` within `timeout`: whether the Fleet at `base` is up (any 2xx).
pub fn ready(base: &str, timeout: Duration) -> bool {
    let agent: ureq::Agent = builder().timeout_global(Some(timeout)).build().into();
    agent.get(url(base, "/readyz")).call().is_ok()
}

/// The session list's rows (`GET /api/sessions`, one page of 100). A refusal says what the server said (`error_text`).
pub fn sessions(base: &str, retention: super::store::Retention) -> Result<Value, String> {
    let (status, body) = get_page(base, &super::store::list_path(retention))?;
    if !(200..300).contains(&status) {
        return Err(super::store::error_text(status, &body));
    }
    serde_json::from_str(&body).map_err(|_| "No data returned".to_string())
}

/// The projects (`GET /api/projects`): the session list's groups and their order.
pub fn projects(base: &str) -> Result<Value, String> {
    get(base, "/api/projects")
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
    /// The pictures that go with the prompt (`attachments`): their type, name and base64 text.
    pub attachments: Vec<crate::composer::attach::HeldImage>,
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
    if !pick.attachments.is_empty() {
        body["attachments"] =
            pick.attachments.iter().map(|a| json!({ "mime": a.mime, "filename": a.filename, "data": &*a.data })).collect();
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
    let agent: ureq::Agent = builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).build().into();
    let mut response = agent
        .post(url(base, &format!("/api/sessions/{session}/prompt")))
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    let answer: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(answer);
    }
    // The server's own reason is what the error box says (`{error}`, or a problem's `detail` or `title`).
    Err(super::queue::error_message(&answer, format!("HTTP {}", status.as_u16())))
}

/// `POST /api/sessions`: starts a session from the New session page's request (`NewSessionRequest::body`). The answer is a
/// `CreateSessionResponse`; a refusal's words are the server's own (`store::error_text`: its `error`, `detail` or `title`,
/// else the body, else "HTTP n"), which the page shows in its red box.
///
/// It waits two minutes where other calls wait 15 s: Fleet's client waits as long as the server takes, and a new worktree
/// (a fetch, a checkout) can take a while.
pub fn create_session(base: &str, body: &Value) -> Result<Value, String> {
    let agent: ureq::Agent = builder().timeout_global(Some(Duration::from_secs(120))).http_status_as_error(false).build().into();
    let mut response = agent
        .post(url(base, "/api/sessions"))
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if (200..300).contains(&status) {
        return serde_json::from_str(&text).map_err(|_| "No data returned".to_string());
    }
    Err(super::store::error_text(status, &text))
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

/// `GET /api/config/client`: what this Fleet allows (`terminalEnabled`, `cloudMode`).
pub fn client_config(base: &str) -> Result<Value, String> {
    get(base, "/api/config/client")
}

/// `POST /api/setup/terminals`: a shell in the home folder, outside any session, where a harness's installer is shown. Gives the
/// terminal's id, or the sentence to show (the server's `error`, else the status).
pub fn setup_terminal_create(base: &str, cols: u32, rows: u32) -> Result<String, String> {
    let (status, text) = text_with("POST", base, "/api/setup/terminals", Some(&json!({ "cols": cols, "rows": rows })))
        .map_err(|_| "The terminal couldn't start.".to_string())?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        return body.get("id").and_then(Value::as_str).map(str::to_string).ok_or_else(|| "The terminal couldn't start.".to_string());
    }
    Err(body
        .get("error")
        .and_then(Value::as_str)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("The terminal request failed (HTTP {status}).")))
}

/// `DELETE /api/setup/terminals/{id}`; the caller ignores the answer.
pub fn setup_terminal_close(base: &str, id: &str) -> Result<(), String> {
    text_with("DELETE", base, &format!("/api/setup/terminals/{}", escape(id)), None).map(drop)
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

/// One path segment or query value, escaped (`encodeURIComponent`).
pub fn escape(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~!*'()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// `POST /api/sessions/{id}/attachments?name=<name>`: the raw file in the body, with its own type. The answer is
/// `{path, name, size, lines, mime}`; a refusal's reason is the server's own `{error}` text (`uploadSessionAttachment`).
pub fn upload(base: &str, session: &str, name: &str, content_type: &str, body: Vec<u8>) -> Result<Value, String> {
    let agent: ureq::Agent = builder().timeout_global(Some(Duration::from_secs(300))).http_status_as_error(false).build().into();
    let target = url(base, &format!("/api/sessions/{session}/attachments?name={}", escape(name)));
    let mut response = agent.post(target).header("content-type", content_type).send(body).map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    let answer: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(answer);
    }
    match answer.get("error").and_then(Value::as_str) {
        Some(reason) if !reason.is_empty() => Err(reason.to_string()),
        _ => Err(format!("Failed to upload {name}: HTTP {}", status.as_u16())),
    }
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
    let (status, text) = text_with(method, base, path, body)?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// `status_with` that hands back the body as it came, for a caller that reads the server's reason out of it.
pub fn text_with(method: &str, base: &str, path: &str, body: Option<&Value>) -> Result<(u16, String), String> {
    let agent: ureq::Agent = builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).build().into();
    let target = url(base, path);
    let json = || body.map_or("{}".to_string(), Value::to_string);
    let response = match method {
        "GET" => agent.get(target).call(),
        "DELETE" => agent.delete(target).call(),
        "PATCH" => agent.patch(target).header("content-type", "application/json").send(json()),
        "PUT" => agent.put(target).header("content-type", "application/json").send(json()),
        _ => agent.post(target).header("content-type", "application/json").send(json()),
    };
    let mut response = response.map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    Ok((status, response.body_mut().read_to_string().unwrap_or_default()))
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

/// `GET /api/sessions/{id}/permissions`: what waits on the user, oldest first (`use-session-permissions.ts` `load`).
pub fn permissions(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/permissions"))
}

/// `GET /api/sessions/{id}/permission-rules`: what "Don't ask again" allowed in the session (`use-session-permission-rules.ts`).
pub fn rules(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/permission-rules"))
}

/// `GET /api/sessions/{id}/canvases`: the canvases the agent keeps for the session (`use-server-canvases.ts`).
pub fn canvases(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("GET", base, &format!("/api/sessions/{session}/canvases"))
}

/// `DELETE /api/sessions/{id}/permission-rules/{ruleId}`: removes one rule.
pub fn rule_remove(base: &str, session: &str, rule: &str) -> Result<(u16, Value), String> {
    status_of("DELETE", base, &format!("/api/sessions/{session}/permission-rules/{rule}"))
}

/// `DELETE /api/sessions/{id}/permission-rules`: removes every rule of the session.
pub fn rules_remove_all(base: &str, session: &str) -> Result<(u16, Value), String> {
    status_of("DELETE", base, &format!("/api/sessions/{session}/permission-rules"))
}

/// `POST /api/sessions/{id}/permissions/{ask}`: answers one ask; `message` is what Deny tells the agent.
pub fn reply(base: &str, session: &str, ask: &str, reply: &str, message: Option<&str>) -> Result<(u16, Value), String> {
    let body = json!({ "reply": reply, "message": message });
    status_with("POST", base, &format!("/api/sessions/{session}/permissions/{ask}"), Some(&body))
}

/// `POST /api/sessions/{id}/questions/{request}/answer`: what the user picked or typed, one list per question
/// (`use-question-answer.ts` `answerQuestion`). The request id is the question call's `callId`.
pub fn question_answer(base: &str, session: &str, request: &str, answers: &[Vec<String>]) -> Result<(u16, Value), String> {
    let body = json!({ "answers": answers });
    status_with("POST", base, &format!("/api/sessions/{session}/questions/{request}/answer"), Some(&body))
}

/// `POST /api/sessions/{id}/questions/{request}/reject`: Skip, no body (`rejectQuestion`).
pub fn question_reject(base: &str, session: &str, request: &str) -> Result<(u16, Value), String> {
    status_of("POST", base, &format!("/api/sessions/{session}/questions/{request}/reject"))
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

// ─── The New session page's data (component 7, piece C): each answer is the status and the body, as the server sent it,
// so `new_session::feeds` can say a refusal in the server's own words. ───

/// `GET /api/repositories`.
pub fn repositories(base: &str) -> Result<(u16, String), String> {
    text_with("GET", base, "/api/repositories", None)
}

/// `POST /api/repositories/refresh`: scans again.
pub fn repositories_refresh(base: &str) -> Result<(u16, String), String> {
    text_with("POST", base, "/api/repositories/refresh", Some(&json!({})))
}

/// `GET /api/repositories/detail?path=`.
pub fn repository_detail(base: &str, path: &str) -> Result<(u16, String), String> {
    text_with("GET", base, &format!("/api/repositories/detail?path={}", query_value(path)), None)
}

/// `GET /api/repositories/worktrees?path=`.
pub fn repository_worktrees(base: &str, path: &str) -> Result<(u16, String), String> {
    text_with("GET", base, &format!("/api/repositories/worktrees?path={}", query_value(path)), None)
}

/// `GET /api/workspace-roots`.
pub fn workspace_roots(base: &str) -> Result<(u16, String), String> {
    text_with("GET", base, "/api/workspace-roots", None)
}

/// `POST /api/workspace-roots` `{path}`: lets sessions run in the folder.
pub fn add_workspace_root(base: &str, path: &str) -> Result<(u16, String), String> {
    text_with("POST", base, "/api/workspace-roots", Some(&json!({ "path": path })))
}

/// `GET /api/directories/defaults`.
pub fn directory_defaults(base: &str) -> Result<(u16, String), String> {
    text_with("GET", base, "/api/directories/defaults", None)
}

/// `GET /api/directories/inspect?path=`.
pub fn inspect_directory(base: &str, path: &str) -> Result<(u16, String), String> {
    text_with("GET", base, &format!("/api/directories/inspect?path={}", query_value(path)), None)
}

/// `GET /api/directories?path=&unconstrained=true`: the folders in `path`, anywhere on the machine.
pub fn list_directory(base: &str, path: &str) -> Result<(u16, String), String> {
    text_with("GET", base, &format!("/api/directories?path={}&unconstrained=true", query_value(path)), None)
}

/// `POST /api/directories` `{path, git, branch}`: makes the folder and the ones above it that are missing; with `git`, a
/// repository with an empty first commit on `branch` (the machine's default when none).
pub fn create_directory(base: &str, path: &str, git: bool, branch: Option<&str>) -> Result<(u16, String), String> {
    text_with("POST", base, "/api/directories", Some(&json!({ "path": path, "git": git, "branch": branch })))
}

/// `POST /api/directories/clone` `{repository, path}`: the status, and the body still to be read (newline-delimited JSON that
/// ends in `done` or `error`). A clone takes as long as git does, so only connecting is timed.
pub fn clone_directory(base: &str, repository: &str, path: &str) -> Result<(u16, Box<dyn std::io::Read + Send>), String> {
    let agent: ureq::Agent =
        builder().timeout_global(None).timeout_connect(Some(Duration::from_secs(15))).http_status_as_error(false).build().into();
    let body = json!({ "repository": repository, "path": path });
    let response = agent
        .post(url(base, "/api/directories/clone"))
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    Ok((status, Box::new(response.into_body().into_reader())))
}

/// `GET /api/harnesses`, or `?fresh=true` to have the machine check every harness again first.
pub fn harness_list(base: &str, fresh: bool) -> Result<(u16, String), String> {
    text_with("GET", base, if fresh { "/api/harnesses?fresh=true" } else { "/api/harnesses" }, None)
}

/// `GET /api/preferences` on any machine.
pub fn preference_map(base: &str) -> Result<(u16, String), String> {
    text_with("GET", base, "/api/preferences", None)
}

/// `GET /api/harnesses/{type}/profiles`.
pub fn harness_profiles(base: &str, harness: &str) -> Result<(u16, String), String> {
    text_with("GET", base, &format!("/api/harnesses/{}/profiles", escape(harness)), None)
}

/// `GET /api/harnesses/{type}/catalog?directory=&profile=`: what the harness offers in a folder before a session exists.
pub fn harness_catalog(base: &str, harness: &str, directory: Option<&str>, profile: Option<&str>) -> Result<(u16, String), String> {
    let mut query = Vec::new();
    if let Some(d) = directory.filter(|d| !d.is_empty()) {
        query.push(format!("directory={}", query_value(d)));
    }
    if let Some(p) = profile.filter(|p| !p.is_empty()) {
        query.push(format!("profile={}", query_value(p)));
    }
    let tail = if query.is_empty() { String::new() } else { format!("?{}", query.join("&")) };
    text_with("GET", base, &format!("/api/harnesses/{}/catalog{tail}", escape(harness)), None)
}

/// `GET /api/worktrees/naming?directory=`.
pub fn worktree_naming(base: &str, directory: Option<&str>) -> Result<(u16, String), String> {
    let tail = directory.filter(|d| !d.is_empty()).map_or(String::new(), |d| format!("?directory={}", query_value(d)));
    text_with("GET", base, &format!("/api/worktrees/naming{tail}"), None)
}
