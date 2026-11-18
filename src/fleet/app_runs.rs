//! Apps Fleet runs for sessions, as browser tabs show them (component 14, piece G; rows 14.59 and 14.65). Ports
//! `stores/app-runs.ts` and `applyAppEvent` of `composables/use-server-canvases.ts`. `app.updated` events keep them current;
//! a tab loads its app once when it opens and after a reconnect. Output isn't pushed: a tab asks for the lines after the ones
//! it has while it shows them.
//!
//! Events can overtake a response that was read earlier, so every request remembers the event clock it was asked at and its
//! answer lands only if no event for that app came after.
use super::events::{Event, Kind};
use super::http;
use crate::State;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Fleet keeps this many lines of an app's output; so does the tab.
pub const MAX_OUTPUT_LINES: usize = 2000;

/// The line Fleet writes into an app's output when it starts the app again (`AppRunner`).
pub const RESTART_MARKER: &str = "── restarted by Fleet ──";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppStatus {
    Starting,
    Running,
    BuildFailed,
    Exited,
    Stopped,
}

impl AppStatus {
    pub fn read(name: &str) -> Option<AppStatus> {
        Some(match name {
            "starting" => AppStatus::Starting,
            "running" => AppStatus::Running,
            "build-failed" => AppStatus::BuildFailed,
            "exited" => AppStatus::Exited,
            "stopped" => AppStatus::Stopped,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRun {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub status: AppStatus,
    pub exit_code: Option<i64>,
    /// The page Fleet found, once one answered.
    pub url: Option<String>,
    pub ports: Vec<u16>,
    /// The addresses on this machine it printed, in order; loaded with the app, not carried by events.
    pub printed_urls: Vec<String>,
    /// Why it last changed, when that came from an event (`started`, `ready`, `restarted`, `stopped`, `exited`,
    /// `build-failed`, `reloaded`).
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppOutput {
    pub lines: Vec<String>,
    /// The `after` to ask with next time.
    pub next: i64,
}

/// The session's apps, and the command that last served a page in its project (for the + menu).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionApps {
    pub apps: Vec<AppRun>,
    pub preview_command: Option<String>,
}

/// An app as `GET .../apps/{id}`, a `POST .../apps` and the list say it (`fromResponse`).
fn from_response(session: &str, body: &Value) -> Option<AppRun> {
    Some(AppRun {
        id: body.get("id")?.as_str()?.to_string(),
        session_id: session.to_string(),
        command: body.get("command").and_then(Value::as_str).unwrap_or_default().to_string(),
        status: AppStatus::read(body.get("status")?.as_str()?)?,
        exit_code: body.get("exitCode").and_then(Value::as_i64),
        url: body.get("url").and_then(Value::as_str).map(str::to_string),
        ports: ports_of(body),
        printed_urls: body
            .get("printedUrls")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |all| all.iter().filter_map(Value::as_str).map(str::to_string).collect()),
        reason: None,
    })
}

fn ports_of(body: &Value) -> Vec<u16> {
    body.get("ports")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |all| all.iter().filter_map(Value::as_u64).filter_map(|p| u16::try_from(p).ok()).collect())
}

/// The server's own words for a refusal: its `error`, else "HTTP {status}" (`errorText`).
fn error_text(status: u16, body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {status}"))
}

fn session_path(session: &str) -> String {
    format!("/api/sessions/{}", http::escape(session))
}

fn app_path(session: &str, app: &str) -> String {
    format!("{}/apps/{}", session_path(session), http::escape(app))
}

/// The apps and their output the app knows of, and the event clock that keeps a late answer from undoing a newer event.
#[derive(Debug, Default)]
pub struct AppRuns {
    by_id: HashMap<String, AppRun>,
    output: HashMap<String, AppOutput>,
    clock: u64,
    changed_at: HashMap<String, u64>,
}

impl AppRuns {
    pub fn app(&self, id: &str) -> Option<&AppRun> {
        self.by_id.get(id)
    }

    pub fn output(&self, id: &str) -> Option<&AppOutput> {
        self.output.get(id)
    }

    /// The ids of the apps kept for a session, for reloading them after a reconnect.
    pub fn ids_of(&self, session: &str) -> Vec<String> {
        self.by_id.values().filter(|a| a.session_id == session).map(|a| a.id.clone()).collect()
    }

    fn set_if_current(&mut self, run: AppRun, asked_at: u64) {
        if self.changed_at.get(&run.id).copied().unwrap_or(0) > asked_at {
            return;
        }
        self.by_id.insert(run.id.clone(), run);
    }

    /// `applyEvent`: an `app.updated` payload sets the app. Printed links stay across events and go on a restart. False for a
    /// payload that isn't one.
    pub fn apply_event(&mut self, payload: &Value) -> bool {
        let text = |key: &str| payload.get(key).and_then(Value::as_str);
        let (Some(id), Some(session), Some(status)) = (text("appId"), text("sessionId"), text("status").and_then(AppStatus::read)) else {
            return false;
        };
        self.clock += 1;
        self.changed_at.insert(id.to_string(), self.clock);
        let reason = text("reason").map(str::to_string);
        let printed_urls = if reason.as_deref() == Some("restarted") {
            Vec::new()
        } else {
            self.by_id.get(id).map(|a| a.printed_urls.clone()).unwrap_or_default()
        };
        self.by_id.insert(
            id.to_string(),
            AppRun {
                id: id.to_string(),
                session_id: session.to_string(),
                command: text("command").unwrap_or_default().to_string(),
                status,
                exit_code: payload.get("exitCode").and_then(Value::as_i64),
                url: text("url").map(str::to_string),
                ports: ports_of(payload),
                printed_urls,
                reason,
            },
        );
        true
    }

    /// Sets an app's output outright (scenes and tests that draw one).
    pub fn set_output(&mut self, app: &str, lines: &[&str]) {
        let next = lines.len() as i64;
        self.output.insert(app.to_string(), AppOutput { lines: lines.iter().map(|l| l.to_string()).collect(), next });
    }

    /// What `GET .../output?after=N` said. Fewer lines than before means Fleet restarted and lost them: `false`, and the
    /// caller asks again from the start. An answer with nothing new changes nothing once there is something kept.
    fn take_output(&mut self, app: &str, body: &Value) -> bool {
        let current = self.output.get(app).cloned().unwrap_or_default();
        let next = body.get("next").and_then(Value::as_i64).unwrap_or(0);
        if next < current.next {
            self.output.insert(app.to_string(), AppOutput::default());
            return false;
        }
        let new: Vec<String> = body
            .get("lines")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |all| all.iter().filter_map(Value::as_str).map(str::to_string).collect());
        if new.is_empty() && next == current.next && self.output.contains_key(app) {
            return true;
        }
        let mut lines = current.lines;
        lines.extend(new);
        if lines.len() > MAX_OUTPUT_LINES {
            lines.drain(..lines.len() - MAX_OUTPUT_LINES);
        }
        self.output.insert(app.to_string(), AppOutput { lines, next });
        true
    }
}

/// `addressForPort`: the address to open for one of the app's ports: a link it printed for that port, one with a query string
/// first (a sign-in link, like Aspire's `/login?t=…`, signs the user's browser in), or else plain http on it.
pub fn address_for_port(port: u16, printed: &[String]) -> String {
    let on_port: Vec<(&String, bool)> =
        printed.iter().filter_map(|url| split_address(url).filter(|(p, _)| *p == port).map(|(_, query)| (url, query))).collect();
    on_port
        .iter()
        .find(|(_, query)| *query)
        .or(on_port.first())
        .map_or_else(|| format!("http://localhost:{port}/"), |(url, _)| (*url).clone())
}

/// The port an address is on (the scheme's own when it names none) and whether it has a query longer than `?` (`URL.search`).
fn split_address(url: &str) -> Option<(u16, bool)> {
    let (scheme, rest) = url.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].rsplit('@').next()?;
    let port = match host.rsplit_once(':') {
        Some((_, p)) if !host.ends_with(']') => p.parse::<u16>().ok()?,
        _ if scheme.eq_ignore_ascii_case("https") => 443,
        _ => 80,
    };
    let query = rest[end..].split('#').next().unwrap_or_default().split_once('?').is_some_and(|(_, q)| !q.is_empty());
    Some((port, query))
}

/// `currentRunLines`: the output of the app's current run, the lines after Fleet last started it again.
pub fn current_run_lines(lines: &[String]) -> Vec<String> {
    match lines.iter().rposition(|l| l == RESTART_MARKER) {
        Some(at) => lines[at + 1..].to_vec(),
        None => lines.to_vec(),
    }
}

impl State {
    /// `app.updated` on a session's topic: the store takes it, and a page Fleet reloaded pulses every browser tab that shows
    /// the app (`applyAppEvent`).
    pub fn on_app_event(&mut self, event: &Event) {
        let Event::Other(Kind::AppUpdated, payload) = event else { return };
        if !self.apps.apply_event(payload) {
            return;
        }
        if payload.get("reason").and_then(Value::as_str) != Some("reloaded") {
            return;
        }
        let (Some(session), Some(app)) = (payload.get("sessionId").and_then(Value::as_str), payload.get("appId").and_then(Value::as_str))
        else {
            return;
        };
        let tabs: Vec<String> = self
            .canvases
            .session(session)
            .canvases
            .iter()
            .filter(|c| c.browser().is_some_and(|b| b.app_id.as_deref() == Some(app)))
            .map(|c| c.id.clone())
            .collect();
        for tab in tabs {
            self.canvases.mark_updated(&tab, self.clock);
        }
    }

    /// Loads the app as it is now; `done` gets `Ok(None)` when the session has no such app, and the app as the store holds it
    /// otherwise (a newer event wins over what was read).
    pub fn load_app(&mut self, session: &str, app: &str, done: impl FnOnce(&mut State, Result<Option<AppRun>, String>) + Send + 'static) {
        let Some(link) = self.link_for(session) else { return };
        let asked_at = self.apps.clock;
        let (session, app) = (session.to_string(), app.to_string());
        let path = app_path(&session, &app);
        link.off_thread(
            move |base| http::text_with("GET", base, &path, None),
            move |s, _, result| {
                let answer = match result {
                    Err(e) => Err(e),
                    Ok((404, _)) => Ok(None),
                    Ok((status, body)) if !(200..300).contains(&status) => Err(error_text(status, &body)),
                    Ok((_, body)) => match serde_json::from_str(&body).ok().and_then(|b| from_response(&session, &b)) {
                        Some(run) => {
                            let id = run.id.clone();
                            s.apps.set_if_current(run, asked_at);
                            Ok(s.apps.app(&id).cloned())
                        }
                        None => Err("Fleet's answer wasn't an app.".to_string()),
                    },
                };
                done(s, answer);
            },
        );
    }

    /// Loads again every app of the session the store knows, after a reconnect: the events in between weren't replayed.
    pub fn reload_apps(&mut self, session: &str) {
        for app in self.apps.ids_of(session) {
            self.load_app(session, &app, |_, _| {});
        }
    }

    /// Starts (or restarts) or stops the app. `done` gets why Fleet refused, or `None`.
    pub fn act_on_app(
        &mut self,
        session: &str,
        app: &str,
        action: &'static str,
        done: impl FnOnce(&mut State, Option<String>) + Send + 'static,
    ) {
        let Some(link) = self.link_for(session) else { return };
        let asked_at = self.apps.clock;
        let (session, app) = (session.to_string(), app.to_string());
        let path = format!("{}/{action}", app_path(&session, &app));
        link.off_thread(
            move |base| http::text_with("POST", base, &path, None),
            move |s, _, result| match result {
                Err(e) => done(s, Some(e)),
                Ok((status, body)) if !(200..300).contains(&status) => done(s, Some(error_text(status, &body))),
                Ok((200, body)) => {
                    if let Some(run) = serde_json::from_str(&body).ok().and_then(|b| from_response(&session, &b)) {
                        s.apps.set_if_current(run, asked_at);
                    }
                    done(s, None);
                }
                Ok(_) => {
                    s.load_app(&session, &app, |_, _| {});
                    done(s, None);
                }
            },
        );
    }

    /// Fetches the output after the lines the store has; asks again from the start when Fleet has fewer than the store.
    pub fn fetch_app_output(&mut self, session: &str, app: &str) {
        let Some(link) = self.link_for(session) else { return };
        let after = self.apps.output(app).map_or(0, |o| o.next);
        let (session, app) = (session.to_string(), app.to_string());
        let path = format!("{}/output?after={after}", app_path(&session, &app));
        link.off_thread(
            move |base| http::text_with("GET", base, &path, None),
            move |s, _, result| {
                let Ok((status, body)) = result else { return };
                let Ok(body) =
                    serde_json::from_str::<Value>(&body).map_err(drop).and_then(|b| (200..300).contains(&status).then_some(b).ok_or(()))
                else {
                    return;
                };
                if !s.apps.take_output(&app, &body) {
                    s.fetch_app_output(&session, &app);
                }
            },
        );
    }

    /// The session's apps and the command that last served a page in its project (`listSessionApps`).
    pub fn list_session_apps(&mut self, session: &str, done: impl FnOnce(&mut State, Result<SessionApps, String>) + Send + 'static) {
        let Some(link) = self.link_for(session) else { return };
        let session = session.to_string();
        let path = format!("{}/apps", session_path(&session));
        link.off_thread(
            move |base| http::text_with("GET", base, &path, None),
            move |s, _, result| {
                let answer = match result {
                    Err(e) => Err(e),
                    Ok((status, body)) if !(200..300).contains(&status) => Err(error_text(status, &body)),
                    Ok((_, body)) => {
                        let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                        Ok(SessionApps {
                            apps: body
                                .get("apps")
                                .and_then(Value::as_array)
                                .map_or_else(Vec::new, |a| a.iter().filter_map(|b| from_response(&session, b)).collect()),
                            preview_command: body.get("previewCommand").and_then(Value::as_str).map(str::to_string),
                        })
                    }
                };
                done(s, answer);
            },
        );
    }

    /// Runs a command in the session's folder and shows it in a browser tab; `done` gets that tab's canvas id.
    pub fn start_command(&mut self, session: &str, command: &str, done: impl FnOnce(&mut State, Result<String, String>) + Send + 'static) {
        let Some(link) = self.link_for(session) else { return };
        let asked_at = self.apps.clock;
        let session = session.to_string();
        let (path, body) = (format!("{}/apps", session_path(&session)), json!({"command": command}));
        link.off_thread(
            move |base| http::text_with("POST", base, &path, Some(&body)),
            move |s, _, result| {
                let answer = match result {
                    Err(e) => Err(e),
                    Ok((status, body)) if !(200..300).contains(&status) => Err(error_text(status, &body)),
                    Ok((_, body)) => {
                        let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                        if let Some(run) = body.get("app").and_then(|a| from_response(&session, a)) {
                            s.apps.set_if_current(run, asked_at);
                        }
                        body.get("canvasId")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .ok_or_else(|| "Fleet didn't say which tab shows it.".to_string())
                    }
                };
                done(s, answer);
            },
        );
    }

    /// Shows a page on Fleet's machine in a browser tab; `done` gets that tab's canvas id.
    pub fn open_address(&mut self, session: &str, url: &str, done: impl FnOnce(&mut State, Result<String, String>) + Send + 'static) {
        let Some(link) = self.link_for(session) else { return };
        let (path, body) = (format!("{}/browser", session_path(session)), json!({"url": url}));
        link.off_thread(
            move |base| http::text_with("POST", base, &path, Some(&body)),
            move |s, _, result| {
                let answer = match result {
                    Err(e) => Err(e),
                    Ok((status, body)) if !(200..300).contains(&status) => Err(error_text(status, &body)),
                    Ok((_, body)) => serde_json::from_str::<Value>(&body)
                        .ok()
                        .and_then(|b| b.get("canvasId").and_then(Value::as_str).map(str::to_string))
                        .ok_or_else(|| "Fleet didn't say which tab shows it.".to_string()),
                };
                done(s, answer);
            },
        );
    }
}
