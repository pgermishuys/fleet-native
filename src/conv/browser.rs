//! The steps a tool call took in the agent's own browser (`stores/agent-browser.ts`; the tabs are `agent_tabs.rs`) and what the "+" menu's Browser field means (`lib/browser-target.ts`). Plain logic, no Rune.
use super::agent_tabs::{AgentTab, Tabs};
use serde_json::Value;
use std::collections::HashMap;

/// The element a step acted on, on the tab's 1280×800 screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// One action the agent took in its browser tab (`BrowserStep`).
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserStep {
    /// Its place in the session's steps, from 1.
    pub seq: i64,
    /// The operation, e.g. `click` or `tabs.open`.
    pub kind: String,
    /// What happened in plain words.
    pub summary: String,
    /// The operation and how long it took, as Fleet writes it.
    pub detail: String,
    pub ok: bool,
    pub error: Option<String>,
    /// The tool call that took it, when Fleet saw one running.
    pub call_id: Option<String>,
    pub title: Option<String>,
    /// A screenshot the step took, kept under the session: (id, width, height).
    pub screenshot: Option<(String, u32, u32)>,
    /// The tab it was taken on, and the page that tab showed after it.
    pub tab_id: Option<String>,
    pub url: Option<String>,
    /// When it happened, as ISO text: Agent's view rings the element for a few seconds after.
    pub at: Option<String>,
    pub bounds: Option<StepBox>,
}

impl BrowserStep {
    /// A step as the server sends it (the `browser.step` payload, and each item of `GET .../agent-browser`'s `steps`).
    /// `None` without a `seq`: such an item can't be told from another.
    pub fn read(v: &Value) -> Option<BrowserStep> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let shot = v.get("screenshot").filter(|s| s.is_object()).and_then(|s| {
            let size = |k: &str| s.get(k).and_then(Value::as_f64).filter(|n| n.is_finite() && *n > 0.0).map(|n| n as u32);
            Some((s.get("id")?.as_str()?.to_string(), size("width")?, size("height")?))
        });
        Some(BrowserStep {
            seq: v.get("seq").and_then(Value::as_i64)?,
            kind: text("kind").unwrap_or_default(),
            summary: text("summary").unwrap_or_default(),
            detail: text("detail").unwrap_or_default(),
            ok: v.get("ok").and_then(Value::as_bool).unwrap_or(false),
            error: text("error"),
            call_id: text("callId"),
            title: text("title"),
            screenshot: shot,
            tab_id: text("tabId"),
            url: text("url"),
            at: text("at"),
            bounds: v.get("box").and_then(|b| {
                let n = |k: &str| b.get(k).and_then(Value::as_f64).filter(|n| n.is_finite()).map(|n| n as f32);
                Some(StepBox { x: n("x")?, y: n("y")?, width: n("width")?, height: n("height")? })
            }),
        })
    }
}

/// The agent's steps per session (`useAgentBrowserStore`, steps only), each list in `seq` order and without repeats.
#[derive(Default)]
pub struct Steps {
    sessions: HashMap<String, Vec<BrowserStep>>,
    tabs: HashMap<String, Tabs>,
}

impl Steps {
    /// A new step, added once (`applyStep`): an event can be seen twice, and the list may already hold it.
    pub fn apply(&mut self, session: &str, step: BrowserStep) {
        let list = self.sessions.entry(session.to_string()).or_default();
        if list.iter().any(|known| known.seq == step.seq) {
            return;
        }
        if let (Some(tab), true) = (&step.tab_id, step.ok) {
            self.tabs.entry(session.to_string()).or_default().apply(tab, &step.kind, step.url.as_deref(), step.title.as_deref());
        }
        list.push(step);
    }

    /// The tabs and focus the server listed when the session loaded; they replace what steps made of them.
    pub fn set_tabs(&mut self, session: &str, tabs: Vec<AgentTab>, focused: Option<String>) {
        self.tabs.insert(session.to_string(), Tabs { tabs, focused });
    }

    pub fn tabs(&self, session: &str) -> Option<&Tabs> {
        self.tabs.get(session)
    }

    /// The agent's tab on the same app as `url`, when it has one open (`tabFor`).
    pub fn tab_for(&self, session: &str, url: Option<&str>) -> Option<&AgentTab> {
        self.tabs.get(session)?.tab_for(url)
    }

    /// The newest step taken on `tab_id` (`lastStepOn`).
    pub fn last_step_on(&self, session: &str, tab_id: &str) -> Option<&BrowserStep> {
        self.sessions.get(session)?.iter().rev().find(|s| s.tab_id.as_deref() == Some(tab_id))
    }

    /// What Fleet said when the session loaded; steps that arrived meanwhile stay (`setFromServer`).
    pub fn set_from_server(&mut self, session: &str, loaded: Vec<BrowserStep>) {
        let later: Vec<BrowserStep> =
            self.sessions.remove(session).unwrap_or_default().into_iter().filter(|s| !loaded.iter().any(|l| l.seq == s.seq)).collect();
        let mut all = loaded;
        all.extend(later);
        all.sort_by_key(|s| s.seq);
        self.sessions.insert(session.to_string(), all);
    }

    /// The steps a tool call took, oldest first (`stepsOf`). A call without an id took none.
    pub fn of(&self, session: &str, call_id: &str) -> Vec<&BrowserStep> {
        if call_id.is_empty() {
            return Vec::new();
        }
        let mut steps: Vec<&BrowserStep> =
            self.sessions.get(session).into_iter().flatten().filter(|s| s.call_id.as_deref() == Some(call_id)).collect();
        steps.sort_by_key(|s| s.seq);
        steps
    }

    pub fn count(&self, session: &str, call_id: &str) -> usize {
        self.of(session, call_id).len()
    }
}

/// The card's head: `Using its browser · 3 steps` while the call runs, `Used its browser · 1 step` after.
pub fn heading(running: bool, steps: usize) -> String {
    format!("{} · {steps} step{}", if running { "Using its browser" } else { "Used its browser" }, if steps == 1 { "" } else { "s" })
}

/// What the + menu's Browser field means: an address on Fleet's machine to show, or a command for Fleet to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserTarget {
    Address(String),
    Command(String),
}

/// `5173`, `localhost:5173/cart` and `http://127.0.0.1:8080` are addresses; anything else is a command; nothing is nothing.
pub fn browser_target(input: &str) -> Option<BrowserTarget> {
    let value = input.trim();
    if value.is_empty() {
        return None;
    }
    let no_space = !value.chars().any(char::is_whitespace);
    if value.len() <= 5 && value.bytes().all(|b| b.is_ascii_digit()) {
        return Some(BrowserTarget::Address(format!("http://localhost:{value}/")));
    }
    let lower = value.to_ascii_lowercase();
    if no_space
        && (lower.starts_with("http://") || lower.starts_with("https://"))
        && value.split_once("://").is_some_and(|(_, r)| !r.is_empty())
    {
        return Some(BrowserTarget::Address(normalize(value)));
    }
    if is_local_host(value) {
        return Some(BrowserTarget::Address(normalize(&format!("http://{value}"))));
    }
    Some(BrowserTarget::Command(value.to_string()))
}

/// `^(localhost|[\w-]+\.localhost|127(\.\d{1,3}){3}|0\.0\.0\.0|\[::1?\])(:\d{1,5})?(/\S*)?$`, case-insensitive.
fn is_local_host(value: &str) -> bool {
    let (authority, path) = value.find('/').map_or((value, ""), |i| value.split_at(i));
    if path.chars().any(char::is_whitespace) {
        return false;
    }
    let (host, port) = match authority.rfind(':') {
        // The colons of `[::1]` are the host's own.
        Some(i) if !authority[i..].contains(']') => (&authority[..i], Some(&authority[i + 1..])),
        _ => (authority, None),
    };
    if port.is_some_and(|p| p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    let host = host.to_ascii_lowercase();
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    let octets = |h: &str| {
        let parts: Vec<&str> = h.split('.').collect();
        parts.len() == 4
            && parts[0] == "127"
            && parts[1..].iter().all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
    };
    host == "localhost"
        || host.strip_suffix(".localhost").is_some_and(word)
        || octets(&host)
        || host == "0.0.0.0"
        || host == "[::1]"
        || host == "[::]"
}

/// `new URL(url).toString()` for the shapes the field takes: scheme and host in lower case, the scheme's own port left
/// out, a path of at least `/`. Anything it can't read is returned as typed, as Fleet's `catch` does.
fn normalize(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let scheme = scheme.to_ascii_lowercase();
    let at = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(at);
    let (host, port) = match authority.rfind(':') {
        Some(i) if !authority[i..].contains(']') => (&authority[..i], &authority[i + 1..]),
        _ => (authority, ""),
    };
    if host.is_empty() {
        return url.to_string();
    }
    let default = if scheme == "https" { "443" } else { "80" };
    let port = if port == default || port.is_empty() { String::new() } else { format!(":{port}") };
    let tail = if tail.is_empty() || !tail.starts_with('/') { format!("/{tail}") } else { tail.to_string() };
    format!("{scheme}://{}{port}{tail}", host.to_ascii_lowercase())
}
