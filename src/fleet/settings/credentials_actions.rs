//! Settings → API keys' calls and what they change (`CredentialsSection.vue`: `loadCredentials`, `storeCredential`,
//! `updateCredential`, `removeCredential`). Every call goes to the machine Settings shows (`State::settings_target`), never home
//! by default. Nothing here blocks: a thread sends and the answer comes back to the UI thread. With no Fleet to ask the page
//! reads the invented keys in `mock::credentials` and the buttons change them.
//!
//! A secret leaves its text box only in the body of the request, built on the UI thread and moved to the sending thread; it is
//! not logged, and the box is wiped when the request is accepted, when a form closes and when the page is left.
use super::credentials::{self as c, Changed, CredentialsPage};
use crate::fleet::settings_nav::Page;
use crate::{Rail, State, Tx};
use serde_json::Value;

/// What a change did, for the one place that handles every answer.
#[derive(Clone, Copy)]
enum Change {
    Store,
    Update,
    Remove,
}

impl State {
    /// Called from `poll`: while the page is open, reads the machine Settings shows when the page hasn't, or when the switch
    /// moved to another machine; leaving the page forgets what it read and what was typed (`onMounted(load)`).
    pub fn keep_credentials(&mut self, _tx: &mut Tx) -> bool {
        if self.rail != Rail::Settings || self.settings.page != Page::Credentials {
            let had = self.settings.credentials.loaded_for.is_some();
            self.settings.credentials = CredentialsPage::default();
            return had;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.credentials.loaded_for.as_deref() == Some(key.as_str()) {
            return false;
        }
        let page = &mut self.settings.credentials;
        *page = CredentialsPage { loaded_for: Some(key.clone()), request: page.request + 1, loading: true, ..CredentialsPage::default() };
        self.read_credentials(&key);
        true
    }

    /// The link to the machine shown, or none when there is no Fleet to ask.
    fn credentials_link(&self, key: &str) -> Option<crate::fleet::link::Link> {
        self.link_for_machine(key).filter(|_| self.settings_target().is_some())
    }

    fn read_credentials(&mut self, key: &str) {
        let request = self.settings.credentials.request;
        let Some(link) = self.credentials_link(key) else {
            let page = &mut self.settings.credentials;
            page.loading = false;
            page.keys = crate::mock::credentials::keys();
            return;
        };
        link.off_thread(
            |base| c::send(base, &c::list(), None).and_then(|b| c::parse_list(&b)),
            move |s: &mut State, _tx: &mut Tx, got: Result<Vec<c::Credential>, String>| {
                let page = &mut s.settings.credentials;
                if page.request != request {
                    return;
                }
                page.loading = false;
                match got {
                    Ok(keys) => page.keys = keys,
                    Err(why) => page.load_error = Some(why),
                }
            },
        );
    }

    fn machine_key(&self) -> String {
        self.settings.choice.key(&self.machines).to_string()
    }

    /// "Add API key": opens the form.
    pub fn open_credential_form(&mut self) {
        let page = &mut self.settings.credentials;
        page.form_open = true;
        page.form.error = None;
    }

    /// Cancel on the add form; it waits while a save is under way.
    pub fn close_credential_form(&mut self) {
        let page = &mut self.settings.credentials;
        if !page.form.saving {
            page.close_form();
        }
    }

    pub fn pick_preset(&mut self, namespace: &str) {
        if let Some(p) = c::PRESETS.iter().find(|p| p.namespace == namespace) {
            self.settings.credentials.form.apply_preset(p);
        }
    }

    /// "Save API key" and Enter in a field.
    pub fn store_credential(&mut self) {
        let page = &mut self.settings.credentials;
        if page.form.saving {
            return;
        }
        let body = match page.form.body() {
            Ok(b) => b,
            Err(why) => {
                page.form.error = Some(why.into());
                return;
            }
        };
        page.form.saving = true;
        page.form.error = None;
        self.send_change(Change::Store, &c::store(), Some(body), None);
    }

    /// "Update key": opens the inline edit of one key (only one at a time).
    pub fn begin_credential_edit(&mut self, id: &str) {
        self.settings.credentials.begin_edit(id);
    }

    pub fn cancel_credential_edit(&mut self) {
        self.settings.credentials.cancel_edit();
    }

    /// "Save" in the inline edit.
    pub fn update_credential(&mut self) {
        let page = &mut self.settings.credentials;
        let Some(edit) = page.editing.as_mut().filter(|e| !e.updating) else { return };
        let value = edit.value.text.trim().to_string();
        if value.is_empty() {
            page.action_error = Some(c::VALUE_REQUIRED.into());
            return;
        }
        edit.updating = true;
        let id = edit.id.clone();
        page.action_error = None;
        self.send_change(Change::Update, &c::update(&id), Some(serde_json::json!({ "value": value })), Some(id));
    }

    /// "Remove": no question first (`removeCredential`).
    pub fn remove_credential(&mut self, id: &str) {
        let page = &mut self.settings.credentials;
        if page.deleting.is_some() {
            return;
        }
        page.deleting = Some(id.to_string());
        page.action_error = None;
        self.send_change(Change::Remove, &c::remove(id), None, Some(id.to_string()));
    }

    /// One change, then the list again, on the machine shown. With no Fleet the invented list changes.
    fn send_change(&mut self, what: Change, req: &super::skills::Request, body: Option<Value>, id: Option<String>) {
        let key = self.machine_key();
        let request = self.settings.credentials.request;
        let Some(link) = self.credentials_link(&key) else {
            let page = &mut self.settings.credentials;
            match what {
                Change::Store => {
                    let label = page.form.label.text.trim().to_string();
                    let (namespace, kind) = (page.form.namespace.text.trim().to_string(), page.form.kind.text.trim().to_string());
                    let n = page.keys.len() + 1;
                    page.keys.push(c::Credential { id: format!("cred-{n}"), label, namespace, kind, hint: String::new() });
                }
                Change::Remove => page.keys.retain(|k| Some(&k.id) != id.as_ref()),
                Change::Update => {}
            }
            self.finish_change(what, Ok(Ok(self.settings.credentials.keys.clone())), id);
            return;
        };
        let (method, path, fallback) = (req.method, req.path.clone(), req.fallback);
        link.off_thread(
            move |base| {
                let req = super::skills::Request { method, path, fallback };
                c::change_then_read(base, &req, body.as_ref())
            },
            move |s: &mut State, _tx: &mut Tx, got: Changed| {
                if s.settings.credentials.request == request {
                    s.finish_change(what, got, id);
                }
            },
        );
    }

    /// What every answer does: a refusal stays on the form or card that asked; a change that worked takes the new list and
    /// closes it, and a list that wouldn't read says so where a load failure goes.
    fn finish_change(&mut self, what: Change, got: Changed, id: Option<String>) {
        let page = &mut self.settings.credentials;
        let (read, refused) = match got {
            Ok(read) => (Some(read), None),
            Err(why) => (None, Some(why)),
        };
        match what {
            Change::Store => page.form.saving = false,
            Change::Update => {
                if let Some(e) = page.editing.as_mut() {
                    e.updating = false;
                }
            }
            Change::Remove => page.deleting = None,
        }
        if let Some(why) = refused {
            match what {
                Change::Store => page.form.error = Some(why),
                _ => page.action_error = Some(why),
            }
            return;
        }
        match read {
            Some(Ok(keys)) => {
                page.keys = keys;
                page.load_error = None;
            }
            Some(Err(why)) => page.load_error = Some(why),
            None => {}
        }
        match what {
            Change::Store => page.close_form(),
            Change::Update => page.cancel_edit(),
            Change::Remove => {
                if page.editing.as_ref().map(|e| &e.id) == id.as_ref() {
                    page.cancel_edit();
                }
            }
        }
    }
}
