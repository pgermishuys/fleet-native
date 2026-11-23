//! Component 16, piece AO: Settings → API keys. Fleet has no test for `CredentialsSection.vue` but one: "API keys are read from
//! and saved to that machine" (`settings-follow-machine.test.ts`), ported under its name. The rest is written from the rows
//! (7.8 to 7.28): the quick-select chips, what the form asks before it sends, the bodies, the refusals, the reload after every
//! change, and that no secret is kept anywhere but its box. Every key and value here is invented.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::credentials::{self as c, AddForm, Credential};
use crate::tests::{FRAME, app_tall, click};
use crate::{Rail, State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    keys: Vec<Value>,
    /// Every call heard: method, path and body.
    heard: Vec<(String, String, Value)>,
    /// What a change (PUT or DELETE) answers instead of success.
    refuse: Option<(u16, String)>,
    /// What `GET /api/credentials` answers instead of the list.
    broken: Option<(u16, String)>,
    machines: Vec<Value>,
}

type Shared = Arc<Mutex<Fake>>;

fn key(id: &str, label: &str, hint: &str) -> Value {
    json!({ "id": id, "label": label, "namespace": "anthropic", "kind": "api-key", "displayHint": hint })
}

fn serve(fake: Shared) -> String {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let req = crate::test_http::read(&mut stream);
    let (method, path) = (req.method, req.path);
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
    let mut f = fake.lock().unwrap();
    if path.starts_with("/api/credentials") {
        f.heard.push((method.clone(), path.clone(), body.clone()));
    }
    let (status, reply) = match (method.as_str(), path.as_str()) {
        ("GET", "/api/credentials") => match f.broken.clone() {
            Some(b) => b,
            None => (200, Value::Array(f.keys.clone()).to_string()),
        },
        ("GET", "/api/machines") => (200, json!({ "machines": f.machines }).to_string()),
        (m, p) if p.starts_with("/api/credentials") && m != "GET" => match f.refuse.clone() {
            Some(b) => b,
            None => {
                if m == "PUT" && p == "/api/credentials" {
                    let n = f.keys.len() + 1;
                    let label = body["label"].as_str().unwrap_or_default().to_string();
                    f.keys.push(key(&format!("k{n}"), &label, "9999"));
                } else if m == "DELETE" {
                    let id = p.trim_start_matches("/api/credentials/").to_string();
                    f.keys.retain(|k| k["id"] != id.as_str());
                }
                (204, String::new())
            }
        },
        _ => (200, "{}".to_string()),
    };
    let out = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn link_to(base: String) -> (Link, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    (Link::start(base, Post::new(move |job| drop(jobs.lock().unwrap().send(job)))), queue)
}

fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let fake: Shared = Default::default();
    setup(&mut fake.lock().unwrap());
    let (link, queue) = link_to(serve(fake.clone()));
    let mut rt = app_tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.rail = Rail::Settings;
        s.open_settings("credentials", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.wait("the keys to be read", |s| !s.settings.credentials.loading && s.settings.credentials.loaded_for.is_some());
    r.settle();
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.credentials));
    }
    fn settle(&mut self) {
        for _ in 0..4 {
            self.rt.frame(FRAME);
        }
    }
    fn exists(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn text(&self, key: &str) -> String {
        let all = self.rt.elements();
        let it = all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        all.iter().filter(|e| e.path.starts_with(&it.path)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn press(&mut self, key: &str) {
        click(&mut self.rt, key);
        self.settle();
    }
    fn page(&self) -> &c::CredentialsPage {
        &self.rt.state.settings.credentials
    }
    /// Calls to `/api/credentials…` heard so far.
    fn heard(&self) -> Vec<(String, String, Value)> {
        self.fake.lock().unwrap().heard.clone()
    }
    fn changes(&self) -> Vec<(String, String, Value)> {
        self.heard().into_iter().filter(|(m, ..)| m != "GET").collect()
    }
    fn reads(&self) -> usize {
        self.heard().iter().filter(|(m, ..)| m == "GET").count()
    }
    fn fill(&mut self, label: &str, namespace: &str, kind: &str, value: &str) {
        let (label, namespace, kind, value) = (label.to_string(), namespace.to_string(), kind.to_string(), value.to_string());
        self.rt.update(move |s, _| {
            let f = &mut s.settings.credentials.form;
            f.label.replace_text(&label);
            f.namespace.replace_text(&namespace);
            f.kind.replace_text(&kind);
            f.value.replace_text(&value);
        });
        self.settle();
    }
    fn wait_saved(&mut self) {
        self.wait("the change", |s| {
            let p = &s.settings.credentials;
            !p.busy() && !p.loading
        });
        self.settle();
    }
}

fn two_keys(f: &mut Fake) {
    f.keys = vec![key("k1", "Build key", "0001"), key("k2", "Review key", "")];
}

// ─── settings-follow-machine.test.ts ────────────────────────────────────────────

#[test]
fn api_keys_are_read_from_and_saved_to_that_machine() {
    let home: Shared = Default::default();
    let other: Shared = Default::default();
    other.lock().unwrap().keys = vec![key("k1", "Harbor key", "7777")];
    let other_base = serve(other.clone());
    home.lock().unwrap().machines = vec![json!({ "id": "m-harbor", "name": "harbor", "baseUrl": other_base, "token": "t-harbor" })];
    let (link, queue) = link_to(serve(home.clone()));
    let mut rt = app_tall();
    rt.update(move |s, _| {
        s.show_settings_machines(true);
        s.link = Some(link);
        let machine = s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor");
        machine.base_url = other_base;
        machine.addresses.clear();
        s.rail = Rail::Settings;
        s.open_settings("credentials", None);
        s.settings.choice.pick("m-harbor");
    });
    let mut r = Rig { rt, queue, fake: home.clone() };
    r.wait("harbor's keys", |s| !s.settings.credentials.keys.is_empty());
    r.settle();
    assert_eq!(r.text("credential-k1"), "Harbor keyanthropic \u{b7} api-keyStored\u{2022}\u{2022}\u{2022}\u{2022} 7777Update keyRemove");
    assert!(other.lock().unwrap().heard.iter().any(|(m, p, _)| m == "GET" && p == "/api/credentials"), "harbor was asked");
    // Saving goes to harbor too.
    r.press("credential-add");
    r.fill("Spare key", "openai", "api-key", "test-0003");
    r.press("credential-store");
    r.wait_saved();
    assert!(other.lock().unwrap().heard.iter().any(|(m, ..)| m == "PUT"), "harbor was written");
    assert!(home.lock().unwrap().heard.is_empty(), "home was not asked at all");
}

// ─── The page ───────────────────────────────────────────────────────────────────

#[test]
fn shows_the_empty_state_with_the_add_button() {
    let r = rig(|_| {});
    assert_eq!(r.text("credentials-empty"), "No API keys storedAdd a credential to unlock provider-backed sessions and integrations.");
    assert!(r.text("credentials-card").contains("Credentials"));
    assert!(r.text("credentials-card").contains("Store encrypted provider credentials for sessions that require authenticated access."));
    assert_eq!(r.text("credential-add"), "Add API key");
    assert!(!r.exists("credential-form"));
}

#[test]
fn the_empty_state_hides_while_the_form_is_open_and_the_button_goes() {
    let mut r = rig(|_| {});
    r.press("credential-add");
    assert!(r.exists("credential-form"));
    assert!(!r.exists("credentials-empty"));
    assert!(!r.exists("credential-add"), "the button is shown only while the form is closed");
    assert!(r.text("credential-form").contains("The secret value is never returned after it is saved."));
}

#[test]
fn lists_each_key_with_its_hint_or_stored_securely_and_never_the_secret() {
    let r = rig(two_keys);
    assert_eq!(r.text("credential-k1"), "Build keyanthropic \u{b7} api-keyStored\u{2022}\u{2022}\u{2022}\u{2022} 0001Update keyRemove");
    assert_eq!(r.text("credential-hint-k1"), "\u{2022}\u{2022}\u{2022}\u{2022} 0001");
    assert_eq!(r.text("credential-hint-k2"), "Stored securely");
    assert!(!r.exists("credentials-empty"));
}

#[test]
fn reads_again_after_every_change_so_the_order_is_the_servers() {
    let mut r = rig(two_keys);
    assert_eq!(r.reads(), 1);
    r.press("credential-remove-k1");
    r.wait_saved();
    assert_eq!(r.reads(), 2, "the list is read again after a removal");
    assert_eq!(r.page().keys.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["k2"]);
}

// ─── Quick select and the form ──────────────────────────────────────────────────

#[test]
fn a_chip_fills_provider_and_type_and_the_label_only_when_it_is_empty() {
    let mut r = rig(|_| {});
    r.press("credential-add");
    assert_eq!(r.page().form.kind.text, "api-key", "Type starts as api-key");
    r.press("credential-preset-anthropic");
    let f = &r.page().form;
    assert_eq!((f.namespace.text.as_str(), f.kind.text.as_str(), f.label.text.as_str()), ("anthropic", "api-key", "My Anthropic API Key"));
    r.press("credential-preset-openai");
    let f = &r.page().form;
    assert_eq!(f.namespace.text, "openai");
    assert_eq!(f.label.text, "My Anthropic API Key", "a label already there stays");
    r.rt.update(|s, _| s.settings.credentials.form.label.replace_text("  "));
    r.press("credential-preset-openai");
    assert_eq!(r.page().form.label.text, "My OpenAI API Key", "spaces count as empty");
}

#[test]
fn every_field_is_required_and_nothing_is_sent_without_them() {
    let mut r = rig(|_| {});
    r.press("credential-add");
    r.press("credential-store");
    assert_eq!(r.text("credential-form-error"), "All fields are required.");
    for blank in 0..4 {
        let mut vals = ["Label", "anthropic", "api-key", "test-0001"];
        vals[blank] = "   ";
        r.fill(vals[0], vals[1], vals[2], vals[3]);
        r.press("credential-store");
        assert_eq!(r.page().form.error.as_deref(), Some(c::ALL_REQUIRED), "field {blank} empty");
    }
    assert!(r.changes().is_empty(), "no request left the app");
    assert!(r.page().form_open);
}

#[test]
fn saving_sends_the_four_trimmed_fields_without_metadata_then_reads_again_and_closes() {
    let mut r = rig(|_| {});
    r.press("credential-add");
    r.fill("  Build key ", " anthropic ", " api-key ", "  test-0001 ");
    r.press("credential-store");
    r.wait_saved();
    let changes = r.changes();
    assert_eq!(changes.len(), 1);
    assert_eq!((changes[0].0.as_str(), changes[0].1.as_str()), ("PUT", "/api/credentials"));
    assert_eq!(changes[0].2, json!({ "label": "Build key", "namespace": "anthropic", "kind": "api-key", "value": "test-0001" }));
    assert_eq!(r.reads(), 2, "the list is read again");
    assert!(!r.exists("credential-form"), "the form closes");
    assert!(r.exists("credential-k1"), "the new key is in the list");
    assert!(r.page().form.value.text.is_empty() && r.page().form.label.text.is_empty(), "every field was cleared");
}

#[test]
fn enter_in_a_field_submits_the_form() {
    let mut r = rig(|_| {});
    r.press("credential-add");
    r.fill("Build key", "anthropic", "api-key", "test-0001");
    click(&mut r.rt, "credential-label");
    r.rt.key(KeyEvent { key: Key::Enter, mods: Mods::default() });
    r.settle();
    r.wait_saved();
    assert_eq!(r.changes().len(), 1);
}

#[test]
fn a_refused_save_keeps_the_form_open_with_the_servers_words_or_ours() {
    let mut r = rig(|f| f.refuse = Some((400, json!({ "error": "Label is taken." }).to_string())));
    r.press("credential-add");
    r.fill("Build key", "anthropic", "api-key", "test-0001");
    r.press("credential-store");
    r.wait_saved();
    assert_eq!(r.text("credential-form-error"), "Label is taken.");
    assert!(r.page().form_open);
    assert_eq!(r.page().form.label.text, "Build key", "what was typed stays");
    r.fake.lock().unwrap().refuse = Some((500, String::new()));
    r.press("credential-store");
    r.wait_saved();
    assert_eq!(r.text("credential-form-error"), "Failed to save API key.");
}

#[test]
fn cancel_closes_the_form_and_clears_every_field_and_the_error() {
    let mut r = rig(|_| {});
    r.press("credential-add");
    r.fill("Build key", "openai", "token", "test-0001");
    r.press("credential-store"); // refused by nothing: this one saves; open it again for the cancel case
    r.wait_saved();
    r.press("credential-add");
    r.fill("Build key", "openai", "token", "");
    r.press("credential-store");
    assert!(r.exists("credential-form-error"));
    r.press("credential-form-cancel");
    assert!(!r.exists("credential-form"));
    let f = &r.page().form;
    assert!(f.label.text.is_empty() && f.namespace.text.is_empty() && f.value.text.is_empty() && f.error.is_none());
    assert_eq!(f.kind.text, "api-key", "Type goes back to its start");
    r.press("credential-add");
    assert!(!r.exists("credential-form-error"));
}

// ─── Update key and Remove ──────────────────────────────────────────────────────

#[test]
fn update_key_opens_one_edit_at_a_time_and_cancel_wipes_the_value() {
    let mut r = rig(two_keys);
    r.press("credential-update-k1");
    assert!(r.exists("credential-edit"));
    assert!(!r.exists("credential-update-k1"), "the card's own buttons give way to the edit");
    assert!(r.exists("credential-update-k2"));
    r.rt.update(|s, _| s.settings.credentials.editing.as_mut().unwrap().value.replace_text("test-0002"));
    r.press("credential-update-k2");
    assert_eq!(r.page().editing.as_ref().unwrap().id, "k2", "another edit replaces it");
    assert!(r.page().editing.as_ref().unwrap().value.text.is_empty(), "and starts empty");
    r.press("credential-cancel");
    assert!(r.page().editing.is_none());
    assert!(r.changes().is_empty());
}

#[test]
fn an_empty_new_value_is_refused_in_that_card_and_nothing_is_sent() {
    let mut r = rig(two_keys);
    r.press("credential-update-k1");
    r.press("credential-save");
    assert_eq!(r.text("credential-error"), "API key value is required.");
    r.rt.update(|s, _| s.settings.credentials.editing.as_mut().unwrap().value.replace_text("   "));
    r.press("credential-save");
    assert_eq!(r.page().action_error.as_deref(), Some(c::VALUE_REQUIRED));
    assert!(r.changes().is_empty());
}

#[test]
fn saving_a_new_value_puts_it_trimmed_to_that_key_reads_again_and_closes_the_edit() {
    let mut r = rig(two_keys);
    r.press("credential-update-k2");
    r.rt.update(|s, _| s.settings.credentials.editing.as_mut().unwrap().value.replace_text("  test-0002 "));
    r.press("credential-save");
    r.wait_saved();
    let changes = r.changes();
    assert_eq!(changes.len(), 1);
    assert_eq!((changes[0].0.as_str(), changes[0].1.as_str()), ("PUT", "/api/credentials/k2"));
    assert_eq!(changes[0].2, json!({ "value": "test-0002" }));
    assert_eq!(r.reads(), 2);
    assert!(r.page().editing.is_none() && !r.exists("credential-edit"));
}

#[test]
fn a_refused_update_stays_open_with_the_message() {
    let mut r = rig(|f| {
        two_keys(f);
        f.refuse = Some((422, json!({ "message": "That value isn't valid." }).to_string()));
    });
    r.press("credential-update-k1");
    r.rt.update(|s, _| s.settings.credentials.editing.as_mut().unwrap().value.replace_text("test-0002"));
    r.press("credential-save");
    r.wait_saved();
    assert_eq!(r.text("credential-error"), "That value isn't valid.");
    assert!(r.page().editing.is_some());
    r.fake.lock().unwrap().refuse = Some((500, String::new()));
    r.press("credential-save");
    r.wait_saved();
    assert_eq!(r.text("credential-error"), "Failed to update API key.");
}

#[test]
fn remove_asks_nothing_deletes_reads_again_and_closes_that_keys_edit() {
    let mut r = rig(two_keys);
    r.press("credential-update-k1");
    r.press("credential-remove-k2");
    r.wait_saved();
    let changes = r.changes();
    assert_eq!((changes[0].0.as_str(), changes[0].1.as_str()), ("DELETE", "/api/credentials/k2"));
    assert!(r.page().editing.is_some(), "another key's edit stays");
    assert!(!r.exists("credential-k2"));
    // The edit's own key goes: its edit closes with it.
    r.rt.update(|s, _| s.settings.credentials.cancel_edit());
    r.settle();
    r.press("credential-update-k1");
    r.rt.update(|s, _| s.remove_credential("k1"));
    r.wait_saved();
    assert!(r.page().editing.is_none());
    assert!(r.exists("credentials-empty"));
}

#[test]
fn a_refused_removal_says_so_under_the_grid_or_in_the_editing_card() {
    let mut r = rig(|f| {
        two_keys(f);
        f.refuse = Some((409, json!({ "error": "In use." }).to_string()));
    });
    r.press("credential-remove-k1");
    r.wait_saved();
    assert_eq!(r.text("credential-error"), "In use.");
    assert!(r.exists("credential-k1"), "nothing is removed before the server says so");
    r.fake.lock().unwrap().refuse = Some((500, String::new()));
    r.press("credential-remove-k2");
    r.wait_saved();
    assert_eq!(r.text("credential-error"), "Failed to remove API key.");
    r.press("credential-update-k1");
    assert!(!r.exists("credential-error"), "opening an edit clears the last failure");
}

// ─── Loading and failing ────────────────────────────────────────────────────────

#[test]
fn a_refused_list_shows_the_servers_words_or_ours_and_an_empty_answer_says_so() {
    let r = rig(|f| f.broken = Some((403, json!({ "error": "Not allowed." }).to_string())));
    assert_eq!(r.text("credentials-load-error"), "Not allowed.");
    let r = rig(|f| f.broken = Some((500, String::new())));
    assert_eq!(r.text("credentials-load-error"), "Failed to load API keys.");
    let r = rig(|f| f.broken = Some((200, String::new())));
    assert_eq!(r.text("credentials-load-error"), "No data returned");
}

#[test]
fn reads_again_when_the_page_is_left_and_forgets_what_was_typed() {
    let mut r = rig(two_keys);
    r.press("credential-add");
    r.fill("Build key", "anthropic", "api-key", "test-0001");
    r.rt.update(|s, _| s.open_settings("appearance", None));
    r.settle();
    assert!(r.page().loaded_for.is_none() && !r.page().form_open && r.page().form.value.text.is_empty());
    r.rt.update(|s, _| s.open_settings("credentials", None));
    r.wait("the keys again", |s| !s.settings.credentials.loading && !s.settings.credentials.keys.is_empty());
    assert_eq!(r.reads(), 2);
    assert!(!r.exists("credential-form"));
}

#[test]
fn a_secret_is_never_in_what_the_state_prints() {
    let mut r = rig(two_keys);
    r.press("credential-add");
    r.fill("Build key", "anthropic", "api-key", "test-SECRET-0001");
    r.press("credential-update-k1");
    r.rt.update(|s, _| s.settings.credentials.editing.as_mut().unwrap().value.replace_text("test-SECRET-0002"));
    let printed = format!("{:?}", r.page());
    assert!(!printed.contains("SECRET"), "{printed}");
}

// ─── The model ──────────────────────────────────────────────────────────────────

#[test]
fn reads_the_list_the_server_sends() {
    let body =
        json!([{ "id": "k1", "label": "Build key", "namespace": "openai", "kind": "api-key", "displayHint": "0001", "createdAt": "x" }]);
    let keys = c::parse_list(&body).unwrap();
    assert_eq!(
        keys,
        vec![Credential {
            id: "k1".into(),
            label: "Build key".into(),
            namespace: "openai".into(),
            kind: "api-key".into(),
            hint: "0001".into()
        }]
    );
    assert_eq!(keys[0].line(), "openai \u{b7} api-key");
    assert_eq!(c::parse_list(&Value::Null), Err("No data returned".into()));
    assert_eq!(c::parse_list(&json!([])).unwrap(), vec![]);
}

#[test]
fn a_hint_of_only_spaces_reads_as_stored_securely() {
    let mut k = Credential { id: "k".into(), label: "l".into(), namespace: "n".into(), kind: "t".into(), hint: "  ".into() };
    assert_eq!(k.hint_text(), "Stored securely");
    k.hint = "abcd".into();
    assert_eq!(k.hint_text(), "\u{2022}\u{2022}\u{2022}\u{2022} abcd");
}

#[test]
fn the_form_body_is_trimmed_and_all_four_are_needed() {
    let mut f = AddForm::default();
    assert_eq!(f.body(), Err(c::ALL_REQUIRED));
    f.label.replace_text(" a ");
    f.namespace.replace_text(" b ");
    f.value.replace_text(" v ");
    assert_eq!(f.body().unwrap(), json!({ "label": "a", "namespace": "b", "kind": "api-key", "value": "v" }));
}
