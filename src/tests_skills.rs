//! Improve a skill, piece T of docs/parity/09-app-frame.md (rows 12.1, 12.3, 12.4's diff). Fleet has no component test for
//! the skills client or for SkillDiff, so these are written from reading `lib/skill-versions.ts`, `stores/built-in-skills.ts`,
//! `SkillDiff.vue` and `ToolCard.vue`: the token estimate, the request bodies, the turn Improve reads, which cards offer
//! the button, and what a fake Fleet is sent.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::skills::*;
use crate::tests::{FRAME, app, click};
use crate::{State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

// ── The plain parts ─────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn estimate_tokens_is_a_quarter_of_the_length_rounded_up() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcd"), 1);
    assert_eq!(estimate_tokens("abcde"), 2);
    assert_eq!(estimate_tokens(&"x".repeat(4000)), 1000);
    // The browser counts UTF-16 units: a letter outside the first plane is two.
    assert_eq!(estimate_tokens("\u{1F600}\u{1F600}"), 1);
}

#[test]
fn version_label_names_which_copy_sessions_get() {
    assert_eq!(version_label(None), "Fleet's");
    assert_eq!(version_label(Some(0)), "Fleet's");
    assert_eq!(version_label(Some(3)), "Yours \u{b7} v3");
}

#[test]
fn the_improve_request_sends_only_what_was_chosen() {
    let bare = ImproveRequest { session_id: "s1".into(), note: "Shorter".into(), ..Default::default() };
    assert_eq!(improve_body(&bare), json!({"sessionId": "s1", "note": "Shorter"}));
    let with = ImproveRequest { context: Some("The user asked:\nx".into()), ..bare.clone() };
    assert_eq!(improve_body(&with), json!({"sessionId": "s1", "note": "Shorter", "context": "The user asked:\nx"}));
    let whole = ImproveRequest { whole_conversation: true, ..bare };
    assert_eq!(improve_body(&whole), json!({"sessionId": "s1", "note": "Shorter", "wholeConversation": true}));
}

#[test]
fn keep_and_switch_bodies_and_the_skills_path() {
    assert_eq!(save_body("text", Some("why"), Some("s1")), json!({"content": "text", "note": "why", "sessionId": "s1"}));
    assert_eq!(save_body("text", None, None), json!({"content": "text", "note": null, "sessionId": null}));
    assert_eq!(switch_body(Some(2)), json!({"version": 2}));
    assert_eq!(switch_body(None), json!({"version": null}));
    assert_eq!(at("tidy-docs"), "/api/skills/built-in/tidy-docs");
    assert_eq!(at("a b/c"), "/api/skills/built-in/a%20b%2Fc");
}

#[test]
fn the_list_of_built_in_skills_reads_a_bare_array_and_drops_an_entry_with_no_name() {
    let list = parse_skills(&json!([
        {"name": "tidy-docs", "description": "d", "enabled": true, "version": 2, "fleetChanged": true, "versionCount": 3},
        {"description": "no name"},
        {"name": "release-notes"}
    ]));
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].version, list[0].fleet_changed, list[0].version_count), (Some(2), true, 3));
    assert_eq!((list[1].version, list[1].enabled), (None, false));
    assert!(parse_skills(&json!({"skills": []})).is_empty());
}

#[test]
fn a_proposal_reads_its_tokens_and_needs_text() {
    let p = parse_proposal(&json!({"name": "tidy-docs", "base": "a", "baseVersion": null, "content": "b", "asks": 2,
        "tokens": {"total": 900, "fromCache": 600}}))
    .unwrap();
    assert_eq!((p.base_version, p.asks, p.tokens), (None, 2, Some(Tokens { total: 900, from_cache: 600 })));
    assert!(parse_proposal(&json!({"name": "tidy-docs"})).is_none());
}

fn msg(id: &str, role: &str, body: &str, tools: Vec<ImproveTool>) -> ImproveMessage {
    ImproveMessage { id: id.into(), role: role.into(), body: body.into(), tools }
}

#[test]
fn the_turn_runs_from_the_prompt_to_the_next_prompt() {
    let tool = |title: &str, status: &str, output: &str| ImproveTool {
        title: title.into(),
        kind: Some("skill".into()),
        status: Some(status.into()),
        output: Some(output.into()).filter(|o: &String| !o.is_empty()),
    };
    let messages = vec![
        msg("u0", "user", "Earlier", vec![]),
        msg("a0", "assistant", "Earlier reply", vec![]),
        msg("u1", "user", "  Tidy the docs  ", vec![]),
        msg("a1", "assistant", "On it", vec![tool("tidy-docs", "Completed", "loaded\nsecond line")]),
        msg("a2", "assistant", "Done", vec![tool("bash", "Error", "")]),
        msg("u2", "user", "Later", vec![]),
    ];
    let turn = improve_turn(&messages, "a1");
    assert_eq!(turn.exchange, "The user asked:\nTidy the docs\n\nThe agent answered:\nOn it\n\nDone");
    assert_eq!(turn.tool_calls, "The tool calls in that turn:\n- skill: tidy-docs\n  loaded\n  second line\n- skill: bash (Error)");
    assert_eq!(improve_turn(&messages, "nope"), ImproveTurn::default());
}

#[test]
fn a_tool_calls_output_is_cut_at_1500_characters() {
    let long = ImproveTool { title: "t".into(), kind: None, status: None, output: Some("x".repeat(1600)) };
    let turn = improve_turn(&[msg("u", "user", "q", vec![]), msg("a", "assistant", "", vec![long])], "a");
    let want = format!("The tool calls in that turn:\n- tool: t\n  {}\n  \u{2026}", "x".repeat(1500));
    assert_eq!(turn.tool_calls, want);
    assert_eq!(turn.exchange, "The user asked:\nq");
}

// ── The card and the diff ───────────────────────────────────────────────────────────────────────────────────────────

fn scene() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_conversation(crate::mock::tools::of_skills());
        s.skills.list = Some(mock_skills());
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

#[test]
fn a_finished_call_of_a_built_in_skill_offers_improve_and_no_other_call_does() {
    let rt = scene();
    assert!(has(&rt, "tool-k1-improve"), "tidy-docs is built in and finished");
    assert!(!has(&rt, "tool-k2-improve"), "release-notes is still running");
    assert!(!has(&rt, "tool-k3-improve"), "my-checklist is the user's own");
}

#[test]
fn no_list_of_skills_yet_means_no_button() {
    let mut rt = scene();
    rt.update(|s, _| s.skills.list = None);
    rt.frame(FRAME);
    assert!(!has(&rt, "tool-k1-improve"));
}

#[test]
fn pressing_improve_asks_for_that_skill_and_leaves_the_card_as_it_was() {
    let mut rt = scene();
    let before = rt.state.tool_open.clone();
    click(&mut rt, "tool-k1-improve");
    assert_eq!(rt.state.skills.improving, Some(Improving { skill: "tidy-docs".into(), tool_id: "k1".into() }));
    assert_eq!(rt.state.tool_open, before, "the press didn't open the card");
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

#[test]
fn the_skill_diff_says_its_file_counts_the_lines_and_says_when_nothing_differs() {
    let mut rt = app();
    rt.update(|s, _| {
        s.open_settings("general", None);
        s.skills.demo_diff = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(has(&rt, "skill-diff-changed") && has(&rt, "skill-diff-changed-diff") && has(&rt, "skill-diff-same"));
    let all = texts(&rt);
    assert!(all.iter().any(|t| t == "tidy-docs/SKILL.md"));
    assert!(all.iter().any(|t| t == "+3") && all.iter().any(|t| t == "\u{2212}2"), "counts: {all:?}");
    assert!(all.iter().any(|t| t == "No differences."));
    assert!(!has(&rt, "skill-diff-same-diff") && !has(&rt, "skill-diff-same-stats"));
}

// ── A fake Fleet ────────────────────────────────────────────────────────────────────────────────────────────────────

pub(crate) type Heard = Arc<Mutex<Vec<(String, String, Value)>>>;

pub(crate) fn fake() -> (String, Heard) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().unwrap());
    let heard: Heard = Default::default();
    let log = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let log = log.clone();
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.method.is_empty() {
                    return;
                }
                let (head, body) = (req.head, req.body);
                let mut first = head.lines().next().unwrap_or("").split(' ');
                let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                let posted: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                log.lock().unwrap().push((method.clone(), path.clone(), posted.clone()));
                let skill = json!({"name": "tidy-docs", "description": "d", "enabled": true, "version": 1, "versions": [{}]});
                let (status, reply) = match (method.as_str(), path.as_str()) {
                    ("GET", "/api/skills/built-in/tidy-docs") => (
                        200,
                        json!({"name": "tidy-docs", "description": "d", "enabled": true, "fleetContent": "a\n", "yourContent": null,
                            "version": null, "versions": []})
                        .to_string(),
                    ),
                    ("GET", "/api/skills/built-in") => (200, json!([{"name": "tidy-docs", "enabled": true}]).to_string()),
                    ("POST", "/api/skills/built-in/tidy-docs/improve") if posted["note"] == "refuse" => {
                        (422, json!({"error": "The model is busy."}).to_string())
                    }
                    ("POST", "/api/skills/built-in/tidy-docs/improve") => (
                        200,
                        json!({"name": "tidy-docs", "base": "a\n", "baseVersion": null, "content": "b\n", "asks": 1,
                            "tokens": {"total": 10, "fromCache": 4}})
                        .to_string(),
                    ),
                    ("POST", "/api/skills/built-in/tidy-docs/versions") | ("PUT", "/api/skills/built-in/tidy-docs/active") => {
                        (200, skill.to_string())
                    }
                    _ => (404, "{}".to_string()),
                };
                let out = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(out.as_bytes());
            });
        }
    });
    (base, heard)
}

pub(crate) fn rig(base: String) -> (Runtime<State>, Receiver<Job>) {
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| s.link = Some(link));
    (rt, queue)
}

pub(crate) fn calls(heard: &Heard, method: &str, needle: &str) -> Vec<(String, String, Value)> {
    heard.lock().unwrap().iter().filter(|c| c.0 == method && c.1.contains(needle)).cloned().collect()
}

#[test]
fn the_list_is_asked_once_and_names_the_skills_the_server_says() {
    let (base, heard) = fake();
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| {
        s.skills_ensure_loaded();
        s.skills_ensure_loaded();
    });
    test_wait::drive(&mut rt, &queue, "the list of built-in skills", |s| s.skills.list.is_some(), test_wait::app_state);
    assert!(rt.state.skills.is_built_in("tidy-docs") && !rt.state.skills.is_built_in("other"));
    rt.update(|s, _| s.skills_ensure_loaded());
    assert_eq!(calls(&heard, "GET", "/api/skills/built-in").len(), 1);
}

#[test]
fn asking_sends_the_request_and_keeps_the_proposal() {
    let (base, heard) = fake();
    let (mut rt, queue) = rig(base);
    let request = ImproveRequest { session_id: "s1".into(), note: "Shorter".into(), context: Some("c".into()), whole_conversation: false };
    rt.update(move |s, _| s.skill_ask("tidy-docs", &request));
    assert!(rt.state.skills.asking);
    test_wait::drive(&mut rt, &queue, "the proposal", |s| s.skills.proposal.is_some(), test_wait::app_state);
    let p = rt.state.skills.proposal.clone().unwrap().unwrap();
    assert_eq!((p.content.as_str(), p.tokens), ("b\n", Some(Tokens { total: 10, from_cache: 4 })));
    assert!(!rt.state.skills.asking);
    let sent = calls(&heard, "POST", "/improve");
    assert_eq!(sent[0].2, json!({"sessionId": "s1", "note": "Shorter", "context": "c"}));
}

#[test]
fn a_refusal_comes_back_in_the_servers_words() {
    let (base, _) = fake();
    let (mut rt, queue) = rig(base);
    let request = ImproveRequest { session_id: "s1".into(), note: "refuse".into(), ..Default::default() };
    rt.update(move |s, _| s.skill_ask("tidy-docs", &request));
    test_wait::drive(&mut rt, &queue, "the refusal", |s| s.skills.proposal.is_some(), test_wait::app_state);
    assert_eq!(rt.state.skills.proposal, Some(Err("The model is busy.".to_string())));
}

#[test]
fn keeping_posts_a_version_and_switching_puts_the_active_one() {
    let (base, heard) = fake();
    let (mut rt, queue) = rig(base);
    rt.update(|s, _| s.skill_keep("tidy-docs", "text", Some("why"), Some("s1")));
    test_wait::drive(&mut rt, &queue, "the kept version", |s| s.skills.saved.is_some(), test_wait::app_state);
    let kept = rt.state.skills.saved.take().unwrap().unwrap();
    assert_eq!((kept.name.as_str(), kept.version, kept.version_count), ("tidy-docs", Some(1), 1));
    assert_eq!(calls(&heard, "POST", "/versions")[0].2, json!({"content": "text", "note": "why", "sessionId": "s1"}));
    rt.update(|s, _| s.skill_switch("tidy-docs", None));
    test_wait::drive(&mut rt, &queue, "the switch", |s| s.skills.saved.is_some(), test_wait::app_state);
    assert_eq!(calls(&heard, "PUT", "/active")[0].2, json!({"version": null}));
}
