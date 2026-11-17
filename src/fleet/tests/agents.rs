//! The Agents tab's data (`fleet::agents`): the Agents cases of `lib/__tests__/session-lineage.test.ts` ("buildAgentsLineage",
//! "summarizeAgentActivity", and the Agents half of "moving a session out of its parent") under Fleet's names, and the rows of
//! `AgentsCanvas.vue`, `AgentsCanvasRow.vue` and `AgentDetail.vue` that are logic. Names and ids are invented.
use crate::composer::choice::ModelOption;
use crate::composer::status::HarnessInfo;
use crate::fleet::agents::*;
use crate::fleet::groups::LineageKind;
use crate::fleet::model::{Msg, PartData, Tokens};
use crate::fleet::sessions::parse_sessions;
use crate::fleet::work::{RunningWorkItem, WorkKind, WorkStatus, to_item};
use crate::mock::Session;
use crate::view::conversation::Role;
use serde_json::json;
use std::collections::HashMap;

const NOW: i64 = 1_791_367_200_000; // 2026-10-07T10:00:00Z

fn session(id: &str, title: &str, extra: serde_json::Value) -> Session {
    let mut row = json!({
        "session": {"id": id, "title": title, "time": {"created": 1, "updated": 2}},
        "sessionStatus": "idle", "activityStatus": "idle", "projectName": "tidytempo", "harnessType": "claude-code",
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        row[k] = v.clone();
    }
    parse_sessions(&json!([row]), 0).remove(0)
}

fn fork(id: &str, of: &str, status: &str) -> Session {
    session(id, &format!("Fork {id}"), json!({"forkedFromSessionId": of, "spawnKind": "fork", "sessionStatus": status}))
}

fn started(id: &str, by: &str, status: &str) -> Session {
    session(id, &format!("Started {id}"), json!({"spawnedBySessionId": by, "sessionStatus": status}))
}

fn work(id: &str, kind: &str, child: Option<&str>, status: &str) -> RunningWorkItem {
    let mut v = json!({"id": id, "sessionId": "root", "kind": kind, "title": "explore", "status": status,
        "startedAt": "2026-10-07T09:58:00Z"});
    if let Some(c) = child {
        v["childSessionId"] = json!(c);
    }
    if status != "running" {
        v["endedAt"] = json!("2026-10-07T09:58:38Z");
    }
    to_item(&v).unwrap()
}

fn ended(id: &str, at: &str) -> RunningWorkItem {
    let mut w = work(id, "subagent", None, "completed");
    w.ended_at = Some(at.into());
    w
}

fn refs(list: &[Session]) -> Vec<&Session> {
    list.iter().collect()
}

fn keys(rows: &[AgentRow]) -> Vec<&str> {
    rows.iter().map(|r| r.key.as_str()).collect()
}

// ─── session-lineage.test.ts ────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn moving_a_session_out_puts_it_at_the_top_of_the_list_and_out_of_its_parents_agents_tab() {
    let mut f = fork("f", "root", "idle");
    assert_eq!(keys(&build_agents_lineage("root", &refs(std::slice::from_ref(&f)), &[]).started), ["session:f"]);
    f.listing.detached = true;
    let lineage = build_agents_lineage("root", &refs(&[f]), &[]);
    assert!(lineage.started.is_empty() && lineage.running.is_empty());
}

#[test]
fn build_agents_lineage_splits_running_agents_sessions_it_started_and_earlier_agents_newest_first() {
    let list = [fork("f-busy", "root", "active"), started("s-ask", "root", "waiting_input"), started("s-idle", "root", "idle")];
    let items = [
        work("w-run", "subagent", None, "running"),
        ended("w-old", "2026-10-07T09:00:10Z"),
        ended("w-new", "2026-10-07T09:30:00Z"),
        work("w-shell", "shell", None, "running"),
    ];
    let l = build_agents_lineage("root", &refs(&list), &items);
    // Work first, then the sessions that run or wait; shells never show.
    assert_eq!(keys(&l.running), ["work:w-run", "session:f-busy", "session:s-ask"]);
    assert_eq!(
        l.running.iter().map(|r| r.state).collect::<Vec<_>>(),
        [AgentRowState::Running, AgentRowState::Running, AgentRowState::Waiting]
    );
    assert_eq!(keys(&l.started), ["session:s-idle"]);
    assert_eq!(keys(&l.earlier), ["work:w-new", "work:w-old"]);
    assert_eq!(l.summary().as_deref(), Some("2 running · 1 waiting for you"));
    assert_eq!(l.active_count(), 3);
    assert_eq!(l.earlier_label(), "Earlier agents · 2");
    assert_eq!(l.rows().count(), 6);
}

#[test]
fn build_agents_lineage_lists_only_its_own_children_a_childs_children_are_in_that_childs_tab() {
    let list = [fork("child", "root", "idle"), started("grandchild", "child", "idle")];
    assert_eq!(keys(&build_agents_lineage("root", &refs(&list), &[]).started), ["session:child"]);
    assert_eq!(keys(&build_agents_lineage("child", &refs(&list), &[]).started), ["session:grandchild"]);
    // Another session's work isn't listed either.
    let mut other = work("w", "subagent", None, "running");
    other.session_id = "elsewhere".into();
    assert!(build_agents_lineage("root", &refs(&list), &[other]).running.is_empty());
}

#[test]
fn build_agents_lineage_lists_a_subagents_session_once_through_its_work() {
    let mut hidden = session("hidden", "Hidden", json!({"sessionStatus": "active"}));
    hidden.listing.parent = Some("root".into());
    let l = build_agents_lineage("root", &refs(&[hidden]), &[work("w", "subagent", Some("hidden"), "running")]);
    assert_eq!(keys(&l.running), ["work:w"]);
    assert_eq!(l.running[0].session_id.as_deref(), Some("hidden"));
    assert!(l.started.is_empty());
    // Even a listed session of the work's isn't a second row.
    let listed = fork("hidden", "root", "active");
    assert_eq!(
        keys(&build_agents_lineage("root", &refs(&[listed]), &[work("w", "subagent", Some("hidden"), "running")]).running),
        ["work:w"]
    );
}

fn text(role: Role, id: &str, text: &str) -> Msg {
    msg(role, vec![PartData::Text { id: id.into(), text: text.into() }], None, None)
}

fn msg(role: Role, parts: Vec<PartData>, tokens: Option<(i64, i64)>, model: Option<&str>) -> Msg {
    Msg {
        id: "m".into(),
        session_id: "child".into(),
        role,
        parts,
        cost: None,
        tokens: tokens.map(|(input, output)| Tokens { input, output, reasoning: 0 }),
        created_at: None,
        completed_at: None,
        agent: None,
        model_id: model.map(str::to_string),
        parent_id: None,
        turn_error: None,
        finish: None,
        steered: false,
        compaction_summary: false,
        command: None,
        mod_mark: None,
    }
}

fn tool(name: &str) -> PartData {
    PartData::Tool { id: format!("p-{name}"), tool: name.into(), call_id: "c".into(), state: json!({"status": "completed"}) }
}

#[test]
fn summarize_agent_activity_reads_what_it_was_asked_its_tool_calls_tokens_latest_words_and_model() {
    let messages = [
        text(Role::User, "u1", "Find where pages are sized\nand report"),
        msg(Role::Assistant, vec![tool("read"), tool("grep"), tool("read")], Some((30_000, 1_000)), Some("gpt-5.5")),
        msg(
            Role::Assistant,
            vec![PartData::Text { id: "t".into(), text: "Looking.\n\n  Found it in paginate.py  \n".into() }, tool("read")],
            Some((8_200, 900)),
            Some("gpt-5.5"),
        ),
        text(Role::User, "u2", "Second prompt"),
        msg(Role::Assistant, vec![PartData::Text { id: "t2".into(), text: "   ".into() }], None, None),
    ];
    let a = summarize_agent_activity(&messages);
    assert_eq!(a.asked.as_deref(), Some("Find where pages are sized"));
    assert_eq!(a.tool_calls, 4);
    assert_eq!(a.top_tools, ["Read", "Grep"]);
    assert_eq!((a.tokens_in, a.tokens_out), (38_200, 1_900));
    assert_eq!(a.latest.as_deref(), Some("Found it in paginate.py"), "a blank text part doesn't replace it");
    assert_eq!(a.model_id.as_deref(), Some("gpt-5.5"));
    assert_eq!(tools_text(&a).as_deref(), Some("4 calls · Read, Grep"));
    assert_eq!(tokens_text(&a).as_deref(), Some("38.2k in · 1.9k out"));
    assert_eq!(summarize_agent_activity(&[]), AgentActivity::default());
}

#[test]
fn summarize_agent_activity_names_tools_and_keeps_the_top_three() {
    let messages =
        [msg(Role::Assistant, vec![tool("read"), tool("mcp__fleet__bash"), tool("grep"), tool("glob"), tool("bash")], None, None)];
    let a = summarize_agent_activity(&messages);
    assert_eq!(a.top_tools.len(), 3);
    assert_eq!(a.top_tools[0], "Bash", "most used first, by display name");
    assert_eq!(
        tools_text(&AgentActivity { tool_calls: 1, top_tools: vec!["Read".into()], ..Default::default() }).as_deref(),
        Some("1 call · Read")
    );
    assert_eq!(tools_text(&AgentActivity::default()), None);
    assert_eq!(tokens_text(&AgentActivity::default()), None);
}

#[test]
fn latest_output_line_is_the_last_non_empty_trimmed_line() {
    assert_eq!(latest_output_line("one\ntwo  \n\n  \n").as_deref(), Some("two"));
    assert_eq!(latest_output_line("\n \n"), None);
}

// ─── the rows ───────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn work_state_reads_running_failed_stopped_and_done() {
    let mut w = work("w", "subagent", None, "running");
    assert_eq!(work_state(&w), AgentRowState::Running);
    w = work("w", "subagent", None, "error");
    assert_eq!(work_state(&w), AgentRowState::Failed);
    assert_eq!(work_state(&work("w", "subagent", None, "cancelled")), AgentRowState::Stopped);
    assert_eq!(work_state(&work("w", "task", None, "completed")), AgentRowState::Done);
}

#[test]
fn the_state_word_says_done_for_one_that_answered_at_once() {
    let mut w = work("w", "subagent", None, "completed");
    w.ended_at = Some("2026-10-07T09:58:00Z".into());
    let row = only_row(w);
    assert_eq!(state_text(&row, NOW), "done");
}

fn only_row(w: RunningWorkItem) -> AgentRow {
    let l = build_agents_lineage("root", &[], &[w]);
    l.running.into_iter().chain(l.earlier).next().unwrap()
}

#[test]
fn a_lost_agent_says_lost() {
    let mut w = work("w", "subagent", None, "error");
    w.ended_reason = Some(crate::fleet::work::EndedReason::Lost);
    assert_eq!(state_text(&only_row(w.clone()), NOW), "lost");
    w.ended_reason = Some(crate::fleet::work::EndedReason::Error);
    assert_eq!(state_text(&only_row(w), NOW), "failed");
}

#[test]
fn the_state_word_reads_time_for_work_and_working_for_a_session() {
    let running = only_row(work("w", "subagent", None, "running"));
    assert_eq!(state_text(&running, NOW), "2 min");
    let mut w = work("w2", "subagent", None, "running");
    w.started_at = "2026-10-07T09:59:22Z".into();
    assert_eq!(state_text(&only_row(w), NOW), "38 s");
    let l = build_agents_lineage("root", &refs(&[fork("f", "root", "active"), started("s", "root", "waiting_input")]), &[]);
    assert_eq!(state_text(&l.running[0], NOW), "working");
    assert_eq!(state_text(&l.running[1], NOW), "needs you");
    assert_eq!(state_text(&only_row(work("e", "subagent", None, "completed")), NOW), "38 s");
    assert_eq!(state_text(&only_row(work("c", "subagent", None, "cancelled")), NOW), "stopped");
}

#[test]
fn a_row_names_its_work_and_its_task() {
    let mut w = work("w", "task", None, "running");
    w.title = "".into();
    w.label = Some("Port the lineage".into());
    let row = only_row(w.clone());
    assert_eq!((row.name.as_str(), row.task.as_deref(), row.kind), ("Port the lineage", Some("Port the lineage"), AgentKind::Task));
    w.title = "Port the lineage".into();
    assert_eq!(only_row(w.clone()).task, None, "a label that is the title says nothing more");
    w.title = "explore".into();
    assert_eq!(only_row(w.clone()).task.as_deref(), Some("Port the lineage"));
    w.title = "".into();
    w.label = None;
    assert_eq!(only_row(w).name, "subagent");
    let l = build_agents_lineage("root", &refs(&[session("c", "  ", json!({"spawnedBySessionId": "root"}))]), &[]);
    assert_eq!(l.started[0].name, "Untitled session");
    assert_eq!(kind_label(AgentKind::Task), "task");
    assert_eq!(kind_label(AgentKind::Fork), "fork");
}

#[test]
fn notes_say_when_it_asked_and_when_it_has_no_session() {
    assert_eq!(note(&only_row(work("w", "subagent", None, "running"))), Some("no session of its own"));
    assert_eq!(note(&only_row(work("w", "subagent", Some("c"), "running"))), None);
    let l = build_agents_lineage("root", &refs(&[started("s", "root", "waiting_input")]), &[]);
    assert_eq!(note(&l.running[0]), Some("asked a question"));
    assert_eq!(parent_note(LineageKind::Fork), "forked into this session");
    assert_eq!(parent_note(LineageKind::Subagent), "delegated this session");
    assert_eq!(parent_note(LineageKind::Started), "started this session");
}

fn harnesses() -> Vec<HarnessInfo> {
    vec![
        HarnessInfo { kind: "claude-code".into(), display_name: Some("Claude Code".into()), ..Default::default() },
        HarnessInfo { kind: "opencode-2".into(), display_name: Some("OpenCode 2".into()), ..Default::default() },
    ]
}

fn models() -> Vec<ModelOption> {
    vec![ModelOption { id: "openai/gpt-5.5".into(), name: "GPT-5.5".into(), ..Default::default() }]
}

#[test]
fn runs_on_names_the_harness_and_the_model_a_subagent_runs_in_its_parents_harness() {
    let sub = only_row(work("w", "subagent", None, "running"));
    assert_eq!(runs_on(&sub, Some("claude-code"), &HashMap::new(), &harnesses(), &models()).as_deref(), Some("Claude Code"));
    assert_eq!(runs_on(&sub, None, &HashMap::new(), &harnesses(), &models()), None);
    // What the row's own session answered with wins, and a bare id finds a qualified one.
    let seen = HashMap::from([("work:w".to_string(), "gpt-5.5".to_string())]);
    assert_eq!(runs_on(&sub, Some("opencode-2"), &seen, &harnesses(), &models()).as_deref(), Some("OpenCode 2 · GPT-5.5"));
    let own = build_agents_lineage(
        "root",
        &refs(&[session("c", "Child", json!({"spawnedBySessionId": "root", "harnessType": "mystery", "lastAssistantModelId": "x-1"}))]),
        &[],
    );
    assert_eq!(runs_on(&own.started[0], Some("claude-code"), &HashMap::new(), &harnesses(), &models()).as_deref(), Some("mystery · x-1"));
}

#[test]
fn the_parent_row_names_where_it_runs_and_what_it_is() {
    let parent = session(
        "root",
        "Parent title",
        json!({"sessionStatus": "active", "selectedModel": {"providerID": "openai", "modelID": "gpt-5.5"}}),
    );
    let row = parent_row("root", LineageKind::Started, Some("Parent title".into()), Some(&parent));
    assert_eq!((row.state, row.name.as_str()), (AgentRowState::Running, "Parent title"));
    let listed = row.session.clone();
    assert_eq!(parent_runs_on(listed.as_ref(), &harnesses(), &models()).as_deref(), Some("tidytempo · Claude Code · GPT-5.5"));
    let unknown = parent_row("gone", LineageKind::Fork, None, None);
    assert_eq!((unknown.state, unknown.name.as_str(), unknown.session.is_none()), (AgentRowState::Idle, "Parent session", true));
    assert_eq!(parent_runs_on(None, &harnesses(), &models()), None);
    assert!(label(&row, true, Some("x"), NOW).starts_with("Parent title, x, "), "no kind on the parent");
}

#[test]
fn the_label_joins_what_the_row_says() {
    let mut w = work("w", "subagent", None, "running");
    w.label = Some("Map the repo".into());
    let row = only_row(w);
    assert_eq!(label(&row, false, Some("Claude Code"), NOW), "explore, Map the repo, subagent, Claude Code, no session of its own, 2 min");
}

#[test]
fn says_what_it_will_show_when_there_is_nothing_yet() {
    let empty = AgentsLineage::default();
    assert!(empty.is_empty(false));
    assert!(!empty.is_empty(true), "a parent is something to show");
    assert_eq!(empty.summary(), None);
    assert!(!build_agents_lineage("root", &[], &[ended("e", "2026-10-07T09:58:38Z")]).is_empty(false));
}

// ─── selection ──────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_first_running_row_with_work_is_open_until_another_is_picked_and_pressing_it_again_closes_it() {
    let list = [fork("f", "root", "active")];
    let l = build_agents_lineage("root", &refs(&list), &[work("w", "subagent", None, "running")]);
    let mut chosen: Option<String> = None;
    assert_eq!(shown_key(chosen.as_deref(), &l), Some("work:w"));
    select(&mut chosen, &l, "session:f");
    assert_eq!(shown_key(chosen.as_deref(), &l), Some("session:f"));
    select(&mut chosen, &l, "session:f");
    assert_eq!(shown_key(chosen.as_deref(), &l), None, "closed, and the automatic one doesn't come back");
    // Pressing the automatically opened one closes it, too.
    let mut fresh: Option<String> = None;
    select(&mut fresh, &l, "work:w");
    assert_eq!(shown_key(fresh.as_deref(), &l), None);
    // With no running work, nothing opens by itself.
    assert_eq!(shown_key(None, &build_agents_lineage("root", &refs(&list), &[])), None);
}

// ─── the detail ─────────────────────────────────────────────────────────────────────────────────────────────────────

fn input<'a>(row: &'a AgentRow, activity: Option<&'a AgentActivity>) -> DetailInput<'a> {
    DetailInput { row, this_session: "root", activity, output_line: None, stopping: false, stop_error: None }
}

#[test]
fn offers_only_what_the_harness_allows_no_open_or_stop_for_a_subagent_without_a_session_or_a_stop() {
    let bare = only_row(work("w", "subagent", None, "running"));
    let d = detail(&input(&bare, None));
    assert_eq!((d.open.clone(), d.can_stop, d.inside_turn), (None, false, true));
    let mut stoppable = work("w", "subagent", None, "running");
    stoppable.can_stop = true;
    let d = detail(&input(&only_row(stoppable), None));
    assert_eq!((d.can_stop, d.inside_turn), (true, false));
    let with_session = detail(&input(&only_row(work("w", "subagent", Some("c"), "running")), None));
    assert_eq!(with_session.open, Some(OpenTarget::Child { child: "c".into(), instance_id: None, parent: "root".into() }));
    assert!(!with_session.inside_turn);
    // Ended work offers no Stop and no note.
    let gone = detail(&input(&only_row(ended("e", "2026-10-07T09:58:38Z")), None));
    assert_eq!((gone.can_stop, gone.inside_turn, gone.ended.as_deref()), (false, false, Some("done")));
}

#[test]
fn a_fork_opens_on_its_own_with_its_instance() {
    let list = [session("f", "Fork", json!({"forkedFromSessionId": "root", "spawnKind": "fork", "instanceId": "inst-2"}))];
    let l = build_agents_lineage("root", &refs(&list), &[]);
    let d = detail(&input(&l.started[0], None));
    assert_eq!(d.open, Some(OpenTarget::Own { session: "f".into(), instance_id: Some("inst-2".into()) }));
    assert!(d.reading, "its session hasn't said anything yet");
}

#[test]
fn the_detail_says_what_it_was_asked_its_tools_tokens_and_latest_output() {
    let mut w = work("w", "subagent", Some("c"), "running");
    w.label = Some("Map the repo".into());
    w.can_stop = true;
    let row = only_row(w);
    let a = AgentActivity {
        asked: Some("From its session".into()),
        tool_calls: 2,
        top_tools: vec!["Read".into(), "Grep".into()],
        tokens_in: 38_200,
        tokens_out: 1_900,
        latest: Some("Found it".into()),
        model_id: None,
    };
    let d = detail(&DetailInput { stopping: true, stop_error: Some("It couldn't be stopped."), ..input(&row, Some(&a)) });
    assert_eq!(d.asked.as_deref(), Some("Map the repo"), "the work's label first");
    assert_eq!(
        (d.tools.as_deref(), d.tokens.as_deref(), d.latest.as_deref()),
        (Some("2 calls · Read, Grep"), Some("38.2k in · 1.9k out"), Some("Found it"))
    );
    assert!((d.can_stop && d.stopping && !d.reading) && d.stop_error.is_some());
    let plain = only_row(work("w2", "subagent", Some("c"), "running"));
    assert_eq!(detail(&input(&plain, Some(&a))).asked.as_deref(), Some("From its session"));
}

#[test]
fn work_with_no_session_reads_its_output_when_the_server_can() {
    let mut w = work("w", "task", None, "running");
    w.can_read_output = true;
    let row = only_row(w);
    let d = detail(&DetailInput { output_line: Some("compiled 12 files"), ..input(&row, None) });
    assert_eq!((d.latest.as_deref(), d.reading), (Some("compiled 12 files"), false));
    let no_read = only_row(work("w", "task", None, "running"));
    assert_eq!(detail(&DetailInput { output_line: Some("ignored"), ..input(&no_read, None) }).latest, None);
}

// ─── the history, kept per session ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_history_keeps_ended_work_and_a_stops_answer_moves_a_row_to_earlier_agents() {
    let mut s = crate::State::default();
    let running = work("w", "subagent", None, "running");
    s.on_agents_history(
        "root",
        Ok((
            200,
            json!([{"id": "old", "sessionId": "root", "kind": "subagent", "title": "old", "status": "completed",
        "startedAt": "2026-10-07T09:00:00Z", "endedAt": "2026-10-07T09:01:00Z"}]),
        )),
    );
    s.on_agents_history("root", Ok((500, json!({}))));
    s.on_agents_history("root", Err("down".into()));
    assert_eq!(s.agents_history["root"].len(), 1);
    s.agents_history.get_mut("root").unwrap().push(running.clone());
    assert_eq!(keys(&s.agents_lineage("root").running), ["work:w"]);
    let mut stopped = running;
    stopped.status = WorkStatus::Cancelled;
    stopped.ended_at = Some("2026-10-07T09:59:00Z".into());
    stopped.kind = WorkKind::Subagent;
    s.on_work_stopped("w", Ok((200, serde_json::from_str(&serde_json::to_string(&wire(&stopped)).unwrap()).unwrap())));
    let l = s.agents_lineage("root");
    assert!(l.running.is_empty());
    assert_eq!(keys(&l.earlier), ["work:w", "work:old"]);
    assert_eq!(l.earlier[0].state, AgentRowState::Stopped);
}

fn wire(w: &RunningWorkItem) -> serde_json::Value {
    json!({"id": w.id, "sessionId": w.session_id, "kind": "subagent", "title": w.title, "status": "cancelled",
        "startedAt": w.started_at, "endedAt": w.ended_at, "endedReason": "cancelled"})
}
