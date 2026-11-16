//! One machine, several addresses: which one this app talks to a machine on (`lib/machine-routes.ts`; the kinds and the
//! order are in `machine.rs`).
//!
//! A machine reports where it can be reached and home keeps the list in the order to try them. This app picks its own
//! address from it, because it may sit on another network than home: it starts with the one that worked last, and when
//! that doesn't answer it asks the others in order (`GET /api/machine/id`, no token), the next starting before the last
//! gives up. Only an address where the machine's own id answers is used, so a token never goes to a device that took over
//! an old address. `Machine::base` is what every call uses; a pick changes it for all of them at once, and the machine's
//! token goes with it (`auth`), and leaves the address it came from.
//!
//! A pick lasts for the run (Fleet keeps it in the browser's storage; this app has no such store, and the first read after
//! a start costs one probe). Everything here blocks: it runs on the threads that read.
use super::auth;
use super::http;
use super::machine::Machine;
use super::machines::Answer;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

pub const PROBE_TIMEOUT: Duration = Duration::from_millis(3000);
pub const PROBE_GAP: Duration = Duration::from_millis(750);
/// How often a connection on a lower address looks for a better one.
pub const RECHECK: Duration = Duration::from_secs(60);

/// What answered at an address: the machine, nothing, another Fleet, or something that can't say who it is (`ProbeOutcome`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    NoAnswer,
    OtherMachine,
    NoProbe,
}

/// Asks who answers at an address.
pub type Probe = Arc<dyn Fn(&str) -> Outcome + Send + Sync>;

static PICKS: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

/// The address picked for machine `id`, if any. `Machine::base` takes it only while it is still one of the machine's.
pub fn picked(id: &str) -> Option<String> {
    PICKS.read().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(id).cloned())
}

pub fn has_choices(m: &Machine) -> bool {
    m.addresses.len() > 1
}

/// Whether the address in use is the machine's first, so there is nothing better to look for.
pub fn is_best(m: &Machine) -> bool {
    m.addresses.len() < 2 || m.addresses[0].url == m.base()
}

/// Makes `url` the address used for the machine, and moves its token there from the address it leaves (`useAddress`).
pub fn use_address(m: &Machine, url: &str) {
    let old = m.base().to_string();
    if old == url {
        return;
    }
    let token = auth::token_for(&old);
    auth::set_for(&old, None);
    auth::set_for(url, token.as_ref().map(|t| t.expose()));
    PICKS.write().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(m.id.clone(), url.to_string());
}

/// Asks who answers at `url`, without a token (`probeAddress`).
pub fn probe_address(url: &str, machine_id: &str, timeout: Duration) -> Outcome {
    let Ok((status, kind, body)) = http::get_plain(url, "/api/machine/id", timeout) else { return Outcome::NoAnswer };
    if !(200..300).contains(&status) || !kind.contains("application/json") {
        return Outcome::NoProbe;
    }
    match serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|b| b.get("id").and_then(|i| i.as_str()).map(str::to_string)) {
        Some(id) if id == machine_id => Outcome::Ok,
        Some(_) => Outcome::OtherMachine,
        None => Outcome::NoProbe,
    }
}

fn real_probe(m: &Machine) -> Probe {
    let id = m.id.clone();
    Arc::new(move |url| probe_address(url, &id, PROBE_TIMEOUT))
}

/// Probes `urls` in order, each starting `gap` after the one before or as soon as every one before it has failed, and
/// gives the first in the order where the machine answered: a later one that answers sooner waits for those above it. Only
/// `Ok` counts (`raceAddresses`). The outcomes are those known when it decided.
pub fn race(urls: &[String], probe: &Probe, gap: Duration) -> (Option<String>, HashMap<String, Outcome>) {
    let n = urls.len();
    let mut results: Vec<Option<Outcome>> = vec![None; n];
    let report = |results: &[Option<Outcome>]| -> HashMap<String, Outcome> {
        urls.iter().zip(results).filter_map(|(u, o)| o.map(|o| (u.clone(), o))).collect()
    };
    if n == 0 {
        return (None, HashMap::new());
    }
    let (tx, rx) = channel::<(usize, Outcome)>();
    let mut started = 0;
    let mut next_at = Instant::now();
    let start = |started: &mut usize, next_at: &mut Instant| {
        let at = *started;
        *started += 1;
        *next_at = Instant::now() + gap;
        let (tx, probe, url) = (tx.clone(), probe.clone(), urls[at].clone());
        std::thread::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| probe(&url))).unwrap_or(Outcome::NoAnswer);
            let _ = tx.send((at, outcome));
        });
    };
    start(&mut started, &mut next_at);
    loop {
        // The first undecided address decides what happens: a winner above it ends the race, and when everything above it
        // has failed it is the one to ask now.
        let mut undecided = false;
        for (at, result) in results.iter().enumerate() {
            match result {
                None => {
                    if at >= started {
                        start(&mut started, &mut next_at);
                    }
                    undecided = true;
                    break;
                }
                Some(Outcome::Ok) => return (Some(urls[at].clone()), report(&results)),
                Some(_) => {}
            }
        }
        if !undecided {
            return (None, report(&results));
        }
        let heard = if started < n {
            match rx.recv_timeout(next_at.saturating_duration_since(Instant::now())) {
                Ok(h) => Some(h),
                Err(RecvTimeoutError::Timeout) => {
                    start(&mut started, &mut next_at);
                    None
                }
                Err(RecvTimeoutError::Disconnected) => None,
            }
        } else {
            rx.recv().ok()
        };
        if let Some((at, outcome)) = heard {
            results[at] = Some(outcome);
        }
    }
}

type Slot = Arc<(Mutex<Option<String>>, Condvar)>;
static PICKING: Mutex<Vec<(String, Slot)>> = Mutex::new(Vec::new());

/// The address to talk to `m` on: the one in use when it answers as this machine, else the first in the order that does
/// (and it becomes the one in use), else the one in use, so the request that follows says why it can't. One pick per machine
/// at a time; callers that ask meanwhile share it (`pickAddress`).
pub fn pick_address(m: &Machine, probe: &Probe) -> String {
    if !has_choices(m) {
        return m.base().to_string();
    }
    let slot = {
        let mut running = PICKING.lock().unwrap_or_else(|e| e.into_inner());
        match running.iter().find(|(id, _)| *id == m.id) {
            Some((_, slot)) => Err(slot.clone()),
            None => {
                let slot: Slot = Arc::default();
                running.push((m.id.clone(), slot.clone()));
                Ok(slot)
            }
        }
    };
    let slot = match slot {
        Err(slot) => {
            let mut answer = slot.0.lock().unwrap_or_else(|e| e.into_inner());
            while answer.is_none() {
                answer = slot.1.wait(answer).unwrap_or_else(|e| e.into_inner());
            }
            return answer.clone().unwrap_or_default();
        }
        Ok(slot) => slot,
    };
    let current = m.base().to_string();
    let outcome = probe(&current);
    // The address it was added at is trusted as before when the Fleet there is too old to say who it is.
    let chosen = if outcome == Outcome::Ok || (outcome == Outcome::NoProbe && current == m.base_url) {
        current
    } else {
        let others: Vec<String> = m.addresses.iter().map(|a| a.url.clone()).filter(|u| *u != current).collect();
        match race(&others, probe, PROBE_GAP).0 {
            Some(winner) => {
                use_address(m, &winner);
                winner
            }
            None => current,
        }
    };
    *slot.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(chosen.clone());
    slot.1.notify_all();
    PICKING.lock().unwrap_or_else(|e| e.into_inner()).retain(|(id, _)| *id != m.id);
    chosen
}

/// An address above the one in use where the machine answers now, if any. Nothing moves (`betterAddress`).
pub fn better_address(m: &Machine, probe: &Probe) -> Option<String> {
    let urls: Vec<String> = m.addresses.iter().map(|a| a.url.clone()).collect();
    let in_use = urls.iter().position(|u| u == m.base())?;
    if in_use == 0 {
        return None;
    }
    race(&urls[..in_use], probe, PROBE_GAP).0
}

static CHECKED: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

/// For a machine on a lower address, once a minute: looks for a better one and moves to it. Whether it moved. The read that
/// calls this has just succeeded, so the machine is there; a hub opened on the old address stays where it was.
pub fn look_for_better(m: &Machine) -> bool {
    if is_best(m) {
        return false;
    }
    {
        let mut last = CHECKED.lock().unwrap_or_else(|e| e.into_inner());
        if last.iter().any(|(id, at)| *id == m.id && at.elapsed() < RECHECK) {
            return false;
        }
        last.retain(|(id, _)| *id != m.id);
        last.push((m.id.clone(), Instant::now()));
    }
    match better_address(m, &real_probe(m)) {
        Some(url) => {
            use_address(m, &url);
            true
        }
        None => false,
    }
}

/// Requests that can safely go again: they change nothing there (`isRead`).
pub fn is_read(method: &str) -> bool {
    method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD")
}

/// Makes a call on `m`, and when a read can't reach it picks again and calls once more at the address that answers. A write
/// is never repeated (`fetchOnMachine`). `Err` of `send` means nothing answered; a refusal is an answer.
pub fn retrying<T>(
    m: &Machine,
    method: &str,
    send: impl Fn(&str) -> Result<T, String>,
    repick: impl Fn(&Machine) -> bool,
) -> Result<T, String> {
    match send(m.base()) {
        Err(_) if is_read(method) && repick(m) => send(m.base()),
        done => done,
    }
}

/// For a request that failed to reach `m`: picks again, and says whether it moved to another address (`repickAddress`).
pub fn repick(m: &Machine) -> bool {
    if !has_choices(m) {
        return false;
    }
    let before = m.base().to_string();
    pick_address(m, &real_probe(m)) != before
}

/// `GET path` on `m`, once more at another address when the first can't be reached.
pub fn get_on(m: &Machine, path: &str) -> Answer {
    retrying(m, "GET", |base| http::get_page(base, path), repick)
}
