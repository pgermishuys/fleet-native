//! The automations of one machine (`stores/automations.ts`): the list the sidebar and the page share, `is_loading` until the
//! first load ends, the error of a load that failed, and the routes that change it. One store per machine key, and every
//! request goes to the machine the store is for, never to the live one by mistake. Every request runs on a thread and hands
//! its answer back on the UI thread. Fleet pushes no events for automations, so the list is read when a view that shows it
//! opens (`automations_refresh`) and after every change.
use super::model::{Automation, AutomationDraft, AutomationRequest, AutomationRun};
use crate::State;
use crate::conv::image_sniff::encode_component;
use crate::fleet::http;
use rune::Tx;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

pub const BASE: &str = "/api/automations";

/// `itemPath`: `/api/automations/{id}` followed by `suffix`.
pub fn item_path(id: &str, suffix: &str) -> String {
    format!("{BASE}/{}{suffix}", encode_component(id))
}

/// What waits for a load that is in flight, on the UI thread.
type Waiter = Box<dyn FnOnce(&mut State, &mut Tx) + Send>;

/// What a request hands back on the UI thread: the answer or the words of why not.
pub type Done<T> = Box<dyn FnOnce(&mut State, &mut Tx, Result<T, String>) + Send>;

/// One machine's automations and what the load is doing.
pub struct MachineAutomations {
    pub list: Vec<Automation>,
    /// True until the first load finishes; later reloads keep showing the list they replace (`isLoading`).
    pub is_loading: bool,
    /// Why the last load failed; cleared when the next one starts.
    pub error: Option<String>,
    /// A load is running (`pending`): a refresh asked meanwhile waits for it instead of sending another.
    in_flight: bool,
    /// Behind a lock only so the empty store every machine starts from can be shared (`Sync`).
    waiting: Mutex<Vec<Waiter>>,
}

impl Default for MachineAutomations {
    fn default() -> Self {
        MachineAutomations { list: Vec::new(), is_loading: true, error: None, in_flight: false, waiting: Mutex::new(Vec::new()) }
    }
}

impl MachineAutomations {
    pub fn get(&self, id: &str) -> Option<&Automation> {
        self.list.iter().find(|a| a.id == id)
    }
}

/// Every machine's automations, by the machine's key (`defineMachineStore`).
#[derive(Default)]
pub struct Automations {
    by_machine: HashMap<String, MachineAutomations>,
}

impl Automations {
    /// A machine's store; none until something asked.
    pub fn of(&self, machine: &str) -> Option<&MachineAutomations> {
        self.by_machine.get(machine)
    }

    pub fn of_mut(&mut self, machine: &str) -> &mut MachineAutomations {
        self.by_machine.entry(machine.to_string()).or_default()
    }
}

/// `readError`: the server's `error` words, else `HTTP <status>`.
pub fn read_error(status: u16, body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {status}"))
}

/// `send`: a good answer's body, or the words of the refusal. A request that never reached the machine says why.
fn send(method: &str, base: &str, path: &str, body: Option<&Value>) -> Result<String, String> {
    let (status, text) = http::text_with(method, base, path, body)?;
    if (200..300).contains(&status) { Ok(text) } else { Err(read_error(status, &text)) }
}

fn json_of(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|_| "The server's answer couldn't be read.".to_string())
}

const NO_LINK: &str = "Couldn't reach that machine.";

/// One request to a machine's routes: what to send and how to read a good answer.
struct Call<T> {
    method: &'static str,
    path: String,
    body: Option<Value>,
    /// The list is read again before the answer is handed back (every change; not a read of runs or a draft).
    reload: bool,
    read: fn(&str) -> Result<T, String>,
}

impl<T> Call<T> {
    fn change(method: &'static str, path: String, body: Option<Value>, read: fn(&str) -> Result<T, String>) -> Call<T> {
        Call { method, path, body, reload: true, read }
    }

    fn read(path: String, read: fn(&str) -> Result<T, String>) -> Call<T> {
        Call { method: "GET", path, body: None, reload: false, read }
    }
}

fn nothing(_: &str) -> Result<(), String> {
    Ok(())
}

impl State {
    /// A machine's automations; the initial state (loading, empty) until something asked.
    pub fn automations_of(&self, machine: &str) -> &MachineAutomations {
        static NONE: std::sync::OnceLock<MachineAutomations> = std::sync::OnceLock::new();
        self.automations.of(machine).unwrap_or_else(|| NONE.get_or_init(MachineAutomations::default))
    }

    /// `refresh`: reads the list again; `then` runs once the list is in (or the read failed, which `error` says). A call made
    /// while a load is in flight shares it. With no server (scenes, tests) there is nothing to read and `then` runs at once.
    pub fn automations_refresh(&mut self, machine: &str, tx: &mut Tx, then: Option<Waiter>) {
        let link = self.link_for_machine(machine);
        let store = self.automations.of_mut(machine);
        let Some(link) = link else {
            if let Some(then) = then {
                then(self, tx);
            }
            return;
        };
        store.waiting.get_mut().unwrap_or_else(|e| e.into_inner()).extend(then);
        if store.in_flight {
            return;
        }
        store.in_flight = true;
        store.error = None;
        let key = machine.to_string();
        link.off_thread(
            |base| Ok::<_, String>(Automation::list_from_json(&json_of(&send("GET", base, BASE, None)?)?)),
            move |s, tx, result| {
                let store = s.automations.of_mut(&key);
                match result {
                    Ok(list) => store.list = list,
                    Err(e) => store.error = Some(e),
                }
                store.is_loading = false;
                store.in_flight = false;
                for waiter in std::mem::take(store.waiting.get_mut().unwrap_or_else(|e| e.into_inner())) {
                    waiter(s, tx);
                }
            },
        );
    }

    /// Sends a call to `machine`; on a good answer the list is reloaded (for a change) before `done` hears the body. A refusal
    /// or a machine that didn't answer goes to `done` as its words, and the list is not touched.
    fn automations_call<T: Send + 'static>(&mut self, machine: &str, call: Call<T>, tx: &mut Tx, done: Done<T>) {
        let Some(link) = self.link_for_machine(machine) else { return done(self, tx, Err(NO_LINK.into())) };
        let key = machine.to_string();
        let Call { method, path, body, reload, read } = call;
        link.off_thread(
            move |base| send(method, base, &path, body.as_ref()).and_then(|text| read(&text)),
            move |s, tx, result| match result {
                Ok(value) if reload => s.automations_refresh(&key, tx, Some(Box::new(move |s, tx| done(s, tx, Ok(value))))),
                result => done(s, tx, result),
            },
        );
    }

    /// `createAutomation`: the automation the server made, once the list shows it.
    pub fn automation_create(&mut self, machine: &str, request: &AutomationRequest, tx: &mut Tx, done: Done<Automation>) {
        let call = Call::change("POST", BASE.into(), Some(request.to_json()), |t| Ok(Automation::from_json(&json_of(t)?)));
        self.automations_call(machine, call, tx, done);
    }

    /// `updateAutomation`.
    pub fn automation_update(&mut self, machine: &str, id: &str, request: &AutomationRequest, tx: &mut Tx, done: Done<()>) {
        self.automations_call(machine, Call::change("PUT", item_path(id, ""), Some(request.to_json()), nothing), tx, done);
    }

    /// `deleteAutomation`.
    pub fn automation_delete(&mut self, machine: &str, id: &str, tx: &mut Tx, done: Done<()>) {
        self.automations_call(machine, Call::change("DELETE", item_path(id, ""), None, nothing), tx, done);
    }

    /// `enableAutomation`: the server answers with no body.
    pub fn automation_enable(&mut self, machine: &str, id: &str, tx: &mut Tx, done: Done<()>) {
        self.automations_call(machine, Call::change("POST", item_path(id, "/enable"), None, nothing), tx, done);
    }

    /// `disableAutomation`.
    pub fn automation_disable(&mut self, machine: &str, id: &str, tx: &mut Tx, done: Done<()>) {
        self.automations_call(machine, Call::change("POST", item_path(id, "/disable"), None, nothing), tx, done);
    }

    /// `runAutomation`: starts a run now; the answer is the run, still starting, and the list reloads to show it.
    pub fn automation_run(&mut self, machine: &str, id: &str, tx: &mut Tx, done: Done<AutomationRun>) {
        let call = Call::change("POST", item_path(id, "/run"), None, |t| Ok(AutomationRun::from_json(&json_of(t)?)));
        self.automations_call(machine, call, tx, done);
    }

    /// `fetchRuns`: an automation's runs, newest first. Nothing reloads.
    pub fn automation_runs(&mut self, machine: &str, id: &str, tx: &mut Tx, done: Done<Vec<AutomationRun>>) {
        let call = Call::read(item_path(id, "/runs"), |t| Ok(AutomationRun::list_from_json(&json_of(t)?)));
        self.automations_call(machine, call, tx, done);
    }

    /// `fetchDraftFromSession`: a new automation's starting point from a session.
    pub fn automation_draft_from_session(&mut self, machine: &str, session_id: &str, tx: &mut Tx, done: Done<AutomationDraft>) {
        let path = format!("{BASE}/draft-from-session/{}", encode_component(session_id));
        self.automations_call(machine, Call::read(path, |t| Ok(AutomationDraft::from_json(&json_of(t)?))), tx, done);
    }

    /// `fetchEventCatalog`: the event types an automation can wait for.
    pub fn automation_event_catalog(&mut self, machine: &str, tx: &mut Tx, done: Done<Vec<String>>) {
        let call = Call::read(format!("{BASE}/event-catalog"), |t| {
            Ok(json_of(t)?.as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect())
        });
        self.automations_call(machine, call, tx, done);
    }
}
