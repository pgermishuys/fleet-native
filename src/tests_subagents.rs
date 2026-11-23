//! Sub-agents and running work (component 26, piece M): the line that stands for the composer in a sub-agent's session
//! (`SessionRoute.view-mode.test.tsx`'s case, which is where `SubagentPromptLine` is tested), the sub-agent rows the list
//! keeps apart, and what the work strip remembers.
use crate::fleet::{sessions, store::Retention};
use crate::mock::header::Case;
use crate::tests::{FRAME, app, click};
use rune::Runtime;
use serde_json::json;

fn scene() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_header(Case::Subagent, true, true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn cannot_prompt(rt: &mut Runtime<crate::State>) {
    rt.update(|s, _| {
        let row = &mut s.session_mut().unwrap().row;
        row.can_prompt = Some(false);
        row.prompt_disabled_reason = Some("Claude Code can't prompt a subagent on its own. Ask the session that started it.".into());
    });
    for _ in 0..2 {
        rt.frame(FRAME);
    }
}

#[test]
fn shows_a_line_pointing_at_the_parent_instead_of_the_composer_when_a_subagent_session_cannot_be_prompted() {
    let mut rt = scene();
    assert!(has(&rt, "composer-section") && !has(&rt, "subagent-prompt-line"), "a sub-agent that takes prompts keeps its composer");
    cannot_prompt(&mut rt);
    assert!(!has(&rt, "composer-section"));
    let line = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("subagent-prompt-line")).expect("the line is drawn");
    assert!(line.help.as_deref().is_some_and(|h| h.starts_with("Claude Code can't prompt a subagent")), "the reason is its tooltip");
    assert!(has(&rt, "subagent-prompt-parent"), "the parent's title is the link");
}

#[test]
fn the_line_without_a_known_parent_title_says_to_ask_the_session_that_started_it() {
    let (before, link, after) = crate::view::subagent_prompt_line::words(None);
    assert_eq!((before, link, after), ("This is a subagent's session. Ask the session that started it.", None, ""));
    let (before, link, after) = crate::view::subagent_prompt_line::words(Some("Migrate the cache"));
    assert_eq!((before, link, after), ("This is a subagent of ", Some("Migrate the cache"), ". Ask there."));
}

#[test]
fn the_parent_link_goes_back_to_the_parent() {
    let mut rt = scene();
    cannot_prompt(&mut rt);
    click(&mut rt, "subagent-prompt-parent");
    assert_eq!(rt.state.open_id().as_deref(), Some("a-cache"));
    assert!(has(&rt, "composer-section"), "the parent has its composer");
}

#[test]
fn an_archived_or_promptable_subagent_keeps_the_composer() {
    let mut rt = scene();
    cannot_prompt(&mut rt);
    rt.update(|s, _| s.session_mut().unwrap().row.archived = true);
    rt.frame(FRAME);
    assert!(has(&rt, "composer-section") && !has(&rt, "subagent-prompt-line"));
    rt.update(|s, _| {
        s.session_mut().unwrap().row.archived = false;
        s.session_mut().unwrap().row.can_prompt = Some(true);
    });
    rt.frame(FRAME);
    assert!(has(&rt, "composer-section"));
}

#[test]
fn a_top_level_session_that_cannot_be_prompted_keeps_its_composer() {
    let mut rt = app();
    rt.update(|s, _| s.session_mut().unwrap().row.can_prompt = Some(false));
    rt.frame(FRAME);
    assert!(!has(&rt, "subagent-prompt-line"));
}

fn rows() -> serde_json::Value {
    let row = |id: &str, parent: Option<&str>| {
        let mut r = json!({
            "session": {"id": id, "title": id, "time": {"created": 1, "updated": 2}},
            "instanceId": format!("i-{id}"),
            "sessionStatus": "idle",
            "capabilities": {"canPrompt": false, "promptDisabledReason": "Ask the session that started it."},
        });
        if let Some(p) = parent {
            r["parentSessionId"] = json!(p);
        }
        r
    };
    json!([row("top", None), row("kid", Some("top"))])
}

#[test]
fn the_list_leaves_sub_agents_out_but_keeps_them_reachable() {
    let listed = sessions::parse_sessions_for(&rows(), 0, Retention::Active);
    assert_eq!(listed.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["top"]);
    let kids = sessions::parse_subagents(&rows(), 0, Retention::Active);
    assert_eq!(kids.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["kid"]);
    assert_eq!(kids[0].row.can_prompt, Some(false));
    assert_eq!(kids[0].row.prompt_disabled_reason.as_deref(), Some("Ask the session that started it."));
}

#[test]
fn open_shows_a_sub_agent_the_list_does_not_draw() {
    let mut rt = app();
    let kid = sessions::parse_subagents(&rows(), 0, Retention::Active).remove(0);
    rt.update(move |s, tx| {
        s.store.subagents = vec![kid];
        assert!(s.open_child("scene", "kid", tx));
    });
    assert_eq!(rt.state.session().id, "kid");
    assert_eq!(rt.state.work.opened_from.as_deref(), Some("scene"));
}

#[test]
fn the_strips_fold_is_kept_in_the_settings_file() {
    let mut s = crate::fleet::settings::Sidebar::default();
    assert!(!s.strip_collapsed());
    s.set_strip_collapsed(true);
    assert!(crate::fleet::settings::Sidebar::parse(&s.to_json()).strip_collapsed());
    s.set_strip_collapsed(false);
    assert!(!crate::fleet::settings::Sidebar::parse(&s.to_json()).strip_collapsed());
}

#[test]
fn a_call_shows_the_state_of_the_work_it_started() {
    use crate::conv::background::{work_of_call, work_status};
    use crate::conv::tools::ToolStatus;
    let w = crate::fleet::work::to_item(&json!({"id": "w1", "sessionId": "s1", "workId": "b7x", "kind": "shell", "title": "Bash",
        "label": "sh dev.sh", "status": "running", "background": true, "toolCallId": "c1", "canStop": true, "startedAt": "2026-10-04T10:00:00Z"}))
    .unwrap();
    let items = [w];
    assert_eq!(work_status(work_of_call(&items, &[], "c1")), Some(ToolStatus::Background));
    assert_eq!(work_status(work_of_call(&items, &[], "other")), None);
    assert_eq!(work_status(work_of_call(&items, &[], "")), None);
}

// ─── rows 2.21: the view clusters by the list's own clusters; the fold plan on a long session ───────────────────────────

fn long_session(turns: usize) -> Vec<crate::fleet::model::Msg> {
    use crate::fleet::model::{Msg, PartData};
    use crate::view::conversation::Role;
    let mut messages = Vec::new();
    for n in 0..turns {
        let mut user = Msg::new(format!("u{n}"), "s1", Role::User);
        user.parts = vec![PartData::Text { id: format!("u{n}-t"), text: format!("Change number {n}, please.") }];
        // A turn that took 35 s, so Fleet folds it once it is finished.
        let base = 1_000_000 + n as i64 * 100_000;
        user.created_at = Some(base);
        let mut reply = Msg::new(format!("a{n}"), "s1", Role::Assistant);
        let call = json!({"status": "completed", "input": {"command": "cargo test"}, "output": "ok"});
        reply.parts = vec![
            PartData::Tool { id: format!("a{n}-c"), tool: "bash".into(), call_id: format!("call-{n}"), state: call },
            PartData::Text { id: format!("a{n}-t"), text: "Done.".into() },
        ];
        reply.created_at = Some(base + 1_000);
        reply.completed_at = Some(base + 35_000);
        messages.extend([user, reply]);
    }
    messages
}

#[test]
fn the_plan_holds_each_messages_run_of_one_sender() {
    use crate::conv::{fold::FoldOptions, fold_plan::FoldPlan, message_list::ListContext};
    let mut messages = long_session(2);
    // Two assistant messages in a row are one run: the first has a neighbour below, the second one above.
    let mut extra = messages[1].clone();
    extra.id = "a0-more".into();
    messages.insert(2, extra);
    let finished = Default::default();
    let context = ListContext {
        delegations: &[],
        finished: &finished,
        running: &[],
        older: &[],
        models: &[],
        thinking: crate::conv::gist::Thinking::Folded,
    };
    let open = Default::default();
    let options = FoldOptions { running: false, keep_last: false, directory: None, open: &open };
    let plan = FoldPlan::build(&messages, &[], &context, &options);
    assert_eq!(plan.grouped("u0"), Some((false, false)));
    assert_eq!(plan.grouped("a0"), Some((false, true)));
    assert_eq!(plan.grouped("a0-more"), Some((true, false)));
    assert_eq!(plan.grouped("nobody"), None, "a message the plan doesn't hold is read by its neighbours' roles");
}

#[test]
fn the_fold_plan_of_a_two_thousand_message_session_is_built_in_seconds_even_in_a_debug_build() {
    use crate::conv::{fold::FoldOptions, fold_plan::FoldPlan, message_list::ListContext};
    let messages = long_session(1_000);
    assert_eq!(messages.len(), 2_000);
    let finished = Default::default();
    let context = ListContext {
        delegations: &[],
        finished: &finished,
        running: &[],
        older: &[],
        models: &[],
        thinking: crate::conv::gist::Thinking::Folded,
    };
    let open = Default::default();
    let options = FoldOptions { running: false, keep_last: false, directory: None, open: &open };
    let started = std::time::Instant::now();
    let plan = FoldPlan::build(&messages, &[], &context, &options);
    let took = started.elapsed();
    eprintln!("fold plan of 2,000 messages: {took:?} (debug build)");
    assert!(plan.kept.len() >= 900, "the finished turns fold: only {} messages changed", plan.kept.len());
    // A debug build is several times slower than a release one: a generous bound that still catches a quadratic plan.
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
}
