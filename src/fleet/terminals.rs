//! Terminal tabs per session, which one is showing, and whether a session's drawer is open (component 13, piece C; rows 13.33
//! to 13.36, 13.39 and 13.40 of `docs/parity/13-terminal.md`). Ports `stores/terminals.ts`. Plain data, no Rune and no I/O:
//! what the drawer remembers across runs is read from and written to the settings file by `State` (`session_terminals.rs`).
use super::http::Terminal;
use std::collections::{BTreeSet, HashMap};

/// `DEFAULT_DRAWER_HEIGHT`.
pub const DEFAULT_DRAWER_HEIGHT: u32 = 272;
/// `MIN_DRAWER_HEIGHT`.
pub const MIN_DRAWER_HEIGHT: u32 = 140;

/// What `terminal.opened` and `terminal.closed` carry (`TerminalEvent`).
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalEvent {
    Opened { session: String, terminal: String, title: String },
    Closed { session: String, terminal: String },
}

impl TerminalEvent {
    pub fn session(&self) -> &str {
        match self {
            TerminalEvent::Opened { session, .. } | TerminalEvent::Closed { session, .. } => session,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Tabs {
    terminals: Vec<Terminal>,
    active: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Terminals {
    sessions: HashMap<String, Tabs>,
    /// Sessions whose list has come from the server at least once.
    loaded: BTreeSet<String>,
    /// Sessions whose drawer is open.
    open: BTreeSet<String>,
    height: u32,
    /// Something the settings file keeps (an open drawer, the height) changed since it was last taken.
    dirty: bool,
}

impl Default for Terminals {
    fn default() -> Self {
        Terminals { sessions: HashMap::new(), loaded: BTreeSet::new(), open: BTreeSet::new(), height: DEFAULT_DRAWER_HEIGHT, dirty: false }
    }
}

impl Terminals {
    /// What the settings file held (`readOpenSessions`, `readHeight`: a height under the minimum is not one).
    pub fn restore(&mut self, open: impl IntoIterator<Item = String>, height: Option<u32>) {
        self.open = open.into_iter().collect();
        self.height = height.filter(|h| *h >= MIN_DRAWER_HEIGHT).unwrap_or(DEFAULT_DRAWER_HEIGHT);
        self.dirty = false;
    }

    /// The drawer's height, one value for the window.
    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn open_sessions(&self) -> impl Iterator<Item = &String> {
        self.open.iter()
    }

    /// What to write to the settings file, once after a change.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn terminals_for(&self, session: &str) -> &[Terminal] {
        self.sessions.get(session).map_or(&[], |s| &s.terminals)
    }

    pub fn active_for(&self, session: &str) -> Option<&Terminal> {
        let tabs = self.sessions.get(session)?;
        tabs.terminals.iter().find(|t| Some(&t.id) == tabs.active.as_ref())
    }

    /// Replaces the session's tabs with what the server listed, keeping the active tab when it is still there.
    pub fn set_terminals(&mut self, session: &str, list: Vec<Terminal>) {
        self.loaded.insert(session.to_string());
        let tabs = self.sessions.entry(session.to_string()).or_default();
        tabs.terminals = list;
        if !tabs.terminals.iter().any(|t| Some(&t.id) == tabs.active.as_ref()) {
            tabs.active = tabs.terminals.first().map(|t| t.id.clone());
        }
    }

    /// Adds or updates a tab. A new tab shows when asked to, or when nothing else is showing.
    pub fn add(&mut self, session: &str, terminal: Terminal, activate: bool) {
        let tabs = self.sessions.entry(session.to_string()).or_default();
        let id = terminal.id.clone();
        match tabs.terminals.iter().position(|t| t.id == id) {
            Some(at) => tabs.terminals[at] = terminal,
            None => tabs.terminals.push(terminal),
        }
        if activate || tabs.active.is_none() {
            tabs.active = Some(id);
        }
    }

    /// Removes a tab; the one next to it shows instead (the same place, else the last).
    pub fn remove(&mut self, session: &str, terminal: &str) {
        let Some(tabs) = self.sessions.get_mut(session) else { return };
        let Some(at) = tabs.terminals.iter().position(|t| t.id == terminal) else { return };
        tabs.terminals.remove(at);
        if tabs.active.as_deref() == Some(terminal) {
            tabs.active = tabs.terminals.get(at.min(tabs.terminals.len().saturating_sub(1))).map(|t| t.id.clone());
        }
    }

    pub fn set_active(&mut self, session: &str, terminal: &str) {
        if let Some(tabs) = self.sessions.get_mut(session).filter(|t| t.terminals.iter().any(|x| x.id == terminal)) {
            tabs.active = Some(terminal.to_string());
        }
    }

    /// A tab opened or closed elsewhere (another window, or the shell ended). One opened elsewhere doesn't take the view.
    pub fn apply_event(&mut self, event: &TerminalEvent) {
        match event {
            TerminalEvent::Opened { session, terminal, title } => {
                if self.terminals_for(session).iter().any(|t| &t.id == terminal) {
                    return;
                }
                self.add(session, Terminal { id: terminal.clone(), title: title.clone(), running: true }, false);
            }
            TerminalEvent::Closed { session, terminal } => self.remove(session, terminal),
        }
    }

    /// Whether the session's list has come from the server at least once.
    pub fn is_loaded(&self, session: &str) -> bool {
        self.loaded.contains(session)
    }

    pub fn is_open(&self, session: &str) -> bool {
        self.open.contains(session)
    }

    pub fn set_open(&mut self, session: &str, open: bool) {
        if open == self.is_open(session) {
            return;
        }
        if open {
            self.open.insert(session.to_string());
        } else {
            self.open.remove(session);
        }
        self.dirty = true;
    }

    pub fn toggle_open(&mut self, session: &str) {
        self.set_open(session, !self.is_open(session));
    }

    /// Never shorter than the minimum; whole pixels.
    pub fn set_height(&mut self, height: f32) {
        let height = (height.round().max(0.0) as u32).max(MIN_DRAWER_HEIGHT);
        if height != self.height {
            self.height = height;
            self.dirty = true;
        }
    }

    /// Drops a session's tabs and closes its drawer. Fleet's client calls this from nowhere; it is ported with its test only.
    pub fn forget_session(&mut self, session: &str) {
        self.sessions.remove(session);
        self.loaded.remove(session);
        self.set_open(session, false);
    }
}
