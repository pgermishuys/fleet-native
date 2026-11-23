//! Add your phone, before the code (`AddPhonePanel.vue`, `lib/devices-api.ts` `createPairingCode` and `savePublicUrl`): where a
//! phone will reach home, **Add your phone** and **Use my own address**. The card that shows the code (its QR, the typed code and
//! the countdown) waits for the owner's phones decision (26, row 9.1), so a code that is made is only said to exist.
//!
//! Requests block and run on a thread, on home's link.
use super::reach::{WayLine, reach_lines};
use crate::fleet::http;
use crate::fleet::machine_actions::AccessState;
use crate::fleet::machines::read_error;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::json;

/// The panel's own state (`code`, `busy`, `error`, `editingUrl`, `phoneUrl`).
#[derive(Default)]
pub struct Phone {
    pub busy: bool,
    pub error: Option<String>,
    /// The "Use my own address" card is open.
    pub editing_url: bool,
    pub url: TextEdit,
    /// Home made a code. The card that shows it isn't built, so the panel says so instead.
    pub made: bool,
}

/// Why no code was made: `NoAddress` is home saying it listens on this machine only (a 400 with no address of the person's own),
/// the one case that opens the address field (`NoPhoneAddressError`).
#[derive(Debug, PartialEq, Eq)]
pub enum CodeFailure {
    NoAddress(String),
    Other(String),
}

/// `createPairingCode`: `POST /api/machine/pairing`, with the person's own address first when there is one.
pub fn create_pairing_code(base: &str, own: Option<&str>) -> Result<(), CodeFailure> {
    let fallback = "Couldn't make a pairing code.";
    let body = own.map_or_else(|| json!({}), |url| json!({ "baseUrl": url }));
    let (status, text) =
        http::text_with("POST", base, "/api/machine/pairing", Some(&body)).map_err(|_| CodeFailure::Other(fallback.into()))?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let why = read_error(&text, fallback.into());
    Err(if own.is_none() && status == 400 { CodeFailure::NoAddress(why) } else { CodeFailure::Other(why) })
}

/// `savePublicUrl`: the address phones should use for home; empty clears it.
pub fn save_public_url(base: &str, url: &str) -> Result<(), String> {
    let fallback = "Couldn't save the phone address.";
    let (status, text) =
        http::text_with("PUT", base, "/api/machine", Some(&json!({ "publicUrl": url }))).map_err(|_| fallback.to_string())?;
    if (200..300).contains(&status) { Ok(()) } else { Err(read_error(&text, fallback.into())) }
}

impl State {
    /// `reaching` before a code: home's own address, then the ones Fleet listens on (else the ones letting others reach it opened).
    pub fn phone_ways(&self) -> Vec<WayLine> {
        let mut urls: Vec<String> =
            self.machines.home.as_ref().and_then(|h| h.public_url.clone()).filter(|u| !u.is_empty()).into_iter().collect();
        let listed: Vec<String> = match &self.machine_page.access {
            AccessState::Loaded(a) => a.addresses.iter().map(|x| x.url.clone()).collect(),
            _ => Vec::new(),
        };
        if listed.is_empty() {
            urls.extend(self.machine_data.reach.iter().flat_map(|r| r.listening.iter().map(|a| a.url.clone())));
        } else {
            urls.extend(listed);
        }
        reach_lines(&urls.iter().map(String::as_str).collect::<Vec<_>>())
    }

    /// **Add your phone** (`start`): asks home for a code, with home's own address first when it has one.
    pub fn add_phone(&mut self, _tx: &mut Tx) {
        let own = self.machines.home.as_ref().and_then(|h| h.public_url.clone()).filter(|u| !u.is_empty());
        self.ask_for_code(own);
    }

    fn ask_for_code(&mut self, own: Option<String>) {
        let Some(link) = self.link.clone() else { return };
        let phone = &mut self.machine_data.phone;
        phone.busy = true;
        phone.error = None;
        phone.made = false;
        link.off_thread(
            move |base| create_pairing_code(base, own.as_deref()),
            |s, _, result| {
                let phone = &mut s.machine_data.phone;
                phone.busy = false;
                match result {
                    Ok(()) => phone.made = true,
                    // Fleet knows no address a phone could reach: the one thing left to ask for.
                    Err(CodeFailure::NoAddress(why)) => {
                        phone.editing_url = true;
                        phone.error = Some(why);
                    }
                    Err(CodeFailure::Other(why)) => phone.error = Some(why),
                }
            },
        );
    }

    /// **Use my own address**: opens or closes its card.
    pub fn toggle_phone_address(&mut self) {
        let phone = &mut self.machine_data.phone;
        phone.editing_url = !phone.editing_url;
    }

    pub fn cancel_phone_address(&mut self) {
        self.machine_data.phone.editing_url = false;
    }

    /// **Show the code** (`saveUrl`): saves the address on home, then asks for a code that carries it first.
    pub fn save_phone_address(&mut self, _tx: &mut Tx) {
        let url = self.machine_data.phone.url.text.trim().to_string();
        let phone = &mut self.machine_data.phone;
        if phone.busy || url.is_empty() {
            return;
        }
        let Some(link) = self.link.clone() else { return };
        phone.busy = true;
        phone.error = None;
        link.off_thread(
            {
                let url = url.clone();
                move |base| save_public_url(base, &url)
            },
            move |s, _, saved| {
                s.machine_data.phone.busy = false;
                if let Err(why) = saved {
                    s.machine_data.phone.error = Some(why);
                    return;
                }
                if let Some(home) = s.machines.home.as_mut() {
                    home.public_url = Some(url.clone());
                }
                s.machine_data.phone.editing_url = false;
                // A code made before doesn't carry the new address.
                s.ask_for_code(Some(url));
            },
        );
    }
}
