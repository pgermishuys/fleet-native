//! Progress data (component 10, piece F): `session-progress.test.ts` (the detail, the plan, code spans), the store's six
//! cases over a fake server, `use-session-progress.test.ts` and `use-session-progress-updates.test.ts`, under Fleet's names,
//! and the rules `ProgressCanvas.vue` and the strip keep over the data.
use super::super::delegations::DelegationStatus;
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::progress::*;
use super::super::progress_feed::ProgressFeed;
use super::super::progress_line as line;
use super::super::row_chips::Progress;
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use std::io::Write;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

fn detail_wire(session: &str, done: u32, updated: &str) -> Value {
    json!({
        "sessionId": session, "kind": "todos", "done": done, "total": 3, "current": "Next",
        "todos": [{"content": "Next", "status": "in_progress", "priority": "medium"}],
        "updatedAt": updated, "plan": null, "subagents": [],
    })
}

fn detail(done: u32, updated: &str) -> ProgressDetail {
    parse_detail(&detail_wire("s1", done, updated)).unwrap()
}

fn summary(done: u32, current: Option<&str>) -> Progress {
    Progress { kind: "todos".into(), done, total: 3, current: current.map(str::to_string) }
}

// ─── lib/__tests__/session-progress.test.ts ─────────────────────────────────

#[test]
fn session_progress_reads_the_detail_fills_in_missing_priorities_and_drops_todos_without_text() {
    let detail = parse_detail(&json!({
        "sessionId": "s1", "kind": "todos", "done": 1, "total": 2, "current": "Drop the indexes",
        "todos": [
            {"content": "Write the migration", "status": "completed", "priority": "high"},
            {"content": "Drop the indexes", "status": "in_progress", "priority": null},
            {"content": "", "status": "pending"},
            {"status": "pending"},
            {"content": "Bad status", "status": "later"},
        ],
        "updatedAt": "2026-09-13T12:24:00.0000000Z",
    }))
    .unwrap();
    let todo = |content: &str, status, priority| TodoItem { content: content.into(), status, priority };
    assert_eq!(
        detail.todos,
        [
            todo("Write the migration", TodoStatus::Completed, TodoPriority::High),
            todo("Drop the indexes", TodoStatus::InProgress, TodoPriority::Medium),
            todo("Bad status", TodoStatus::Pending, TodoPriority::Medium),
        ]
    );
    assert_eq!(detail.updated_at, "2026-09-13T12:24:00.0000000Z");
    assert_eq!(detail.summary.current.as_deref(), Some("Drop the indexes"));
}

#[test]
fn session_progress_rejects_a_detail_without_an_update_time() {
    assert!(parse_detail(&json!({"sessionId": "s1", "kind": "todos", "done": 0, "total": 1, "todos": []})).is_none());
}

fn plan_wire() -> Value {
    json!({
        "path": ".weave/plans/p.md", "title": "P", "trackedSince": "2026-09-13T11:00:00Z",
        "groups": [
            {"title": "Phase 1", "steps": [{"key": "1", "number": "1", "title": "One", "checked": true, "subDone": 1, "subTotal": 2,
                "tickedAt": "2026-09-13T11:05:00Z", "tickedInMessageId": "m1"}]},
            {"title": "Empty", "steps": []},
            {"title": null, "steps": [{"key": "Two", "number": null, "title": "Two", "checked": false}, {"title": "no key"}]},
        ],
    })
}

fn with_plan(plan: Value) -> ProgressDetail {
    parse_detail(&json!({"sessionId": "s1", "kind": "plan", "done": 1, "total": 2, "todos": [], "updatedAt": "x", "plan": plan})).unwrap()
}

#[test]
fn session_plan_reads_the_plan_in_a_detail_and_drops_empty_groups_and_malformed_steps() {
    let plan = with_plan(plan_wire()).plan.unwrap();
    assert_eq!(plan.groups.iter().map(|g| g.title.as_deref()).collect::<Vec<_>>(), [Some("Phase 1"), None]);
    assert_eq!(
        plan.groups[0].steps[0],
        PlanStep {
            key: "1".into(),
            number: Some("1".into()),
            title: "One".into(),
            checked: true,
            sub_done: 1,
            sub_total: 2,
            ticked_at: Some("2026-09-13T11:05:00Z".into()),
            ticked_in_message_id: Some("m1".into()),
        }
    );
    assert_eq!(
        plan.groups[1].steps,
        [PlanStep {
            key: "Two".into(),
            number: None,
            title: "Two".into(),
            checked: false,
            sub_done: 0,
            sub_total: 0,
            ticked_at: None,
            ticked_in_message_id: None
        }]
    );
    assert_eq!(plan.tracked_since, "2026-09-13T11:00:00Z");
}

#[test]
fn session_plan_has_no_plan_when_the_detail_carries_none() {
    let bare = json!({"sessionId": "s1", "kind": "todos", "done": 0, "total": 1, "todos": [], "updatedAt": "x"});
    assert!(parse_detail(&bare).unwrap().plan.is_none());
    assert!(with_plan(json!({"path": "p.md", "groups": []})).plan.is_none());
    assert!(with_plan(json!({"groups": [{"steps": [{"key": "a", "title": "A"}]}]})).plan.is_none(), "a plan needs its path");
}

#[test]
fn a_plan_without_a_tracking_time_or_a_checked_flag_reads_empty_and_unticked() {
    let plan = with_plan(json!({"path": "p.md", "groups": [{"steps": [{"key": "a", "title": "A", "checked": "yes"}]}]})).plan.unwrap();
    assert_eq!(plan.tracked_since, "");
    assert!(!plan.groups[0].steps[0].checked, "only exactly true ticks a step");
}

#[test]
fn session_plan_finds_the_current_step_the_first_unticked_one() {
    let plan = with_plan(plan_wire()).plan.unwrap();
    let current = current_plan_step(&plan).unwrap();
    assert_eq!((current.step.key.as_str(), current.group_index), ("Two", 1));
    let first_only = Plan { groups: vec![plan.groups[0].clone()], ..plan };
    assert!(current_plan_step(&first_only).is_none());
}

fn span(text: &str, code: bool) -> Span {
    Span { text: text.into(), code }
}

#[test]
fn session_plan_splits_code_spans_and_strips_backticks() {
    assert_eq!(
        code_spans("Map `SessionSnapshot` to `Fleet`"),
        [span("Map ", false), span("SessionSnapshot", true), span(" to ", false), span("Fleet", true)]
    );
    assert_eq!(code_spans("``"), [span("``", false)]);
    assert_eq!(code_spans("`a` `b`"), [span("a", true), span(" ", false), span("b", true)]);
    assert_eq!(code_spans("a ` b"), [span("a ` b", false)]);
    assert_eq!(code_spans(""), []);
    assert_eq!(plain_text("Remove `ApplyStreamingDeltas` from `SessionEventsHub`"), "Remove ApplyStreamingDeltas from SessionEventsHub");
    // The regex pairs a backtick with the next one that has something between them, as Fleet's does.
    assert_eq!(plain_text("keep `` and a lone ` here"), "keep ` and a lone  here");
    assert_eq!(plain_text("no ticks, one ` only"), "no ticks, one ` only");
}

fn sub(title: Option<&str>) -> Subagent {
    Subagent {
        delegation_id: "d".into(),
        child_session_id: None,
        agent: "shuttle".into(),
        title: title.map(str::to_string),
        status: DelegationStatus::Running,
        step_key: None,
        done: 0,
        total: 0,
        current: None,
    }
}

#[test]
fn a_subagents_title_drops_its_trailing_agent_tag() {
    assert_eq!(subagent_title(&sub(Some("Add the cursor (@shuttle subagent)"))).as_deref(), Some("Add the cursor"));
    assert_eq!(subagent_title(&sub(Some("Add the cursor (@Shuttle Subagent)  "))).as_deref(), Some("Add the cursor"));
    assert_eq!(subagent_title(&sub(Some("(@shuttle subagent)"))), None);
    assert_eq!(subagent_title(&sub(Some("Fix (a) thing (@x subagent)"))).as_deref(), Some("Fix (a) thing"));
    assert_eq!(subagent_title(&sub(Some("Plain task"))).as_deref(), Some("Plain task"));
    assert_eq!(subagent_title(&sub(Some("  "))), None);
    assert_eq!(subagent_title(&sub(None)), None);
}

#[test]
fn a_subagent_reads_its_status_and_counts_with_fallbacks() {
    let detail = parse_detail(&json!({
        "sessionId": "s1", "kind": "todos", "done": 0, "total": 0, "updatedAt": "x",
        "subagents": [
            {"delegationId": "d1", "agent": "shuttle", "childSessionId": "c1", "title": "T", "status": "running", "stepKey": "k", "done": 2, "total": 5, "current": "Now"},
            {"delegationId": "d2", "agent": "tapestry", "status": 7, "done": -1, "total": 1.5},
            {"agent": "no id"},
            {"delegationId": "d3"},
        ],
    }))
    .unwrap();
    assert_eq!(detail.subagents.len(), 2);
    assert_eq!((detail.subagents[0].status, detail.subagents[0].done, detail.subagents[0].total), (DelegationStatus::Running, 2, 5));
    assert_eq!(detail.subagents[0].step_key.as_deref(), Some("k"));
    assert_eq!((detail.subagents[1].status, detail.subagents[1].done, detail.subagents[1].total), (DelegationStatus::Pending, 0, 0));
}

// ─── ProgressCanvas.vue's rules ─────────────────────────────────────────────

#[test]
fn a_flat_plan_has_one_group_or_none_and_a_finished_group_starts_folded() {
    let plan = with_plan(plan_wire()).plan.unwrap();
    assert!(!is_flat(&plan));
    assert!(is_flat(&Plan { groups: vec![plan.groups[0].clone()], ..plan.clone() }));
    assert!(!group_open_default(&plan, &plan.groups[0]), "Phase 1 is all ticked");
    assert!(group_open_default(&plan, &plan.groups[1]));
    let flat = Plan { groups: vec![plan.groups[0].clone()], ..plan.clone() };
    assert!(group_open_default(&flat, &flat.groups[0]), "a flat plan is always open");
    assert_eq!(group_key(1, &plan.groups[1]), "1:");
    assert_eq!(group_key(0, &plan.groups[0]), "0:Phase 1");
    assert_eq!(last_tick(&plan.groups[0]), Some("2026-09-13T11:05:00Z"));
    assert_eq!(last_tick(&plan.groups[1]), None);
}

#[test]
fn the_now_line_names_the_group_and_the_step_to_do_next() {
    let plan = with_plan(plan_wire()).plan.unwrap();
    // The current step is in the second group, which has no title, and has no number.
    assert_eq!(now_line(&plan), NowLine { lead: None, rest: "next up: the next step".into() });
    let mut titled = plan.clone();
    titled.groups[1].title = Some("Phase 2".into());
    titled.groups[1].steps[0].number = Some("2".into());
    assert_eq!(now_line(&titled), NowLine { lead: Some("Phase 2".into()), rest: " · next up: step 2".into() });
    let mut flat = Plan { groups: vec![titled.groups[1].clone()], ..titled.clone() };
    assert_eq!(now_line(&flat), NowLine { lead: None, rest: "next up: step 2".into() }, "a flat plan names no group");
    flat.groups[0].steps[0].checked = true;
    assert_eq!(now_line(&flat), NowLine { lead: Some("Every step is ticked".into()), rest: String::new() });
}

#[test]
fn a_subagent_shows_under_its_step_and_any_other_goes_in_the_bottom_list() {
    let mut detail = with_plan(plan_wire());
    let at = |id: &str, key: Option<&str>| Subagent { delegation_id: id.into(), step_key: key.map(str::to_string), ..sub(None) };
    detail.subagents = vec![at("a", Some("1")), at("b", Some("nope")), at("c", None), at("d", Some("Two"))];
    let step = &detail.plan.as_ref().unwrap().groups[1].steps[0];
    assert_eq!(subagents_for(&detail, step).iter().map(|s| s.delegation_id.as_str()).collect::<Vec<_>>(), ["d"]);
    assert_eq!(unplaced(&detail).iter().map(|s| s.delegation_id.as_str()).collect::<Vec<_>>(), ["b", "c"]);
    detail.plan = None;
    assert_eq!(unplaced(&detail).len(), 4, "with no plan every subagent is in the list");
}

#[test]
fn a_step_says_ticked_next_up_or_not_started() {
    let plan = with_plan(plan_wire()).plan.unwrap();
    let mut plan = plan;
    let third = PlanStep { key: "3".into(), checked: false, ..plan.groups[1].steps[0].clone() };
    plan.groups[1].steps.push(third);
    assert_eq!(step_state(&plan, &plan.groups[0].steps[0], 0), "ticked at 11:05 AM");
    assert_eq!(step_state(&plan, &plan.groups[1].steps[0], 0), "next up");
    assert_eq!(step_state(&plan, &plan.groups[1].steps[1], 0), "not started");
    plan.groups[0].steps[0].ticked_at = None;
    assert_eq!(step_state(&plan, &plan.groups[0].steps[0], 0), "ticked");
}

#[test]
fn a_time_reads_as_a_clock_and_nothing_reads_as_nothing() {
    assert_eq!(format_time(Some("2026-09-13T14:07:00Z"), 0), "2:07 PM");
    assert_eq!(format_time(Some("2026-09-13T00:05:00Z"), 0), "12:05 AM");
    assert_eq!(format_time(Some("2026-09-13T14:07:00Z"), 2 * 3600), "4:07 PM");
    assert_eq!(format_time(Some(""), 0), "");
    assert_eq!(format_time(Some("soon"), 0), "");
    assert_eq!(format_time(None, 0), "");
}

// ─── the strip's plan form (08 rows 9.2-9.4) ────────────────────────────────

#[test]
fn the_strip_in_the_plan_form_heads_with_the_group_and_names_the_next_step() {
    let mut plan = with_plan(plan_wire()).plan.unwrap();
    plan.groups[1].steps[0].number = Some("2".into());
    plan.groups[1].steps[0].title = "Map `SessionSnapshot` to `Fleet`".into();
    let p = Progress { kind: "plan".into(), done: 1, total: 2, current: Some("x".into()) };
    assert_eq!(line::heading(&p, Some(&plan)), "P", "the current group has no title, so the plan's");
    plan.groups[1].title = Some("The `Fleet` side".into());
    assert_eq!(line::heading(&p, Some(&plan)), "The Fleet side");
    assert_eq!(line::current_line(&p, Some(&plan)), "Next: 2. Map SessionSnapshot to Fleet");
    assert_eq!(line::label(&p, Some(&plan)), "1 of 2 steps done. Next: 2. Map SessionSnapshot to Fleet. Open the Progress tab");
    plan.title = None;
    plan.groups[1].title = None;
    assert_eq!(line::heading(&p, Some(&plan)), ".weave/plans/p.md", "no group title and no plan title: the path");
    plan.groups[1].steps[0].checked = true;
    assert_eq!(line::current_line(&p, Some(&plan)), "Every step is ticked");
    assert_eq!(line::heading(&p, None), "Todos");
}

// ─── stores/__tests__/session-progress.test.ts ──────────────────────────────

#[derive(Default)]
struct Seen {
    /// Each request for a session's progress, in the order they came.
    asked: Vec<String>,
    /// What the server answers: a status (0 hangs up with no answer) and a body.
    answer: (u16, String),
}

struct Live {
    rt: rune::Runtime<State>,
    id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let request = heard.head.lines().next().unwrap_or("").to_string();
    let path = request.split(' ').nth(1).unwrap_or("").to_string();
    let (status, body) = if path.starts_with("/hubs/session-events/negotiate") {
        (200, "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string())
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        (200, fixture("api-sessions.json"))
    } else if path.ends_with("/progress") {
        let mut seen = seen.lock().unwrap();
        seen.asked.push(path.clone());
        let session = path.trim_start_matches("/api/sessions/").trim_end_matches("/progress");
        (seen.answer.0, seen.answer.1.replace("SESSION", session))
    } else {
        (404, "{}".to_string())
    };
    if status == 0 {
        return;
    }
    let reason = if status == 204 { "No Content" } else { "OK" };
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

impl Live {
    /// The fixture's first session open on a server that answers its progress with `answer`.
    fn start(answer: (u16, String)) -> Live {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen { answer, ..Default::default() }));
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
        Live { rt, id, jobs, seen }
    }

    /// Runs the app until `done` holds of its state and the requests the server has seen.
    fn wait(&mut self, what: &str, done: impl Fn(&State, &Seen) -> bool) {
        let seen = self.seen.clone();
        let described = self.seen.clone();
        crate::test_wait::drive(
            &mut self.rt,
            &self.jobs,
            what,
            |s| done(s, &seen.lock().unwrap()),
            |s| format!("{}; asked {:?}", crate::test_wait::app_state(s), described.lock().unwrap().asked),
        );
    }

    fn asked(&self) -> usize {
        self.seen.lock().unwrap().asked.len()
    }

    fn answer(&self, status: u16, body: Value) {
        self.seen.lock().unwrap().answer = (status, body.to_string());
    }
}

/// An answer for whichever session asks: "SESSION" in the body becomes its id.
fn ok(done: u32, updated: &str) -> (u16, String) {
    (200, detail_wire("SESSION", done, updated).to_string())
}

#[test]
fn loads_a_sessions_progress_once() {
    let mut live = Live::start(ok(1, "2026-09-13T12:00:00Z"));
    live.wait("the first load", |s, seen| s.progress.of(&seen_id(s)).is_some() && !seen.asked.is_empty());
    let id = live.id.clone();
    assert_eq!(live.seen.lock().unwrap().asked, [format!("/api/sessions/{id}/progress")]);
    live.rt.update(|s, _| {
        s.ensure_progress_loaded(&id, false);
        s.ensure_progress_loaded(&id, false);
    });
    live.rt.frame(1.0 / 60.0);
    assert_eq!(live.asked(), 1, "a loaded session is not asked again");
    assert_eq!(live.rt.state.progress.of(&id).map(|d| d.summary.done), Some(1));
}

fn seen_id(s: &State) -> String {
    s.open_id().unwrap()
}

#[test]
fn a_second_call_while_one_is_in_flight_shares_it() {
    let mut live = Live::start((204, String::new()));
    let id = live.id.clone();
    // Opening the session started the load, and no job has run yet to finish it.
    assert!(live.rt.state.progress.is_loading(&id));
    live.rt.update(|s, _| {
        s.ensure_progress_loaded(&id, false);
        s.ensure_progress_loaded(&id, true);
    });
    live.wait("the load to finish", |s, _| s.progress.is_loaded(&seen_id(s)));
    assert_eq!(live.asked(), 1);
}

#[test]
fn remembers_that_a_session_has_no_progress() {
    let mut live = Live::start((204, String::new()));
    let id = live.id.clone();
    live.wait("the answer", |s, _| s.progress.is_loaded(&seen_id(s)));
    live.rt.update(|s, _| s.ensure_progress_loaded(&id, false));
    live.rt.frame(1.0 / 60.0);
    assert_eq!(live.asked(), 1);
    assert!(live.rt.state.progress.of(&id).is_none());
    assert!(live.rt.state.progress.is_loaded(&id), "a remembered null counts as loaded");
}

#[test]
fn loads_again_when_forced() {
    let mut live = Live::start(ok(1, "2026-09-13T12:00:00Z"));
    live.wait("the first load", |s, _| s.progress.of(&seen_id(s)).is_some());
    live.answer(200, detail_wire("SESSION", 2, "2026-09-13T12:05:00Z"));
    live.rt.update(|s, _| s.ensure_progress_loaded(&seen_id(s), true));
    live.wait("the forced load", |s, _| s.progress.of(&seen_id(s)).is_some_and(|d| d.summary.done == 2));
    assert_eq!(live.asked(), 2);
}

#[test]
fn keeps_a_newer_push_over_an_older_load() {
    let mut feed = ProgressFeed::default();
    feed.apply(detail(2, "2026-09-13T12:05:00Z"));
    feed.apply(detail(1, "2026-09-13T12:00:00Z"));
    assert_eq!(feed.of("s1").map(|d| d.summary.done), Some(2));
    feed.apply(detail(3, "2026-09-13T12:10:00Z"));
    assert_eq!(feed.of("s1").map(|d| d.summary.done), Some(3));
}

#[test]
fn forgets_a_loaded_detail_when_a_row_summary_shows_its_out_of_date() {
    let mut feed = ProgressFeed::default();
    feed.apply(detail(1, "2026-09-13T12:00:00Z"));
    feed.note_summary("s1", &summary(1, Some("Next")));
    assert!(feed.is_loaded("s1"));
    assert_eq!(feed.of("s1").map(|d| d.summary.done), Some(1));
    feed.note_summary("s1", &summary(2, Some("Later")));
    assert!(!feed.is_loaded("s1"), "the next view asks again");
    assert_eq!(
        feed.of("s1").map(|d| d.summary.done),
        Some(1),
        "but the tab draws the old detail until the new one lands, not its empty state"
    );
    feed.apply(detail(2, "2026-09-13T12:01:00Z"));
    assert!(feed.is_loaded("s1"));
    feed.note_summary("other", &summary(9, None));
    assert!(!feed.is_loaded("other"), "a session never asked about stays never asked");
}

#[test]
fn keeps_working_when_the_request_fails() {
    for answer in [(500, "{}".to_string()), (0, String::new()), (200, "not json".to_string()), (200, r#"{"sessionId":"s"}"#.to_string())] {
        let mut live = Live::start(answer.clone());
        let id = live.id.clone();
        live.wait("the request", |s, seen| !seen.asked.is_empty() && !s.progress.is_loading(&seen_id(s)));
        assert!(live.rt.state.progress.of(&id).is_none(), "{answer:?}");
        assert!(!live.rt.state.progress.is_loaded(&id), "{answer:?}: nothing is stored, so the next view asks again");
        live.answer(204, json!(null));
        live.rt.update(|s, _| s.ensure_progress_loaded(&seen_id(s), false));
        live.wait("the second ask", |s, _| s.progress.is_loaded(&seen_id(s)));
        assert_eq!(live.asked(), 2);
    }
}

#[test]
fn a_204_does_not_wipe_a_detail_that_a_push_brought_meanwhile() {
    let mut live = Live::start((204, String::new()));
    let id = live.id.clone();
    live.rt.update(|s, _| s.dispatch(&Topic::Session(seen_id(s)), &[push(&detail_wire(&seen_id(s), 2, "2026-09-13T12:00:00Z"))]));
    live.wait("the 204", |s, seen| !seen.asked.is_empty() && !s.progress.is_loading(&seen_id(s)));
    assert_eq!(live.rt.state.progress.of(&id).map(|d| d.summary.done), Some(2));
}

// ─── composables/__tests__/use-session-progress.test.ts ─────────────────────

fn push(detail: &Value) -> Event {
    Event::Other(Kind::ProgressUpdated, detail.clone())
}

#[test]
fn use_session_progress_loads_the_sessions_todos_and_follows_pushes_on_its_topic() {
    let mut live = Live::start(ok(1, "2026-09-13T12:00:00Z"));
    live.wait("the load", |s, _| s.progress.of(&seen_id(s)).is_some());
    let id = live.id.clone();
    live.rt.update(|s, _| s.dispatch(&Topic::Session(seen_id(s)), &[push(&detail_wire(&seen_id(s), 2, "2026-09-13T12:05:00Z"))]));
    assert_eq!(live.rt.state.progress.of(&id).map(|d| d.summary.done), Some(2));
    assert_eq!(live.rt.state.progress.of(&id).map(|d| d.todos.len()), Some(1));
}

#[test]
fn use_session_progress_moves_to_the_new_sessions_topic_when_the_session_changes() {
    let mut rt = app();
    rt.update(|s, _| {
        s.dispatch(&Topic::Session("b".into()), &[push(&detail_wire("b", 2, "2026-09-13T12:00:00Z"))]);
        s.dispatch(&Topic::Session("b".into()), &[push(&detail_wire("a", 3, "2026-09-13T12:00:00Z"))]);
        assert_eq!(s.progress.of("b").map(|d| d.summary.done), Some(2));
        assert!(s.progress.of("a").is_none(), "a push is applied only on its own session's topic");
        s.dispatch(&Topic::Session("a".into()), &[push(&detail_wire("a", 1, "2026-09-13T12:00:00Z"))]);
        assert_eq!(s.progress.of("a").map(|d| d.summary.done), Some(1));
    });
}

#[test]
fn use_session_progress_loads_again_after_a_reconnect() {
    let mut live = Live::start(ok(1, "2026-09-13T12:00:00Z"));
    live.wait("the load", |s, _| s.progress.of(&seen_id(s)).is_some());
    live.answer(200, detail_wire("SESSION", 3, "2026-09-13T13:00:00Z"));
    live.rt.update(|s, _| s.on_reconnected());
    live.wait("the reload", |s, _| s.progress.of(&seen_id(s)).is_some_and(|d| d.summary.done == 3));
    assert_eq!(live.asked(), 2);
}

// ─── composables/__tests__/use-session-progress-updates.test.ts ─────────────

fn summary_event(done: u32, current: &str) -> Event {
    Event::Other(Kind::SessionProgress, json!({"sessionId": "s1", "kind": "todos", "done": done, "total": 3, "current": current}))
}

#[test]
fn use_session_progress_updates_drops_a_loaded_detail_that_the_summary_shows_is_out_of_date() {
    let mut rt = app();
    rt.update(|s, _| {
        s.progress.apply(detail(1, "2026-09-13T12:00:00Z"));
        s.dispatch(&Topic::Sessions, &[summary_event(1, "Next")]);
        assert!(s.progress.is_loaded("s1"));
        s.dispatch(&Topic::Sessions, &[summary_event(2, "Later")]);
        assert!(!s.progress.is_loaded("s1"));
    });
}

#[test]
fn use_session_progress_updates_ignores_other_events_and_malformed_payloads() {
    let mut rt = app();
    rt.update(|s, _| {
        s.progress.apply(detail(1, "2026-09-13T12:00:00Z"));
        let wrong_type = Event::Other(Kind::SessionTokens, json!({"sessionId": "s1", "kind": "todos", "done": 2, "total": 3}));
        let malformed = Event::Other(Kind::SessionProgress, json!({"sessionId": "s1", "kind": "todos", "done": "many", "total": 3}));
        s.dispatch(&Topic::Sessions, &[wrong_type, malformed]);
        assert!(s.progress.is_loaded("s1"));
        // `progress.updated` on the sessions topic is not a session's own push.
        s.dispatch(&Topic::Sessions, &[push(&detail_wire("s1", 3, "2026-09-13T12:30:00Z"))]);
        assert_eq!(s.progress.of("s1").map(|d| d.summary.done), Some(1));
    });
}
