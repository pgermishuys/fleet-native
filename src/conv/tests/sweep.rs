//! The pure-logic cases of the tests sweep: `session-interruption.test.ts`, `retry-status.test.ts`,
//! `shell-commands.test.ts` ("a shell command arriving live"), `snapshot-tool-output-hydration.test.ts`,
//! `reopened-session.test.ts` ("keeps a generic tool card and the canvas card"), `tool-labels.test.ts` ("a shell tool
//! card", "isBashTool"). Each is named as Fleet's case.
use crate::conv::card::tool_body;
use crate::conv::message_list::*;
use crate::conv::tools::{ToolStatus, get_tool, is_shell_row};
use crate::conv::working::{changed_files_text, retry_countdown};
use crate::fleet::events::Lifecycle;
use crate::fleet::model::{PartData, StreamState};
use crate::fleet::reducer::{Event, apply, create_state};
use crate::fleet::rows::{Interruption, Retrying};
use crate::fleet::work::parse_iso_ms;
use crate::view::conversation::Role;
use serde_json::{Value, json};

mod session_interruption {
    use super::*;

    #[test]
    fn to_session_interruption_reads_a_full_interruption() {
        let got = Interruption::from_wire(
            &json!({"at": "2026-10-17T14:02:00Z", "waiting": false, "lastCall": "bash: bun run build", "changedFiles": 3}),
        );
        assert_eq!(
            got,
            Some(Interruption {
                at: "2026-10-17T14:02:00Z".into(),
                waiting: false,
                last_call: Some("bash: bun run build".into()),
                changed_files: Some(3),
                auto: false
            })
        );
        assert!(Interruption::from_wire(&json!({"at": "2026-10-17T14:02:00Z", "auto": true})).unwrap().auto);
    }

    #[test]
    fn to_session_interruption_fills_in_what_is_missing() {
        assert_eq!(
            Interruption::from_wire(&json!({"at": "2026-10-17T14:02:00Z"})),
            Some(Interruption { at: "2026-10-17T14:02:00Z".into(), waiting: false, last_call: None, changed_files: None, auto: false })
        );
    }

    #[test]
    fn to_session_interruption_is_null_without_a_usable_time_or_an_object() {
        for bad in [Value::Null, json!("x"), json!({"at": "not a time"}), json!({"waiting": true})] {
            assert_eq!(Interruption::from_wire(&bad), None, "{bad}");
        }
    }

    #[test]
    fn wording_counts_files() {
        assert_eq!(changed_files_text(1), "1 file changed in this worktree");
        assert_eq!(changed_files_text(4), "4 files changed in this worktree");
    }
}

mod retry_status {
    use super::*;

    #[test]
    fn says_the_attempt_out_of_how_many_when_and_why() {
        let now = parse_iso_ms("2026-10-07T10:00:00Z").unwrap();
        let retry = Retrying {
            attempt: Some(3),
            max_attempts: Some(10),
            message: Some("API overloaded (529)".into()),
            next: Some("2026-10-07T10:00:12Z".into()),
        };
        assert_eq!(
            crate::conv::working::describe_retry(&retry, now),
            "Retrying \u{b7} attempt 3 of 10 \u{b7} in 12 s \u{b7} API overloaded (529)"
        );
    }

    #[test]
    fn counts_down_in_seconds_then_minutes_then_says_now() {
        let now = parse_iso_ms("2026-10-07T10:00:00Z").unwrap();
        let at = |next: &str| retry_countdown(&Retrying { next: Some(next.into()), ..Default::default() }, now);
        assert_eq!(at("2026-10-07T10:00:00.400Z").as_deref(), Some("in 1 s"));
        assert_eq!(at("2026-10-07T10:02:30Z").as_deref(), Some("in 3 min"));
        assert_eq!(at("2026-10-07T09:59:59Z").as_deref(), Some("now"));
        assert_eq!(at("not a time"), None);
    }
}

fn part_event(state: Value, tool: &str, id: &str, message: &str) -> Event {
    Event::PartUpdated(
        json!({"type": "tool", "id": id, "sessionID": "session-1", "messageID": message, "tool": tool, "callID": "call-1", "state": state}),
    )
}

fn empty_state() -> StreamState {
    create_state(&json!({"session": {}, "messages": [], "delegations": [], "activityStatus": "idle", "lastEventId": null}))
}

mod a_shell_command_arriving_live {
    use super::*;

    #[test]
    fn becomes_the_shells_message_when_its_part_came_before_the_message() {
        // OpenCode (1.x) can send the command's part before the message that says whose it is.
        let mut state = empty_state();
        apply(&mut state, &part_event(json!({"status": "running", "input": {"command": "git status"}}), "bash", "prt_1", "msg_1"));
        assert_eq!(state.messages[0].role, Role::Assistant);
        let life =
            Lifecycle { info: json!({"id": "msg_1", "role": "shell", "sessionID": "session-1", "time": {"created": 5}}), parts: None };
        apply(&mut state, &Event::MessageUpdated(life));
        assert_eq!(state.messages[0].role, Role::Shell);
        assert_eq!(state.messages[0].parts.len(), 1);
    }

    #[test]
    fn starts_as_the_shells_message_when_the_message_comes_first() {
        let mut state = empty_state();
        let life =
            Lifecycle { info: json!({"id": "msg_1", "role": "shell", "sessionID": "session-1", "time": {"created": 5}}), parts: None };
        apply(&mut state, &Event::MessageCreated(life));
        assert_eq!(state.messages[0].role, Role::Shell);
    }
}

/// Hydrates one tool part through the reducer and gives back what the card would show for it.
fn hydrate(tool: &str, state: Value) -> (String, crate::conv::card::ToolBody, Value) {
    let mut stream = empty_state();
    let life =
        Lifecycle { info: json!({"id": "msg-test", "role": "assistant", "sessionID": "session-1", "time": {"created": 1}}), parts: None };
    apply(&mut stream, &Event::MessageCreated(life));
    apply(&mut stream, &part_event(state, tool, "part-tool-1", "msg-test"));
    assert_eq!(stream.messages.len(), 1);
    assert_eq!(stream.messages[0].parts.len(), 1);
    let PartData::Tool { tool, state, .. } = &stream.messages[0].parts[0] else { panic!("expected a tool part") };
    (tool.clone(), tool_body(tool, tool, state), state.clone())
}

mod snapshot_tool_output_hydration {
    use super::*;

    #[test]
    fn hydrates_completed_tool_state_with_output_from_snapshot_payload() {
        let state = json!({"type": "completed", "input": {"command": "echo test"}, "output": {"result": "test output"}, "metadata": null});
        let (tool, body, raw) = hydrate("bash", state);
        assert_eq!(tool, "bash");
        assert_eq!(raw["output"], json!({"result": "test output"}));
        assert!(body.output.contains("test output"));
    }

    #[test]
    fn handles_tool_state_with_nested_json_output() {
        let state = json!({"type": "completed", "input": {"filePath": "/test/file.txt"},
            "output": {"content": "file contents here\nline 2\nline 3", "lineCount": 3}, "metadata": null});
        let (_, body, _) = hydrate("read", state);
        assert!(body.output.contains("file contents here"));
        assert!(body.output.contains("lineCount"));
    }

    #[test]
    fn handles_tool_state_with_error_output() {
        let state =
            json!({"type": "error", "status": "error", "input": {"command": "invalid-command"}, "output": {"error": "Command not found"}});
        let (_, body, raw) = hydrate("bash", state);
        assert!(body.output.contains("Command not found"));
        assert_eq!(ToolStatus::from_word(raw["status"].as_str().map(crate::conv::tools::capitalize).as_deref()), ToolStatus::Error);
    }

    #[test]
    fn shows_no_output_for_pending_tool_state() {
        let state = json!({"type": "pending", "status": "pending", "input": {"command": "sleep 10"}});
        let (_, _, raw) = hydrate("bash", state);
        assert_eq!(ToolStatus::from_word(raw["status"].as_str().map(crate::conv::tools::capitalize).as_deref()), ToolStatus::Pending);
    }
}

mod reopened_session {
    use super::*;

    #[test]
    fn keeps_a_generic_tool_card_and_the_canvas_card() {
        let snap = json!({"session": {}, "delegations": [], "activityStatus": "busy", "lastEventId": null, "messages": [{
        "info": {"id": "msg_1", "role": "assistant", "sessionID": "fleet-1", "agent": "build", "time": {"created": 1000, "completed": null}},
        "parts": [
            {"type": "tool", "id": "prt_search", "tool": "search_code", "callID": "toolu_search", "state": {"status": "completed",
                "input": {"query": "authenticate"}, "output": "src/middleware/auth.ts:12: export function authenticate",
                "title": "Search code", "metadata": {"truncated": false}}},
            {"type": "tool", "id": "prt_canvas", "tool": "fleet_page_show", "callID": "toolu_canvas", "state": {"status": "completed",
                "input": {}, "output": "Opened it.", "title": "Request path", "metadata": {"canvasId": "cv_1"}}},
        ]}]});
        let stream = create_state(&snap);
        let calls: Vec<(String, Value)> = stream.messages[0]
            .parts
            .iter()
            .filter_map(|p| if let PartData::Tool { tool, state, .. } = p { Some((tool.clone(), state.clone())) } else { None })
            .collect();
        let search = calls.iter().find(|(t, _)| t == "search_code").unwrap();
        assert!(tool_body(&search.0, &search.0, &search.1).output.contains("authenticate"));
        // The canvas the call opened is what the card offers to show.
        let context =
            ListContext { delegations: &[], finished: &Default::default(), models: &[], thinking: crate::conv::gist::Thinking::Folded };
        let list = build_list(&stream.messages, &[], &context);
        let canvas = list[0].tools.iter().find(|t| t.kind == "fleet_page_show").unwrap();
        assert!(canvas.canvas);
        assert!(!list[0].tools.iter().find(|t| t.kind == "search_code").unwrap().canvas);
    }
}

mod a_shell_tool_card {
    use super::*;

    #[test]
    fn shows_opencode_2s_command_and_output() {
        let state = json!({"status": "completed", "input": {"command": "echo from-tool", "description": "Echo"},
            "output": "from-tool\nCommand exited with code 0.", "metadata": {"exit": 0}});
        let title = get_tool("shell").label(&state["input"]);
        let body = tool_body("shell", &title, &state);
        assert_eq!(title, "Echo");
        assert_eq!(ToolStatus::from_word(Some("Completed")), ToolStatus::Completed);
        assert_eq!(body.output, "from-tool\nCommand exited with code 0.");
        assert_eq!(body.preview.as_deref(), Some("\u{2514} from-tool (2 lines)"));
    }
}

mod is_bash_tool {
    use super::*;

    #[test]
    fn is_the_shell_tool_in_both_opencodes() {
        assert!(is_shell_row("bash"));
        assert!(is_shell_row("shell"));
        assert!(!is_shell_row("read"));
    }
}

mod to_tool_card_item_diff {
    use super::*;
    use crate::conv::diff::Kind;

    #[test]
    fn shows_the_unified_diff_kept_on_the_calls_metadata_with_its_line_numbers() {
        let state = json!({"status": "completed",
            "input": {"filePath": "/work/calc.py", "oldString": "def sub(a, b):", "newString": "def subtract(a, b):"},
            "metadata": {"diff": "--- /work/calc.py\n+++ /work/calc.py\n@@ -2,4 +2,4 @@\n     return a + b\n \n-def sub(a, b):\n+def subtract(a, b):\n"}});
        let lines = tool_body("edit", "edit", &state).diff;
        assert_eq!(lines.iter().map(|l| l.kind).collect::<Vec<_>>(), [Kind::Context, Kind::Context, Kind::Remove, Kind::Add]);
        assert_eq!((lines[2].content.as_str(), lines[2].old), ("def sub(a, b):", Some(4)));
        assert_eq!((lines[3].content.as_str(), lines[3].new), ("def subtract(a, b):", Some(4)));
    }

    #[test]
    fn prefers_diff_lines_the_harness_attached() {
        let state =
            json!({"status": "completed", "diffLines": [{"type": "add", "content": "x"}], "metadata": {"diff": "@@ -1 +1 @@\n-a\n+b\n"}});
        let lines = tool_body("edit", "edit", &state).diff;
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].kind, lines[0].content.as_str()), (Kind::Add, "x"));
    }
}

mod to_tool_card_item {
    use super::*;
    use crate::conv::background::work_status;
    use crate::conv::diff::{DiffLine, Kind};
    use crate::fleet::work::to_item;

    fn work(extra: Value) -> crate::fleet::work::RunningWorkItem {
        let mut item = json!({"id": "w1", "sessionId": "s1", "workId": "b7x", "kind": "shell", "title": "Bash", "label": "sh scripts/dev.sh",
            "status": "running", "background": true, "toolCallId": "call-1", "canStop": true, "startedAt": "2026-10-04T10:00:00Z"});
        for (k, v) in extra.as_object().unwrap() {
            item[k] = v.clone();
        }
        to_item(&item).unwrap()
    }

    #[test]
    fn draws_a_call_whose_work_runs_on_in_the_background_as_background_whatever_the_call_says() {
        // Claude Code reports a call that started background work done at once; the work says it still runs.
        assert_eq!(work_status(None), None);
        assert_eq!(work_status(Some(&work(json!({})))), Some(ToolStatus::Background));
        // Foreground work (a subagent the call waits on) is the call's own running.
        assert_eq!(work_status(Some(&work(json!({"background": false})))), None);
        // Finished on its own: the call's own state says it.
        let done = work(json!({"status": "completed", "endedAt": "2026-10-04T10:05:00Z", "endedReason": "completed"}));
        assert_eq!(work_status(Some(&done)), None);
    }

    #[test]
    fn says_stopped_for_work_fleet_stopped_on_every_harness() {
        let stopped = work(json!({"status": "cancelled", "endedAt": "2026-10-04T10:05:00Z", "endedReason": "cancelled"}));
        assert_eq!(work_status(Some(&stopped)), Some(ToolStatus::Stopped));
        // OpenCode 2's call stays running with the handle; its notice says how it ended.
        let call = json!({"status": "running", "background": true, "metadata": {"shellID": "sh_1"}});
        let mut finished = crate::conv::background::Finished::new();
        finished.insert("sh_1".into(), crate::conv::background::BackgroundState::Cancelled);
        assert_eq!(crate::conv::background::background_status(&call, &finished), Some(ToolStatus::Stopped));
    }

    #[test]
    fn uses_result_as_output_when_output_is_absent() {
        assert_eq!(tool_body("bash", "bash", &json!({"status": "completed", "result": "hello"})).output, "hello");
    }

    #[test]
    fn derives_title_status_summary_and_collapsed_state() {
        let state = json!({"status": "completed", "input": {"description": "Lists files in current directory", "command": "ls -la"},
            "summary": "Applied patch", "stdout": "file-a"});
        let body = tool_body("bash", "Lists files in current directory", &state);
        assert_eq!(get_tool("bash").label(&state["input"]), "Lists files in current directory");
        assert_eq!(ToolStatus::from_word(Some("Completed")), ToolStatus::Completed);
        assert_eq!((body.summary.as_str(), body.output.as_str(), body.initially_collapsed), ("Applied patch", "file-a", true));
    }

    #[test]
    fn falls_back_to_remaining_state_output_and_normalizes_diff_lines() {
        let state = json!({"status": "completed", "summary": "Applied patch", "input": {"command": "ignored in output"},
            "diff": [{"content": "+added line", "newLineNumber": 3}, {"line": "-removed line", "oldLineNumber": 2}, {"text": " unchanged line "}],
            "metadata": {"changed": true}});
        let body = tool_body("custom-tool", "custom-tool", &state);
        assert_eq!(get_tool("custom-tool").label(&state["input"]), "custom-tool");
        assert_eq!(body.output, "{\n  \"metadata\": {\n    \"changed\": true\n  }\n}");
        let line = |kind, content: &str, old, new| DiffLine { kind, content: content.into(), old, new };
        assert_eq!(
            body.diff,
            [
                line(Kind::Add, "+added line", None, Some(3)),
                line(Kind::Remove, "-removed line", Some(2), None),
                line(Kind::Context, " unchanged line ", None, None)
            ]
        );
    }

    #[test]
    fn preserves_falsy_output_values_and_avoids_diff_only_json_fallback() {
        assert_eq!(tool_body("bash", "bash", &json!({"status": "completed", "output": 0})).output, "0");
        assert_eq!(tool_body("bash", "bash", &json!({"status": "completed", "result": false})).output, "false");
        let diff_only =
            tool_body("edit", "edit", &json!({"status": "completed", "diff": [{"content": "+added line", "newLineNumber": 1}]}));
        assert_eq!(diff_only.output, "");
        assert_eq!(diff_only.diff.len(), 1);
        assert_eq!((diff_only.diff[0].kind, diff_only.diff[0].new), (Kind::Add, Some(1)));
    }

    #[test]
    fn returns_pending_defaults_when_state_is_not_a_record() {
        let body = tool_body("read", "read", &Value::Null);
        assert_eq!(get_tool("read").label(&Value::Null), "read");
        assert_eq!(ToolStatus::from_word(None), ToolStatus::Pending);
        assert_eq!((body.output.as_str(), body.initially_collapsed), ("", true));
        assert!(body.diff.is_empty());
    }
}

mod use_running_work {
    use crate::conv::work_clock::WorkStrip;
    use crate::fleet::work::{parse_iso_ms, to_item};
    use serde_json::json;

    // use-running-work.test.ts: "doesn't show work that had long ended when the session opened"
    #[test]
    fn doesnt_show_work_that_had_long_ended_when_the_session_opened() {
        let now = parse_iso_ms("2026-10-04T10:05:00Z").unwrap();
        let item = to_item(&json!({"id": "w-shell", "sessionId": "s1", "workId": "sh_1", "kind": "shell", "title": "shell",
            "label": "bun run test:e2e", "status": "completed", "background": true, "toolCallId": "call_bg", "canStop": true,
            "canReadOutput": true, "startedAt": "2026-10-04T10:00:00Z", "endedAt": "2026-10-04T10:00:30Z", "endedReason": "completed",
            "detail": "exit 0"}))
        .unwrap();
        let list = [item];
        let mut strip = WorkStrip::default();
        strip.observe(&list, now);
        assert_eq!(list.len(), 1);
        assert!(strip.visible(&list, now).is_empty());
    }
}

mod tool_card_label_and_detail {
    use super::*;
    use crate::conv::tools::ToolIcon;

    // ToolCard.rows.test.ts: it.each "labels a %s call %s"
    #[test]
    fn labels_a_call_by_its_heading() {
        for (kind, heading) in [
            ("read", "Read"),
            ("edit", "Edit"),
            ("bash", "Bash"),
            ("webfetch", "Web Fetch"),
            ("task", "Task"),
            ("execute", "Code"),
            ("fleet_page_show", "Show page"),
            ("todowrite", "Update todos"),
            ("mystery_tool", "Mystery_tool"),
        ] {
            assert_eq!(get_tool(kind).heading, heading, "{kind}");
        }
    }

    // ToolCard.rows.test.ts: "uses one icon for a tool it has no icon for, the same one tools sharing a mapping get"
    #[test]
    fn uses_one_icon_for_a_tool_it_has_no_icon_for_the_same_one_tools_sharing_a_mapping_get() {
        let icon = |kind: &str| get_tool(kind).icon;
        assert_ne!(icon("todowrite"), icon("mystery_tool"));
        assert_ne!(icon("todowrite"), icon("read"));
        assert_ne!(icon("todoread"), icon("todowrite"));
        assert_eq!(icon("notebookedit"), icon("edit"));
        assert_eq!(icon("patch"), icon("edit"));
        assert_ne!(icon("list"), icon("mystery_tool"));
        assert_eq!(icon("write"), icon("edit"));
        assert_eq!(icon("glob"), icon("grep"));
        assert_eq!(icon("mystery_tool"), ToolIcon::Wrench);
    }
}

mod message_bubble_tool_rows_browser_steps {
    use super::*;

    // MessageBubble.tool-rows.test.ts: it.each "adds BrowserSteps under a %s call", it.each "adds no BrowserSteps under a %s call"
    // and "matches the kind in any case: Execute gets them too". Whether a call gets them is the registry's `uses_browser`.
    #[test]
    fn adds_browser_steps_under_the_calls_that_drive_the_browser_and_no_other_in_any_case() {
        for kind in ["execute", "fleet_browser_read", "fleet_browser_act", "Execute"] {
            assert!(get_tool(kind).uses_browser, "{kind}");
        }
        for kind in ["fleet_browser_open", "fleet_browser_screenshot", "fleet_app_start", "bash", "read", "task"] {
            assert!(!get_tool(kind).uses_browser, "{kind}");
        }
    }
}

mod message_bubble_tool_rows_pages {
    use super::*;

    // MessageBubble.tool-rows.test.ts: "has no paging of its own: every call is drawn, however many"
    #[test]
    fn has_no_paging_of_its_own_every_call_is_drawn_however_many() {
        let mut m = crate::fleet::model::Msg::new("m1", "s1", Role::Assistant);
        m.parts = (0..60)
            .map(|n| PartData::Tool {
                id: format!("p{n}"),
                tool: "bash".into(),
                call_id: format!("c{n}"),
                state: json!({"status": "completed"}),
            })
            .collect();
        let context =
            ListContext { delegations: &[], finished: &Default::default(), models: &[], thinking: crate::conv::gist::Thinking::Folded };
        assert_eq!(build_list(&[m], &[], &context)[0].tools.len(), 60);
    }
}
