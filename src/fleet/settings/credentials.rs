//! Settings → API keys as plain data (`CredentialsSection.vue`): the keys a machine stores, what the add and update forms hold,
//! the words they say when a field is missing, and the four calls under `/api/credentials`. No Rune widgets and no I/O here but
//! [`send`], which a thread calls. What touches the app is `credentials_actions.rs`.
//!
//! The secret never comes back from the server (only a short `displayHint`), and here it lives only in two secure text fields:
//! they draw one bullet per character, keep no undo history, hide the text from `Debug` and wipe their buffer when replaced.
//! Nothing in this file or the page copies a secret into anything else.
use super::skills;
use rune::TextEdit;
use serde_json::{Value, json};

pub const LOAD_FAILED: &str = "Failed to load API keys.";
pub const SAVE_FAILED: &str = "Failed to save API key.";
pub const UPDATE_FAILED: &str = "Failed to update API key.";
pub const REMOVE_FAILED: &str = "Failed to remove API key.";
pub const ALL_REQUIRED: &str = "All fields are required.";
pub const VALUE_REQUIRED: &str = "API key value is required.";
/// The kind a new key starts with, and the one a quick-select fills.
pub const DEFAULT_KIND: &str = "api-key";

/// One stored key (`CredentialSummary`): everything the server tells about it except the secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credential {
    pub id: String,
    pub label: String,
    pub namespace: String,
    pub kind: String,
    /// The last few characters the server keeps to recognise the key by; may be empty.
    pub hint: String,
}

impl Credential {
    /// `getHintText`: what stands under the key's name.
    pub fn hint_text(&self) -> String {
        if self.hint.trim().is_empty() { "Stored securely".into() } else { format!("\u{2022}\u{2022}\u{2022}\u{2022} {}", self.hint) }
    }

    /// The line under the label: `<namespace> · <kind>`.
    pub fn line(&self) -> String {
        format!("{} \u{b7} {}", self.namespace, self.kind)
    }
}

/// `commonProviders`: the two quick-select chips.
pub struct Preset {
    pub label: &'static str,
    pub namespace: &'static str,
}

pub const PRESETS: [Preset; 2] = [Preset { label: "Anthropic", namespace: "anthropic" }, Preset { label: "OpenAI", namespace: "openai" }];

/// `GET /api/credentials`: a list; an empty body is "No data returned", a body that isn't a list reads as no keys.
pub fn parse_list(body: &Value) -> Result<Vec<Credential>, String> {
    if body.is_null() {
        return Err("No data returned".into());
    }
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(body
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| Credential {
                    id: text(c, "id"),
                    label: text(c, "label"),
                    namespace: text(c, "namespace"),
                    kind: text(c, "kind"),
                    hint: text(c, "displayHint"),
                })
                .collect()
        })
        .unwrap_or_default())
}

fn secure() -> TextEdit {
    TextEdit::new(String::new()).secure()
}

/// The add form (`form`, `formError`, `isSaving`).
#[derive(Clone, Debug, PartialEq)]
pub struct AddForm {
    pub label: TextEdit,
    pub namespace: TextEdit,
    pub kind: TextEdit,
    pub value: TextEdit,
    pub error: Option<String>,
    pub saving: bool,
}

impl Eq for AddForm {}
impl Eq for Edit {}
impl Eq for CredentialsPage {}

impl Default for AddForm {
    fn default() -> Self {
        AddForm {
            label: TextEdit::new(String::new()),
            namespace: TextEdit::new(String::new()),
            kind: TextEdit::new(DEFAULT_KIND.to_string()),
            value: secure(),
            error: None,
            saving: false,
        }
    }
}

impl AddForm {
    /// `applyPreset`: the provider and type are set; the label only when it is empty.
    pub fn apply_preset(&mut self, preset: &Preset) {
        self.namespace = TextEdit::new(preset.namespace.to_string());
        self.kind = TextEdit::new(DEFAULT_KIND.to_string());
        if self.label.text.trim().is_empty() {
            self.label = TextEdit::new(format!("My {} API Key", preset.label));
        }
    }

    /// `storeCredential` up to the request: the body (all four trimmed, no metadata), or the words the form shows.
    pub fn body(&self) -> Result<Value, &'static str> {
        let (label, namespace, kind, value) =
            (self.label.text.trim(), self.namespace.text.trim(), self.kind.text.trim(), self.value.text.trim());
        if label.is_empty() || namespace.is_empty() || kind.is_empty() || value.is_empty() {
            return Err(ALL_REQUIRED);
        }
        Ok(json!({ "label": label, "namespace": namespace, "kind": kind, "value": value }))
    }
}

/// The inline edit of one key (`editingId`, `editingValue`, `updatingId`).
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub id: String,
    pub value: TextEdit,
    pub updating: bool,
}

/// What the page keeps while it is open. Leaving the page, or moving the machine switch, drops it all (the typed secrets with it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CredentialsPage {
    /// The machine key it was read from.
    pub loaded_for: Option<String>,
    /// Answers to an older ask (another machine, an older read) are dropped.
    pub request: u64,
    pub loading: bool,
    pub load_error: Option<String>,
    pub keys: Vec<Credential>,
    pub form_open: bool,
    pub form: AddForm,
    pub editing: Option<Edit>,
    pub deleting: Option<String>,
    /// `actionError`: an update's or a removal's failure.
    pub action_error: Option<String>,
}

impl CredentialsPage {
    /// `beginEdit`.
    pub fn begin_edit(&mut self, id: &str) {
        self.editing = Some(Edit { id: id.to_string(), value: secure(), updating: false });
        self.action_error = None;
    }

    /// `cancelEdit`: the typed value is wiped with the box.
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.action_error = None;
    }

    /// `closeAddForm` / `resetForm`: every field and the error cleared.
    pub fn close_form(&mut self) {
        self.form_open = false;
        self.form = AddForm::default();
    }

    /// Something is being sent: the page's buttons wait.
    pub fn busy(&self) -> bool {
        self.form.saving || self.deleting.is_some() || self.editing.as_ref().is_some_and(|e| e.updating)
    }
}

// ─── Requests ───────────────────────────────────────────────────────────────

pub fn list() -> skills::Request {
    skills::Request { method: "GET", path: "/api/credentials".into(), fallback: LOAD_FAILED }
}

pub fn store() -> skills::Request {
    skills::Request { method: "PUT", path: "/api/credentials".into(), fallback: SAVE_FAILED }
}

pub fn update(id: &str) -> skills::Request {
    skills::Request {
        method: "PUT",
        path: format!("/api/credentials/{}", crate::conv::export_history::encode_component(id)),
        fallback: UPDATE_FAILED,
    }
}

pub fn remove(id: &str) -> skills::Request {
    skills::Request {
        method: "DELETE",
        path: format!("/api/credentials/{}", crate::conv::export_history::encode_component(id)),
        fallback: REMOVE_FAILED,
    }
}

/// Sends one call to the machine at `base`; a thread calls it.
pub fn send(base: &str, req: &skills::Request, body: Option<&Value>) -> Result<Value, String> {
    skills::send_with(base, req, body)
}

/// What a change thread hands back: the change's own answer and, when it worked, the list read again after it
/// (`await loadCredentials()`).
pub type Changed = Result<Result<Vec<Credential>, String>, String>;

/// A change, then the list again: the order is the server's, nothing is changed optimistically.
pub fn change_then_read(base: &str, req: &skills::Request, body: Option<&Value>) -> Changed {
    send(base, req, body)?;
    Ok(send(base, &list(), None).and_then(|b| parse_list(&b)))
}
