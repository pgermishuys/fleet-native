//! Component 23, piece C: mission control kept live, against two fake machines (home, and `m-mini` with a token of its own)
//! that record every request with the key it carried. Fleet's `MissionControlPage.test.ts` "follows each machine's now line
//! every few seconds" and `inbox-feed.test.ts`'s reads under their names, then what this app adds: a rename keeps a feed, a
//! new address or token replaces it, leaving the Fleet view closes every feed and stops the 4 s reads, and no request or
//! token goes to the wrong machine. The clock moves only where a test says so (`later`), as in the Analytics store's tests.
use crate::fleet::link::{Connection, Job, Link, Post};
use crate::fleet::machines::parse_machine_list;
use crate::fleet::mission::FeedStatus;
use crate::fleet::mission::feed::{FOLD, POLL};
use crate::fleet::mission::requests::question_answer_request;
use crate::fleet::mission::store::NOW_EVERY;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{FRAME, app};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

pub(crate) const TOKEN: &str = "mini-token-0123456789abcdef";

#[derive(Clone, Debug)]
pub(crate) struct Heard {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) auth: Option<String>,
    pub(crate) body: String,
}

/// A fake machine: what it was asked, with the `Authorization` it carried, and the answer of each route (404 for the rest).
pub(crate) struct Node {
    pub(crate) base: String,
    pub(crate) heard: Arc<Mutex<Vec<Heard>>>,
    routes: Arc<Mutex<HashMap<String, (u16, String)>>>,
}

pub(crate) fn node(sessions: Value) -> Node {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard: Arc<Mutex<Vec<Heard>>> = Arc::default();
    let routes: Arc<Mutex<HashMap<String, (u16, String)>>> = Arc::default();
    routes.lock().unwrap().insert("/api/sessions".into(), (200, sessions.to_string()));
    routes.lock().unwrap().insert("/api/sessions/now".into(), (200, "{}".into()));
    let (log, table) = (heard.clone(), routes.clone());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, table) = (log.clone(), table.clone());
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                log.lock().unwrap().push(Heard {
                    method: req.method.clone(),
                    path: req.path.clone(),
                    auth: req.header("authorization"),
                    body: req.body_text(),
                });
                let (status, body) = table.lock().unwrap().get(req.route()).cloned().unwrap_or((404, "{}".into()));
                crate::test_http::respond_json(&mut stream, format!("{status} X"), &body);
            });
        }
    });
    Node { base, heard, routes }
}

impl Node {
    pub(crate) fn route(&self, path: &str, status: u16, body: Value) {
        self.routes.lock().unwrap().insert(path.to_string(), (status, body.to_string()));
    }

    /// How many requests of `method` for a path starting with `prefix` were heard.
    pub(crate) fn count(&self, method: &str, prefix: &str) -> usize {
        self.heard.lock().unwrap().iter().filter(|h| h.method == method && h.path.starts_with(prefix)).count()
    }

    pub(crate) fn counter(&self, method: &'static str, prefix: &'static str) -> impl Fn() -> usize + use<> {
        let heard = self.heard.clone();
        move || heard.lock().unwrap().iter().filter(|h| h.method == method && h.path.starts_with(prefix)).count()
    }

    pub(crate) fn everything(&self) -> Vec<Heard> {
        self.heard.lock().unwrap().iter().filter(|h| h.path.starts_with("/api/sessions")).cloned().collect()
    }
}

pub(crate) const LIST: &str = "/api/sessions?limit=100&offset=0";

pub(crate) fn row(id: &str, status: &str) -> Value {
    json!({
        "session": {"id": id, "title": id, "time": {"created": 1_000, "updated": 2_000}},
        "sessionStatus": status, "activityStatus": "busy", "retentionStatus": "active", "isHidden": false,
        "harnessType": "opencode2", "projectName": "ledger-api", "workspaceDirectory": "/work/ledger-api",
    })
}

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    pub(crate) queue: Receiver<Job>,
    pub(crate) home: Node,
    pub(crate) mini: Node,
}

/// The app connected to a fake home, with a second machine listed at another fake, on the Fleet view with Mission control on.
pub(crate) fn rig() -> Rig {
    let (home, mini) = (node(json!([row("h1", "active")])), node(json!([row("m1", "active"), row("m2", "waiting_input")])));
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    let list = listing(&mini.base, "Mini", TOKEN);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs.adopt(&json!({"features.mission-control.enabled": "true"}));
        s.machines.apply_list(list);
    });
    rt.frame(FRAME);
    Rig { rt, queue, home, mini }
}

fn listing(base: &str, name: &str, token: &str) -> crate::fleet::machines::Listed {
    let body = json!({"machines": [{"id": "m-mini", "name": name, "baseUrl": base, "addresses": [{"url": base, "kind": "other", "learned": false}], "token": token}]});
    parse_machine_list(200, &body.to_string())
}

impl Rig {
    /// Applies what the threads hand back and draws frames, without moving the clock, until `done` holds.
    pub(crate) fn wait(&mut self, what: &str, mut done: impl FnMut(&State) -> bool) {
        let queue = &self.queue;
        test_wait::until(
            &mut self.rt,
            what,
            |rt| {
                while let Ok(job) = queue.try_recv() {
                    rt.update(job);
                }
                rt.frame(0.0);
                done(&rt.state)
            },
            |rt| {
                let m = &rt.state.mission;
                format!("open {:?}, home {:?}, mini {:?} {:?}", m.open_keys(), m.status("home"), m.status("m-mini"), m.error("m-mini"))
            },
        );
    }

    /// Shows the Fleet view and waits for both feeds' first reads.
    pub(crate) fn open(&mut self) {
        self.rt.update(|s, _| s.rail = Rail::Fleet);
        self.rt.frame(FRAME);
        let (home, mini) = (self.home.counter("GET", LIST), self.mini.counter("GET", LIST));
        self.wait("both first reads", move |_| home() == 1 && mini() == 1);
        self.wait("both answers", |s| s.mission.machines().iter().all(|m| m.last_heard_at.is_some()));
    }

    /// Moves the clock on, so a timer falls due.
    pub(crate) fn later(&mut self, secs: f64) {
        self.rt.frame(secs);
    }

    fn machine(&self, key: &str) -> crate::fleet::mission::MissionMachine {
        self.rt.state.mission.machines().into_iter().find(|m| m.id == key).expect("the machine has a feed")
    }
}

// ─── inbox-feed.test.ts ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn reads_sessions_and_what_the_waiting_ones_wait_on_with_the_machine_s_token() {
    let r = &mut rig();
    r.mini.route(
        "/api/sessions/m2/permissions",
        200,
        json!([{"id": "perm1", "sessionId": "m2", "tool": "bash", "kind": "shell", "title": "pnpm install"}]),
    );
    r.open();
    let mini = r.machine("m-mini");
    assert_eq!(mini.sessions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["m1", "m2"]);
    assert!(matches!(mini.asks.get("m2"), Some(crate::fleet::mission::Ask::Permission(ask)) if ask.id == "perm1"), "the waiting one's ask");
    assert!(!mini.asks.contains_key("m1"), "a working session has none to read");
    assert_eq!(r.mini.count("GET", "/api/sessions/m2/permissions"), 1);
    assert_eq!(r.mini.count("GET", "/api/sessions/m1/"), 0, "and none is asked for");
    assert_eq!(r.machine("home").sessions.len(), 1);

    // No request or token goes to the wrong machine.
    assert!(r.mini.everything().iter().all(|h| h.auth.as_deref() == Some(&format!("Bearer {TOKEN}"))), "{:?}", r.mini.everything());
    assert!(r.home.everything().iter().all(|h| h.auth.as_deref() != Some(&format!("Bearer {TOKEN}"))), "{:?}", r.home.everything());
    assert_eq!(r.home.count("GET", "/api/sessions/m2"), 0, "home is not asked about the other machine's sessions");
}

#[test]
fn finds_a_question_in_the_last_messages_when_there_is_no_permission() {
    let r = &mut rig();
    r.mini.route("/api/sessions/m2/permissions", 200, json!([]));
    r.mini.route(
        "/api/sessions/m2/messages",
        200,
        // Fleet's own message shape, as v0.74 writes it: a tool part has a numeric state and its `arguments` on the part.
        json!({"messages": [{"id": "x1", "role": "assistant", "timestamp": "2026-11-21T16:39:01.184+00:00", "textContent": "", "parts": [
            {"type": "tool", "toolCallId": "call-q", "toolName": "question", "state": 1,
             "arguments": {"questions": [{"header": "Retries", "question": "Three or five?", "options": [{"label": "Three", "description": ""}]}]}}]}]}),
    );
    r.open();
    assert!(
        matches!(r.machine("m-mini").asks.get("m2"), Some(crate::fleet::mission::Ask::Question { request_id, .. }) if request_id == "call-q")
    );
    assert_eq!(r.mini.count("GET", "/api/sessions/m2/messages?limit=6"), 1);
}

#[test]
fn re_reads_when_the_sessions_topic_says_something_changed() {
    let r = &mut rig();
    r.open();
    assert_eq!((r.home.count("GET", LIST), r.mini.count("GET", LIST)), (1, 1));
    r.mini.route("/api/sessions", 200, json!([row("m1", "active"), row("m2", "active"), row("m3", "idle")]));

    // A burst of events on the mini's topic is one read of the mini, and home is not read at all.
    r.rt.update(|s, _| {
        for _ in 0..3 {
            s.mission_sessions_event("m-mini");
        }
    });
    r.later(FOLD - 0.1);
    assert_eq!(r.mini.count("GET", LIST), 1, "the burst is folded");
    r.later(0.2);
    let mini = r.mini.counter("GET", LIST);
    r.wait("the re-read", move |_| mini() == 2);
    r.wait("its answer", |s| s.mission.machines().iter().find(|m| m.id == "m-mini").is_some_and(|m| m.sessions.len() == 3));
    assert_eq!(r.home.count("GET", LIST), 1, "an event of one machine reads that machine only");
}

#[test]
fn polls_when_the_hub_won_t_connect() {
    let r = &mut rig();
    r.open();
    r.wait("the hubs to say they are down", |s| s.mission.status("m-mini") == Some(FeedStatus::Polling));
    assert_eq!(r.mini.count("GET", LIST), 1);
    r.later(POLL - 1.0);
    assert_eq!(r.mini.count("GET", LIST), 1, "not before its time");
    r.later(1.5);
    let mini = r.mini.counter("GET", LIST);
    r.wait("the poll", move |_| mini() == 2);
}

#[test]
fn keeps_the_last_list_when_the_machine_stops_answering() {
    let r = &mut rig();
    r.open();
    r.wait("the hubs to say they are down", |s| s.mission.status("m-mini") == Some(FeedStatus::Polling));
    r.mini.route("/api/sessions", 500, json!({}));
    r.later(POLL + 0.5);
    r.wait("the failed read", |s| s.mission.status("m-mini") == Some(FeedStatus::Unreachable));
    let mini = r.machine("m-mini");
    assert_eq!(mini.sessions.len(), 2, "the last list stays");
    assert!(mini.last_heard_at.is_some(), "with when it was last heard");
    assert_eq!(r.rt.state.mission.error("m-mini"), Some("It answered 500."));
    assert_eq!(r.rt.state.mission.status("home"), Some(FeedStatus::Polling), "the other machine is not marked");
}

#[test]
fn says_a_refused_key_stopped_working() {
    let r = &mut rig();
    r.mini.route("/api/sessions", 401, json!({}));
    r.rt.update(|s, _| s.rail = Rail::Fleet);
    r.wait("the refusal", |s| s.mission.status("m-mini") == Some(FeedStatus::Unreachable));
    assert_eq!(r.rt.state.mission.error("m-mini"), Some("This phone's key for this machine stopped working."));
}

// ─── MissionControlPage.test.ts ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn follows_each_machine_s_now_line_every_few_seconds() {
    let r = &mut rig();
    r.home.route("/api/sessions/now", 200, json!({"h1": {"text": "bash · pnpm test webhooks", "kind": "tool", "at": "t"}}));
    r.open();
    let (home, mini) = (r.home.counter("GET", "/api/sessions/now"), r.mini.counter("GET", "/api/sessions/now"));
    r.wait("the first now reads", move |_| home() == 1 && mini() == 1);
    r.wait("home's line", |s| s.mission.machines().iter().find(|m| m.id == "home").is_some_and(|m| m.now.contains_key("h1")));
    assert_eq!(r.machine("home").now["h1"].text, "bash · pnpm test webhooks");

    r.home.route("/api/sessions/now", 200, json!({"h1": {"text": "edit · src/sync/rules.ts", "kind": "tool", "at": "t"}}));
    r.later(NOW_EVERY - 0.5);
    assert_eq!(r.home.count("GET", "/api/sessions/now"), 1, "not before its time");
    r.later(0.6);
    let home = r.home.counter("GET", "/api/sessions/now");
    r.wait("the second read", move |_| home() == 2);
    r.wait("the new line", |s| s.mission.machines().iter().find(|m| m.id == "home").is_some_and(|m| m.now["h1"].text.starts_with("edit")));
    assert!(r.machine("m-mini").now.is_empty(), "a machine with nothing to say has no lines, and no error");
}

#[test]
fn a_machine_too_old_for_now_lines_gives_none_and_no_error() {
    let r = &mut rig();
    r.mini.routes.lock().unwrap().remove("/api/sessions/now");
    r.open();
    let mini = r.mini.counter("GET", "/api/sessions/now");
    r.wait("the read of now lines", move |_| mini() == 1);
    r.later(0.1);
    assert!(r.machine("m-mini").now.is_empty());
    assert_ne!(r.rt.state.mission.status("m-mini"), Some(FeedStatus::Unreachable));
}

#[test]
fn asks_no_now_line_while_the_window_is_off_screen_and_at_once_when_it_is_back() {
    let r = &mut rig();
    r.open();
    let mini = r.mini.counter("GET", "/api/sessions/now");
    r.wait("the first now read", {
        let mini = r.mini.counter("GET", "/api/sessions/now");
        move |_| mini() == 1
    });
    r.rt.update(|s, _| s.quiet.visible = false);
    r.later(3.0 * NOW_EVERY);
    assert_eq!(mini(), 1, "nothing is asked while it is hidden");
    r.rt.update(|s, _| s.quiet.visible = true);
    r.later(0.1);
    r.wait("the read when it is back", move |_| mini() == 2);
}

// ─── what this app adds ─────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn opens_a_feed_for_home_and_each_machine_only_while_the_fleet_view_is_shown() {
    let r = &mut rig();
    r.rt.update(|s, _| s.rail = Rail::Sessions);
    r.later(NOW_EVERY * 3.0);
    assert!(!r.rt.state.mission.is_open());
    assert_eq!((r.home.count("GET", "/api/sessions"), r.mini.count("GET", "/api/sessions")), (0, 0), "nothing is asked off the view");
    r.open();
    assert_eq!(r.rt.state.mission.open_keys(), ["home", "m-mini"]);
}

#[test]
fn asks_nothing_with_the_switch_off() {
    let r = &mut rig();
    r.rt.update(|s, _| {
        s.pickers.prefs.adopt(&json!({}));
        s.rail = Rail::Fleet;
    });
    r.later(NOW_EVERY * 3.0);
    assert!(!r.rt.state.mission.is_open());
    // The app's own calls (preferences, the update check) are not the feeds': only the feeds' paths count.
    let feeds = |n: &Node| n.count("GET", "/api/sessions");
    assert_eq!(feeds(&r.mini) + feeds(&r.home), 0, "{:?} {:?}", r.home.everything(), r.mini.everything());
}

#[test]
fn leaving_the_rail_closes_every_feed_and_stops_the_4_s_reads() {
    let r = &mut rig();
    r.open();
    r.wait("the now reads", {
        let (home, mini) = (r.home.counter("GET", "/api/sessions/now"), r.mini.counter("GET", "/api/sessions/now"));
        move |_| home() >= 1 && mini() >= 1
    });
    r.rt.update(|s, _| s.rail = Rail::Sessions);
    r.later(0.1);
    assert!(!r.rt.state.mission.is_open(), "every feed is closed");
    let before = (r.home.everything().len(), r.mini.everything().len());
    r.later(NOW_EVERY * 4.0);
    r.later(POLL * 2.0);
    r.rt.update(|s, _| s.mission_sessions_event("m-mini"));
    r.later(1.0);
    assert_eq!((r.home.everything().len(), r.mini.everything().len()), before, "nothing is asked once the view is left");
}

#[test]
fn a_rename_keeps_the_feed_and_a_new_address_or_token_replaces_it() {
    let r = &mut rig();
    r.open();
    let first = r.rt.state.mission.reads("m-mini");
    let base = r.mini.base.clone();

    // A rename alone: the same feed, no new read, the new name.
    let renamed = listing(&base, "Mini upstairs", TOKEN);
    r.rt.update(move |s, _| s.machines.apply_list(renamed));
    r.later(0.1);
    assert_eq!((r.rt.state.mission.reads("m-mini"), r.machine("m-mini").name.as_str()), (first, "Mini upstairs"));
    assert_eq!(r.mini.count("GET", LIST), 1);

    // A new token: a new feed, read at once with the new key.
    let retoken = listing(&base, "Mini upstairs", "second-token-0123456789abcd");
    r.rt.update(move |s, _| s.machines.apply_list(retoken));
    r.later(0.1);
    let mini = r.mini.counter("GET", LIST);
    r.wait("the new feed's read", move |_| mini() == 2);
    assert_eq!(
        r.mini.everything().iter().rfind(|h| h.path == LIST).and_then(|h| h.auth.clone()).as_deref(),
        Some("Bearer second-token-0123456789abcd")
    );

    // A new address: the old one is left, and the new one is read.
    let other = node(json!([row("n1", "active")]));
    let moved = listing(&other.base, "Mini upstairs", "second-token-0123456789abcd");
    r.rt.update(move |s, _| s.machines.apply_list(moved));
    r.later(0.1);
    let counted = other.counter("GET", LIST);
    r.wait("the new address's read", move |_| counted() == 1);
    r.wait("its answer", |s| s.mission.machines().iter().find(|m| m.id == "m-mini").is_some_and(|m| m.sessions.len() == 1));
    r.later(NOW_EVERY * 2.0);
    assert_eq!(r.mini.count("GET", LIST), 2, "the old address hears no more");
}

#[test]
fn a_machine_that_left_the_list_loses_its_feed() {
    let r = &mut rig();
    r.open();
    r.rt.update(|s, _| s.machines.apply_list(parse_machine_list(200, &json!({"machines": []}).to_string())));
    r.later(0.1);
    assert_eq!(r.rt.state.mission.open_keys(), ["home"]);
    let before = r.mini.everything().len();
    r.later(NOW_EVERY * 3.0);
    assert_eq!(r.mini.everything().len(), before);
}

#[test]
fn reads_the_machine_again_at_once_after_an_answer() {
    let r = &mut rig();
    r.open();
    let sent: Arc<Mutex<Option<crate::fleet::mission::requests::AnswerOutcome>>> = Arc::default();
    let slot = sent.clone();
    r.mini.route("/api/sessions/m2/questions/call-q/answer", 200, json!({}));
    r.rt.update(move |s, _| {
        let request = question_answer_request("m2", "call-q", &[vec!["Five".to_string()]]);
        s.mission_send("m-mini", request, move |_, outcome| *slot.lock().unwrap() = Some(outcome));
    });
    let mini = r.mini.counter("GET", LIST);
    r.wait("the machine to be read again", move |_| mini() == 2);
    r.wait("the outcome", |_| sent.lock().unwrap().is_some());
    assert!(sent.lock().unwrap().as_ref().unwrap().ok);
    assert_eq!(r.mini.count("POST", "/api/sessions/m2/questions/call-q/answer"), 1);
    assert_eq!(r.home.count("POST", "/api/sessions"), 0, "the answer went to its own machine only");
    assert_eq!(r.home.count("GET", LIST), 1, "and only that machine is read again");
}

#[test]
fn an_answer_for_a_machine_with_no_feed_says_so_and_sends_nothing() {
    let r = &mut rig();
    r.open();
    let sent: Arc<Mutex<Option<String>>> = Arc::default();
    let slot = sent.clone();
    r.rt.update(move |s, _| {
        let request = question_answer_request("x1", "call-q", &[vec!["Five".to_string()]]);
        s.mission_send("m-gone", request, move |_, outcome| *slot.lock().unwrap() = outcome.error);
    });
    assert_eq!(sent.lock().unwrap().as_deref(), Some("That machine isn't in the list any more."));
    assert_eq!(r.mini.count("POST", "/api/"), 0, "nothing was sent");
}

#[test]
fn takes_the_hub_s_news_into_the_feed() {
    let r = &mut rig();
    r.open();
    r.rt.update(|s, _| s.set_connection("m-mini", Connection::Online));
    r.later(0.01);
    assert_eq!(r.rt.state.mission.status("m-mini"), Some(FeedStatus::Live));
    r.rt.update(|s, _| s.set_connection("m-mini", Connection::Down("gone".into())));
    r.later(0.01);
    assert_eq!(r.rt.state.mission.status("m-mini"), Some(FeedStatus::Polling));
}
