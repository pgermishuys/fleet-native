//! The autocomplete's loading rules (`use-find-files.ts`, `use-referable-sessions.ts` and the three loads of
//! `use-autocomplete.ts`): the 80 ms wait, a newer search replacing an older, the last answer narrowed while a new one is
//! awaited, the 30 s cache, error wording, and the lists' parses. The three `use-autocomplete.test.ts` cases about
//! requests and timers ("asks again", the 80 ms wait, a new session id) are read here; their logic half is in `composer`.
use super::super::finder::*;
use serde_json::json;

fn session() -> Option<Scope> {
    Some(Scope::Session("s1".into()))
}

fn send(step: Step) -> (u64, String) {
    match step {
        Step::Send { generation, query } => (generation, query),
        other => panic!("expected a request, got {other:?}"),
    }
}

#[test]
fn a_search_waits_80_ms_for_more_typing_and_a_listing_does_not() {
    let mut f = FileSearch::default();
    assert_eq!(send(f.want(session().as_ref(), Some(""), 0.0)).1, "", "an empty query lists the folder at once");
    let mut f = FileSearch::default();
    assert_eq!(send(f.want(session().as_ref(), Some("src/"), 0.0)).1, "src/", "so does one ending in a slash");
    let mut f = FileSearch::default();
    match f.want(session().as_ref(), Some("sto"), 1.0) {
        Step::Wait(w) => assert!((w - 0.08).abs() < 1e-9, "{w}"),
        other => panic!("{other:?}"),
    }
    assert!(f.loading);
    assert_eq!(send(f.want(session().as_ref(), Some("sto"), 1.081)).1, "sto");
    assert_eq!(f.want(session().as_ref(), Some("sto"), 1.2), Step::Idle, "asked once");
}

#[test]
fn a_newer_search_replaces_the_one_in_flight_and_its_answer_is_dropped() {
    let mut f = FileSearch::default();
    let (old, _) = send(f.want(session().as_ref(), Some("a/"), 0.0));
    let (new, _) = send(f.want(session().as_ref(), Some("b/"), 0.1));
    assert_ne!(old, new);
    f.answered(old, "a/", Ok(vec!["a/x".into()]));
    assert!(f.loading, "the old answer says nothing about what is wanted now");
    assert!(f.files("b/").is_empty());
    f.answered(new, "b/", Ok(vec!["b/y".into()]));
    assert!(!f.loading);
    assert_eq!(f.files("b/"), ["b/y"]);
}

#[test]
fn until_the_server_answers_the_last_answer_is_narrowed_to_what_still_matches() {
    let mut f = FileSearch::default();
    let (g, _) = send(f.want(session().as_ref(), Some(""), 0.0));
    f.answered(g, "", Ok(vec!["src/".into(), "README.md".into(), "Cargo.toml".into()]));
    assert_eq!(f.files(""), ["src/", "README.md", "Cargo.toml"]);
    let _ = f.want(session().as_ref(), Some("read"), 1.0);
    assert_eq!(f.files("read"), ["README.md"], "narrowed while the search waits");
    assert_eq!(f.files("  read "), ["README.md"], "the query is trimmed");
}

#[test]
fn another_session_or_a_closed_list_forgets_the_files() {
    let mut f = FileSearch::default();
    let (g, _) = send(f.want(session().as_ref(), Some(""), 0.0));
    f.answered(g, "", Ok(vec!["src/".into()]));
    let other = Some(Scope::Session("s2".into()));
    let (g2, _) = send(f.want(other.as_ref(), Some(""), 1.0));
    assert!(f.files("").is_empty(), "another session's files say nothing about this one's");
    f.answered(g2, "", Ok(vec!["lib/".into()]));
    assert_eq!(f.want(other.as_ref(), None, 2.0), Step::Idle);
    assert!(f.files("").is_empty() && !f.loading);
    // With no session and no folder there is nothing to ask.
    assert_eq!(f.want(None, Some("x"), 3.0), Step::Idle);
}

#[test]
fn a_failed_search_says_why_and_keeps_the_last_answer() {
    let mut f = FileSearch::default();
    let (g, _) = send(f.want(session().as_ref(), Some(""), 0.0));
    f.answered(g, "", Ok(vec!["src/".into()]));
    let (g, q) = send(f.want(session().as_ref(), Some("zz/"), 1.0));
    f.answered(g, &q, Err("HTTP 500".into()));
    assert_eq!(f.error.as_deref(), Some("HTTP 500"));
    assert!(!f.loading);
    assert_eq!(f.files(""), ["src/"]);
    // Asking again clears it.
    let _ = f.want(session().as_ref(), Some("qq/"), 2.0);
    assert_eq!(f.error, None);
}

#[test]
fn the_error_is_the_servers_own_words_or_http_and_the_status() {
    assert_eq!(failure(400, &json!({"error": "Path not allowed"})), "Path not allowed");
    assert_eq!(failure(503, &json!({})), "HTTP 503");
    assert_eq!(failure(500, &json!({"error": "  "})), "HTTP 500");
}

#[test]
fn the_sessions_are_fetched_when_the_list_opens_and_reused_for_30_seconds() {
    let mut s = Fetched::default();
    assert!(!s.should_fetch(false, 0.0), "closed: nothing to fetch");
    assert!(s.should_fetch(true, 1.0), "opened: fetched");
    assert!(!s.should_fetch(true, 1.5), "still open: not again");
    assert!(!s.should_fetch(false, 2.0));
    assert!(!s.should_fetch(true, 3.0), "the request is still out: shared");
    s.arrived(Ok(vec![]), 4.0);
    assert!(!s.should_fetch(false, 5.0));
    assert!(!s.should_fetch(true, 20.0), "fresh for 30 s");
    assert!(!s.should_fetch(false, 21.0));
    assert!(s.should_fetch(true, 35.0), "stale");
}

#[test]
fn a_failed_fetch_is_swallowed_and_tried_again_on_the_next_open() {
    let mut s = Fetched::default();
    assert!(s.should_fetch(true, 0.0));
    s.arrived(Err("down".into()), 1.0);
    assert!(s.sessions.is_empty());
    assert!(!s.should_fetch(false, 2.0));
    assert!(s.should_fetch(true, 3.0));
}

#[test]
fn commands_arrive_or_fail_and_the_old_list_stays_up_on_a_failure() {
    let mut f = Finder::default();
    f.on_commands(
        "s1",
        Ok((200, json!({"commands": [{"name": "review", "description": "Review", "kind": "command"}, {"name": "tidy", "kind": "skill", "userOnly": true}, {"description": "no name"}]}))),
    );
    let c = &f.commands["s1"];
    assert_eq!(c.items.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["review", "tidy"]);
    assert!(c.items[1].user_only && c.error.is_none());
    f.on_commands("s1", Ok((503, json!({"error": "Harness is starting"}))));
    assert_eq!(f.commands["s1"].items.len(), 2);
    assert_eq!(f.commands["s1"].error.as_deref(), Some("Harness is starting"));
    f.on_commands("s1", Err("connection refused".into()));
    assert_eq!(f.commands["s1"].error.as_deref(), Some("connection refused"));
}

#[test]
fn the_agent_list_is_the_pickers_and_keeps_hidden_agents_and_colours() {
    let mut f = Finder::default();
    let body = json!({"agents": [{"name": "build", "description": "Edits", "mode": "primary", "color": "#3B82F6"}, {"name": "title", "hidden": true}]});
    f.on_agents("s1", Ok(&body));
    let a = &f.agents["s1"].items;
    assert_eq!((a[0].name.as_str(), a[0].color.as_deref(), a[0].mode.as_deref()), ("build", Some("#3B82F6"), Some("primary")));
    assert_eq!(a[1].name, "title", "the harness's list, hidden ones too");
    f.on_agents("s1", Err("HTTP 500"));
    assert_eq!(f.agents["s1"].items.len(), 2);
    assert_eq!(f.agents["s1"].error.as_deref(), Some("HTTP 500"));
}

#[test]
fn the_sessions_list_reads_what_the_mention_needs() {
    let body = json!([
        {"session": {"id": "a", "title": "Fix paging", "time": {"created": 1000, "updated": 5000}}, "sessionStatus": "idle", "harnessType": "opencode", "projectName": "harbor", "workspaceDirectory": "/w/harbor", "retentionStatus": "archived"},
        {"session": {"id": "b", "title": "Child", "time": {"created": 2000}}, "parentSessionId": "a"},
        {"nothing": true}
    ]);
    let s = parse_sessions(&body);
    assert_eq!(s.len(), 2);
    assert!(s[0].archived && s[0].updated == 5000.0 && s[0].project_name.as_deref() == Some("harbor"));
    assert_eq!((s[1].updated, s[1].parent_session_id.as_deref()), (2000.0, Some("a")), "created stands in for updated");
}
