//! Mission control's data (component 23, piece B): the cases of weave-fleet's `mission-control.test.ts` and
//! `mission-peek.test.ts` under their names, and the words an answer's outcome gets.
use crate::asks::question::{QuestionInfo, QuestionOption};
use crate::fleet::asks::Reply;
use crate::fleet::mission::peek::{PeekEvent, peek_events};
use crate::fleet::mission::requests::{nudge_request, outcome, permission_answer_request, question_answer_request, steers};
use crate::fleet::mission::{
    Ask, FINISHED_FOR_MS, FeedStatus, Grouping, MissionMachine, NowKind, SessionNow, Summary, build_tiles, group_tiles, needs_you,
    summarize,
};
use crate::fleet::model::{Msg, PartData};
use crate::fleet::sessions::parse_sessions_for;
use crate::fleet::store::Retention;
use crate::view::conversation::Role;
use serde_json::{Value, json};
use std::collections::HashSet;

const NOW: i64 = 1_793_102_400_000; // 2026-10-27T12:00:00Z
const MINUTE: i64 = 60_000;

fn minutes_ago(minutes: i64) -> i64 {
    NOW - minutes * MINUTE
}

/// A list row the way the server writes it; `patch` overrides or adds top-level fields.
fn row(id: &str, updated: i64, patch: Value) -> Value {
    let mut row = json!({
        "session": {"id": id, "title": id, "time": {"created": updated - 1000, "updated": updated}},
        "sessionStatus": "active", "activityStatus": "busy", "retentionStatus": "active", "isHidden": false,
        "harnessType": "opencode2", "projectName": "ledger-api", "workspaceDirectory": "/work/ledger-api", "branch": "main",
    });
    for (key, value) in patch.as_object().into_iter().flatten() {
        row[key] = value.clone();
    }
    row
}

fn machine(id: &str, name: &str, rows: Vec<Value>) -> MissionMachine {
    MissionMachine {
        id: id.into(),
        name: name.into(),
        status: FeedStatus::Live,
        sessions: parse_sessions_for(&Value::Array(rows), 0, Retention::All),
        ..Default::default()
    }
}

fn question() -> Ask {
    Ask::Question {
        request_id: "q1".into(),
        question: QuestionInfo {
            header: "Retries".into(),
            question: "Retry three times or five?".into(),
            options: vec![QuestionOption { label: "Three".into(), description: String::new() }],
            multiple: false,
            custom: true,
        },
        more: 0,
    }
}

fn none() -> HashSet<String> {
    HashSet::new()
}

fn ids(tiles: &[crate::fleet::mission::Tile]) -> Vec<&str> {
    tiles.iter().map(|t| t.session_id.as_str()).collect()
}

// ─── mission-control.test.ts: "mission control tiles" ───────────────────────

#[test]
fn leaves_out_a_session_archived_a_moment_ago() {
    let machines = [machine("home", "atlas", vec![row("a", minutes_ago(1), json!({})), row("b", minutes_ago(1), json!({}))])];
    let archiving: HashSet<String> = ["a".to_string()].into();
    assert_eq!(ids(&build_tiles(&machines, NOW, FINISHED_FOR_MS, &archiving)), ["b"]);
    let mut all = ids(&build_tiles(&machines, NOW, FINISHED_FOR_MS, &none())).into_iter().map(str::to_string).collect::<Vec<_>>();
    all.sort();
    assert_eq!(all, ["a", "b"]);
}

#[test]
fn makes_a_tile_for_every_machines_working_and_waiting_session_newest_first_and_drops_whats_old_archived_or_a_helper() {
    let tiles = build_tiles(
        &[
            machine(
                "home",
                "atlas",
                vec![
                    row("a-work", minutes_ago(2), json!({})),
                    row("a-old", minutes_ago(60 * 30), json!({"sessionStatus": "idle", "activityStatus": "idle"})),
                    row("a-archived", minutes_ago(1), json!({"retentionStatus": "archived"})),
                    row("a-helper", minutes_ago(1), json!({"parentSessionId": "a-work"})),
                ],
            ),
            machine("m2", "mini", vec![row("m-wait", minutes_ago(1), json!({"sessionStatus": "waiting_input"}))]),
        ],
        NOW,
        FINISHED_FOR_MS,
        &none(),
    );
    assert_eq!(tiles.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), ["m2:m-wait", "home:a-work"]);
    assert_eq!(tiles.iter().map(|t| t.machine_name.as_str()).collect::<Vec<_>>(), ["mini", "atlas"]);
}

#[test]
fn shows_the_machines_now_line_while_working_the_question_while_it_waits_and_the_state_when_it_knows_nothing() {
    let mut atlas = machine(
        "home",
        "atlas",
        vec![
            row("working", minutes_ago(1), json!({})),
            row("waiting", minutes_ago(2), json!({"sessionStatus": "waiting_input"})),
            row("quiet-machine", minutes_ago(3), json!({})),
            row("stopped", minutes_ago(4), json!({"sessionStatus": "error"})),
        ],
    );
    atlas.now.insert("working".into(), SessionNow { text: "bash · pnpm test webhooks".into(), kind: NowKind::Tool, at: String::new() });
    atlas.asks.insert("waiting".into(), question());
    let tiles = build_tiles(&[atlas], NOW, FINISHED_FOR_MS, &none());
    let by = |id: &str| tiles.iter().find(|t| t.session_id == id).unwrap().now.clone();
    assert_eq!(by("working"), "bash · pnpm test webhooks");
    assert_eq!(by("waiting"), "? Retry three times or five?");
    assert_eq!(by("quiet-machine"), "Working");
    assert_eq!(by("stopped"), "Stopped with an error");
}

#[test]
fn names_the_harness_the_model_the_todos_and_the_helpers() {
    let patch = json!({
        "harnessType": "claude-code",
        "selectedModel": {"providerID": "anthropic", "modelID": "sonnet-5.5"},
        "progress": {"sessionId": "s", "kind": "todos", "done": 3, "total": 7},
        "runningWorkCount": 2,
    });
    let tiles = build_tiles(&[machine("home", "atlas", vec![row("s", minutes_ago(1), patch)])], NOW, FINISHED_FOR_MS, &none());
    let tile = &tiles[0];
    assert_eq!(
        (tile.harness.as_str(), tile.model.as_deref(), tile.todos, tile.helpers),
        ("Claude Code", Some("sonnet-5.5"), Some((3, 7)), 2)
    );
}

#[test]
fn marks_tiles_of_a_machine_that_stopped_answering() {
    let mut mini = machine("m2", "mini", vec![row("s", minutes_ago(1), json!({}))]);
    mini.status = FeedStatus::Unreachable;
    assert!(build_tiles(&[mini], NOW, FINISHED_FOR_MS, &none())[0].stale);
}

// ─── mission-control.test.ts: "grouping and what needs you" ─────────────────

fn wall() -> Vec<crate::fleet::mission::Tile> {
    let progress = json!({"sessionId": "m1", "kind": "todos", "done": 1, "total": 2});
    let mut atlas = machine(
        "home",
        "atlas",
        vec![
            row("a1", minutes_ago(1), json!({"projectName": "ledger-api"})),
            row("a2", minutes_ago(2), json!({"projectName": "docs-site", "sessionStatus": "waiting_input"})),
        ],
    );
    atlas.asks.insert("a2".into(), question());
    let mini = machine(
        "m2",
        "mini",
        vec![
            row("m1", minutes_ago(3), json!({"projectName": "ledger-api", "progress": progress, "runningWorkCount": 2})),
            row("m2", minutes_ago(4), json!({"projectName": "ledger-ios", "sessionStatus": "error"})),
        ],
    );
    build_tiles(&[atlas, mini], NOW, FINISHED_FOR_MS, &none())
}

#[test]
fn groups_by_folder_across_machines_alphabetically() {
    let groups = group_tiles(&wall(), Grouping::Folder);
    let shown: Vec<(&str, Vec<&str>)> = groups.iter().map(|g| (g.label.as_str(), ids(&g.tiles))).collect();
    assert_eq!(shown, [("docs-site", vec!["a2"]), ("ledger-api", vec!["a1", "m1"]), ("ledger-ios", vec!["m2"])]);
}

#[test]
fn groups_by_machine_and_by_state_with_what_needs_you_first() {
    let tiles = wall();
    let groups = group_tiles(&tiles, Grouping::Machine);
    let by_machine: Vec<(&str, usize)> = groups.iter().map(|g| (g.label.as_str(), g.tiles.len())).collect();
    assert_eq!(by_machine, [("atlas", 2), ("mini", 2)]);
    let by_state: Vec<String> = group_tiles(&tiles, Grouping::State).into_iter().map(|g| g.label).collect();
    assert_eq!(by_state, ["Needs you", "Stopped with an error", "Working"]);
}

#[test]
fn lists_what_waits_on_the_user_and_counts_what_runs() {
    let tiles = wall();
    assert_eq!(needs_you(&tiles).iter().map(|t| t.session_id.as_str()).collect::<Vec<_>>(), ["a2", "m2"]);
    assert_eq!(summarize(&tiles), Summary { running: 2, helpers: 2, need_you: 2 });
}

#[test]
fn a_folder_less_session_is_grouped_as_no_folder_and_named_from_its_path() {
    let bare = json!({"projectName": null, "workspaceDirectory": "/work/", "workspaceDisplayName": null});
    let tiles = build_tiles(&[machine("home", "atlas", vec![row("s", minutes_ago(1), bare)])], NOW, FINISHED_FOR_MS, &none());
    assert_eq!(tiles[0].folder.as_deref(), Some("work"));
    let none_at_all = json!({"projectName": null, "workspaceDirectory": ""});
    let tiles = build_tiles(&[machine("home", "atlas", vec![row("s", minutes_ago(1), none_at_all)])], NOW, FINISHED_FOR_MS, &none());
    assert_eq!(group_tiles(&tiles, Grouping::Folder)[0].label, "No folder");
}

// ─── mission-peek.test.ts ───────────────────────────────────────────────────

fn message(id: &str, role: Role, parts: Vec<PartData>) -> Msg {
    let mut m = Msg::new(id, "s", role);
    m.parts = parts;
    m.created_at = Some(NOW);
    m
}

fn tool(name: &str, input: Value) -> PartData {
    PartData::Tool { id: "p".into(), tool: name.into(), call_id: "c".into(), state: json!({"status": "completed", "input": input}) }
}

fn text(body: &str) -> PartData {
    PartData::Text { id: "t".into(), text: body.into() }
}

fn lines(events: &[PeekEvent]) -> Vec<&str> {
    events.iter().map(|e| e.text.as_str()).collect()
}

#[test]
fn reads_tool_calls_and_replies_of_the_assistant_oldest_first_and_leaves_the_users_words_out() {
    let events = peek_events(
        &[
            message("m1", Role::User, vec![text("please fix it")]),
            message(
                "m2",
                Role::Assistant,
                vec![
                    tool("Read", json!({"filePath": "src/webhooks/retry.ts"})),
                    tool("bash", json!({"command": "pnpm test webhooks\nsecond line"})),
                    text("Found three retry sites.\nMore."),
                ],
            ),
        ],
        6,
    );
    assert_eq!(lines(&events), ["read · src/webhooks/retry.ts", "bash · pnpm test webhooks", "Found three retry sites."]);
    assert_eq!(events[0].key, "m2:0");
}

#[test]
fn keeps_only_the_newest_few() {
    let parts = (0..9).map(|i| tool("read", json!({"path": format!("f{i}")}))).collect();
    assert_eq!(lines(&peek_events(&[message("m", Role::Assistant, parts)], 3)), ["read · f6", "read · f7", "read · f8"]);
}

#[test]
fn cuts_a_long_line_at_ninety_characters_and_skips_a_blank_target() {
    let long = "x".repeat(200);
    let events = peek_events(&[message("m", Role::Assistant, vec![tool("grep", json!({"pattern": "  ", "query": long}))])], 6);
    assert_eq!(events[0].text.chars().count(), "grep · ".chars().count() + 90);
    assert!(events[0].text.ends_with('…'));
}

// ─── mission-peek.test.ts: "a nudge" ────────────────────────────────────────

#[test]
fn goes_into_the_running_turn_where_the_harness_steers() {
    let request = nudge_request("s1", "also cover timeouts", true, true, "u1");
    assert_eq!(request.path, "/api/sessions/s1/prompt");
    let body = request.body.unwrap();
    assert_eq!((body["text"].as_str(), body["delivery"].as_str()), (Some("also cover timeouts"), Some("steer")));
}

#[test]
fn waits_in_the_queue_where_it_doesnt() {
    let request = nudge_request("s1", "also cover timeouts", true, false, "u1");
    assert_eq!(request.path, "/api/sessions/s1/queue");
    assert_eq!(request.body, Some(json!({"text": "also cover timeouts"})));
}

#[test]
fn is_just_a_prompt_for_a_session_that_isnt_working() {
    let request = nudge_request("s1", "go on", false, false, "u1");
    assert_eq!(request.path, "/api/sessions/s1/prompt");
    let body = request.body.unwrap();
    assert!(body.get("delivery").is_none());
    assert_eq!(body["userMessageId"], "u1");
}

// ─── the answers and their words ────────────────────────────────────────────

#[test]
fn answers_go_to_the_asks_own_session() {
    let permission = permission_answer_request("sub-1", "per 1", Reply::Always, None);
    assert_eq!((permission.method, permission.path.as_str()), ("POST", "/api/sessions/sub-1/permissions/per%201"));
    assert_eq!(permission.body, Some(json!({"reply": "always"})));
    let rejected = permission_answer_request("s1", "p", Reply::Reject, Some("not that"));
    assert_eq!(rejected.body, Some(json!({"reply": "reject", "message": "not that"})));
    let answer = question_answer_request("s1", "q1", &[vec!["Three".to_string()]]);
    assert_eq!(answer.path, "/api/sessions/s1/questions/q1/answer");
    assert_eq!(answer.body, Some(json!({"answers": [["Three"]]})));
}

fn said(result: Result<(u16, Value), String>) -> (bool, bool, Option<String>) {
    let o = outcome(&result);
    (o.ok, o.gone, o.error)
}

#[test]
fn says_what_an_answer_came_to() {
    assert_eq!(said(Ok((204, Value::Null))), (true, false, None));
    assert_eq!(said(Ok((404, Value::Null))), (false, true, Some("Already answered.".into())));
    assert_eq!(said(Ok((409, json!({"error": "no"})))), (false, true, Some("Already answered.".into())));
    assert_eq!(said(Ok((500, json!({"detail": "The harness is stopped."})))), (false, false, Some("The harness is stopped.".into())));
    assert_eq!(said(Ok((502, json!({"title": "Bad gateway"})))), (false, false, Some("Bad gateway".into())));
    assert_eq!(said(Ok((503, Value::Null))), (false, false, Some("It answered 503.".into())));
    assert_eq!(said(Err("timed out".into())), (false, false, Some("Couldn't reach the machine.".into())));
}

#[test]
fn a_harness_steers_when_its_machines_list_says_so() {
    let list = json!([
        {"type": "opencode2", "capabilities": {"supportsSteering": true}},
        {"type": "copilot", "capabilities": {"supportsSteering": false}},
        {"type": "pi"},
    ]);
    assert!(steers(Some(&list), "opencode2"));
    assert!(!steers(Some(&list), "copilot"));
    assert!(!steers(Some(&list), "pi"));
    assert!(!steers(Some(&list), "claude-code"));
    assert!(!steers(None, "opencode2"));
}

#[test]
fn a_tile_whose_machine_left_the_list_says_so() {
    use crate::fleet::machine::Machine;
    use crate::fleet::machines::Machines;
    use crate::fleet::mission::requests::{MACHINE_GONE, machine_key};
    let tiles = build_tiles(&[machine("m2", "mini", vec![row("s", minutes_ago(1), json!({}))])], NOW, FINISHED_FOR_MS, &none());
    let mut machines = Machines::default();
    assert_eq!(machine_key(&machines, &tiles[0]), Err(MACHINE_GONE));
    machines.list.push(Machine::from_json(&json!({"id": "m2", "name": "mini", "baseUrl": "http://mini:5732"})).unwrap());
    assert_eq!(machine_key(&machines, &tiles[0]), Ok("m2".to_string()));
    // Home's tiles carry the feed's key, `home`, which is not an id the list holds.
    let home = build_tiles(&[machine("home", "atlas", vec![row("h", minutes_ago(1), json!({}))])], NOW, FINISHED_FOR_MS, &none());
    assert_eq!(machine_key(&Machines::default(), &home[0]), Ok("home".to_string()));
}
