//! What the cards of Settings → Machines do (`MachinesSection.vue`, `stores/machines.ts`): rename a machine, give it a new token,
//! set whether agents may send it work, forget it, read how other devices reach this one, and replace this one's token. Every
//! write is made on a thread; the answer reaches the state through `Post`. With no server (scenes, tests) a change is made on the
//! invented list, so nothing needs a Fleet.
use super::auth;
use super::http;
use super::machines::{AgentWork, Info, Machines, read_error};
use super::settings::HOME_MACHINE_KEY;
use crate::ui::confirm::Confirm;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;

/// The keys of the page's fields, for focus and for tests.
pub const RENAME_FIELD: &str = "machine-rename-input";
pub const TOKEN_FIELD: &str = "machine-token-input";
const COPIED_SECS: f64 = 1.5;

/// One address other devices can use to reach this machine (`MachineAccess.addresses`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessAddress {
    pub url: String,
    /// "tailnet", "lan" or "hostname", as home says it.
    pub kind: String,
}

/// What `GET /api/machine/access` says: how other devices reach this machine (`MachineAccess`). The token is a secret: it is
/// never printed, and a `Debug` of this leaves it out.
#[derive(Clone, PartialEq, Eq)]
pub struct Access {
    pub token: String,
    /// "saved", "environment" or "ephemeral".
    pub token_source: String,
    pub remote_reachable: bool,
    pub requires_token: bool,
    pub addresses: Vec<AccessAddress>,
}

impl std::fmt::Debug for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Access").field("token_source", &self.token_source).field("addresses", &self.addresses).finish_non_exhaustive()
    }
}

impl Access {
    pub fn from_json(v: &Value) -> Option<Access> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Some(Access {
            token: text("token")?,
            token_source: text("tokenSource").unwrap_or_default(),
            remote_reachable: v.get("remoteReachable").and_then(Value::as_bool).unwrap_or(false),
            requires_token: v.get("requiresToken").and_then(Value::as_bool).unwrap_or(false),
            addresses: v
                .get("addresses")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|a| {
                            Some(AccessAddress { url: a.get("url")?.as_str()?.to_string(), kind: a.get("kind")?.as_str()?.to_string() })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// The token as drawn while hidden: 24 dots, whatever its length.
    pub fn hidden_token() -> String {
        "•".repeat(24)
    }
}

/// How the read of this machine's access went (`loadAccess`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AccessState {
    #[default]
    Loading,
    Loaded(Access),
    /// A 403: this app signed in as a paired device, which may not see the token.
    Device,
    /// A 404: a Fleet too old to hand out access tokens.
    Unavailable,
    Failed(String),
}

/// Everything the page keeps while it is open.
#[derive(Default)]
pub struct Page {
    /// The machine whose name is being edited, and the field's text.
    pub renaming: Option<String>,
    pub rename: TextEdit,
    /// The machine that is being given a new token, and the field's text.
    pub token_for: Option<String>,
    pub token: TextEdit,
    /// A write for this machine is on its way: its buttons wait.
    pub busy: Option<String>,
    /// What a write said when it failed, under the card it was for.
    pub errors: HashMap<String, String>,
    /// Which cards have Details open.
    pub details: Vec<String>,
    pub access: AccessState,
    /// The Add a machine form (piece K).
    pub add: super::pairing::AddForm,
    pub show_token: bool,
    pub replacing: bool,
    /// Which copy button shows its tick, and until when.
    pub copied: Option<(String, f64)>,
    /// The page was opened once, so the access is read once per visit.
    pub opened: bool,
    /// The address lists' own state (`MachineAddressList.vue`).
    pub addresses: super::machine_addresses::Lists,
}

/// `PUT`, `POST` or `DELETE` on `base`: the answer's JSON, or the server's own words (`readError`) when it refuses.
pub fn send(method: &str, base: &str, path: &str, body: Option<&Value>, fallback: &str, unreachable: &str) -> Result<Value, String> {
    let (status, text) = http::text_with(method, base, path, body).map_err(|_| unreachable.to_string())?;
    if (200..300).contains(&status) {
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    } else {
        Err(read_error(&text, fallback.to_string()))
    }
}

pub fn machine_path(id: &str) -> String {
    format!("/api/machines/{}", http::escape(id))
}

/// `GET /api/machine/access`: None for a Fleet that has none (404), `Device` for a refusal (403).
pub fn read_access(base: &str) -> AccessState {
    match http::text_with("GET", base, "/api/machine/access", None) {
        Err(_) => AccessState::Failed("Couldn't read this machine's access.".into()),
        Ok((404, _)) => AccessState::Unavailable,
        Ok((403, _)) => AccessState::Device,
        Ok((s, text)) if (200..300).contains(&s) => serde_json::from_str::<Value>(&text)
            .ok()
            .as_ref()
            .and_then(Access::from_json)
            .map_or(AccessState::Unavailable, AccessState::Loaded),
        Ok((_, text)) => AccessState::Failed(read_error(&text, "Couldn't read this machine's access.".into())),
    }
}

impl Machines {
    /// A machine was renamed (by itself): what the cards and the list say.
    fn name_is(&mut self, key: &str, info: &Info) {
        if key == HOME_MACHINE_KEY {
            self.home = Some(info.clone());
            return;
        }
        if let Some(m) = self.list.iter_mut().find(|m| m.id == key) {
            m.name = info.name.clone();
            m.os = Some(info.os.clone()).filter(|o| !o.is_empty()).or(m.os.take());
        }
        self.identities.insert(key.to_string(), info.clone());
    }
}

impl State {
    /// A machine's key as the page uses it, if it is one of the cards.
    fn card_name(&self, key: &str) -> Option<String> {
        self.machine_cards().into_iter().find(|c| c.key == key).map(|c| c.name)
    }

    /// Settings → Machines was shown (by the rail, the list's Add a machine row or the palette): this machine's access is read once
    /// per visit (`onMounted` → `loadAccess`). Leaving Settings lets the next visit read again. Run from `poll`, so every way in counts.
    fn keep_machines_page_open(&mut self) -> bool {
        let here = self.rail == crate::Rail::Settings;
        if !here {
            self.machine_page.opened = false;
            return false;
        }
        if std::mem::replace(&mut self.machine_page.opened, true) {
            return false;
        }
        self.machine_page.access = AccessState::Loading;
        if let Some(link) = self.link.clone() {
            link.off_thread(read_access, |s, _, access| s.machine_page.access = access);
        }
        true
    }

    // ---- rename -----------------------------------------------------------------------------------------------------------

    /// The Rename button (`startRename`): the name in a field, all of it selected.
    pub fn start_card_rename(&mut self, key: &str, tx: &mut Tx) {
        let Some(name) = self.card_name(key) else { return };
        self.machine_page.errors.remove(key);
        self.machine_page.rename = TextEdit::new(name.clone());
        self.machine_page.rename.set_selection(0, name.len());
        self.machine_page.renaming = Some(key.to_string());
        tx.focus(RENAME_FIELD, true);
    }

    pub fn cancel_card_rename(&mut self) {
        self.machine_page.renaming = None;
    }

    /// Save: the machine renames itself (`PUT /api/machine` on it), then home keeps the name (`PUT /api/machines/{id}`).
    pub fn save_card_rename(&mut self, tx: &mut Tx) {
        let Some(key) = self.machine_page.renaming.clone().filter(|k| self.machine_page.busy.as_deref() != Some(k)) else { return };
        // Fleet's field stops at 64 characters.
        let name: String = self.machine_page.rename.text.trim().chars().take(64).collect();
        let unreachable = format!("Can't reach {}.", self.card_name(&key).unwrap_or_default());
        self.machine_page.busy = Some(key.clone());
        let Some(link) = self.link.clone() else {
            // The invented list: the name changes where it is shown.
            let known = if key == HOME_MACHINE_KEY { self.machines.home.clone() } else { self.machines.identities.get(&key).cloned() };
            let info = Info { name: name.clone(), ..known.unwrap_or_else(blank_info) };
            self.card_renamed(&key, Ok(info), tx);
            return;
        };
        let base = if key == HOME_MACHINE_KEY { None } else { self.machines.machine(&key).map(|m| m.base().to_string()) };
        let id = key.clone();
        link.off_thread(
            move |home| {
                let target = base.as_deref().unwrap_or(home);
                let info =
                    send("PUT", target, "/api/machine", Some(&json!({ "name": name })), "Couldn't rename the machine.", &unreachable)?;
                let info = Info::from_json(&info).ok_or_else(|| "Couldn't rename the machine.".to_string())?;
                if base.is_some() {
                    // Home keeps the name it lists the machine by; a failure here shows at the next read, not as an error.
                    let _ = send("PUT", home, &machine_path(&id), Some(&json!({ "name": info.name })), "", "");
                }
                Ok(info)
            },
            move |s, tx, result| s.card_renamed(&key, result, tx),
        );
    }

    fn card_renamed(&mut self, key: &str, result: Result<Info, String>, _tx: &mut Tx) {
        let page = &mut self.machine_page;
        if page.busy.as_deref() == Some(key) {
            page.busy = None;
        }
        match result {
            Ok(info) => {
                page.renaming = None;
                self.machines.name_is(key, &info);
                if let Some(link) = &self.link {
                    link.refresh_machines();
                }
            }
            Err(why) => {
                page.errors.insert(key.to_string(), why);
            }
        }
    }

    // ---- token ------------------------------------------------------------------------------------------------------------

    pub fn start_machine_token(&mut self, key: &str, tx: &mut Tx) {
        if key == HOME_MACHINE_KEY || self.machines.machine(key).is_none() {
            return;
        }
        self.machine_page.errors.remove(key);
        self.machine_page.token = TextEdit::new(String::new());
        self.machine_page.token_for = Some(key.to_string());
        tx.focus(TOKEN_FIELD, true);
    }

    pub fn cancel_machine_token(&mut self) {
        self.machine_page.token_for = None;
    }

    /// Save: home checks the token against the machine first (`PUT /api/machines/{id}` `{token}`) and says so when it isn't taken.
    pub fn save_machine_token(&mut self) {
        let Some(key) = self.machine_page.token_for.clone().filter(|k| self.machine_page.busy.as_deref() != Some(k)) else { return };
        let token = self.machine_page.token.text.trim().to_string();
        if token.is_empty() {
            return;
        }
        self.machine_page.busy = Some(key.clone());
        let Some(link) = self.link.clone() else {
            self.machine_page.busy = None;
            self.machine_page.token_for = None;
            return;
        };
        link.off_thread(
            {
                let key = key.clone();
                move |home| {
                    send(
                        "PUT",
                        home,
                        &machine_path(&key),
                        Some(&json!({ "token": token })),
                        "Couldn't save the token.",
                        "Can't reach Fleet.",
                    )
                }
            },
            move |s, _, result| {
                if s.machine_page.busy.as_deref() == Some(key.as_str()) {
                    s.machine_page.busy = None;
                }
                match result {
                    Ok(_) => {
                        s.machine_page.token_for = None;
                        if let Some(link) = &s.link {
                            link.refresh_machines();
                        }
                    }
                    Err(why) => {
                        s.machine_page.errors.insert(key, why);
                    }
                }
            },
        );
    }

    // ---- agent work -------------------------------------------------------------------------------------------------------

    /// One of Ask / Always / Never (`setAgentWork`): nothing when it already is that.
    pub fn set_agent_work(&mut self, key: &str, value: AgentWork) {
        if self.machines.agent_work.get(key) == Some(&value) || self.machine_page.busy.as_deref() == Some(key) {
            return;
        }
        self.machine_page.errors.remove(key);
        let Some(link) = self.link.clone() else {
            self.machines.agent_work.insert(key.to_string(), value);
            return;
        };
        self.machine_page.busy = Some(key.to_string());
        let (id, word) = (key.to_string(), agent_work_word(value));
        link.off_thread(
            {
                let id = id.clone();
                move |home| {
                    send("PUT", home, &machine_path(&id), Some(&json!({ "agentWork": word })), "Couldn't change it.", "Can't reach Fleet.")
                }
            },
            move |s, _, result| {
                if s.machine_page.busy.as_deref() == Some(id.as_str()) {
                    s.machine_page.busy = None;
                }
                match result {
                    Ok(saved) => {
                        if let Some(work) = saved.get("agentWork").and_then(Value::as_str).and_then(parse_agent_work) {
                            s.machines.agent_work.insert(id, work);
                        }
                    }
                    Err(why) => {
                        s.machine_page.errors.insert(id, why);
                    }
                }
            },
        );
    }

    // ---- forget -----------------------------------------------------------------------------------------------------------

    /// The Forget button (`forget`): asks first. Its sessions stay on the machine; this app just stops listing them.
    pub fn ask_forget_machine(&mut self, key: &str, tx: &mut Tx) {
        let Some(name) = self.card_name(key).filter(|_| key != HOME_MACHINE_KEY) else { return };
        let confirm = Confirm {
            title: format!("Forget {name}?"),
            subject: name.clone(),
            body: format!("Its sessions stay on {name}; this device just stops listing them."),
            confirm_label: "Forget machine".into(),
            busy_label: "Forgetting…".into(),
            error_message: "Couldn't forget the machine.".into(),
            destructive: true,
            busy: false,
            error: None,
            call: crate::fleet::actions::Call::ForgetMachine { key: key.to_string() },
        };
        self.ask(confirm, tx);
    }

    /// Confirmed: the machine's group leaves the list at once, then home is told (`forgetMachine`).
    pub fn forget_machine(&mut self, key: &str) {
        if !self.machines.forget(key) {
            return;
        }
        self.store.forget_elsewhere(key);
        self.prune_links();
        self.machine_page.errors.remove(key);
        if self.machine_page.renaming.as_deref() == Some(key) {
            self.machine_page.renaming = None;
        }
        if self.machine_page.token_for.as_deref() == Some(key) {
            self.machine_page.token_for = None;
        }
        self.save_machines();
        let Some(link) = self.link.clone() else { return };
        let id = key.to_string();
        // If home can't forget it, the next read lists it again, which is the truth.
        link.off_thread(
            move |home| send("DELETE", home, &machine_path(&id), None, "", ""),
            |s, _, _| {
                if let Some(link) = &s.link {
                    link.refresh_machines();
                }
            },
        );
    }

    // ---- this machine -----------------------------------------------------------------------------------------------------

    /// Show token / Hide token.
    pub fn toggle_token(&mut self) {
        self.machine_page.show_token = !self.machine_page.show_token;
    }

    /// A copy button: the text on the clipboard, then a tick for 1.5 s.
    pub fn copy_on_page(&mut self, what: &str, text: String, tx: &mut Tx) {
        tx.copy(text);
        self.machine_page.copied = Some((what.to_string(), self.clock + COPIED_SECS));
        tx.wake_after(Duration::from_millis(100));
    }

    /// The poll's turn: the page's visit and the tick, which ends by itself. True when something drawn changed.
    pub fn keep_machine_page(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = self.keep_machines_page_open() | self.keep_add_form(tx);
        if self.machine_page.copied.as_ref().is_some_and(|(_, until)| now >= *until) {
            self.machine_page.copied = None;
            changed = true;
        }
        changed
    }

    /// The Replace token button: asks first, because every device with the old token loses access.
    pub fn ask_replace_token(&mut self, tx: &mut Tx) {
        if self.machine_page.replacing {
            return;
        }
        let confirm = Confirm {
            title: "Replace this machine's token?".into(),
            subject: self.card_name(HOME_MACHINE_KEY).unwrap_or_else(|| "This machine".into()),
            body: "Every device using the old one loses access until you give it the new one.".into(),
            confirm_label: "Replace token".into(),
            busy_label: "Replacing…".into(),
            error_message: "Couldn't replace the token.".into(),
            destructive: true,
            busy: false,
            error: None,
            call: crate::fleet::actions::Call::ReplaceToken,
        };
        self.ask(confirm, tx);
    }

    /// Confirmed (`replaceToken`): `POST /api/machine/access/token`; the new token shows. The app's own copy of home's token, when
    /// it was given one, moves to the new one so it isn't locked out by its own request.
    pub fn replace_home_token(&mut self) {
        self.machine_page.replacing = true;
        let Some(link) = self.link.clone() else {
            self.machine_page.replacing = false;
            return;
        };
        link.off_thread(
            |home| send("POST", home, "/api/machine/access/token", None, "Couldn't replace the token.", "Can't reach Fleet."),
            |s, _, result| {
                s.machine_page.replacing = false;
                match result.map(|v| Access::from_json(&v)) {
                    Ok(Some(access)) => {
                        if let Some(base) = s.link.as_ref().map(|l| l.base.clone())
                            && auth::token_for(&base).is_some()
                        {
                            auth::set_for(&base, Some(&access.token));
                        }
                        s.machine_page.show_token = true;
                        s.machine_page.access = AccessState::Loaded(access);
                    }
                    Ok(None) => s.machine_page.access = AccessState::Failed("Couldn't replace the token.".into()),
                    Err(why) => s.machine_page.access = AccessState::Failed(why),
                }
            },
        );
    }

    // ---- Details ----------------------------------------------------------------------------------------------------------

    pub fn toggle_machine_details(&mut self, key: &str) {
        let open = &mut self.machine_page.details;
        match open.iter().position(|k| k == key) {
            Some(at) => drop(open.remove(at)),
            None => open.push(key.to_string()),
        }
    }

    /// Try again now (`retryMachine`): the machine is read at once and its last diagnosis let go; the headline clears when it answers.
    pub fn retry_machine(&mut self, key: &str) {
        self.machines.diagnoses.remove(key);
        if let Some(job) = self.machines.begin_read(key) {
            super::machine_poll::spawn_read(job, self.link.as_ref().map_or_else(|| super::link::Post::new(|_| {}), |l| l.post()));
        }
    }
}

fn blank_info() -> Info {
    Info {
        id: String::new(),
        name: String::new(),
        host_name: String::new(),
        os: String::new(),
        version: String::new(),
        api_version: 0,
        auth_mode: String::new(),
        remote_reachable: false,
        requires_token: false,
        web_app: None,
        public_url: None,
        capabilities: None,
        update: None,
    }
}

pub fn agent_work_word(w: AgentWork) -> &'static str {
    match w {
        AgentWork::Ask => "ask",
        AgentWork::Always => "always",
        AgentWork::Never => "never",
    }
}

pub fn parse_agent_work(word: &str) -> Option<AgentWork> {
    match word {
        "ask" => Some(AgentWork::Ask),
        "always" => Some(AgentWork::Always),
        "never" => Some(AgentWork::Never),
        _ => None,
    }
}
