//! Settings → Harnesses → Providers (`HarnessSignInPanel.vue`, `stores/harness-sign-in.ts`, `lib/harness-sign-in.ts`): which of a
//! harness's providers are signed in and which sign-in each uses, with Use this one and Sign out. Listing them asks the harness,
//! which starts it if it isn't running, so a harness that is off waits for a click. The flow a provider's Sign in opens is piece
//! L's; this holds the state that says which provider's is open and what ends it ([`State::sign_in_done`]).
pub mod flow;
mod flow_actions;
#[cfg(test)]
mod tests;

use crate::State;
use crate::fleet::http;
use crate::fleet::new_session::harness_choice::HarnessEntry;
use rune::TextEdit;
use serde_json::Value;
use std::collections::HashMap;

/// One way a provider is signed in (`HarnessSignInConnection`): a credential Fleet keeps, or a variable in its environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    /// `credential` or `env`.
    pub kind: String,
    pub id: String,
    pub label: String,
    pub active: bool,
}

impl Connection {
    /// Only a credential can be switched to or signed out of from here; the environment is advice.
    pub fn is_credential(&self) -> bool {
        self.kind == "credential"
    }
}

/// A provider a harness can sign in to (`HarnessSignInProvider`). `methods` stay as the server sends them: the flow reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub methods: Vec<Value>,
    pub connections: Vec<Connection>,
}

impl Provider {
    /// `signInSummary`: "Signed in", or "From NAME" for one that uses a variable of Fleet's environment; none when no sign-in is
    /// in use.
    pub fn summary(&self) -> Option<String> {
        let active = self.connections.iter().find(|c| c.active)?;
        Some(if active.kind == "env" { format!("From {}", active.id) } else { "Signed in".into() })
    }
}

/// What `GET /api/harnesses/{type}/sign-in` answers (`HarnessSignIns`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignIns {
    pub providers: Vec<Provider>,
    pub note: Option<String>,
}

impl SignIns {
    pub fn parse(body: &str) -> Option<SignIns> {
        let v: Value = serde_json::from_str(body).ok()?;
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let providers = v
            .get("providers")?
            .as_array()?
            .iter()
            .filter_map(|p| {
                let connections = p
                    .get("connections")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|c| {
                        Some(Connection {
                            kind: text(c, "kind")?,
                            id: text(c, "id")?,
                            label: text(c, "label").unwrap_or_default(),
                            active: c.get("active").and_then(Value::as_bool) == Some(true),
                        })
                    })
                    .collect();
                Some(Provider {
                    id: text(p, "id")?,
                    name: text(p, "name").unwrap_or_default(),
                    methods: p.get("methods").and_then(Value::as_array).cloned().unwrap_or_default(),
                    connections,
                })
            })
            .collect();
        Some(SignIns { providers, note: text(&v, "note").filter(|n| !n.is_empty()) })
    }

    /// The providers with a sign-in, in the order the harness lists them.
    pub fn signed_in(&self) -> Vec<&Provider> {
        self.providers.iter().filter(|p| !p.connections.is_empty()).collect()
    }

    /// A harness with one provider (GitHub Copilot) has nothing to search: it's signed in, or its sign-in is offered below.
    pub fn single(&self) -> bool {
        self.providers.len() == 1
    }

    /// The providers to sign in to: none signed in, and whose name or id holds the words typed (case aside).
    pub fn others(&self, query: &str) -> Vec<&Provider> {
        let words = query.trim().to_lowercase();
        self.providers
            .iter()
            .filter(|p| {
                p.connections.is_empty()
                    && (words.is_empty() || p.name.to_lowercase().contains(&words) || p.id.to_lowercase().contains(&words))
            })
            .collect()
    }
}

/// What the server's refusal says (`errorMessage`): its `error`, else its `detail`, else the status.
pub fn error_words(status: u16, body: &str) -> String {
    let said = serde_json::from_str::<Value>(body).ok().and_then(|b| {
        ["error", "detail"].iter().find_map(|k| b.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string))
    });
    said.unwrap_or_else(|| format!("Fleet couldn't reach the harness (HTTP {status})."))
}

/// What the panel keeps for the harness shown, and the lists it has read, by machine and harness.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panel {
    /// The machine and harness the fields below belong to. Another tab is its own panel in Fleet, so they start again.
    key: (String, String),
    /// Asked at least once (`requested`): before that, and with nothing listed, the panel offers "Show providers".
    pub requested: bool,
    pub loading: bool,
    /// Whether the harness was on when last looked at: the list is asked for when it turns on (`autoLoad`).
    auto: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub query: TextEdit,
    /// The provider whose sign-in flow is open (`openProvider`).
    pub open: Option<String>,
    /// The sign-in that was asked "Sign out?" once.
    pub confirming: Option<String>,
    /// The sign-in a request is changing; all buttons wait meanwhile.
    pub working: Option<String>,
    /// The flow of the provider that is open (`HarnessSignInFlow`), and a box to hand `on_edit` when there is none.
    pub flow: Option<flow::Flow>,
    pub spare: TextEdit,
    lists: HashMap<(String, String), SignIns>,
}

impl Eq for Panel {}

impl Panel {
    fn mine(&mut self, key: &(String, String)) {
        if self.key != *key {
            *self = Panel { key: key.clone(), lists: std::mem::take(&mut self.lists), ..Panel::default() };
        }
    }

    /// A scene's panel: the list as if the machine had answered (none for a panel not yet asked), and the harness taken as on, so
    /// the panel doesn't ask a machine there isn't.
    pub fn scene(&mut self, machine: &str, harness: &str, list: Option<SignIns>, on: bool) {
        let key = (machine.to_string(), harness.to_string());
        self.mine(&key);
        self.auto = on;
        if let Some(list) = list {
            self.lists.insert(key, list);
        }
    }
}

impl HarnessEntry {
    /// `offersSignIn`: the harness signs in to providers here, and is installed or only waits for a sign-in.
    pub fn offers_sign_in(&self) -> bool {
        self.card.supports_provider_sign_in && (self.available || self.state == "sign-in-required")
    }
}

/// "Signed in to X. New sessions can use its models." (`done`).
pub fn done_words(message: &str) -> String {
    format!("{message} New sessions can use its models.")
}

type Then = Box<dyn FnOnce(&mut State, Result<(), String>) + Send>;

impl State {
    fn sign_in_key(&self, harness: &str) -> (String, String) {
        (self.sign_in_machine(), harness.to_string())
    }

    /// The machine the panel asks: the setup dialog's rows are the live machine's, Settings' tab is the one it shows.
    pub fn sign_in_machine(&self) -> String {
        if self.harness_setup.is_open { crate::fleet::settings::HOME_MACHINE_KEY.to_string() } else { self.harness_machine() }
    }

    /// Asked each frame the setup dialog is open: a Fleet-installed harness that needs a sign-in lists its providers when its row
    /// first shows (`HarnessSignInPanel` with `auto-load`). True when it asked.
    pub fn keep_setup_sign_in(&mut self) -> bool {
        if !self.harness_setup.is_open || self.harness_setup.cloud {
            return false;
        }
        let list = self.setup_harnesses();
        let Some(h) = list.all.iter().find(|h| h.fleet_installs() && h.setup_state() == "sign-in-required") else { return false };
        let key = self.sign_in_key(&h.kind);
        let panel = &mut self.settings.harnesses.sign_in;
        if panel.key == key && (panel.requested || panel.loading) {
            return false;
        }
        panel.mine(&key);
        self.load_sign_ins(&h.kind, &h.display_name)
    }

    /// The providers of `harness` on the machine shown; none until read.
    pub fn sign_ins(&self, harness: &str) -> Option<&SignIns> {
        self.settings.harnesses.sign_in.lists.get(&self.sign_in_key(harness))
    }

    /// The panel as `harness`'s tab shows it: its own fields, or fresh ones.
    pub fn sign_in_panel(&self, harness: &str) -> Option<&Panel> {
        let panel = &self.settings.harnesses.sign_in;
        (panel.key == self.sign_in_key(harness)).then_some(panel)
    }

    /// Asked each frame the panel is on screen: a harness that is on lists its providers when the panel first shows, and again
    /// when it turns on later (`watch(autoLoad, …, { immediate: true })`). True when it asked.
    pub fn keep_sign_in_panel(&mut self) -> bool {
        let Some(h) = self.harness_tab().filter(|h| h.offers_sign_in()) else {
            self.end_flow();
            return false;
        };
        let key = self.sign_in_key(&h.kind);
        if self.settings.harnesses.sign_in.key != key {
            // Another tab (or machine) is its own panel: what the old one had open ends.
            self.end_flow();
        }
        let auto = self.harness_enabled(&h);
        let panel = &mut self.settings.harnesses.sign_in;
        panel.mine(&key);
        let turned_on = auto && !panel.auto;
        panel.auto = auto;
        turned_on && !panel.loading && self.load_sign_ins(&h.kind, &h.display_name)
    }

    /// `load`: asks the harness which providers it can sign in to. A failure says so, with "Try again" while nothing is listed.
    pub fn load_sign_ins(&mut self, harness: &str, name: &str) -> bool {
        let key = self.sign_in_key(harness);
        let panel = &mut self.settings.harnesses.sign_in;
        panel.mine(&key);
        panel.requested = true;
        panel.error = None;
        let failed = format!("Couldn't list {name}'s providers.");
        self.fetch_sign_ins(
            harness,
            failed,
            Box::new(|s, r| {
                if let Err(why) = r {
                    s.settings.harnesses.sign_in.error = Some(why);
                }
            }),
        )
    }

    /// Reads the list; `then` hears how it went once it is stored. `loading` is on meanwhile.
    fn fetch_sign_ins(&mut self, harness: &str, failed: String, then: Then) -> bool {
        let key = self.sign_in_key(harness);
        let machine = key.0.clone();
        let kind = harness.to_string();
        self.settings.harnesses.sign_in.loading = true;
        let sent = self.feed_call(
            &machine,
            move |base| http::sign_in_list(base, &kind),
            move |s, _, answer| {
                let result = match answer {
                    Ok((status, body)) if (200..300).contains(&status) => SignIns::parse(&body).ok_or(failed),
                    Ok((status, body)) => Err(error_words(status, &body)),
                    Err(_) => Err(failed),
                };
                let panel = &mut s.settings.harnesses.sign_in;
                let result = result.map(|list| {
                    panel.lists.insert(key.clone(), list);
                });
                if panel.key == key {
                    panel.loading = false;
                    then(s, result);
                }
            },
        );
        if !sent {
            self.settings.harnesses.sign_in.loading = false;
        }
        sent
    }

    /// `changed`: after a sign-in the models on offer changed, and a harness that waited for one is ready. Forgets the catalogs of
    /// `harness`, reads the harnesses again and the providers; `then` hears about the last.
    fn sign_in_changed(&mut self, harness: &str, failed: String, then: Then) {
        let machine = self.sign_in_machine();
        if let Some(feed) = self.feeds.catalogs.get_mut(&machine) {
            feed.forget_harness(harness);
        }
        self.read_harnesses(&machine, true);
        self.fetch_sign_ins(harness, failed, then);
    }

    /// A sign-in flow ended well (`done`): it closes, the search clears, and the providers are read again.
    pub fn sign_in_done(&mut self, harness: &str, name: &str, message: &str) {
        let key = self.sign_in_key(harness);
        let panel = &mut self.settings.harnesses.sign_in;
        panel.mine(&key);
        panel.open = None;
        panel.flow = None;
        panel.query.replace_text(String::new());
        panel.notice = Some(done_words(message));
        self.sign_in_changed(
            harness,
            format!("Couldn't list {name}'s providers again."),
            Box::new(|s, r| {
                if let Err(why) = r {
                    s.settings.harnesses.sign_in.error = Some(why);
                }
            }),
        );
    }

    /// `toggle`: a provider's Sign in opens its flow, or closes it.
    pub fn toggle_sign_in(&mut self, harness: &str, provider: &str) {
        let key = self.sign_in_key(harness);
        self.settings.harnesses.sign_in.mine(&key);
        // The flow under the old provider ends, which cancels a sign-in it was following.
        self.end_flow();
        let panel = &mut self.settings.harnesses.sign_in;
        let opening = panel.open.as_deref() != Some(provider);
        panel.open = opening.then(|| provider.to_string());
        panel.notice = None;
        if opening && let Some(p) = self.sign_ins(harness).and_then(|l| l.providers.iter().find(|p| p.id == provider)).cloned() {
            self.open_flow(harness, &p);
        }
    }

    /// `use`: the provider uses another of its sign-ins.
    pub fn use_sign_in(&mut self, harness: &str, provider: &str, connection: &str, label: &str) {
        let words = format!("{provider} now uses {label}.");
        self.change_sign_in(harness, connection, "Couldn't switch sign-ins.", true, words);
    }

    /// `signOut`: the first press asks "Sign out?", the second signs out.
    pub fn sign_out(&mut self, harness: &str, provider: &str, connection: &str, label: &str) {
        let key = self.sign_in_key(harness);
        let panel = &mut self.settings.harnesses.sign_in;
        panel.mine(&key);
        if panel.confirming.as_deref() != Some(connection) {
            panel.confirming = Some(connection.to_string());
            return;
        }
        panel.confirming = None;
        let words = format!("Signed out of {label} on {provider}.");
        self.change_sign_in(harness, connection, "Couldn't sign out.", false, words);
    }

    fn change_sign_in(&mut self, harness: &str, connection: &str, failed: &str, switch: bool, notice: String) {
        let key = self.sign_in_key(harness);
        let machine = key.0.clone();
        let panel = &mut self.settings.harnesses.sign_in;
        panel.mine(&key);
        panel.working = Some(connection.to_string());
        panel.error = None;
        let (kind, id, shown) = (harness.to_string(), connection.to_string(), failed.to_string());
        let name = self.harness_list().all.iter().find(|h| h.kind == harness).map(|h| h.display_name.clone()).unwrap_or_default();
        let sent = self.feed_call(
            &machine,
            {
                let (kind, id) = (kind.clone(), id.clone());
                move |base| if switch { http::sign_in_use(base, &kind, &id) } else { http::sign_in_remove(base, &kind, &id) }
            },
            move |s, _, answer| {
                let panel = &mut s.settings.harnesses.sign_in;
                if panel.key != key {
                    return;
                }
                match answer {
                    Ok((status, _)) if (200..300).contains(&status) => {
                        s.sign_in_changed(
                            &kind,
                            format!("Couldn't list {name}'s providers again."),
                            Box::new(move |s, r| {
                                let panel = &mut s.settings.harnesses.sign_in;
                                panel.working = None;
                                match r {
                                    Ok(()) => panel.notice = Some(notice),
                                    Err(why) => panel.error = Some(why),
                                }
                            }),
                        );
                    }
                    Ok((status, body)) => {
                        panel.working = None;
                        panel.error = Some(error_words(status, &body));
                    }
                    Err(_) => {
                        panel.working = None;
                        panel.error = Some(shown);
                    }
                }
            },
        );
        if !sent {
            let panel = &mut self.settings.harnesses.sign_in;
            panel.working = None;
            panel.error = Some(failed.to_string());
        }
    }
}
