//! The Allowed rules live (component 4, piece E): `use-session-permission-rules.test.ts` under its names, the chip and
//! `AllowedRulesList.test.ts` driven over the app, and what a removal sends, read from a fake server.
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use crate::asks::rules::{PermissionLevel, read_rules};
use rune::Runtime;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const FRAME: f64 = 1.0 / 60.0;

fn rule(id: &str, patterns: &[&str]) -> Value {
    json!({"id": id, "kind": "shell", "tool": "bash", "patterns": patterns, "createdAt": "2026-10-19T09:00:00Z"})
}

fn ids(s: &State, session: &str) -> Vec<String> {
    s.rules.of(session).iter().map(|r| r.id.clone()).collect()
}

fn event(session: &str, items: Value) -> Event {
    Event::Other(Kind::SessionPermissionRules, json!({"sessionId": session, "items": items}))
}

#[derive(Default)]
struct Seen {
    /// Method, path and body of every call to a permission route.
    calls: Vec<(String, String, Value)>,
    /// What `GET …/permission-rules` answers.
    rules: Value,
    gets: usize,
    /// What a removal gets instead of 204.
    refuse: Option<(u16, Value)>,
}

fn serve(mut stream: TcpStream, seen: Arc<Mutex<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut length = 0usize;
    let mut request = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if request.is_empty() {
            request = line.clone();
        }
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = n.trim().parse().unwrap_or(0);
        }
    }
    let mut raw = vec![0; length];
    let _ = reader.read_exact(&mut raw);
    let body: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let mut parts = request.split(' ');
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let reply = |stream: &mut TcpStream, status: &str, body: &str| {
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        );
    };
    let mut seen = seen.lock().unwrap();
    if path.starts_with("/hubs/session-events/negotiate") {
        reply(&mut stream, "200 OK", "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}");
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        reply(&mut stream, "200 OK", &fixture("api-sessions.json"));
    } else if method == "GET" && path.ends_with("/permission-rules") {
        seen.gets += 1;
        let answer = seen.rules.to_string();
        reply(&mut stream, "200 OK", &answer);
    } else if method == "DELETE" && path.contains("/permission-rules") {
        seen.calls.push((method, path, body));
        match seen.refuse.clone() {
            Some((code, answer)) => reply(&mut stream, &format!("{code} Refused"), &answer.to_string()),
            None => reply(&mut stream, "204 No Content", ""),
        }
    } else {
        reply(&mut stream, "404 Not Found", "{}");
    }
}

struct Live {
    rt: Runtime<State>,
    id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

impl Live {
    /// The fixture's first session open on a server that holds `rules`.
    fn start(rules: Value) -> Live {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen { rules, ..Default::default() }));
        let shared = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = shared.clone();
                std::thread::spawn(move || serve(stream, seen));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
        });
        let id = rt.state.open_id().unwrap();
        let mut live = Live { rt, id, jobs, seen };
        live.rt.frame(FRAME);
        live
    }

    fn wait(&mut self, until: &dyn Fn(&Live) -> bool, what: &str) {
        crate::test_wait::until(
            self,
            what,
            |l| {
                if until(l) {
                    return true;
                }
                while let Ok(job) = l.jobs.try_recv() {
                    l.rt.update(job);
                }
                l.rt.frame(FRAME);
                until(l)
            },
            |l| format!("{}; {} calls reached the fake server", crate::test_wait::app_state(&l.rt.state), l.calls().len()),
        );
    }

    fn calls(&self) -> Vec<(String, String, Value)> {
        self.seen.lock().unwrap().calls.clone()
    }
}

// use-session-permission-rules.test.ts: "loads the rules when the session opens"
#[test]
fn loads_the_rules_when_the_session_opens() {
    let mut live = Live::start(json!([rule("rule_1", &["bun test*"])]));
    live.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_rules(&id);
    });
    live.wait(&|l| ids(&l.rt.state, &l.id) == ["rule_1"], "the rules to load");
    assert_eq!(live.rt.state.rules.of(&live.id)[0].patterns, ["bun test*"]);

    // After a reconnect the list is read again.
    let before = live.seen.lock().unwrap().gets;
    live.seen.lock().unwrap().rules = json!([]);
    live.rt.update(|s, _| s.on_reconnected());
    live.wait(&|l| l.rt.state.rules.of(&l.id).is_empty() && l.seen.lock().unwrap().gets > before, "the rules to load again");
}

// use-session-permission-rules.test.ts: "takes the whole list a session.permission-rules event carries, and only for its own session"
#[test]
fn takes_the_whole_list_a_session_permission_rules_event_carries_and_only_for_its_own_session() {
    let mut rt = app();
    rt.update(|s, _| {
        let topic = Topic::Session("s-rules".into());
        s.dispatch(&topic, &[event("another", json!([rule("rule_x", &["*"])]))]);
        assert!(s.rules.of("s-rules").is_empty());
        s.dispatch(&topic, &[event("s-rules", json!([rule("rule_1", &["a*"]), rule("rule_2", &["b*"])]))]);
        assert_eq!(ids(s, "s-rules"), ["rule_1", "rule_2"]);
        s.dispatch(&topic, &[event("s-rules", json!([rule("rule_2", &["b*"])]))]);
        assert_eq!(ids(s, "s-rules"), ["rule_2"], "the list is replaced, not added to");
    });
}

// use-session-permission-rules.test.ts: "removes a rule at once and tells Fleet"
#[test]
fn removes_a_rule_at_once_and_tells_fleet() {
    let mut live = Live::start(json!([rule("rule_1", &["a*"]), rule("rule_2", &["b*"])]));
    live.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_rules(&id);
    });
    live.wait(&|l| l.rt.state.rules.of(&l.id).len() == 2, "the rules to load");
    live.rt.update(|s, _| s.remove_rule("rule_1"));
    assert_eq!(ids(&live.rt.state, &live.id), ["rule_2"], "it goes before Fleet has answered");
    live.wait(&|l| !l.calls().is_empty(), "the removal to be sent");
    let calls = live.calls();
    assert_eq!(
        (calls[0].0.as_str(), calls[0].1.as_str()),
        ("DELETE", format!("/api/sessions/{}/permission-rules/rule_1", live.id).as_str())
    );
}

// use-session-permission-rules.test.ts: "loads the list again when Fleet refuses a removal"
#[test]
fn loads_the_list_again_when_fleet_refuses_a_removal() {
    let mut live = Live::start(json!([rule("rule_1", &["a*"])]));
    live.wait(&|l| l.rt.state.rules.of(&l.id).len() == 1 && l.seen.lock().unwrap().gets >= 1, "the rules to load");
    live.seen.lock().unwrap().refuse = Some((500, Value::Null));
    let before = live.seen.lock().unwrap().gets;
    live.rt.update(|s, _| s.remove_rule("rule_1"));
    assert!(live.rt.state.rules.of(&live.id).is_empty());
    live.wait(&|l| l.seen.lock().unwrap().gets > before && ids(&l.rt.state, &l.id) == ["rule_1"], "the list to come back");
}

#[test]
fn a_404_on_a_removal_is_not_a_failure() {
    // Opening the session already loads the rules; a second load of the test's own would still be in flight when the
    // first one lands, and its GET would arrive after `before` was taken on a slow machine.
    let mut live = Live::start(json!([rule("rule_1", &["a*"])]));
    live.wait(&|l| l.rt.state.rules.of(&l.id).len() == 1 && l.seen.lock().unwrap().gets >= 1, "the rules to load");
    live.seen.lock().unwrap().refuse = Some((404, Value::Null));
    let before = live.seen.lock().unwrap().gets;
    live.rt.update(|s, _| s.remove_rule("rule_1"));
    live.wait(&|l| !l.calls().is_empty(), "the removal to be sent");
    for _ in 0..30 {
        live.rt.frame(FRAME);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(live.seen.lock().unwrap().gets, before, "gone already: nothing to load again");
    assert!(live.rt.state.rules.of(&live.id).is_empty());
}

// use-session-permission-rules.test.ts: "removes every rule"
#[test]
fn removes_every_rule() {
    let mut live = Live::start(json!([rule("rule_1", &["a*"]), rule("rule_2", &["b*"])]));
    live.rt.update(|s, _| {
        let id = s.open_id().unwrap();
        s.load_rules(&id);
    });
    live.wait(&|l| l.rt.state.rules.of(&l.id).len() == 2, "the rules to load");
    live.rt.update(|s, _| s.remove_all_rules());
    assert!(live.rt.state.rules.of(&live.id).is_empty());
    live.wait(&|l| !l.calls().is_empty(), "the removal to be sent");
    assert_eq!(live.calls()[0].1, format!("/api/sessions/{}/permission-rules", live.id));
}

#[test]
fn a_load_that_an_event_or_a_removal_overtook_is_dropped() {
    let mut rules = crate::fleet::rules::Rules::default();
    let first = rules.begin_load("s1");
    rules.replace("s1", read_rules(&json!([rule("rule_1", &["a*"])])));
    rules.loaded("s1", first, Vec::new());
    assert_eq!(rules.of("s1").len(), 1, "the event is newer than the load");
    let second = rules.begin_load("s1");
    rules.remove("s1", "rule_1");
    rules.loaded("s1", second, read_rules(&json!([rule("rule_1", &["a*"])])));
    assert!(rules.of("s1").is_empty(), "a removal made after the load was asked for is not undone by it");
}

#[test]
fn a_rule_without_an_id_or_a_tool_is_ignored_and_a_missing_kind_is_other() {
    let rules = read_rules(&json!([{"id": "r1", "tool": "mcp_x"}, {"tool": "bash"}, {"id": "r3"}, "x", rule("r4", &[])]));
    assert_eq!(rules.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["r1", "r4"]);
    assert_eq!(rules[0].kind, crate::conv::tools::PermissionKind::Other);
    assert!(read_rules(&json!({"not": "a list"})).is_empty());
}

#[test]
fn the_level_is_the_harness_own_else_the_default_else_all() {
    let prefs = |pairs: &'static [(&str, &str)]| move |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string());
    assert_eq!(
        PermissionLevel::of(Some("opencode"), prefs(&[("PermissionLevel.opencode", "ask"), ("PermissionLevel", "all")])),
        PermissionLevel::Ask
    );
    assert_eq!(PermissionLevel::of(Some("opencode"), prefs(&[("PermissionLevel", "edits")])), PermissionLevel::Edits);
    assert_eq!(PermissionLevel::of(Some("opencode"), prefs(&[("PermissionLevel.opencode", "bogus")])), PermissionLevel::All);
    assert_eq!(PermissionLevel::of(None, prefs(&[])), PermissionLevel::All);
    assert_eq!(PermissionLevel::Edits.name(), "Allow edits");
}

// ─── the chip and the list ──────────────────────────────────────────────────

fn shown(level: Option<&str>, rules: Value) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.set_open((0, 0));
        if let Some(level) = level {
            s.pickers.prefs.set("PermissionLevel", level);
        }
        let id = s.draft_owner().unwrap();
        s.rules.replace(&id, read_rules(&rules));
        s.ui.open = Some(crate::ui::Open::Popover("allowed"));
    });
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    rt
}

fn info(rt: &Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key))
}

fn two() -> Value {
    json!([rule("rule_1", &["bun test*"]), rule("rule_2", &["*"])])
}

// AllowedRulesList.test.ts: "reads each rule in plain words"
#[test]
fn reads_each_rule_in_plain_words() {
    let rt = shown(Some("ask"), two());
    let text = |key: &str| info(&rt, key).and_then(|e| e.text).unwrap_or_default();
    assert_eq!(text("allowed-rule-rule_1-what"), "Run bun test* without asking");
    assert_eq!(text("allowed-rule-rule_2-what"), "Run any command without asking");
}

// AllowedRulesList.test.ts: "removes one rule or all of them"
#[test]
fn removes_one_rule_or_all_of_them() {
    let mut rt = shown(Some("edits"), two());
    crate::tests::click(&mut rt, "allowed-remove-rule_1");
    let id = rt.state.draft_owner().unwrap();
    assert_eq!(ids(&rt.state, &id), ["rule_2"]);
    assert!(rt.find("allowed-remove-all").is_none(), "Remove all is for two rules or more");

    let mut rt = shown(Some("edits"), two());
    crate::tests::click(&mut rt, "allowed-remove-all");
    let id = rt.state.draft_owner().unwrap();
    assert!(rt.state.rules.of(&id).is_empty());
    assert!(rt.find("allowed-chip").is_none(), "the chip goes with the last rule");
    assert!(rt.state.ui.open.is_none(), "and so does the popover");
}

// AllowedRulesList.test.ts: "greys the rows and says why when the level allows everything"
#[test]
fn greys_the_rows_and_says_why_when_the_level_allows_everything() {
    let rt = shown(Some("all"), two());
    assert!(rt.find("allowed-dormant").is_some());
    assert!(info(&rt, "allowed-rows").is_some_and(|e| (e.opacity - 0.55).abs() < 0.01));
}

// AllowedRulesList.test.ts: "says nothing about being dormant when asking is on"
#[test]
fn says_nothing_about_being_dormant_when_asking_is_on() {
    let rt = shown(Some("ask"), two());
    assert!(rt.find("allowed-dormant").is_none());
    assert!(info(&rt, "allowed-rows").is_some_and(|e| e.opacity > 0.99));
}

#[test]
fn the_chip_counts_the_rules_hides_with_none_and_in_a_side_conversation_and_flashes_once_when_one_is_added() {
    let mut rt = app();
    rt.update(|s, _| s.set_open((0, 0)));
    rt.frame(FRAME);
    assert!(rt.find("allowed-chip").is_none(), "no rule, no chip");
    rt.update(|s, _| {
        let id = s.draft_owner().unwrap();
        s.clock = 10.0;
        s.dispatch(&Topic::Session(id.clone()), &[event(&id, json!([rule("rule_1", &["a*"])]))]);
        assert_eq!(s.allowed_flash, Some(10.0), "a rule was added: the chip flashes");
        s.dispatch(&Topic::Session(id.clone()), &[event(&id, json!([rule("rule_1", &["a*"])]))]);
    });
    rt.frame(FRAME);
    assert_eq!(info(&rt, "allowed-chip").and_then(|e| e.label).as_deref(), Some("Allowed · 1"));
    rt.update(|s, _| {
        let id = s.draft_owner().unwrap();
        s.allowed_flash = None;
        s.dispatch(&Topic::Session(id.clone()), &[event(&id, json!([]))]);
        assert_eq!(s.allowed_flash, None, "taking a rule away doesn't flash");
    });
}
