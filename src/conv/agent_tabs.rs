//! The agent's open tabs (`stores/agent-browser.ts`, the tabs half): what the server listed when the session loaded, kept
//! current by each step, and which of them is the one a canvas showing some page should offer Agent's view of. Plain
//! logic, no Rune. The steps are in `browser.rs`.
use serde_json::Value;

/// A tab in the agent's own browser (`AgentTab` on the server).
#[derive(Clone, Debug, PartialEq)]
pub struct AgentTab {
    pub id: String,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub load_error: Option<String>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub generation: i64,
}

impl AgentTab {
    /// A tab as `GET .../agent-browser` lists it. `None` without an id: it couldn't be told from another.
    pub fn read(v: &Value) -> Option<AgentTab> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        Some(AgentTab {
            id: text("id")?,
            url: text("url").unwrap_or_default(),
            title: text("title").unwrap_or_default(),
            loading: flag("loading"),
            load_error: text("loadError"),
            can_go_back: flag("canGoBack"),
            can_go_forward: flag("canGoForward"),
            generation: v.get("generation").and_then(Value::as_i64).unwrap_or(0),
        })
    }
}

/// One session's tabs and the focused one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tabs {
    pub tabs: Vec<AgentTab>,
    pub focused: Option<String>,
}

impl Tabs {
    /// What a step did to the tabs (`applyStep`), for one that was ok and named a tab. `tabs.close` removes it, and if it
    /// had focus the last one left takes it; a step with a url updates the tab or adds it, and opening or focusing one
    /// focuses it. A failed step never gets here: it leaves the tabs alone.
    pub fn apply(&mut self, tab_id: &str, kind: &str, url: Option<&str>, title: Option<&str>) {
        if kind == "tabs.close" {
            self.tabs.retain(|t| t.id != tab_id);
            if self.focused.as_deref() == Some(tab_id) {
                self.focused = self.tabs.last().map(|t| t.id.clone());
            }
        } else if let Some(url) = url {
            let existing = self.tabs.iter().position(|t| t.id == tab_id);
            let old = existing.map(|i| &self.tabs[i]);
            let updated = AgentTab {
                id: tab_id.to_string(),
                url: url.to_string(),
                title: title.map(str::to_string).or_else(|| old.map(|t| t.title.clone())).unwrap_or_default(),
                loading: false,
                load_error: old.and_then(|t| t.load_error.clone()),
                can_go_back: old.is_some_and(|t| t.can_go_back),
                can_go_forward: old.is_some_and(|t| t.can_go_forward),
                generation: old.map_or(0, |t| t.generation),
            };
            match existing {
                Some(i) => self.tabs[i] = updated,
                None => self.tabs.push(updated),
            }
            if kind == "tabs.open" || kind == "tabs.focus" {
                self.focused = Some(tab_id.to_string());
            }
        }
    }

    /// The agent's tab on the same app as `url`: the focused one, else the last (`tabFor`). None without a url.
    pub fn tab_for(&self, url: Option<&str>) -> Option<&AgentTab> {
        let url = url.filter(|u| !u.is_empty())?;
        let matching: Vec<&AgentTab> = self.tabs.iter().filter(|t| same_app(&t.url, url)).collect();
        matching.iter().find(|t| Some(&t.id) == self.focused.as_ref()).or(matching.last()).copied()
    }
}

/// `scheme://host[:port]` of a url, read as `new URL` would: the scheme and host in lower case, the scheme's own port
/// left out, and `None` where it throws (no scheme or host, a port that isn't a number up to 65535).
fn origin(url: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    if scheme.is_empty() || !scheme.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let (host, port) = match authority.rfind(':') {
        // The colons of `[::1]` are the host's own.
        Some(i) if !authority[i..].contains(']') => (&authority[..i], &authority[i + 1..]),
        _ => (authority, ""),
    };
    if host.is_empty() || (!port.is_empty() && (!port.bytes().all(|b| b.is_ascii_digit()) || port.parse::<u32>().ok()? > 65535)) {
        return None;
    }
    let port = if port.is_empty() { String::new() } else { port.parse::<u32>().ok()?.to_string() };
    let default = match scheme.as_str() {
        "http" | "ws" => "80",
        "https" | "wss" => "443",
        "ftp" => "21",
        _ => "",
    };
    let port = if port == default { String::new() } else { port };
    Some((scheme, host.to_ascii_lowercase(), port))
}

/// The same origin (scheme, host, port), so a canvas on `localhost:5173` matches the agent's `127.0.0.1:5173` too: the
/// loopback names count as one host, and an empty port reads as "80" whatever the scheme (`sameApp`).
pub fn same_app(a: &str, b: &str) -> bool {
    let (Some((sa, ha, pa)), Some((sb, hb, pb))) = (origin(a), origin(b)) else { return false };
    let loopback = |h: &str| h == "localhost" || h == "127.0.0.1" || h == "[::1]" || h.ends_with(".localhost");
    let host = |h: String| if loopback(&h) { "loopback".to_string() } else { h };
    let port = |p: String| if p.is_empty() { "80".to_string() } else { p };
    sa == sb && host(ha) == host(hb) && port(pa) == port(pb)
}
