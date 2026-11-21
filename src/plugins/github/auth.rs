//! GitHub's connection (`plugins/builtin/github/composables/use-github-auth.ts`): whether Fleet is connected, the device flow
//! (a code to type on GitHub's page, then a poll every few seconds until it ends), a personal access token (tested straight against
//! GitHub, then stored by Fleet), and Disconnect. Every request goes on a thread and its answer comes back to the UI thread; nothing
//! here waits. GitHub is a plugin: core never imports this, the plugin's page (`view::connect`) reads it.
//!
//! Connecting belongs to the Fleet server the app is on (`HOME_MACHINE_KEY`), not to the machine Settings shows: the plugin
//! switches are per server too.
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::settings_nav::Page;
use crate::{Rail, State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};
use std::time::Duration;

/// Where the token is tested. A test points it at a fake; nothing else changes it.
pub const GITHUB_API: &str = "https://api.github.com";

/// `GitHubDeviceFlowState`.
#[derive(Clone, Debug, PartialEq)]
pub enum Device {
    Idle,
    Initiating,
    Awaiting { user_code: String, verification_uri: String, device_code: String, expires_at: f64, interval: u64 },
    Complete,
    Expired,
    Denied,
    Error(String),
}

/// `GitHubPatState`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pat {
    Idle,
    Testing,
    Success(Option<String>),
    Error(String),
}

impl Pat {
    /// `patMessage`: what stands under the box, and whether it is an error.
    pub fn message(&self) -> Option<(String, bool)> {
        match self {
            Pat::Error(m) => Some((m.clone(), true)),
            Pat::Success(Some(name)) => Some((format!("Connected as @{name}"), false)),
            Pat::Success(None) => Some(("GitHub connected successfully.".into(), false)),
            _ => None,
        }
    }
}

/// What the page keeps while it is open.
#[derive(Clone)]
pub struct Github {
    pub connected: bool,
    pub checking: bool,
    /// The page has asked for the status on this visit.
    pub(crate) asked: bool,
    status_seq: u64,
    pub device: Device,
    /// Bumped to end a poll (`pollGeneration`): an answer that comes back for an older one is dropped.
    generation: u64,
    /// When the next poll is due, and how many seconds after that (the server may slow it down).
    poll_due: Option<f64>,
    pub(crate) polling: bool,
    pub(crate) poll_every: u64,
    /// The whole seconds last drawn in the countdown.
    shown_left: Option<u64>,
    pub pat_open: bool,
    pub pat: TextEdit,
    pub pat_state: Pat,
    /// A hash of the token as last seen, to tell that it was typed in again (the token itself is kept nowhere else).
    pat_seen: u64,
    pub connecting_pat: bool,
    pub disconnecting: bool,
    /// Where the token is tested.
    pub api: String,
}

impl Default for Github {
    fn default() -> Self {
        Github {
            connected: false,
            checking: false,
            asked: false,
            status_seq: 0,
            device: Device::Idle,
            generation: 0,
            poll_due: None,
            polling: false,
            poll_every: 5,
            shown_left: None,
            pat_open: false,
            pat: TextEdit::new(String::new()).secure(),
            pat_state: Pat::Idle,
            pat_seen: 0,
            connecting_pat: false,
            disconnecting: false,
            api: GITHUB_API.to_string(),
        }
    }
}

impl Github {
    /// `pollStatusLabel`: the time left on the code, counted down.
    pub fn poll_label(&self, now: f64) -> Option<String> {
        let Device::Awaiting { expires_at, interval, .. } = &self.device else { return None };
        let left = (expires_at - now).max(0.0).ceil() as u64;
        Some(format!("Polling GitHub every {interval}s. Code expires in {}:{:02}.", left / 60, left % 60))
    }

    /// A token is typed (trimmed): the buttons need one.
    pub fn has_token(&self) -> bool {
        !self.pat.text.trim().is_empty()
    }

    fn stop_polling(&mut self) {
        self.generation += 1;
        self.poll_due = None;
        self.polling = false;
    }
}

fn ok(status: u16) -> bool {
    (200..300).contains(&status)
}

/// `readErrorMessage`: the server's `error`, else its `message`, else `fallback`.
pub fn words(body: &str, fallback: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    ["error", "message"].iter().find_map(|k| v.get(k).and_then(Value::as_str)).unwrap_or(fallback).to_string()
}

impl State {
    /// The page is on screen: GitHub's own settings, while the plugin is on.
    fn github_page_open(&self) -> bool {
        self.rail == Rail::Settings && self.settings.page == Page::Plugin("github".into()) && self.plugin_enabled("github")
    }

    /// Called from `poll`. While the page is open: reads the status once per visit, polls a device flow when it is due and counts
    /// its code down. Leaving the page ends a poll and forgets what was typed (`onUnmounted`'s `stopPolling`; a code still waited
    /// for goes back to the button, where Fleet leaves it on screen with nothing polling). True when something drawn changed.
    pub fn keep_github(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.github_page_open() {
            let g = super::auth(&mut self.places);
            let had = g.asked || !g.pat.text.is_empty() || g.pat_state != Pat::Idle || matches!(g.device, Device::Awaiting { .. });
            if had {
                g.stop_polling();
                g.asked = false;
                g.checking = false;
                g.pat.replace_text("");
                g.pat_state = Pat::Idle;
                if matches!(g.device, Device::Awaiting { .. } | Device::Initiating) {
                    g.device = Device::Idle;
                }
                g.shown_left = None;
            }
            return had;
        }
        let mut changed = super::auth(&mut self.places).token_retyped();
        if !super::auth(&mut self.places).asked {
            super::auth(&mut self.places).asked = true;
            self.load_github_status();
            changed = true;
        }
        let g = super::auth(&mut self.places);
        let Device::Awaiting { expires_at, .. } = &g.device else { return changed };
        let left = (expires_at - now).max(0.0).ceil() as u64;
        if g.shown_left != Some(left) {
            g.shown_left = Some(left);
            changed = true;
        }
        match g.poll_due {
            Some(at) if now >= at && !g.polling => self.poll_github(),
            Some(at) => tx.wake_after(Duration::from_secs_f64((at - now).clamp(0.01, 1.0))),
            None => tx.wake_after(Duration::from_secs(1)),
        }
        changed
    }

    /// `loadStatus`: `GET .../auth/status`; a failure means not connected.
    pub fn load_github_status(&mut self) {
        let g = super::auth(&mut self.places);
        g.status_seq += 1;
        g.checking = true;
        let seq = g.status_seq;
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            |base| http::text_with("GET", base, "/api/integrations/github/auth/status", None),
            move |s, _, answer| {
                let g = super::auth(&mut s.places);
                if g.status_seq != seq {
                    return;
                }
                g.checking = false;
                g.connected = match answer {
                    Ok((status, body)) if ok(status) => {
                        serde_json::from_str::<Value>(&body).ok().and_then(|v| v.get("connected")?.as_bool()).unwrap_or(false)
                    }
                    _ => false,
                };
                s.github_connection_changed();
            },
        );
        if !sent {
            super::auth(&mut self.places).checking = false;
            super::auth(&mut self.places).connected = false;
            self.github_connection_changed();
        }
    }

    /// `watch(isConnected)`: connected asks for the repository cache (kept 15 minutes), not connected forgets it.
    fn github_connection_changed(&mut self) {
        if super::auth_ref(&self.places).connected {
            self.github_repos_refresh(false);
            self.github_panel_connected();
        } else {
            super::state(&mut self.places).repos.clear();
        }
    }

    /// `connectWithDeviceFlow`: asks for a code, then polls after its interval.
    pub fn github_connect_device(&mut self) {
        let g = super::auth(&mut self.places);
        g.stop_polling();
        g.device = Device::Initiating;
        let generation = g.generation;
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            |base| http::text_with("POST", base, "/api/integrations/github/auth/device-code", None),
            move |s, tx, answer| {
                let clock = s.clock;
                let g = super::auth(&mut s.places);
                if g.generation != generation {
                    return;
                }
                g.device = match answer {
                    Ok((status, body)) if ok(status) => match device_from(&body, clock) {
                        Some(d) => {
                            if let Device::Awaiting { interval, .. } = &d {
                                g.poll_every = *interval;
                                g.poll_due = Some(clock + *interval as f64);
                                tx.wake_after(Duration::from_secs(1));
                            }
                            d
                        }
                        None => Device::Error("Failed to start GitHub authorization.".into()),
                    },
                    Ok((_, body)) => Device::Error(words(&body, "Failed to start GitHub authorization.")),
                    Err(_) => Device::Error("Failed to start GitHub authorization.".into()),
                };
            },
        );
        if !sent {
            super::auth(&mut self.places).device = Device::Error("Failed to start GitHub authorization.".into());
        }
    }

    /// `resetDeviceFlow`: Cancel and Try again.
    pub fn github_reset_device(&mut self) {
        let g = super::auth(&mut self.places);
        g.stop_polling();
        g.device = Device::Idle;
    }

    /// `pollOnce`: one look at the authorization. Pending asks again after the interval the server named, else the last one.
    fn poll_github(&mut self) {
        let g = super::auth(&mut self.places);
        let Device::Awaiting { device_code, .. } = g.device.clone() else { return };
        g.polling = true;
        g.poll_due = None;
        let generation = g.generation;
        let every = g.poll_every;
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with("POST", base, "/api/integrations/github/auth/poll", Some(&json!({ "deviceCode": device_code }))),
            move |s, tx, answer| {
                let clock = s.clock;
                let g = super::auth(&mut s.places);
                if g.generation != generation {
                    return;
                }
                g.polling = false;
                let (status, body) = match answer {
                    Ok(a) => a,
                    Err(_) => return end_with(g, Device::Error("Failed while waiting for GitHub authorization.".into())),
                };
                let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                if !ok(status) {
                    return end_with(g, Device::Error(words(&body, &format!("HTTP {status}"))));
                }
                match payload.get("status").and_then(Value::as_str) {
                    Some("complete") => s.confirm_github(generation),
                    Some("expired") => end_with(g, Device::Expired),
                    Some("denied") => end_with(g, Device::Denied),
                    Some("error") => {
                        let why = payload.get("message").and_then(Value::as_str).unwrap_or("GitHub authorization failed.");
                        end_with(g, Device::Error(why.to_string()))
                    }
                    _ => {
                        let next = payload.get("interval").and_then(Value::as_u64).unwrap_or(every);
                        g.poll_every = next;
                        g.poll_due = Some(clock + next as f64);
                        tx.wake_after(Duration::from_secs(1));
                    }
                }
            },
        );
        if !sent {
            end_with(super::auth(&mut self.places), Device::Error("Failed while waiting for GitHub authorization.".into()));
        }
    }

    /// `verifyConnectedStatus` after `complete`: the status is read again to confirm.
    fn confirm_github(&mut self, generation: u64) {
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            |base| http::text_with("GET", base, "/api/integrations/github/auth/status", None),
            move |s, _, answer| {
                let g = super::auth(&mut s.places);
                if g.generation != generation {
                    return;
                }
                let connected = match &answer {
                    Ok((status, body)) if ok(*status) => {
                        let c = serde_json::from_str::<Value>(body).ok().and_then(|v| v.get("connected")?.as_bool()).unwrap_or(false);
                        g.connected = c;
                        Ok(c)
                    }
                    Ok((status, body)) => Err(words(body, &format!("HTTP {status}"))),
                    Err(_) => Err("Failed while waiting for GitHub authorization.".to_string()),
                };
                end_with(
                    g,
                    match connected {
                        Ok(true) => Device::Complete,
                        Ok(false) => Device::Error("GitHub authorization completed, but the saved connection was not confirmed.".into()),
                        Err(why) => Device::Error(why),
                    },
                );
                s.github_connection_changed();
            },
        );
        if !sent {
            end_with(super::auth(&mut self.places), Device::Error("Failed while waiting for GitHub authorization.".into()));
        }
    }

    /// `copyUserCode`.
    pub fn github_copy_code(&mut self, tx: &mut Tx) {
        if let Device::Awaiting { user_code, .. } = &super::auth_ref(&self.places).device {
            tx.copy(user_code.clone());
        }
    }

    /// `testPersonalAccessToken`: the token goes to GitHub itself, from here, and nowhere else.
    pub fn github_test_pat(&mut self) {
        let g = super::auth(&mut self.places);
        let token = g.pat.text.trim().to_string();
        if token.is_empty() {
            g.pat_state = Pat::Error("Enter a personal access token.".into());
            return;
        }
        if g.pat_state == Pat::Testing {
            return;
        }
        g.pat_state = Pat::Testing;
        let api = g.api.clone();
        let Some(link) = &self.link else {
            super::auth(&mut self.places).pat_state = Pat::Error("Unable to validate GitHub token.".into());
            return;
        };
        let post = link.post();
        std::thread::spawn(move || {
            let answer = http::github_user(&api, &token);
            post.update(move |s, _| {
                let g = super::auth(&mut s.places);
                if g.pat_state != Pat::Testing {
                    return;
                }
                g.pat_state = match answer {
                    Ok((status, body)) if ok(status) => {
                        match serde_json::from_str::<Value>(&body).ok().and_then(|v| v.get("login")?.as_str().map(str::to_string)) {
                            Some(login) => Pat::Success(Some(login)),
                            None => Pat::Error("Unable to validate GitHub token.".into()),
                        }
                    }
                    Ok((_, body)) => Pat::Error(words(&body, "Invalid GitHub token.")),
                    Err(_) => Pat::Error("Unable to validate GitHub token.".into()),
                };
            });
        });
    }

    /// `connectWithPersonalAccessToken`: Fleet validates and stores it; the box is wiped once it has.
    pub fn github_connect_pat(&mut self) {
        let g = super::auth(&mut self.places);
        let token = g.pat.text.trim().to_string();
        if token.is_empty() {
            g.pat_state = Pat::Error("Enter a personal access token.".into());
            return;
        }
        if g.connecting_pat {
            return;
        }
        g.connecting_pat = true;
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with("POST", base, "/api/integrations/github/auth/token", Some(&json!({ "token": token }))),
            |s, _, answer| {
                let g = super::auth(&mut s.places);
                g.connecting_pat = false;
                match answer {
                    Ok((status, _)) if ok(status) => {
                        g.pat.replace_text("");
                        g.pat_state = Pat::Success(None);
                        g.connected = true;
                        g.device = Device::Complete;
                    }
                    Ok((_, body)) => g.pat_state = Pat::Error(words(&body, "Failed to connect GitHub.")),
                    Err(_) => g.pat_state = Pat::Error("Failed to connect GitHub.".into()),
                }
                s.github_connection_changed();
            },
        );
        if !sent {
            let g = super::auth(&mut self.places);
            g.connecting_pat = false;
            g.pat_state = Pat::Error("Failed to connect GitHub.".into());
        }
    }

    /// `disconnectGitHub`: Fleet forgets the token.
    pub fn github_disconnect(&mut self) {
        let g = super::auth(&mut self.places);
        if g.disconnecting {
            return;
        }
        g.disconnecting = true;
        let sent = self.feed_call(
            HOME_MACHINE_KEY,
            |base| http::text_with("DELETE", base, "/api/integrations/github/auth", None),
            |s, _, answer| {
                let g = super::auth(&mut s.places);
                g.disconnecting = false;
                match answer {
                    Ok((status, _)) if ok(status) => {
                        g.stop_polling();
                        g.connected = false;
                        g.device = Device::Idle;
                        g.pat_state = Pat::Idle;
                        g.pat.replace_text("");
                    }
                    Ok((status, body)) => g.device = Device::Error(words(&body, &format!("HTTP {status}"))),
                    Err(_) => g.device = Device::Error("Failed to disconnect GitHub.".into()),
                }
                s.github_connection_changed();
            },
        );
        if !sent {
            super::auth(&mut self.places).disconnecting = false;
        }
    }

    /// The token box, for the field's edit handler.
    pub fn github_pat_edit(&mut self) -> &mut TextEdit {
        &mut super::auth(&mut self.places).pat
    }
}

impl Github {
    /// Typing the token again clears the message under it (`watch(personalAccessToken)`). True when it did.
    pub(crate) fn token_retyped(&mut self) -> bool {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.pat.text.hash(&mut h);
        let seen = h.finish();
        if seen == self.pat_seen {
            return false;
        }
        self.pat_seen = seen;
        !matches!(self.pat_state, Pat::Idle | Pat::Testing) && {
            self.pat_state = Pat::Idle;
            true
        }
    }
}

fn end_with(g: &mut Github, device: Device) {
    g.stop_polling();
    g.device = device;
}

/// `DeviceCodeResponse`: `{userCode, verificationUri, deviceCode, expiresIn, interval}`.
fn device_from(body: &str, clock: f64) -> Option<Device> {
    let v: Value = serde_json::from_str(body).ok()?;
    let text = |k: &str| v.get(k)?.as_str().map(str::to_string);
    Some(Device::Awaiting {
        user_code: text("userCode")?,
        verification_uri: text("verificationUri")?,
        device_code: text("deviceCode")?,
        expires_at: clock + v.get("expiresIn")?.as_f64()?,
        interval: v.get("interval")?.as_u64()?,
    })
}

/// The token never reaches a log: the box is drawn as `[redacted]` (`TextEdit::secure`), and a copy of this is only for the settings'
/// own `Clone`, `Debug` and `==`, which compare what the page shows.
impl std::fmt::Debug for Github {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Github")
            .field("connected", &self.connected)
            .field("device", &self.device)
            .field("pat_state", &self.pat_state)
            .finish()
    }
}

impl PartialEq for Github {
    fn eq(&self, other: &Self) -> bool {
        self.connected == other.connected
            && self.checking == other.checking
            && self.device == other.device
            && self.pat_open == other.pat_open
            && self.pat_state == other.pat_state
            && self.connecting_pat == other.connecting_pat
            && self.disconnecting == other.disconnecting
    }
}

impl Eq for Github {}
