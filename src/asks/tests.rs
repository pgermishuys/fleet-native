//! `lib/__tests__/permission-wording.test.ts`, `allowed-rule-wording.test.ts` and the reading half of
//! `use-session-permissions.test.ts`, under Fleet's names (as snake_case, with its sentence in a comment).
use super::permission::{HandoffAsk, PermissionAsk, to_permission_ask};
use super::wording::*;
use crate::conv::tools::PermissionKind as K;
use serde_json::json;

fn ask(kind: K, tool: &str) -> PermissionAsk {
    let mut ask = PermissionAsk::new(kind, tool);
    ask.title = Some("the thing".into());
    ask.always = vec!["*".into()];
    ask
}

fn cases() -> Vec<PermissionAsk> {
    vec![
        ask(K::Shell, "bash"),
        ask(K::Edit, "edit"),
        ask(K::Read, "read"),
        ask(K::Web, "webfetch"),
        ask(K::Other, "mcp__acme__lookup_order"),
        ask(K::Other, "external_directory"),
    ]
}

fn dont(lead: &str, code: Option<&str>) -> DontAsk {
    DontAsk { lead: lead.into(), code: code.map(str::to_string) }
}

#[test]
fn the_desktop_cards_heading_and_dont_ask_again_line_per_kind() {
    // "the desktop card's heading and 'Don't ask again' line, per kind"
    let got: Vec<(String, DontAsk)> = cases().iter().map(|a| (permission_heading(a), dont_ask_again_wording(a))).collect();
    let want = vec![
        ("Run a command", dont("Don't ask again for commands", None)),
        ("Edit a file", dont("Don't ask again for file edits", None)),
        ("Read a file", dont("Don't ask again for", Some("read"))),
        ("Go online", dont("Don't ask again for web access", None)),
        ("Use mcp__acme__lookup_order", dont("Don't ask again for", Some("mcp__acme__lookup_order"))),
        ("Work outside the folder", dont("Don't ask again for", Some("external_directory"))),
    ];
    assert_eq!(got, want.into_iter().map(|(h, d)| (h.to_string(), d)).collect::<Vec<_>>());
}

#[test]
fn the_card_names_the_harnesss_patterns_when_it_sent_some() {
    let mut a = ask(K::Shell, "bash");
    a.always = vec!["dotnet test *".into(), "*".into()];
    assert_eq!(dont_ask_again_wording(&a), dont("Don't ask again for", Some("dotnet test *")));
}

#[test]
fn the_phones_heading_and_dont_ask_again_per_kind() {
    // The shared wording half: the heading and the line are the same functions the card reads.
    let headings: Vec<String> = cases().iter().map(permission_heading).collect();
    assert_eq!(
        headings,
        ["Run a command", "Edit a file", "Read a file", "Go online", "Use mcp__acme__lookup_order", "Work outside the folder"]
    );
    let mut a = ask(K::Shell, "bash");
    a.always = vec!["dotnet test *".into()];
    assert_eq!(dont_ask_again_wording(&a), dont("Don't ask again for", Some("dotnet test *")));
}

#[test]
fn a_read_ask_for_a_tool_that_is_not_a_file_read_keeps_the_tools_name() {
    for tool in ["todowrite", "task", "skill", "fleet_page_show"] {
        assert_eq!(permission_heading(&ask(K::Read, tool)), format!("Use {tool}"));
    }
}

#[test]
fn the_phone_inboxs_one_line_preview_per_kind() {
    let got: Vec<Preview> = cases().iter().map(ask_preview_wording).collect();
    let line = |lead: &str| Preview { lead: lead.into(), detail: Some("the thing".into()), code: true };
    assert_eq!(
        got,
        vec![
            line("Wants to run"),
            line("Wants to edit"),
            line("Wants to read"),
            line("Wants to open"),
            line("Wants to use mcp__acme__lookup_order"),
            line("Wants to use external_directory"),
        ]
    );
}

fn handoff() -> PermissionAsk {
    let mut a = ask(K::Other, "fleet_session_start");
    a.title = Some("Start a session on taron".into());
    a.always = Vec::new();
    a.handoff = Some(HandoffAsk {
        machine: "taron".into(),
        os: None,
        folder: None,
        branch: None,
        harness: None,
        task: Some("Run the installer tests.".into()),
        session: None,
        level: None,
    });
    a
}

#[test]
fn is_headed_by_what_the_agent_wants_there_and_always_is_for_that_machine_on_the_phone_too() {
    let a = handoff();
    assert_eq!(permission_heading(&a), "Start a session on taron");
    assert_eq!(dont_ask_again_wording(&a), dont("Always for taron", None));
    assert_eq!(
        ask_preview_wording(&a),
        Preview { lead: "Wants to send work to taron".into(), detail: Some("Run the installer tests.".into()), code: false }
    );
    let mut untitled = handoff();
    untitled.title = None;
    assert_eq!(permission_heading(&untitled), "Send work to taron");
}

fn rule(kind: K, tool: &str, patterns: &[&str]) -> RuleWording {
    allowed_rule_wording(kind, tool, patterns)
}

#[test]
fn says_what_a_command_pattern_lets_the_agent_run() {
    assert_eq!(
        rule(K::Shell, "bash", &["bun test*"]),
        RuleWording { lead: "Run".into(), code: Some("bun test*".into()), tail: " without asking", broad: false }
    );
}

#[test]
fn lists_every_pattern_one_click_allowed() {
    assert_eq!(rule(K::Shell, "bash", &["git status*", "git diff*"]).code.as_deref(), Some("git status*, git diff*"));
}

#[test]
fn marks_a_rule_with_no_pattern_as_broad() {
    let shell = rule(K::Shell, "bash", &["*"]);
    assert_eq!((shell.lead.as_str(), shell.code, shell.broad), ("Run any command", None, true));
    let edit = rule(K::Edit, "edit", &["*"]);
    assert_eq!((edit.lead.as_str(), edit.broad), ("Edit files", true));
    let web = rule(K::Web, "WebFetch", &[]);
    assert_eq!((web.lead.as_str(), web.broad), ("Go online", true));
}

#[test]
fn names_another_tool() {
    let page = rule(K::Other, "fleet_page_show", &["*"]);
    assert_eq!((page.lead.as_str(), page.code.as_deref(), page.broad), ("Use", Some("fleet_page_show"), true));
    assert_eq!(rule(K::Other, "mcp__acme__lookup", &["orders/*"]).code.as_deref(), Some("mcp__acme__lookup on orders/*"));
}

#[test]
fn an_edits_diff_drops_the_file_headers_and_counts_what_it_adds_and_removes() {
    // "shows an edit's diff without the file headers, with what it adds and removes"
    let detail = "Index: src/a.py\n===================================================================\n--- src/a.py\n+++ src/a.py\n@@ -1,3 +1,3 @@\n keep\n-old\n+new\n+newer\n\n";
    let lines = diff_lines(Some(detail));
    assert_eq!(lines.first().map(|l| (l.kind, l.text.as_str())), Some((LineKind::Hunk, "@@ -1,3 +1,3 @@")));
    assert!(lines.iter().all(|l| !l.text.starts_with("Index: ") && !l.text.starts_with("---") && !l.text.starts_with("+++")));
    assert_eq!(counts(&lines), (2, 1));
}

#[test]
fn a_write_that_changes_nothing_has_no_lines_to_count() {
    // "shows no lines for a write that changes nothing"
    let lines = diff_lines(Some("Index: a.py\n=====\n--- a.py\n+++ a.py"));
    assert_eq!(counts(&lines), (0, 0));
    assert_eq!(counts(&diff_lines(None)), (0, 0));
}

#[test]
fn an_ask_is_read_as_the_server_sends_it() {
    let a = to_permission_ask(&json!({
        "id": "per_9", "sessionId": "s1", "kind": "shell", "tool": "bash", "title": "git push", "detail": "",
        "directory": "/work/shop", "always": ["git push *", "", 3], "subagent": "explorer", "askedAt": "2026-10-01T10:00:00Z"
    }))
    .unwrap();
    assert_eq!((a.kind, a.title.as_deref(), a.detail, a.always, a.subagent.as_deref()), (K::Shell, Some("git push"), None, vec!["git push *".to_string()], Some("explorer")));
}

#[test]
fn an_unknown_kind_reads_as_other_and_an_ask_missing_its_ids_is_ignored() {
    // "ignores an ask it can't read" (the reading half)
    let odd = to_permission_ask(&json!({"id": "p", "sessionId": "s", "kind": "teleport", "tool": "x"})).unwrap();
    assert_eq!(odd.kind, K::Other);
    assert!(to_permission_ask(&json!({"sessionId": "s", "tool": "x"})).is_none());
    assert!(to_permission_ask(&json!({"id": "p", "tool": "x"})).is_none());
    assert!(to_permission_ask(&json!({"id": "p", "sessionId": "s"})).is_none());
    assert!(to_permission_ask(&json!("nope")).is_none());
}

#[test]
fn the_other_machines_rows_say_what_goes_there() {
    let h = HandoffAsk {
        machine: "taron".into(),
        os: Some("windows".into()),
        folder: Some("C:\\work\\shop".into()),
        branch: None,
        harness: None,
        task: Some("Run the installer tests.".into()),
        session: None,
        level: Some("edits".into()),
    };
    let rows = handoff_rows(&h);
    let labels: Vec<&str> = rows.iter().map(|r| r.0).collect();
    assert_eq!(labels, ["Machine", "Folder", "Branch", "Harness", "Task"]);
    assert_eq!(rows[0].1, "taron · Windows");
    assert_eq!((rows[2].1.as_str(), rows[3].1.as_str()), ("the folder as it is", "its default"));
    assert_eq!(level_name(h.level.as_deref()), "Allow edits");
    assert_eq!(level_name(None), "unknown (that Fleet doesn't say)");
}
