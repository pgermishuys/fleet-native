//! Component 16, piece K: `HarnessSignInPanel.test.ts`'s cases marked "K" under their names, against a fake Fleet that answers the
//! harness list, the preferences and the sign-in requests, and records every write. The key form and the browser flow are L's.
use crate::fleet::link::{Job, Link, Post};
use crate::tests::FRAME;
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

pub(crate) struct Fake {
    pub harness: &'static str,
    pub name: &'static str,
    pub enabled: bool,
    pub providers: Vec<Value>,
    pub note: Option<&'static str>,
    /// The first this many reads of the providers are refused with this.
    pub refuse_reads: usize,
    pub refuse_write: Option<&'static str>,
    pub writes: Vec<(String, String)>,
    pub reads: usize,
    pub fresh_reads: usize,
    /// The flow's side: what a started browser sign-in answers, what its status says in turn (the last one stays), what a key
    /// sign-in is refused with, and the bodies that were sent (never a real key: invented ones).
    pub attempt: Option<Value>,
    pub statuses: Vec<Value>,
    pub refuse_key: Option<&'static str>,
    pub bodies: Vec<Value>,
}

impl Default for Fake {
    fn default() -> Fake {
        Fake {
            harness: "opencode2",
            name: "OpenCode 2",
            enabled: true,
            providers: anthropic_azure_openai(),
            note: Some("These sign-ins belong to Fleet's separate OpenCode 2 install and are kept in its own database."),
            refuse_reads: 0,
            refuse_write: None,
            writes: Vec::new(),
            reads: 0,
            fresh_reads: 0,
            attempt: None,
            statuses: Vec::new(),
            refuse_key: None,
            bodies: Vec::new(),
        }
    }
}
pub(crate) type Shared = Arc<Mutex<Fake>>;

/// A provider signed in twice and through the environment, and two to sign in to.
pub(crate) fn anthropic_azure_openai() -> Vec<Value> {
    vec![
        json!({"id": "anthropic", "name": "Anthropic", "methods": [{"type": "key", "label": "API key", "fields": []}], "connections": [
            {"kind": "credential", "id": "cred_work", "label": "Work", "active": true},
            {"kind": "credential", "id": "cred_home", "label": "Personal", "active": false},
            {"kind": "env", "id": "EXAMPLE_API_KEY", "label": "EXAMPLE_API_KEY", "active": false}]}),
        json!({"id": "azure", "name": "Azure", "methods": [{"type": "key", "label": "API key", "fields": []}], "connections": []}),
        json!({"id": "openai", "name": "OpenAI", "methods": [{"type": "key", "label": "API key", "fields": []}], "connections": []}),
    ]
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
    if req.method.is_empty() {
        return;
    }
    let sent = req.json();
    let (method, path) = (req.method, req.path);
    let mut f = fake.lock().unwrap();
    let base = format!("/api/harnesses/{}/sign-in", f.harness);
    let (status, reply) = if method == "GET" && path.starts_with("/api/harnesses") && !path.contains("sign-in") {
        if path.contains("fresh") {
            f.fresh_reads += 1;
        }
        let list = json!([{
            "type": f.harness, "displayName": f.name, "available": true, "userEnabled": f.enabled, "state": "ready",
            "capabilities": {"supportsProviderSignIn": true, "supportsPermissionLevels": true},
            "presentation": {"eyebrow": "CLI harness", "description": "The runtime."}, "settings": []
        }]);
        ("200 OK", list)
    } else if method == "GET" && path == base {
        f.reads += 1;
        if f.refuse_reads > 0 {
            f.refuse_reads -= 1;
            ("500 Internal Server Error", json!({"error": "OpenCode 2 isn't answering."}))
        } else {
            ("200 OK", json!({"providers": f.providers, "note": f.note}))
        }
    } else if let Some(rest) = path.strip_prefix(&format!("{base}/connections/")) {
        f.writes.push((method.clone(), path.clone()));
        if let Some(why) = f.refuse_write {
            ("400 Bad Request", json!({"error": why}))
        } else {
            let (id, using) = match rest.strip_suffix("/use") {
                Some(id) => (id.to_string(), true),
                None => (rest.to_string(), false),
            };
            for p in &mut f.providers {
                let connections = p["connections"].as_array_mut().unwrap();
                if using && connections.iter().any(|c| c["id"] == id) {
                    connections.iter_mut().for_each(|c| c["active"] = json!(c["id"] == id));
                } else if !using {
                    connections.retain(|c| c["id"] != id);
                }
            }
            ("200 OK", json!({}))
        }
    } else if let Some(rest) = path.strip_prefix(&format!("{base}/")).filter(|r| r.contains('/')) {
        // /{provider}/key, /{provider}/attempts, /{provider}/attempts/{id}[/code|/callback]
        f.writes.push((method.clone(), path.clone()));
        f.bodies.push(sent);
        let (provider, tail) = rest.split_once('/').unwrap_or((rest, ""));
        let provider = provider.to_string();
        if tail == "key" {
            if let Some(why) = f.refuse_key {
                ("400 Bad Request", json!({"error": why}))
            } else {
                for p in &mut f.providers {
                    if p["id"] == provider {
                        p["connections"] = json!([{"kind": "credential", "id": "cred_new", "label": provider, "active": true}]);
                    }
                }
                ("200 OK", json!({}))
            }
        } else if tail == "attempts" {
            ("200 OK", f.attempt.clone().unwrap_or(json!({})))
        } else if method == "GET" {
            let status = if f.statuses.len() > 1 {
                f.statuses.remove(0)
            } else {
                f.statuses.first().cloned().unwrap_or(json!({"status": "pending"}))
            };
            if status["status"] == "complete" {
                for p in &mut f.providers {
                    if p["id"] == provider {
                        p["connections"] = json!([{"kind": "credential", "id": "cred_new", "label": provider, "active": true}]);
                    }
                }
            }
            ("200 OK", status)
        } else {
            ("200 OK", json!({}))
        }
    } else if path == "/api/weave" {
        // Weave asks nothing of these pages: an empty answer draws no Weave group (the rigs wait for it, so the layout is still).
        ("200 OK", json!({"source": "own", "files": {}, "harnesses": []}))
    } else if path == "/api/agent-browser/settings" {
        ("200 OK", json!({"enabled": true}))
    } else {
        ("200 OK", json!({}))
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

pub(crate) struct Rig {
    pub rt: Runtime<State>,
    pub queue: Receiver<Job>,
    pub fake: Shared,
}

pub(crate) fn rig(fake: Fake) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(fake));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = crate::tests::app_tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.rail = Rail::Settings;
        s.settings.set("harnesses", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.rt.frame(FRAME);
    // The page also reads the preferences; one that lands after a test sets a switch would replace it, so wait for it too.
    r.wait("the harness list and the preferences", |s| {
        s.harness_list().all.len() == 1 && s.pickers.prefs.fetched && s.settings.harnesses.weave.view.is_some()
    });
    r
}

impl Rig {
    pub fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        let fake = self.fake.clone();
        test_wait::drive(&mut self.rt, &self.queue, what, done, move |s| {
            format!("{:?}; writes {:?}", s.settings.harnesses.sign_in, fake.lock().map(|f| f.writes.clone()))
        });
        self.frames();
    }
    pub fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    pub fn has(&self, key: &str) -> bool {
        self.rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    pub fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone().or_else(|| e.label.clone())).collect::<Vec<_>>().join("\n")
    }
    pub fn text_of(&self, key: &str) -> String {
        self.rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.text.or(e.label).unwrap_or_default())
            .unwrap_or_default()
    }
    pub fn click(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.frames();
    }
    pub fn listed(&mut self) {
        let kind = self.fake.lock().unwrap().harness;
        self.wait("the providers", move |s| s.sign_ins(kind).is_some());
    }
}

#[test]
fn has_no_provider_search_for_a_harness_with_one_provider_signed_in_or_not() {
    let github = |connections: Value| json!({"id": "github-copilot", "name": "GitHub Copilot", "methods": [{"type": "oauth", "label": "Device code", "fields": []}], "connections": connections});
    let signed_in = json!([{"kind": "credential", "id": "gh", "label": "octo-dev", "active": true}]);
    let mut r = rig(Fake { harness: "copilot", name: "GitHub Copilot", providers: vec![github(signed_in)], note: None, ..Fake::default() });
    r.listed();
    assert!(!r.has("harness-sign-in-search"));
    assert!(!r.words().contains("No provider matches"));
    assert!(!r.words().contains("Sign in to a provider"));

    let mut r = rig(Fake { harness: "copilot", name: "GitHub Copilot", providers: vec![github(json!([]))], note: None, ..Fake::default() });
    r.listed();
    assert!(!r.has("harness-sign-in-search"));
    assert!(r.words().contains("Not signed in yet."));
    assert!(r.has("harness-sign-in-open-github-copilot"));
}

#[test]
fn shows_the_signed_in_providers_with_the_sign_in_each_uses() {
    let mut r = rig(Fake::default());
    r.listed();
    assert!(r.text_of("harness-sign-in-note").contains("separate OpenCode 2 install"));
    assert!(r.has("harness-sign-in-inuse-cred_work"));
    assert!(!r.has("harness-sign-in-use-cred_work"), "the one in use has no Use this one");
    assert!(r.has("harness-sign-in-use-cred_home"));
    assert!(r.words().contains("from Fleet's environment"));
    assert!(!r.has("harness-sign-out-EXAMPLE_API_KEY"), "the environment is advice, not a sign-in to sign out of");
    assert!(!r.has("harness-sign-in-use-EXAMPLE_API_KEY"));
    assert_eq!(r.text_of("harness-sign-in-summary-anthropic"), "Signed in");
    // Azure and OpenAI are to sign in to, signed-in Anthropic is not offered again.
    assert!(r.has("harness-sign-in-open-azure") && r.has("harness-sign-in-open-openai"));
    assert!(r.has("harness-sign-in-open-anthropic"), "its own button says Add a sign-in");
    assert_eq!(r.text_of("harness-sign-in-open-anthropic"), "Add a sign-in");
}

#[test]
fn waits_for_a_click_before_asking_a_harness_that_isn_t_turned_on() {
    let mut r = rig(Fake { enabled: false, ..Fake::default() });
    r.frames();
    assert_eq!(r.fake.lock().unwrap().reads, 0, "listing starts the harness, so it waits");
    assert!(r.words().contains("Sign in to the providers OpenCode 2 uses, with a key or in the browser."));
    // With no harness on, Fleet's first-run setup opens over the page; the user has dealt with it.
    r.rt.update(|s, _| {
        s.harness_setup.close();
        s.open_harness("opencode2", Some("providers"))
    });
    r.frames();
    r.click("harness-sign-in-show");
    r.listed();
    assert_eq!(r.fake.lock().unwrap().reads, 1);
    assert!(r.has("harness-sign-in-provider-azure"));
    assert!(!r.has("harness-sign-in-show"));
}

#[test]
fn asks_when_the_harness_is_turned_on_after_the_panel_showed() {
    let mut r = rig(Fake { enabled: false, ..Fake::default() });
    r.frames();
    assert_eq!(r.fake.lock().unwrap().reads, 0);
    r.rt.update(|s, _| drop(s.pickers.prefs.set("opencode2.enabled", "true")));
    r.listed();
    assert_eq!(r.fake.lock().unwrap().reads, 1, "asked once, not on every frame");
}

#[test]
fn says_it_is_asking_until_the_harness_answers() {
    let mut r = rig(Fake::default());
    r.rt.frame(FRAME);
    // The list may have come back already; a refusal keeps the line for a moment too.
    r.listed();
    assert!(!r.has("harness-sign-in-asking"), "no asking line once the providers are listed");
}

#[test]
fn finds_a_provider_to_sign_in_to() {
    let mut r = rig(Fake::default());
    r.listed();
    r.rt.update(|s, _| s.settings.harnesses.sign_in.query.replace_text("azu"));
    r.frames();
    assert!(r.has("harness-sign-in-provider-azure"));
    assert!(!r.has("harness-sign-in-provider-openai"));
    // The id matches too, and the case doesn't matter.
    r.rt.update(|s, _| s.settings.harnesses.sign_in.query.replace_text("  OPENAI "));
    r.frames();
    assert!(r.has("harness-sign-in-provider-openai") && !r.has("harness-sign-in-provider-azure"));
}

#[test]
fn says_when_nothing_matches_and_when_every_provider_is_signed_in() {
    let mut r = rig(Fake::default());
    r.listed();
    r.rt.update(|s, _| s.settings.harnesses.sign_in.query.replace_text("zzz"));
    r.frames();
    assert_eq!(r.text_of("harness-sign-in-empty"), "No provider matches \u{201c}zzz\u{201d}.");
    let mut both = anthropic_azure_openai();
    both[1]["connections"] = json!([{"kind": "credential", "id": "cred_azure", "label": "Azure", "active": true}]);
    both.truncate(2);
    let mut r = rig(Fake { providers: both, ..Fake::default() });
    r.listed();
    assert!(r.words().contains("Every provider is signed in."), "{}", r.words());
}

#[test]
fn signs_out_after_asking_once_more() {
    let mut r = rig(Fake::default());
    r.listed();
    r.click("harness-sign-out-cred_home");
    assert!(r.fake.lock().unwrap().writes.is_empty(), "the first press only asks");
    assert_eq!(r.text_of("harness-sign-out-cred_home"), "Sign out?");
    r.click("harness-sign-out-cred_home");
    r.wait("the notice", |s| s.settings.harnesses.sign_in.notice.is_some());
    assert_eq!(
        r.fake.lock().unwrap().writes,
        [("DELETE".to_string(), "/api/harnesses/opencode2/sign-in/connections/cred_home".to_string())]
    );
    assert_eq!(r.text_of("harness-sign-in-notice"), "Signed out of Personal on Anthropic.");
    assert!(!r.has("harness-sign-in-connection-cred_home"), "the list is read again");
}

#[test]
fn switches_to_another_sign_in_and_reads_the_harnesses_and_providers_again() {
    let mut r = rig(Fake::default());
    r.listed();
    let (reads, fresh) = {
        let f = r.fake.lock().unwrap();
        (f.reads, f.fresh_reads)
    };
    r.click("harness-sign-in-use-cred_home");
    r.wait("the notice", |s| s.settings.harnesses.sign_in.notice.is_some());
    assert_eq!(
        r.fake.lock().unwrap().writes[0],
        ("POST".to_string(), "/api/harnesses/opencode2/sign-in/connections/cred_home/use".to_string())
    );
    assert_eq!(r.text_of("harness-sign-in-notice"), "Anthropic now uses Personal.");
    assert!(r.has("harness-sign-in-inuse-cred_home") && !r.has("harness-sign-in-inuse-cred_work"));
    // The harness list is read first and the providers second, but the fake serves each connection on its own thread: the
    // providers' answer can be in before the fake has counted the harness read, so wait for it.
    let seen = r.fake.clone();
    test_wait::until_true("the fake to have heard the harnesses read again", move || seen.lock().unwrap().fresh_reads > fresh);
    assert_eq!(r.fake.lock().unwrap().reads, reads + 1, "the providers");
}

#[test]
fn says_the_harness_s_reason_when_a_switch_or_a_sign_out_is_refused() {
    let mut r = rig(Fake { refuse_write: Some("That sign-in is locked."), ..Fake::default() });
    r.listed();
    r.click("harness-sign-in-use-cred_home");
    r.wait("the refusal", |s| s.settings.harnesses.sign_in.error.is_some());
    assert_eq!(r.text_of("harness-sign-in-error"), "That sign-in is locked.");
    assert!(r.has("harness-sign-in-inuse-cred_work"), "the list is as it was");
    assert!(r.rt.state.settings.harnesses.sign_in.working.is_none());
    assert!(!r.has("harness-sign-in-retry"), "Try again is for a list that couldn't be read");
}

#[test]
fn says_a_list_that_could_not_be_read_and_tries_again() {
    let mut r = rig(Fake { refuse_reads: 1, ..Fake::default() });
    r.wait("the refusal", |s| s.settings.harnesses.sign_in.error.is_some());
    assert_eq!(r.text_of("harness-sign-in-error"), "OpenCode 2 isn't answering.");
    r.click("harness-sign-in-retry");
    r.listed();
    assert!(!r.has("harness-sign-in-error"));
    assert_eq!(r.fake.lock().unwrap().reads, 2);
}

#[test]
fn after_a_sign_in_is_done_the_flow_closes_the_search_clears_and_the_list_is_read_again() {
    let mut r = rig(Fake::default());
    r.listed();
    r.click("harness-sign-in-open-azure");
    assert_eq!(r.rt.state.settings.harnesses.sign_in.open.as_deref(), Some("azure"));
    assert_eq!(r.text_of("harness-sign-in-open-azure"), "Close");
    r.rt.update(|s, _| s.settings.harnesses.sign_in.query.replace_text("azu"));
    r.fake.lock().unwrap().providers[1]["connections"] =
        json!([{"kind": "credential", "id": "cred_azure", "label": "Azure", "active": true}]);
    r.rt.update(|s, _| s.sign_in_done("opencode2", "OpenCode 2", "Signed in to Azure."));
    r.wait("the list again", |s| s.sign_ins("opencode2").is_some_and(|l| l.signed_in().len() == 2));
    assert_eq!(r.text_of("harness-sign-in-notice"), "Signed in to Azure. New sessions can use its models.");
    assert!(r.has("harness-sign-in-inuse-cred_azure"));
    assert!(r.rt.state.settings.harnesses.sign_in.open.is_none());
    assert!(r.rt.state.settings.harnesses.sign_in.query.text.is_empty());
}

#[test]
fn a_provider_s_sign_in_button_opens_and_closes_its_flow_and_clears_the_notice() {
    let mut r = rig(Fake::default());
    r.listed();
    r.rt.update(|s, _| s.settings.harnesses.sign_in.notice = Some("Something.".into()));
    r.click("harness-sign-in-open-anthropic");
    assert_eq!(r.rt.state.settings.harnesses.sign_in.open.as_deref(), Some("anthropic"));
    assert!(r.rt.state.settings.harnesses.sign_in.notice.is_none());
    r.click("harness-sign-in-open-anthropic");
    assert!(r.rt.state.settings.harnesses.sign_in.open.is_none());
}

#[test]
fn another_harness_s_tab_is_its_own_panel_and_the_providers_stay_read() {
    let mut r = rig(Fake::default());
    r.listed();
    r.click("harness-sign-in-open-azure");
    assert!(r.rt.state.sign_in_panel("opencode2").is_some());
    r.rt.update(|s, _| s.toggle_sign_in("copilot", "github-copilot"));
    assert!(r.rt.state.sign_in_panel("opencode2").is_none(), "its fields start again");
    assert!(r.rt.state.sign_ins("opencode2").is_some(), "the list it read stays");
    assert!(r.rt.state.sign_ins("copilot").is_none());
}
