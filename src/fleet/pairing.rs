//! Adding a machine from Settings → Machines (`MachinesSection.vue`'s form, `stores/machines.ts` `addMachine`, `pairMachine` and
//! `previewPairing`). Home does the work: it looks at the machine behind a link without using the code up, redeems the code and
//! keeps the token it was given, or checks an address and a token and keeps those. This file holds the form's state, what it
//! decides from what was typed, and the three requests, made on a thread. The words of a refusal are the server's own, with the
//! diagnosis it sends behind them (`DiagnosedError`).
use super::http;
use super::machine::{PairingInput, describe_reach, normalize_manual_code, parse_pairing_input};
use super::machine_diagnosis::Diagnosis;
use super::machines::read_error;
use crate::{State, Tx};
use rune::TextEdit;
use serde_json::{Value, json};

/// The keys of the form's fields, for focus and for tests.
pub const LINK_FIELD: &str = "machines-add-link";
pub const CODE_FIELD: &str = "machines-add-code";
pub const TOKEN_FIELD: &str = "machines-add-token";

/// Why something couldn't be done, and the diagnosis behind it when the server sent one.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub message: String,
    pub diagnosis: Option<Diagnosis>,
}

impl Failure {
    pub fn said(message: &str) -> Failure {
        Failure { message: message.to_string(), diagnosis: None }
    }
}

/// The machine behind a pairing link (`PairingPreview`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub name: String,
    pub os: String,
    pub version: String,
    pub base_url: String,
}

impl Preview {
    fn from_json(v: &Value) -> Option<Preview> {
        let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let name = text("name");
        (!name.is_empty()).then(|| Preview { name, os: text("os"), version: text("version"), base_url: text("baseUrl") })
    }

    /// "<name> · <OS> · Fleet <version>" (`foundLine`): what is not known is left out.
    pub fn line(&self) -> String {
        let os = (!self.os.is_empty()).then(|| os_label(&self.os));
        let version = (!self.version.is_empty()).then(|| format!("Fleet {}", self.version));
        [Some(self.name.clone()), os, version].into_iter().flatten().collect::<Vec<_>>().join(" · ")
    }

    /// "Reached over Tailscale at <host>" (`foundReach`).
    pub fn reach(&self) -> String {
        format!("Reached {}", describe_reach(&self.base_url))
    }
}

/// `osLabel`.
pub fn os_label(os: &str) -> String {
    match os.to_lowercase().as_str() {
        "linux" => "Linux".into(),
        "macos" => "macOS".into(),
        "windows" => "Windows".into(),
        _ => os.to_string(),
    }
}

/// What the form keeps while it is open.
#[derive(Default)]
pub struct AddForm {
    pub open: bool,
    pub link: TextEdit,
    pub code: TextEdit,
    pub token: TextEdit,
    /// "Advanced: use the machine's access token" was pressed.
    pub advanced: bool,
    pub looking: bool,
    pub found: Option<Preview>,
    pub preview_error: Option<Failure>,
    pub adding: bool,
    pub error: Option<Failure>,
    /// The name of the machine that was just added, for the line the page says it with.
    pub added: Option<String>,
    /// The address and code the last look was for, and which look it was, so an old answer can't replace a newer one.
    looked_at: Option<(String, String)>,
    look: u64,
}

impl AddForm {
    /// What the Pairing link field holds (`pasted`).
    pub fn pasted(&self) -> Option<PairingInput> {
        parse_pairing_input(&self.link.text)
    }

    /// The address and code to look at: a whole link, or an address with a code typed under it (`pairTarget`).
    pub fn target(&self) -> Option<(String, String)> {
        match self.pasted()? {
            PairingInput::Link { base_url, code } => Some((base_url, code)),
            PairingInput::Address { base_url } => normalize_manual_code(&self.code.text).map(|code| (base_url, code)),
            PairingInput::Invalid(_) => None,
        }
    }

    /// With the machine's token typed under Advanced there is no code to look at (`usesToken`).
    pub fn uses_token(&self) -> bool {
        matches!(self.pasted(), Some(PairingInput::Address { .. })) && !self.token.text.trim().is_empty()
    }

    /// "Add <name>" for a machine found by link, else "Add machine" (`addLabel`).
    pub fn label(&self) -> String {
        match &self.found {
            Some(found) if !self.uses_token() => format!("Add {}", found.name),
            _ => "Add machine".into(),
        }
    }

    pub fn can_add(&self) -> bool {
        self.uses_token() || (self.found.is_some() && self.target().is_some())
    }

    /// For a scene or a test that sets `found` by hand: the poll must not look again at what is already shown.
    pub fn mark_looked(&mut self) {
        self.looked_at = self.target();
    }

    pub fn clear(&mut self) {
        self.link = TextEdit::new(String::new());
        self.code = TextEdit::new(String::new());
        self.token = TextEdit::new(String::new());
    }
}

/// What the server said to a refused call: its words, and its diagnosis when it sent one (`readFailure`).
fn refusal(status: u16, text: &str, fallback: &str) -> Failure {
    if status == 404 && read_error(text, String::new()).is_empty() {
        return Failure::said("This Fleet can't pair a machine from here. Add it by address with its token instead.");
    }
    let diagnosis = serde_json::from_str::<Value>(text).ok().and_then(|b| b.get("diagnosis").and_then(Diagnosis::from_json));
    Failure { message: read_error(text, fallback.to_string()), diagnosis }
}

fn call(path: &str, body: &Value, fallback: &str, home: &str) -> Result<Value, Failure> {
    let (status, text) = http::text_with("POST", home, path, Some(body)).map_err(|_| Failure::said("Can't reach Fleet."))?;
    if (200..300).contains(&status) {
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    } else {
        Err(refusal(status, &text, fallback))
    }
}

/// `POST /api/machines/pair/preview`: who is behind the link, without using its code up.
pub fn preview(home: &str, base_url: &str, code: &str) -> Result<Preview, Failure> {
    let answer = call(
        "/api/machines/pair/preview",
        &json!({ "baseUrl": base_url.trim(), "code": code.trim() }),
        "Couldn't reach the machine.",
        home,
    )?;
    Preview::from_json(&answer).ok_or_else(|| Failure::said("Couldn't reach the machine."))
}

/// `POST /api/machines/pair`: home redeems the code, keeps the token it got and answers with the machine's name.
pub fn pair(home: &str, base_url: &str, code: &str) -> Result<String, Failure> {
    let answer =
        call("/api/machines/pair", &json!({ "baseUrl": base_url.trim(), "code": code.trim() }), "Couldn't pair the machine.", home)?;
    name_of(&answer, "Couldn't pair the machine.")
}

/// `POST /api/machines`: home asks the machine who it is with this token, then keeps both.
pub fn add_by_token(home: &str, base_url: &str, token: &str) -> Result<String, Failure> {
    let token = token.trim();
    if token.is_empty() {
        return Err(Failure::said("Paste the machine's access token."));
    }
    let answer = call("/api/machines", &json!({ "baseUrl": base_url, "token": token }), "Couldn't add the machine.", home)?;
    name_of(&answer, "Couldn't add the machine.")
}

fn name_of(answer: &Value, fallback: &str) -> Result<String, Failure> {
    super::machine::Machine::from_json(answer).map(|m| m.name).ok_or_else(|| Failure::said(fallback))
}

impl State {
    /// The Add a machine button, the list's row or the palette (`openAddForm`).
    pub fn open_add_form(&mut self, tx: &mut Tx) {
        let form = &mut self.machine_page.add;
        form.open = true;
        form.error = None;
        form.added = None;
        tx.focus(LINK_FIELD, true);
    }

    /// Cancel, Escape or a machine added: the form goes away and is emptied (`clearAddForm`).
    pub fn close_add_form(&mut self) {
        let form = &mut self.machine_page.add;
        if form.adding {
            return;
        }
        form.open = false;
        form.advanced = false;
        form.error = None;
        form.clear();
        // The look that is still on its way is no longer wanted.
        form.look += 1;
        form.looked_at = None;
        form.looking = false;
        form.found = None;
        form.preview_error = None;
    }

    pub fn show_add_advanced(&mut self, tx: &mut Tx) {
        self.machine_page.add.advanced = true;
        tx.focus(TOKEN_FIELD, true);
    }

    /// The poll's turn: the form was asked for, and a link or a code that has just been typed is looked at.
    pub fn keep_add_form(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        if self.add_machine_asked && self.rail == crate::Rail::Settings {
            self.add_machine_asked = false;
            self.open_add_form(tx);
            changed = true;
        }
        let form = &mut self.machine_page.add;
        if !form.open {
            return changed;
        }
        let target = form.target();
        if target == form.looked_at {
            return changed;
        }
        form.look += 1;
        form.looked_at = target.clone();
        form.found = None;
        form.preview_error = None;
        form.looking = target.is_some();
        let (Some((base_url, code)), Some(link)) = (target, self.link.clone()) else {
            form.looking = false;
            return true;
        };
        let mine = form.look;
        link.off_thread(
            move |home| preview(home, &base_url, &code),
            move |s, _, result| {
                let form = &mut s.machine_page.add;
                if form.look != mine {
                    return;
                }
                form.looking = false;
                match result {
                    Ok(found) => form.found = Some(found),
                    Err(why) => form.preview_error = Some(why),
                }
            },
        );
        true
    }

    /// Add <name> / Add machine (`addMachine`): pairs with the code, or adds by address and token.
    pub fn submit_add(&mut self) {
        let form = &mut self.machine_page.add;
        if form.adding || !form.can_add() {
            return;
        }
        let Some(link) = self.link.clone() else {
            form.error = Some(Failure::said("Can't reach Fleet."));
            return;
        };
        form.adding = true;
        form.error = None;
        let token = form.uses_token().then(|| form.token.text.clone());
        let Some(base_url) = (match form.pasted() {
            Some(PairingInput::Link { base_url, .. } | PairingInput::Address { base_url }) => Some(base_url),
            _ => None,
        }) else {
            form.adding = false;
            return;
        };
        let code = form.target().map(|(_, code)| code).unwrap_or_default();
        link.off_thread(
            move |home| match token {
                Some(token) => add_by_token(home, &base_url, &token),
                None => pair(home, &base_url, &code),
            },
            |s, _, result| {
                s.machine_page.add.adding = false;
                match result {
                    Ok(name) => {
                        s.close_add_form();
                        s.machine_page.add.added = Some(name);
                        if let Some(link) = &s.link {
                            link.refresh_machines();
                        }
                    }
                    Err(why) => s.machine_page.add.error = Some(why),
                }
            },
        );
    }
}
