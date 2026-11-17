//! Settings → What agents can do › Browser (`use-agent-browser-settings.ts`): whether agents get a browser of their own, which
//! pages it may open and whether page scripts run. One copy per machine, shared by the Browser group, the Browser chip and each
//! harness's "without the browser" words, as Fleet's `copies` map is. Plain Rust over the requests `State` makes.
use crate::State;
use crate::fleet::http;
use serde_json::{Value, json};

/// Which pages the agent's tab may go to (`BrowserPages`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pages {
    Session,
    Machine,
    Any,
}

impl Pages {
    pub const ALL: [Pages; 3] = [Pages::Session, Pages::Machine, Pages::Any];

    pub fn value(self) -> &'static str {
        match self {
            Pages::Session => "session",
            Pages::Machine => "machine",
            Pages::Any => "any",
        }
    }

    pub fn parse(word: &str) -> Option<Pages> {
        Pages::ALL.into_iter().find(|p| p.value() == word)
    }

    pub fn label(self) -> &'static str {
        match self {
            Pages::Session => "This session's pages",
            Pages::Machine => "Any page on this machine",
            Pages::Any => "Any address",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Pages::Session => "Apps it started and pages in its canvases, on this machine. Recommended.",
            Pages::Machine => "Any localhost address, such as a server you started yourself.",
            Pages::Any => "Includes the internet. Pages can then send what they read anywhere.",
        }
    }
}

/// `BrowserSettings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub pages: Pages,
    pub scripts: bool,
}

impl Default for Settings {
    /// What the page shows before the machine has answered.
    fn default() -> Self {
        Settings { enabled: true, pages: Pages::Session, scripts: false }
    }
}

impl Settings {
    /// The machine's answer; `None` when it isn't the three fields.
    pub fn parse(body: &str) -> Option<Settings> {
        let v: Value = serde_json::from_str(body).ok()?;
        Some(Settings {
            enabled: v.get("enabled")?.as_bool()?,
            pages: Pages::parse(v.get("pages")?.as_str()?)?,
            scripts: v.get("scripts")?.as_bool()?,
        })
    }
}

/// One change, sent alone (`save({ enabled })`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Enabled(bool),
    Pages(Pages),
    Scripts(bool),
}

impl Change {
    pub fn body(self) -> Value {
        match self {
            Change::Enabled(on) => json!({ "enabled": on }),
            Change::Pages(p) => json!({ "pages": p.value() }),
            Change::Scripts(on) => json!({ "scripts": on }),
        }
    }

    fn applied(self, mut s: Settings) -> Settings {
        match self {
            Change::Enabled(on) => s.enabled = on,
            Change::Pages(p) => s.pages = p,
            Change::Scripts(on) => s.scripts = on,
        }
        s
    }
}

pub const LOAD_ERROR: &str = "Fleet couldn't load the browser settings. Try again in a moment.";
pub const SAVE_ERROR: &str = "Fleet couldn't save that. Try again.";

/// What one machine's copy holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Copy {
    pub settings: Settings,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
}

impl State {
    /// The copy of the machine Settings shows.
    pub fn browser_copy(&self) -> Copy {
        self.settings.browser.get(&self.harness_machine()).cloned().unwrap_or_default()
    }

    /// Whether the machine shown says the browser is off (it is on until the machine says otherwise).
    pub fn browser_off(&self) -> bool {
        !self.browser_copy().settings.enabled
    }

    /// `load`: asks the machine, and keeps what it says. A machine that can't be reached from here (no server) is not waited on.
    pub fn read_browser(&mut self, machine: &str) {
        let key = machine.to_string();
        let asked = self.feed_call(
            machine,
            |base| http::text_with("GET", base, "/api/agent-browser/settings", None).ok().filter(|(s, _)| (200..300).contains(s)),
            {
                let key = key.clone();
                move |s: &mut State, _, answer: Option<(u16, String)>| {
                    let copy = s.settings.browser.entry(key).or_default();
                    copy.loading = false;
                    match answer.and_then(|(_, body)| Settings::parse(&body)) {
                        Some(settings) => {
                            copy.settings = settings;
                            copy.error = None;
                        }
                        None => copy.error = Some(LOAD_ERROR.to_string()),
                    }
                }
            },
        );
        if asked {
            self.settings.browser.entry(key).or_default().loading = true;
        }
    }

    /// `save`: the change shows at once, the machine gets only that field, its answer replaces the settings, and a refusal puts
    /// the old ones back. Ignored while the machine is loading or saving.
    pub fn save_browser(&mut self, change: Change) {
        let machine = self.harness_machine();
        let copy = self.browser_copy();
        if copy.loading || copy.saving {
            return;
        }
        let before = copy.settings;
        let sent = self.feed_call(
            &machine,
            move |base| {
                http::text_with("PUT", base, "/api/agent-browser/settings", Some(&change.body()))
                    .ok()
                    .filter(|(s, _)| (200..300).contains(s))
            },
            {
                let machine = machine.clone();
                move |s: &mut State, _, answer: Option<(u16, String)>| {
                    let copy = s.settings.browser.entry(machine).or_default();
                    copy.saving = false;
                    match answer.and_then(|(_, body)| Settings::parse(&body)) {
                        Some(settings) => copy.settings = settings,
                        None => {
                            copy.settings = before;
                            copy.error = Some(SAVE_ERROR.to_string());
                        }
                    }
                }
            },
        );
        let copy = self.settings.browser.entry(machine).or_default();
        copy.settings = change.applied(before);
        if sent {
            copy.saving = true;
            copy.error = None;
        }
    }
}
