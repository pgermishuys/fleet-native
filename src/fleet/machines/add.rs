//! Add a machine's tabs (`MachinesSection.vue`'s `addMode`) and what the Found and On a new server tabs keep while they are shown
//! (`FoundMachinesCard.vue`, `NewServerCard.vue`). Fleet mounts a card per tab, so its timers start when the tab is picked and stop
//! when it is left or the form closes; here the poll does the same from the tab and the app's clock, and an answer for a tab
//! that was left since is dropped. "Set it up from here" (SSH) is not a tab here: it is the owner's decision 5.
use super::found::{FoundMachine, JoinCode, LOOK_EVERY, load_found, make_join_code};
use crate::fleet::http;
use crate::fleet::pairing::{Failure, pair};
use crate::fleet::work::parse_iso_ms;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Where `install.sh` is published with each release (`INSTALL_URL`).
pub const INSTALL_URL: &str = "https://github.com/pgermishuys/weave-fleet/releases/latest/download/install.sh";
/// How often On a new server reads the machines for the one that joined (`LOOK_EVERY_MS`).
pub const JOIN_LOOK_EVERY: f64 = 3.0;
/// How long Copy reads "Copied".
const COPIED_SECS: f64 = 2.0;
/// `maxlength="9"` on Found's code field: the whole code with its dash.
const CODE_MAX: usize = 9;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AddMode {
    /// Where the form opens: a waiting machine needs nothing pasted.
    #[default]
    Found,
    Server,
    Link,
}

impl AddMode {
    pub const ALL: [AddMode; 3] = [AddMode::Found, AddMode::Server, AddMode::Link];

    /// The tab's `data-testid` ends in this.
    pub fn id(self) -> &'static str {
        match self {
            AddMode::Found => "found",
            AddMode::Server => "server",
            AddMode::Link => "link",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AddMode::Found => "Found",
            AddMode::Server => "On a new server",
            AddMode::Link => "Paste a pairing link",
        }
    }
}

/// Found: the rest of each waiting machine's code as typed, the one being added and why one couldn't be.
#[derive(Default)]
pub struct FoundTab {
    pub rest: HashMap<String, TextEdit>,
    pub busy: Option<String>,
    pub failure: HashMap<String, Failure>,
    /// When the next look is due on the app's clock; None looks at once.
    next_look: Option<f64>,
    pub(crate) looking: bool,
}

impl FoundTab {
    pub fn typed(&self, id: &str) -> &str {
        self.rest.get(id).map_or("", |e| e.text.as_str())
    }
}

/// On a new server: the code, the countdown and the machines there were when it was made.
#[derive(Default)]
pub struct ServerTab {
    pub code: Option<JoinCode>,
    pub failure: Option<String>,
    pub making: bool,
    pub copied_until: Option<f64>,
    /// "in 30 min", from the code's expiry and the wall clock; kept by the poll so the view only reads it.
    pub countdown: String,
    pub expired: bool,
    /// The machines' ids when the code was made: whatever shows up after that is the one that joined.
    known: HashSet<String>,
    next_look: Option<f64>,
    pub(crate) looking: bool,
}

impl ServerTab {
    /// `curl -fsSL <install.sh> | sh -s -- --join '<target>'`.
    pub fn line(&self) -> String {
        self.code.as_ref().map_or_else(String::new, |c| format!("curl -fsSL {INSTALL_URL} | sh -s -- --join '{}'", c.target))
    }
}

/// Each machine home lists, by id and name (the ids are what `knownIds` compares). None when home couldn't be read.
fn machine_ids(base: &str) -> Option<Vec<(String, String)>> {
    let (status, body) = http::text_with("GET", base, "/api/machines", None).ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    let v: Value = serde_json::from_str(&body).ok()?;
    let list = v.get("machines")?.as_array()?;
    Some(
        list.iter()
            .filter_map(|m| {
                let word = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_string);
                Some((word("id")?, word("name").unwrap_or_default()))
            })
            .collect(),
    )
}

impl State {
    /// A tab of the strip (`pickMode`): a tab's card starts afresh, as Fleet mounts it again.
    pub fn pick_add_mode(&mut self, mode: AddMode, tx: &mut Tx) {
        let form = &mut self.machine_page.add;
        form.mode = mode;
        form.error = None;
        form.link_hint = None;
        form.tab += 1;
        form.waiting.next_look = None;
        form.waiting.looking = false;
        form.server = ServerTab::default();
        match mode {
            AddMode::Server => self.make_new_server_code(),
            AddMode::Link => tx.focus(crate::fleet::pairing::LINK_FIELD, true),
            AddMode::Found => {}
        }
    }

    /// Make a new code / Try again, and the tab's first code (`make`): the machines are read first, so the one that joins can be
    /// told from those already there.
    pub fn make_new_server_code(&mut self) {
        let form = &mut self.machine_page.add;
        let server = &mut form.server;
        server.making = true;
        server.failure = None;
        server.copied_until = None;
        let Some(link) = self.link.clone() else {
            server.making = false;
            server.failure = Some("Can't reach Fleet.".into());
            return;
        };
        let mine = form.tab;
        link.off_thread(
            |home| {
                let known = machine_ids(home).unwrap_or_default();
                make_join_code(home).map(|code| (known, code))
            },
            move |s, _, result| {
                if s.machine_page.add.tab != mine {
                    return;
                }
                let clock = s.clock;
                let server = &mut s.machine_page.add.server;
                server.making = false;
                match result {
                    Ok((known, code)) => {
                        server.known = known.into_iter().map(|(id, _)| id).collect();
                        server.code = Some(code);
                        server.next_look = Some(clock + JOIN_LOOK_EVERY);
                    }
                    Err(why) => {
                        server.code = None;
                        server.failure = Some(why);
                    }
                }
                s.keep_server_countdown();
            },
        );
    }

    /// Copy: the whole line, then "Copied" for 2 s.
    pub fn copy_new_server_line(&mut self, tx: &mut Tx) {
        let server = &mut self.machine_page.add.server;
        if server.code.is_none() || server.expired {
            return;
        }
        tx.copy(server.line());
        server.copied_until = Some(self.clock + COPIED_SECS);
        tx.wake_after(Duration::from_secs_f64(COPIED_SECS));
    }

    /// Add <name> on a found machine's row (`add`): home pairs with the whole code; the list is looked at again either way.
    pub fn add_found(&mut self, id: &str) {
        let Some(found) = self.machine_data.found.as_ref().and_then(|l| l.iter().find(|f| f.machine_id == id)).cloned() else { return };
        let tab = &mut self.machine_page.add.waiting;
        let Some(code) = found.code_for(tab.typed(id)) else { return };
        if tab.busy.is_some() {
            return;
        }
        let Some(link) = self.link.clone() else { return };
        tab.busy = Some(id.to_string());
        tab.failure.remove(id);
        let id = id.to_string();
        let base_url = found.base_url.clone();
        link.off_thread(
            move |home| pair(home, &base_url, &code),
            move |s, _, result| {
                let tab = &mut s.machine_page.add.waiting;
                tab.busy = None;
                tab.next_look = None;
                match result {
                    Ok(name) => s.machine_added(name),
                    Err(why) => {
                        s.machine_page.add.waiting.failure.insert(id, why);
                    }
                }
            },
        );
    }

    /// A machine was added or joined (`onSetUp`): the form closes and the page says so.
    pub fn machine_added(&mut self, name: String) {
        self.close_add_form();
        self.machine_page.add.added = Some(name);
        if let Some(link) = &self.link {
            link.refresh_machines();
        }
    }

    /// The countdown and whether the code has expired, from the wall clock. True when either changed.
    fn keep_server_countdown(&mut self) -> bool {
        let now = crate::view::message::now_ms(self);
        let server = &mut self.machine_page.add.server;
        let Some(code) = &server.code else { return false };
        let left = parse_iso_ms(&code.expires_at).map_or(0, |at| at - now);
        let (countdown, expired) = (crate::conv::retry::format_countdown(left), left <= 0);
        let changed = countdown != server.countdown || expired != server.expired;
        server.countdown = countdown;
        server.expired = expired;
        changed
    }

    /// The poll's turn for the open tab: Found looks every 4 s, On a new server counts down and looks for the joined machine
    /// every 3 s. True when something drawn changed.
    pub(crate) fn keep_add_tabs(&mut self, tx: &mut Tx) -> bool {
        let form = &self.machine_page.add;
        if !form.open {
            return false;
        }
        match form.mode {
            AddMode::Found => self.keep_found_tab(tx),
            AddMode::Server => self.keep_server_tab(tx),
            AddMode::Link => false,
        }
    }

    fn keep_found_tab(&mut self, _tx: &mut Tx) -> bool {
        let (clock, mine) = (self.clock, self.machine_page.add.tab);
        let mut changed = false;
        let tab = &mut self.machine_page.add.waiting;
        for edit in tab.rest.values_mut() {
            if edit.text.chars().count() > CODE_MAX {
                let kept: String = edit.text.chars().take(CODE_MAX).collect();
                *edit = TextEdit::new(kept);
                edit.focused = true;
                changed = true;
            }
        }
        if tab.looking || tab.next_look.is_some_and(|at| clock < at) {
            return changed;
        }
        let Some(link) = self.link.clone() else {
            // No Fleet: nothing can be waiting.
            if self.machine_data.found.is_none() {
                self.machine_data.found = Some(Vec::new());
                return true;
            }
            return changed;
        };
        tab.looking = true;
        link.off_thread(load_found, move |s, tx, machines: Vec<FoundMachine>| {
            if s.machine_page.add.tab != mine {
                return;
            }
            s.machine_data.found = Some(machines);
            let tab = &mut s.machine_page.add.waiting;
            tab.looking = false;
            // The next look waits for this one to end (`setTimeout` after `await`).
            tab.next_look = Some(s.clock + LOOK_EVERY.as_secs_f64());
            tx.wake_after(LOOK_EVERY);
        });
        changed
    }

    fn keep_server_tab(&mut self, tx: &mut Tx) -> bool {
        let mut changed = self.keep_server_countdown();
        let (clock, mine) = (self.clock, self.machine_page.add.tab);
        let server = &mut self.machine_page.add.server;
        if server.copied_until.is_some_and(|until| clock >= until) {
            server.copied_until = None;
            changed = true;
        }
        if server.code.is_none() || server.expired {
            return changed;
        }
        // The countdown reads in seconds at the end.
        tx.wake_after(Duration::from_millis(500));
        if server.looking || server.next_look.is_some_and(|at| clock < at) {
            return changed;
        }
        let Some(link) = self.link.clone() else { return changed };
        server.looking = true;
        link.off_thread(machine_ids, move |s, tx, listed| {
            let form = &mut s.machine_page.add;
            if form.tab != mine || !form.open {
                return;
            }
            form.server.looking = false;
            let arrived = listed.unwrap_or_default().into_iter().find(|(id, _)| !form.server.known.contains(id));
            if let Some((_, name)) = arrived {
                s.machine_added(name);
                return;
            }
            form.server.next_look = Some(s.clock + JOIN_LOOK_EVERY);
            tx.wake_after(Duration::from_secs_f64(JOIN_LOOK_EVERY));
        });
        changed
    }
}
