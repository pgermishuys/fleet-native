//! Component 22, piece H: the draft cards. `lib/mods/__tests__/kept-api.test.ts` and `stores/__tests__/mods.test.ts` (their draft
//! cases), `ModDraftCards.test.ts` (4) and `ModDraftCard.test.ts` (14), under Fleet's names, against a fake Fleet that answers the
//! Mods switch and the draft calls. Fleet's tests read the DOM; these read what the cards draw.
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::mods::drafts::{self, ModDraft};
use crate::fleet::state::Topic;
use crate::tests::{FRAME, app};
use crate::{State, test_http, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Rt = Runtime<State>;

fn draft(name: &str, extra: Value) -> Value {
    let mut d =
        json!({ "sessionId": "s-1", "name": name, "description": "Test runs as counts", "version": "1", "off": null, "kept": null });
    for (k, v) in extra.as_object().into_iter().flatten() {
        d[k] = v.clone();
    }
    d
}

struct Fake {
    switch: Value,
    /// What `GET …/drafts` answers, and with which status.
    drafts: Value,
    drafts_status: &'static str,
    /// What a POST on/off answers (status line and body).
    toggle: (&'static str, Value),
    check: Value,
    keep: (&'static str, Value),
    dismiss: &'static str,
    stop: bool,
    calls: Vec<(String, String, Value, bool)>,
}

impl Default for Fake {
    fn default() -> Self {
        Fake {
            switch: json!({ "on": true, "safeMode": false }),
            drafts: json!([]),
            drafts_status: "200 OK",
            toggle: ("200 OK", json!({})),
            check: json!({ "check": null }),
            keep: ("200 OK", json!({ "name": "test-chips" })),
            dismiss: "204 No Content",
            stop: false,
            calls: Vec::new(),
        }
    }
}

type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> (String, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let addr = listener.local_addr().expect("an address");
    let base = format!("http://{addr}");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if fake.lock().unwrap().stop {
                break;
            }
            let fake = fake.clone();
            std::thread::spawn(move || {
                let req = test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let mut f = fake.lock().unwrap();
                f.calls.push((req.method.clone(), req.path.clone(), req.json(), req.body.is_empty()));
                let (status, reply) = if req.path == "/api/features/mods" {
                    ("200 OK", f.switch.clone())
                } else if req.method == "DELETE" {
                    (f.dismiss, Value::Null)
                } else if req.path.ends_with("/keep") {
                    f.keep.clone()
                } else if req.path.ends_with("/check") {
                    ("200 OK", f.check.clone())
                } else if req.path.ends_with("/on") || req.path.ends_with("/off") {
                    f.toggle.clone()
                } else if req.path.ends_with("/mods/drafts") {
                    (f.drafts_status, f.drafts.clone())
                } else {
                    ("200 OK", json!({}))
                };
                drop(f);
                let body = if reply.is_null() { String::new() } else { reply.to_string() };
                test_http::respond_json(&mut stream, status, &body);
            });
        }
    });
    (base, addr)
}

struct Rig {
    rt: Rt,
    queue: Receiver<Job>,
    fake: Shared,
    addr: std::net::SocketAddr,
    base: String,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.fake.lock().unwrap().stop = true;
        let _ = std::net::TcpStream::connect(self.addr);
    }
}

/// An app on the open session `s-1` of a fake Fleet.
fn rig(setup: impl FnOnce(&mut Fake)) -> Rig {
    let mut f = Fake::default();
    setup(&mut f);
    let fake: Shared = Arc::new(Mutex::new(f));
    let (base, addr) = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base.clone(),
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.projects[0].sessions[0].id = "s-1".into();
        s.route = crate::fleet::store::Route::Session("s-1".into());
    });
    let mut r = Rig { rt, queue, fake, addr, base };
    r.settle();
    r
}

impl Rig {
    fn settle(&mut self) {
        for _ in 0..3 {
            test_wait::drive(&mut self.rt, &self.queue, "every request to be answered", |s| s.mod_drafts.idle(), |_| String::new());
            for _ in 0..4 {
                self.rt.frame(FRAME);
            }
        }
    }

    fn words(&self) -> String {
        self.rt.elements().iter().filter(|e| !e.exiting).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }

    fn has(&self, key: &str) -> bool {
        self.rt.find(key).is_some()
    }

    fn text_of(&self, key: &str) -> String {
        self.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.clone()).unwrap_or_default()
    }

    fn calls(&self, method: &str, suffix: &str) -> Vec<(String, Value, bool)> {
        let f = self.fake.lock().unwrap();
        f.calls.iter().filter(|c| c.0 == method && c.1.ends_with(suffix)).map(|c| (c.1.clone(), c.2.clone(), c.3)).collect()
    }

    fn with(&self, change: impl FnOnce(&mut Fake)) {
        change(&mut self.fake.lock().unwrap());
    }

    fn press(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.settle();
    }

    fn list(&self, session: &str) -> Vec<ModDraft> {
        self.rt.state.mod_drafts.list(session).to_vec()
    }

    fn emit(&mut self, payload: Value) {
        self.rt.update(move |s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::ModsChanged, payload)]));
        self.settle();
    }
}

fn parsed(v: Value) -> ModDraft {
    drafts::parse_draft(&v, "s-1").expect("a draft")
}

fn card(r: &mut Rig, extra: Value) {
    let d = parsed(draft("test-chips", extra));
    r.rt.update(move |s, _| s.mod_drafts.set("s-1", vec![d]));
    r.settle();
}

// ---- kept-api.test.ts: the draft cases ----

// "fetchDrafts reads a session's drafts from its machine"
#[test]
fn fetch_drafts_reads_a_session_s_drafts_from_its_machine() {
    let r = rig(|f| f.drafts = json!([{ "sessionId": "s-1", "name": "test-chips" }]));
    let got = drafts::fetch_drafts(&r.base, "s-1").expect("drafts");
    assert_eq!(got.len(), 1);
    assert_eq!(r.calls("GET", "/api/sessions/s-1/mods/drafts").len(), 2, "the app's own read, and this one");
}

// "setDraftOn POSTs on or off, encoding the name"
#[test]
fn set_draft_on_posts_on_or_off_encoding_the_name() {
    let r = rig(|f| f.toggle = ("200 OK", json!({ "name": "test chips" })));
    drafts::set_draft_on(&r.base, "s-1", "test chips", false).expect("off");
    assert_eq!(r.calls("POST", "/api/sessions/s-1/mods/drafts/test%20chips/off").len(), 1);
    drafts::set_draft_on(&r.base, "s-1", "test chips", true).expect("on");
    assert_eq!(r.calls("POST", "/api/sessions/s-1/mods/drafts/test%20chips/on").len(), 1);
}

// "checkDraft reads the report, null when there is no checker"
#[test]
fn check_draft_reads_the_report_null_when_there_is_no_checker() {
    let r = rig(|f| f.check = json!({ "check": { "ok": true, "name": "test-chips" } }));
    let report = drafts::check_draft(&r.base, "s-1", "test-chips").expect("a read").expect("a report");
    assert_eq!(report["name"], "test-chips");
    assert_eq!(r.calls("GET", "/api/sessions/s-1/mods/drafts/test-chips/check").len(), 1);
    r.with(|f| f.check = json!({ "check": null }));
    assert_eq!(drafts::check_draft(&r.base, "s-1", "test-chips").expect("a read"), None);
}

// "keepDraft POSTs the trimmed note, or no body when it is empty"
#[test]
fn keep_draft_posts_the_trimmed_note_or_no_body_when_it_is_empty() {
    let r = rig(|_| {});
    drafts::keep_draft(&r.base, "s-1", "test-chips", "  Easier to scan.  ").expect("kept");
    let sent = r.calls("POST", "/api/sessions/s-1/mods/drafts/test-chips/keep");
    assert_eq!(sent[0].1, json!({ "note": "Easier to scan." }));
    drafts::keep_draft(&r.base, "s-1", "test-chips", "   ").expect("kept");
    // This client always sends a JSON body (`{}` for none); the server reads a missing note as none either way.
    assert_eq!(r.calls("POST", "/keep")[1].1, json!({}), "no note");
}

// "dismissKeepRequest DELETEs, accepts an empty 204 and treats 404 as done"
#[test]
fn dismiss_keep_request_deletes_accepts_an_empty_204_and_treats_404_as_done() {
    let r = rig(|_| {});
    assert_eq!(drafts::dismiss_keep_request(&r.base, "s-1", "test-chips"), Ok(()));
    assert_eq!(r.calls("DELETE", "/api/sessions/s-1/mods/drafts/test-chips/keep-request").len(), 1);
    r.with(|f| f.dismiss = "404 Not Found");
    assert_eq!(drafts::dismiss_keep_request(&r.base, "s-1", "test-chips"), Ok(()));
    r.with(|f| f.dismiss = "400 Bad Request");
    assert_eq!(drafts::dismiss_keep_request(&r.base, "s-1", "test-chips").unwrap_err().status, 400);
}

// "throws ModsRequestError with the server's message and status"
#[test]
fn errors_carry_the_server_s_message_and_status() {
    let r = rig(|f| f.toggle = ("404 Not Found", json!({ "error": "That draft is gone." })));
    let e = drafts::set_draft_on(&r.base, "s-1", "nope", true).unwrap_err();
    assert_eq!((e.message.as_str(), e.status), ("That draft is gone.", 404));
}

// ---- mods.test.ts: the draft cases ----

// "fills the switch and a session's drafts"
#[test]
fn fills_the_switch_and_a_session_s_drafts() {
    let r = rig(|f| f.drafts = json!([draft("context-gauge", json!({}))]));
    assert!(r.rt.state.draft_cards("s-1").iter().map(|d| d.name.as_str()).eq(["context-gauge"]));
}

// "follows the user's Mods choice as soon as it's made, and the server's answer until then"
#[test]
fn follows_the_user_s_mods_choice_as_soon_as_it_s_made_and_the_server_s_answer_until_then() {
    let mut r = rig(|f| {
        f.switch = json!({ "on": false, "safeMode": false });
        f.drafts = json!([draft("a", json!({}))]);
    });
    assert!(r.rt.state.draft_cards("s-1").is_empty());
    r.rt.update(|s, _| drop(s.pickers.prefs.set("Mods", "true")));
    r.settle();
    assert_eq!(r.rt.state.draft_cards("s-1").len(), 1);
    r.rt.update(|s, _| drop(s.pickers.prefs.set("Mods", "false")));
    assert!(r.rt.state.draft_cards("s-1").is_empty());
}

// "draftsFor is empty for unknown sessions"
#[test]
fn drafts_for_is_empty_for_unknown_sessions() {
    assert!(rig(|_| {}).list("nope").is_empty());
}

// "coalesces concurrent loads of the same thing"
#[test]
fn coalesces_concurrent_loads_of_the_same_thing() {
    let mut r = rig(|_| {});
    let before = r.calls("GET", "/mods/drafts").len();
    r.rt.update(|s, _| {
        s.load_mod_drafts("s-1");
        s.load_mod_drafts("s-1");
    });
    r.settle();
    assert_eq!(r.calls("GET", "/mods/drafts").len(), before + 1);
    r.rt.update(|s, _| {
        s.load_mod_drafts("s-1");
    });
    r.settle();
    assert_eq!(r.calls("GET", "/mods/drafts").len(), before + 2);
}

// "leaves state empty when Mods are switched off (404)"
#[test]
fn leaves_state_empty_when_mods_are_switched_off() {
    let r = rig(|f| {
        f.drafts_status = "404 Not Found";
        f.drafts = json!({ "error": "Mods are off." });
    });
    assert!(r.list("s-1").is_empty());
}

// "replaces that draft in place"
#[test]
fn set_draft_on_replaces_that_draft_in_place() {
    let mut r = rig(|f| f.drafts = json!([draft("a", json!({})), draft("b", json!({}))]));
    let off = draft("a", json!({ "off": { "by": "user", "at": "2026-10-10T10:00:00Z" } }));
    r.with(|f| f.toggle = ("200 OK", off.clone()));
    r.rt.update(|s, _| s.toggle_mod_draft("s-1", "a"));
    r.settle();
    // The draft was on, so the toggle asked to turn it off.
    assert_eq!(r.calls("POST", "/drafts/a/off").len(), 1);
    let now = r.list("s-1");
    assert_eq!(now[0], parsed(off));
    assert_eq!(now[1].name, "b");
}

// "clears the keep request when turning off, not when turning on"
#[test]
fn clears_the_keep_request_when_turning_off_not_when_turning_on() {
    let asked = json!({ "note": "keep it", "at": "2026-10-10T10:00:00Z" });
    let mut r = rig(|f| f.drafts = json!([draft("a", json!({ "keepRequest": asked })), draft("b", json!({ "keepRequest": asked }))]));
    r.with(|f| f.toggle = ("200 OK", draft("a", json!({ "keepRequest": asked }))));
    r.rt.update(|s, _| s.toggle_mod_draft("s-1", "a"));
    r.settle();
    assert_eq!(r.list("s-1")[0].keep_request, None);
    assert!(r.list("s-1")[1].keep_request.is_some());
    // b is off, so the toggle turns it on and its request stays.
    r.rt.update(|s, _| s.mod_drafts.set("s-1", vec![parsed(draft("b", json!({ "off": { "by": "user" }, "keepRequest": asked })))]));
    r.with(|f| f.toggle = ("200 OK", draft("b", json!({ "keepRequest": asked }))));
    r.rt.update(|s, _| s.toggle_mod_draft("s-1", "b"));
    r.settle();
    assert!(r.list("s-1")[0].keep_request.is_some());
}

// "rethrows the request error"
#[test]
fn set_draft_on_shows_the_request_error() {
    let mut r = rig(|f| f.drafts = json!([draft("x", json!({}))]));
    r.with(|f| f.toggle = ("404 Not Found", json!({ "error": "That draft is gone." })));
    r.rt.update(|s, _| s.toggle_mod_draft("s-1", "x"));
    r.settle();
    assert_eq!(r.rt.state.mod_drafts.action_error("s-1", "x"), Some("That draft is gone."));
}

// "keep reloads that session's drafts, and rethrows a refusal leaving the request"
#[test]
fn keep_reloads_that_session_s_drafts_and_rethrows_a_refusal_leaving_the_request() {
    let asked = json!({ "note": "keep it", "at": "2026-10-10T10:00:00Z" });
    let mut r = rig(|f| f.drafts = json!([draft("a", json!({ "keepRequest": asked }))]));
    r.with(|f| f.keep = ("400 Bad Request", json!({ "error": "The check found a problem." })));
    let seen = Arc::new(Mutex::new(None));
    let sink = seen.clone();
    r.rt.update(move |s, _| s.keep_mod_draft("s-1", "a", "", move |_, _, got| *sink.lock().unwrap() = Some(got)));
    r.settle();
    assert_eq!(seen.lock().unwrap().take().expect("an answer").unwrap_err().status, 400);
    assert!(r.list("s-1")[0].keep_request.is_some());
    r.with(|f| {
        f.keep = ("200 OK", json!({ "name": "a" }));
        f.drafts = json!([draft("a", json!({ "kept": 1 }))]);
    });
    r.rt.update(|s, _| s.keep_mod_draft("s-1", "a", "note", |_, _, _| {}));
    r.settle();
    assert_eq!(r.calls("POST", "/drafts/a/keep").last().expect("sent").1, json!({ "note": "note" }));
    assert_eq!(r.list("s-1")[0].kept, Some(1));
}

// "dismissKeepRequest clears the request and reloads"
#[test]
fn dismiss_keep_request_clears_the_request_and_reloads() {
    let asked = json!({ "note": "keep it", "at": "2026-10-10T10:00:00Z" });
    let mut r = rig(|f| f.drafts = json!([draft("a", json!({ "keepRequest": asked }))]));
    r.with(|f| f.drafts = json!([draft("a", json!({}))]));
    r.rt.update(|s, _| s.dismiss_mod_keep_request("s-1", "a", |_, _, _| {}));
    r.settle();
    assert_eq!(r.calls("DELETE", "/drafts/a/keep-request").len(), 1);
    assert_eq!(r.list("s-1")[0].keep_request, None);
}

// "reloads a tracked session's drafts on %s"
#[test]
fn reloads_a_tracked_session_s_drafts_on_each_reason() {
    for reason in ["draft-written", "keep-requested", "draft-on", "draft-off"] {
        let mut r = rig(|_| {});
        let loads = |r: &Rig| r.calls("GET", "/mods/drafts").len();
        let before = loads(&r);
        r.emit(json!({ "reason": reason, "sessionId": "other", "name": "a" }));
        assert_eq!(loads(&r), before, "{reason}: another session is not tracked");
        r.emit(json!({ "reason": reason, "sessionId": "s-1", "name": "a" }));
        assert_eq!(loads(&r), before + 1, "{reason}");
    }
}

// "reads the switch again when the server says it moved"
#[test]
fn reads_the_switch_again_when_the_server_says_it_moved() {
    let mut r = rig(|f| f.switch = json!({ "on": false, "safeMode": false }));
    assert!(r.rt.state.draft_cards("s-1").is_empty());
    r.with(|f| {
        f.switch = json!({ "on": true, "safeMode": false });
        f.drafts = json!([draft("a", json!({}))]);
    });
    r.emit(json!({ "reason": "switch" }));
    assert_eq!(r.rt.state.draft_cards("s-1").len(), 1);
}

// ---- ModDraftCards.test.ts ----

// "shows one card per draft of the session and loads them"
#[test]
fn mod_draft_cards_shows_one_card_per_draft_of_the_session_and_loads_them() {
    let r = rig(|f| f.drafts = json!([draft("test-chips", json!({})), draft("context-gauge", json!({}))]));
    let w = r.words();
    assert!(w.contains("test-chips") && w.contains("context-gauge"), "{w}");
    assert!(r.has("mod-draft-mark"), "the cards are in the stream");
    assert!(!r.calls("GET", "/api/sessions/s-1/mods/drafts").is_empty());
}

// "renders nothing with the Mods switch off, and doesn't load drafts"
#[test]
fn mod_draft_cards_renders_nothing_with_the_mods_switch_off_and_doesn_t_load_drafts() {
    let r = rig(|f| {
        f.switch = json!({ "on": false, "safeMode": false });
        f.drafts = json!([draft("test-chips", json!({}))]);
    });
    assert!(!r.has("mod-draft-mark") && !r.words().contains("test-chips"));
    assert!(r.calls("GET", "/mods/drafts").is_empty());
}

// "reads the switch when it hasn't been read"
#[test]
fn mod_draft_cards_reads_the_switch_when_it_hasn_t_been_read() {
    let r = rig(|_| {});
    let f = r.fake.lock().unwrap();
    assert!(f.calls.iter().any(|c| c.1 == "/api/features/mods"));
}

// "loads again for another session"
#[test]
fn mod_draft_cards_loads_again_for_another_session() {
    let mut r = rig(|_| {});
    r.rt.update(|s, _| s.route = crate::fleet::store::Route::Session("s-2".into()));
    r.settle();
    assert!(!r.calls("GET", "/api/sessions/s-2/mods/drafts").is_empty());
}

// ---- ModDraftCard.test.ts ----

// "shows a draft that is on in this session only"
#[test]
fn mod_draft_card_shows_a_draft_that_is_on_in_this_session_only() {
    let mut r = rig(|_| {});
    card(&mut r, json!({}));
    assert!(r.has("mod-draft-mark"));
    let w = r.words();
    for want in ["test-chips", "On in this session only", "Test runs as counts", "Turn off"] {
        assert!(w.contains(want), "{want}");
    }
}

// "says when it is off, and turns it on again"
#[test]
fn mod_draft_card_says_when_it_is_off_and_turns_it_on_again() {
    let mut r = rig(|f| f.toggle = ("200 OK", draft("test-chips", json!({}))));
    card(&mut r, json!({ "off": { "by": "user", "at": "2026-10-10T10:00:00Z" } }));
    assert!(r.words().contains("Off in this session"));
    r.press("mod-draft-toggle");
    assert_eq!(r.calls("POST", "/api/sessions/s-1/mods/drafts/test-chips/on").len(), 1);
}

// "says three failures turned it off, with the last error"
#[test]
fn mod_draft_card_says_three_failures_turned_it_off_with_the_last_error() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "off": { "by": "strikes", "at": "2026-10-10T10:00:00Z", "error": "x is not a function" } }));
    let w = r.words();
    assert!(w.contains("Turned off after three failures") && w.contains("x is not a function"));
}

// "says a draft whose manifest is missing is still being written"
#[test]
fn mod_draft_card_says_a_draft_whose_manifest_is_missing_is_still_being_written() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "description": null, "version": null }));
    assert!(r.words().contains("Still being written"));
}

// "says which kept version it replaces"
#[test]
fn mod_draft_card_says_which_kept_version_it_replaces() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "kept": 2 }));
    assert!(r.words().contains("Replaces your kept v2 in this session"));
}

// "turns off at once, disabled while it waits"
#[test]
fn mod_draft_card_turns_off_at_once_disabled_while_it_waits() {
    let mut r = rig(|f| f.toggle = ("200 OK", draft("test-chips", json!({ "off": { "by": "user" } }))));
    card(&mut r, json!({}));
    crate::tests::click(&mut r.rt, "mod-draft-toggle");
    assert!(r.rt.state.mod_drafts.is_toggling("s-1", "test-chips"), "pressed: running until the answer is applied");
    r.settle();
    assert_eq!(r.calls("POST", "/api/sessions/s-1/mods/drafts/test-chips/off").len(), 1);
    assert!(!r.rt.state.mod_drafts.is_toggling("s-1", "test-chips"));
    assert!(r.words().contains("Turn on again"));
}

// "shows why turning off failed"
#[test]
fn mod_draft_card_shows_why_turning_off_failed() {
    let mut r = rig(|f| f.toggle = ("404 Not Found", json!({ "error": "That draft is gone." })));
    card(&mut r, json!({}));
    r.press("mod-draft-toggle");
    assert_eq!(r.text_of("mod-draft-action-error-text"), "That draft is gone.");
}

// "shows the draft's problem in the error colour"
#[test]
fn mod_draft_card_shows_the_draft_s_problem_in_the_error_colour() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "problem": { "code": "x", "message": "Call on() inside register", "line": 12 } }));
    assert_eq!(r.text_of("mod-draft-check-text"), "Check: Call on() inside register (line 12)");
    card(&mut r, json!({ "problem": { "code": "x", "message": "Bad manifest" } }));
    assert_eq!(r.text_of("mod-draft-check-text"), "Check: Bad manifest");
}

// "shows the agent's request to keep, with its note"
#[test]
fn mod_draft_card_shows_the_agent_s_request_to_keep_with_its_note() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "keepRequest": { "note": "Counts are easier to scan.", "at": "2026-10-10T10:00:00Z" } }));
    assert!(r.has("mod-draft-request"));
    let w = r.words();
    assert!(w.contains("The agent asks you to keep this") && w.contains("Counts are easier to scan."));
}

// "shows the request without a note"
#[test]
fn mod_draft_card_shows_the_request_without_a_note() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "keepRequest": { "note": null, "at": "2026-10-10T10:00:00Z" } }));
    assert!(r.words().contains("The agent asks you to keep this"));
    assert!(!r.has("mod-draft-note"));
}

// "shows no line number for a null line"
#[test]
fn mod_draft_card_shows_no_line_number_for_a_null_line() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "problem": { "code": "x", "message": "Bad manifest", "line": null } }));
    assert_eq!(r.text_of("mod-draft-check-text"), "Check: Bad manifest");
}

// "Keep… opens the review, and is disabled while the draft is still being written"
#[test]
fn mod_draft_card_keep_opens_the_review_and_is_disabled_while_the_draft_is_still_being_written() {
    let mut r = rig(|_| {});
    card(&mut r, json!({}));
    assert_eq!(r.rt.state.mod_drafts.review, None);
    r.press("mod-draft-keep");
    assert_eq!(r.rt.state.mod_drafts.review, Some(("s-1".into(), "test-chips".into())));
    r.rt.update(|s, _| s.mod_drafts.review = None);
    card(&mut r, json!({ "description": null, "version": null }));
    r.press("mod-draft-keep");
    assert_eq!(r.rt.state.mod_drafts.review, None, "disabled");
}

// "Review… in the keep request opens the review"
#[test]
fn mod_draft_card_review_in_the_keep_request_opens_the_review() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "keepRequest": { "note": "Counts are easier to scan.", "at": "2026-10-10T10:00:00Z" } }));
    r.press("mod-draft-review");
    assert_eq!(r.rt.state.mod_drafts.review, Some(("s-1".into(), "test-chips".into())));
}

// "offers one way into the review: Review… in the request, no Keep… beside Turn off"
#[test]
fn mod_draft_card_offers_one_way_into_the_review() {
    let mut r = rig(|_| {});
    card(&mut r, json!({ "keepRequest": { "note": null, "at": "2026-10-10T10:00:00Z" } }));
    assert!(r.has("mod-draft-review") && !r.has("mod-draft-keep") && r.has("mod-draft-toggle"));
}

// "on the phone: draws the phone's own buttons, never the desktop ones": the phone app is not part of fleet-native.
