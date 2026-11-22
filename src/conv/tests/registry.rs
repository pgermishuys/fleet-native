//! `lib/tools/__tests__/registry.test.ts`, `lib/__tests__/tool-labels.test.ts` and the registry-facing cases of
//! `lib/__tests__/tool-classification.test.ts` (the ones about turns, the phone's fold-steps, pr-utils and the
//! lineage panel belong to sites this app doesn't have). "Same answers as the sites it will replace" compares the
//! registry with Fleet's old switch statements, which have no counterpart here; its concrete cases are below as
//! `labels_the_tools_that_take_a_file_or_folder_by_it_whichever_field_the_harness_used`.
use crate::conv::tools::*;
use serde_json::{Value, json};

fn label(name: &str, input: Value) -> String {
    get_tool(name).label(&input)
}

mod lookup {
    use super::*;

    #[test]
    fn finds_a_tool_by_its_name_an_alias_and_in_any_case() {
        for name in ["edit", "Edit", "EDIT", "multiedit", "MultiEdit", "strreplaceeditor"] {
            assert_eq!(get_tool(name).name, "edit", "{name}");
        }
        for (name, canonical) in [
            ("apply_patch", "patch"),
            ("applypatch", "patch"),
            ("Apply-Patch", "patch"),
            ("Agent", "task"),
            ("Task", "task"),
            ("terminal", "shell"),
            ("TodoWrite", "todowrite"),
            ("LS", "list"),
            ("WebFetch", "webfetch"),
        ] {
            assert_eq!(get_tool(name).name, canonical, "{name}");
        }
    }

    #[test]
    fn keeps_the_name_as_it_arrived() {
        assert_eq!(get_tool("MultiEdit").raw, "MultiEdit");
        assert_eq!(label("MultiEdit", json!({})), "MultiEdit");
    }

    #[test]
    fn claims_every_name_once() {
        let names: Vec<String> =
            all_names().iter().map(|n| n.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn resolves_fleets_own_mcp_server_to_fleets_tools() {
        let tool = get_tool("mcp__fleet__fleet_page_show");
        assert_eq!(tool.name, "fleet_page_show");
        assert!(tool.titled_from_answer);
        assert_eq!(tool.label(&json!({"title": "Panel", "path": "a.html"})), "Panel · a.html");
    }

    #[test]
    fn knows_nothing_of_another_mcp_servers_tools_but_reads_their_names() {
        let tool = get_tool("mcp__acme_orders__lookup_order");
        assert!(!tool.known);
        assert_eq!(tool.mcp, Some(("acme_orders".into(), "lookup_order".into())));
        assert_eq!(tool.category, Category::Other);
        assert_eq!(tool.permission, PermissionKind::Other);
        assert_eq!(tool.display_name, "Lookup_order");
        assert_eq!(tool.heading, "Mcp__acme_orders__lookup_order");
        assert_eq!(tool.label(&json!({"id": 1})), "mcp__acme_orders__lookup_order");
    }

    #[test]
    fn an_mcp_server_cannot_pose_as_a_built_in_tool() {
        assert!(!get_tool("mcp__acme__read").known);
        assert_eq!(get_tool("mcp__acme__bash").permission, PermissionKind::Other);
    }

    #[test]
    fn parses_mcp_names() {
        assert_eq!(parse_mcp_name("mcp__fleet__fleet_page_show"), Some(("fleet".into(), "fleet_page_show".into())));
        assert_eq!(parse_mcp_name("mcp__x"), None);
        assert_eq!(parse_mcp_name("mcp__x__"), None);
        assert_eq!(parse_mcp_name("read"), None);
    }

    #[test]
    fn falls_back_for_a_tool_it_does_not_know() {
        let tool = get_tool("frobnicate");
        assert!(!tool.known);
        assert_eq!(tool.category, Category::Other);
        assert_eq!(tool.permission, PermissionKind::Other);
        assert_eq!(tool.icon, ToolIcon::Wrench);
        assert_eq!(tool.heading, "Frobnicate");
        assert_eq!(tool.label(&json!({"command": "ls"})), "frobnicate");
        assert_eq!(tool.file_write, None);
    }

    #[test]
    fn copes_with_no_name_at_all() {
        assert_eq!(get_tool("").heading, "");
        assert_eq!(get_tool("").category, Category::Other);
        assert_eq!(get_tool("").permission, PermissionKind::Other);
    }

    #[test]
    fn treats_any_other_fleet_tool_as_fleets() {
        let tool = get_tool("fleet_canvas_open");
        assert!(!tool.known);
        assert_eq!(tool.category, Category::Fleet);
        assert_eq!(tool.permission, PermissionKind::Read);
    }
}

mod categories {
    use super::*;

    #[test]
    fn each_name_is_in_its_category() {
        let expected: [(Category, &[&str]); 11] = [
            (Category::Read, &["read", "list", "LS", "lsp", "NotebookRead"]),
            (Category::Search, &["glob", "grep", "codesearch", "websearch"]),
            (
                Category::Edit,
                &["edit", "write", "patch", "apply_patch", "multiedit", "notebookedit", "NotebookEdit", "strreplaceeditor", "move"],
            ),
            (Category::Shell, &["bash", "Bash", "shell", "terminal", "execute", "BashOutput", "KillShell"]),
            (Category::Web, &["webfetch", "WebFetch"]),
            (Category::Subagent, &["task", "Task", "Agent", "subagent"]),
            (Category::Question, &["question"]),
            (Category::Plan, &["todowrite", "TodoWrite", "todoread"]),
            (Category::Skill, &["skill"]),
            (Category::Fleet, &["fleet_app_start", "fleet_message", "fleet_memory_save", "fleet_mod_write", "fleet_canvas_open"]),
            (Category::Other, &["frobnicate", "mcp__acme__lookup_order", ""]),
        ];
        for (category, names) in expected {
            for name in names {
                assert_eq!((*name, get_tool(name).category), (*name, category));
            }
        }
    }

    #[test]
    fn file_writes_write_and_notebooks_create_the_other_edits_modify() {
        assert_eq!(get_tool("write").file_write, Some(FileWrite::Create));
        assert_eq!(get_tool("NotebookEdit").file_write, Some(FileWrite::Create));
        assert_eq!(get_tool("edit").file_write, Some(FileWrite::Modify));
        assert_eq!(get_tool("multiedit").file_write, Some(FileWrite::Modify));
        assert_eq!(get_tool("apply_patch").file_write, Some(FileWrite::Modify));
        assert_eq!(get_tool("read").file_write, None);
    }

    #[test]
    fn flags_the_tools_whose_card_title_is_fleets_answer_and_those_that_drive_the_browser() {
        assert_eq!(
            names_where(|t| t.titled_from_answer),
            [
                "fleet_app_start",
                "fleet_browser_open",
                "fleet_browser_screenshot",
                "fleet_page_show",
                "fleet_walkthrough_show",
                "fleet_message",
                "fleet_session_read",
                "fleet_machine_list",
                "fleet_session_start",
            ]
        );
        assert_eq!(names_where(|t| t.uses_browser), ["execute", "fleet_browser_read", "fleet_browser_act"]);
        assert_eq!(names_where(|t| t.pattern_label), ["glob", "grep"]);
    }
}

mod permission_kinds_follow_the_server {
    use super::*;

    #[test]
    fn each_name_is_classified_as_permissions_classify_does() {
        let server: [(PermissionKind, &[&str]); 5] = [
            (
                PermissionKind::Read,
                &[
                    "read",
                    "glob",
                    "grep",
                    "list",
                    "lsp",
                    "codesearch",
                    "todowrite",
                    "todoread",
                    "todo",
                    "skill",
                    "question",
                    "task",
                    "subagent",
                    "opencode_list_mcp_resources",
                    "opencode_read_mcp_resource",
                    "LS",
                    "NotebookRead",
                    "Agent",
                    "ExitPlanMode",
                    "ListMcpResourcesTool",
                    "ReadMcpResourceTool",
                    "fleet_page_show",
                    "fleet_whatever",
                ],
            ),
            (PermissionKind::Edit, &["edit", "write", "patch", "apply_patch", "multiedit", "move", "NotebookEdit"]),
            (PermissionKind::Shell, &["bash", "shell", "execute", "BashOutput", "KillShell", "KillBash", "Bash"]),
            (PermissionKind::Web, &["webfetch", "websearch", "WebFetch"]),
            (PermissionKind::Other, &["frobnicate", "external_directory", "", "doom_loop"]),
        ];
        for (kind, names) in server {
            for name in names {
                assert_eq!((*name, get_tool(name).permission), (*name, kind));
            }
        }
    }

    #[test]
    fn decides_the_two_names_the_client_knows_and_the_server_does_not() {
        assert_eq!(get_tool("terminal").permission, PermissionKind::Shell);
        assert_eq!(get_tool("strreplaceeditor").permission, PermissionKind::Edit);
    }
}

mod same_answers_as_the_sites_it_replaces {
    use super::*;

    #[test]
    fn falls_back_to_the_tool_name_where_tool_labels_has_no_case() {
        for name in ["task", "list", "frobnicate", "fleet_message"] {
            assert_eq!(label(name, json!({"description": "x"})), name);
        }
    }

    #[test]
    fn labels_the_tools_that_take_a_file_or_folder_by_it_whichever_field_the_harness_used() {
        assert_eq!(label("list", json!({"path": "src"})), "src");
        assert_eq!(label("notebookedit", json!({"notebook_path": "notebooks/demo.ipynb"})), "notebooks/demo.ipynb");
        assert_eq!(label("patch", json!({"filePath": "src/a.ts"})), "src/a.ts");
        assert_eq!(label("lsp", json!({"filePath": "src/a.ts"})), "src/a.ts");
        assert_eq!(label("list", json!({})), "list");
    }

    #[test]
    fn labels_every_known_tool_from_the_same_inputs_without_panicking() {
        let inputs = [
            Value::Null,
            json!({}),
            json!({"filePath": "src/widgets/panel.ts"}),
            json!({"path": "/home/someone/a/very/long/directory/name/that/keeps/going/on/and/on/file.ts"}),
            json!({"command": "dotnet test --filter Something", "description": "Run the tests"}),
            json!({"command": "x".repeat(90)}),
            json!({"pattern": "**/*.ts"}),
            json!({"url": "https://example.test/docs", "query": "reconnect backoff"}),
            json!({"name": "fleet-run"}),
            json!({"agent": "explore", "description": "Look around"}),
            json!({"questions": [{"question": "Which folder?"}]}),
            json!({"code": "\n  await page.open()\nmore"}),
            json!({"title": "Panel", "command": "bun run dev"}),
            json!({"viewport": "phone", "path": "/settings"}),
            json!({"what": "find", "text": "Reconnect"}),
            json!({"action": "fill", "url": "", "ref": "", "text": "hello"}),
        ];
        for name in all_names() {
            for input in &inputs {
                assert!(!label(name, input.clone()).is_empty() || name.is_empty(), "{name} {input}");
            }
        }
        assert_eq!(label("bash", json!({"command": "x".repeat(90)})), format!("{}…", "x".repeat(60)));
    }
}

mod todowrite {
    use super::*;

    #[test]
    fn has_a_label_a_header_and_a_todo_list_icon() {
        let tool = get_tool("todowrite");
        assert_eq!(tool.icon, ToolIcon::ListTodo);
        assert_eq!(tool.heading, "Update todos");
        assert_eq!(tool.label(&Value::Null), "todowrite");
        assert_eq!(tool.label(&json!({"todos": [{"content": "a"}, {"content": "b"}]})), "2 todos");
        assert_eq!(tool.label(&json!({"todos": [{"content": "a"}]})), "1 todo");
        assert_eq!(label("todowrite", json!({"todos": []})), "todowrite");
        assert_eq!(get_tool("TodoWrite").icon, ToolIcon::ListTodo);
        assert_eq!(get_tool("todoread").icon, ToolIcon::ListChecks);
    }
}

mod parse_tool_input_tests {
    use super::*;

    #[test]
    fn reads_the_file_under_any_harnesss_name() {
        assert_eq!(parse_tool_input(&json!({"filePath": "a.ts"})).file_path.as_deref(), Some("a.ts"));
        assert_eq!(parse_tool_input(&json!({"path": "b.ts"})).file_path.as_deref(), Some("b.ts"));
        assert_eq!(parse_tool_input(&json!({"file_path": "c.ts"})).file_path.as_deref(), Some("c.ts"));
        assert_eq!(parse_tool_input(&json!({"notebook_path": "d.ipynb"})).file_path.as_deref(), Some("d.ipynb"));
        assert_eq!(parse_tool_input(&json!({"filePath": "", "path": "b.ts"})).file_path.as_deref(), Some("b.ts"));
    }

    #[test]
    fn reads_the_common_fields_and_ignores_blanks_and_wrong_types() {
        let input = parse_tool_input(&json!({
            "command": "ls", "description": "  ", "pattern": 3, "url": "https://example.test", "query": "q", "name": "fleet-run",
            "agent": "explore", "content": "one\ntwo", "old_string": "a", "newString": "b", "code": "run()",
        }));
        assert_eq!(input.command.as_deref(), Some("ls"));
        assert_eq!(input.description, None);
        assert_eq!(input.pattern, None);
        assert_eq!(input.url.as_deref(), Some("https://example.test"));
        assert_eq!(input.query.as_deref(), Some("q"));
        assert_eq!(input.skill.as_deref(), Some("fleet-run"));
        assert_eq!(input.agent.as_deref(), Some("explore"));
        assert_eq!(input.content.as_deref(), Some("one\ntwo"));
        assert_eq!(input.old_string.as_deref(), Some("a"));
        assert_eq!(input.new_string.as_deref(), Some("b"));
        assert_eq!(input.code.as_deref(), Some("run()"));
    }

    #[test]
    fn takes_a_skills_id_when_it_has_no_name() {
        assert_eq!(parse_tool_input(&json!({"id": "fleet-design"})).skill.as_deref(), Some("fleet-design"));
    }

    #[test]
    fn reads_questions() {
        let questions =
            parse_tool_input(&json!({"questions": [{"question": "Which?", "header": "Pick"}, "nope", {"header": "Only"}]})).questions;
        assert_eq!(
            questions,
            [
                ToolQuestion { question: Some("Which?".into()), header: Some("Pick".into()) },
                ToolQuestion { question: None, header: Some("Only".into()) },
            ]
        );
        assert!(parse_tool_input(&json!({"questions": "x"})).questions.is_empty());
    }

    #[test]
    fn copes_with_anything_that_is_not_a_record() {
        for value in [Value::Null, json!("x"), json!(4), json!([]), json!([{"command": "ls"}])] {
            let input = parse_tool_input(&value);
            assert_eq!(input.command, None);
            assert!(input.questions.is_empty());
            assert!(input.raw.is_empty());
        }
    }

    #[test]
    fn as_record_accepts_only_plain_objects() {
        assert_eq!(as_record(&json!({"a": 1})).map(|m| m.len()), Some(1));
        assert!(as_record(&json!([])).is_none());
        assert!(as_record(&Value::Null).is_none());
        assert!(as_record(&json!("x")).is_none());
    }
}

mod hydrafusion_passes {
    use super::*;

    #[test]
    fn reads_as_role_model_and_time_once_the_pass_has_finished() {
        assert_eq!(label("hydrafusion_pass", json!({"role": "solver", "model": "model-a"})), "Solver · model-a");
        assert_eq!(label("hydrafusion_pass", json!({"role": "draft", "model": "model-a", "durationMs": 18400})), "Draft · model-a · 18 s");
        assert_eq!(label("hydrafusion_pass", Value::Null), "hydrafusion_pass");
        assert_eq!(get_tool("hydrafusion_pass").heading, "HydraFusion pass");
    }

    #[test]
    fn adds_a_judges_or_critics_verdict_the_credits_it_cost_and_why_a_failed_pass_failed() {
        assert_eq!(
            label(
                "hydrafusion_pass",
                json!({"role": "critic", "model": "model-b", "durationMs": 43042, "verdict": "needs repair", "credits": 0.2})
            ),
            "Critic · model-b · needs repair · 43 s · 0.20 credits"
        );
        assert_eq!(
            label("hydrafusion_pass", json!({"role": "judge", "model": "model-b", "verdict": "approved"})),
            "Judge · model-b · approved"
        );
        assert_eq!(
            label("hydrafusion_pass", json!({"role": "critic", "model": "model-b", "reason": "The critic timed out"})),
            "Critic · model-b · The critic timed out"
        );
    }

    #[test]
    fn heads_a_turn_with_its_workflow_credits_and_requests() {
        assert_eq!(
            label("copilot_turn", json!({"pattern": "critique", "credits": 16.83, "requests": 7})),
            "Critique · 16.83 credits · 7 requests"
        );
        assert_eq!(label("copilot_turn", json!({"pattern": "single"})), "Single");
        assert_eq!(label("copilot_turn", json!({"credits": 0.25, "requests": 1})), "0.25 credits · 1 request");
        assert_eq!(label("copilot_turn", Value::Null), "copilot_turn");
        assert_eq!(get_tool("copilot_turn").heading, "Turn");
    }
}

mod get_tool_label {
    use super::*;

    #[test]
    fn labels_opencodes_tools_from_their_input() {
        assert_eq!(label("bash", json!({"command": "echo hi", "description": "Say hi"})), "Say hi");
        assert_eq!(label("read", json!({"filePath": "/repo/src/lib/tool-labels.ts"})), "/repo/src/lib/tool-labels.ts");
        assert_eq!(
            label("edit", json!({"filePath": "/a/very/long/path/that/goes/on/and/on/for/a/while/src/lib/tool-labels.ts"})),
            "…/lib/tool-labels.ts"
        );
        assert_eq!(label("skill", json!({"name": "fleet-run"})), "fleet-run");
    }

    #[test]
    fn labels_opencode_2s_shell_like_opencodes_bash() {
        assert_eq!(label("shell", json!({"command": "echo from-tool"})), "echo from-tool");
        assert_eq!(label("shell", json!({"command": "echo from-tool", "description": "Echo"})), "Echo");
        assert_eq!(label("shell", Value::Null), "shell");
    }

    #[test]
    fn reads_the_file_opencode_2_names_path() {
        assert_eq!(label("read", json!({"path": "/work/alpha/note.txt"})), "/work/alpha/note.txt");
        assert_eq!(label("edit", json!({"path": "src/app.ts", "oldString": "a", "newString": "b"})), "src/app.ts");
        assert_eq!(label("write", json!({"path": "src/new.ts", "content": ""})), "src/new.ts");
        assert_eq!(label("read", json!({})), "read");
    }

    #[test]
    fn labels_opencode_2s_other_tools() {
        assert_eq!(label("glob", json!({"pattern": "**/*.ts"})), "**/*.ts");
        assert_eq!(label("grep", json!({"pattern": "TODO", "path": "src"})), "TODO");
        assert_eq!(label("webfetch", json!({"url": "https://opencode.ai/v2/docs"})), "https://opencode.ai/v2/docs");
        assert_eq!(label("websearch", json!({"query": "opencode 2 forms"})), "opencode 2 forms");
        assert_eq!(label("skill", json!({"id": "fleet-run"})), "fleet-run");
        assert_eq!(
            label("subagent", json!({"agent": "general", "description": "Child work", "prompt": "say hello"})),
            "general · Child work"
        );
        assert_eq!(label("subagent", json!({"agent": "general"})), "general");
        assert_eq!(label("question", json!({"questions": [{"question": "Pick one", "header": "Pick", "options": []}]})), "Pick one");
        assert_eq!(
            label("execute", json!({"code": "\n  const files = await glob({ pattern: '*.ts' });\nreturn files;"})),
            "const files = await glob({ pattern: '*.ts' });"
        );
    }

    #[test]
    fn labels_a_shown_page_by_its_tab_and_file() {
        assert_eq!(
            label("fleet_page_show", json!({"path": "/tmp/mockups/options.html", "title": "Options"})),
            "Options · /tmp/mockups/options.html"
        );
        assert_eq!(label("fleet_page_show", json!({"path": "/tmp/mockups/options.html"})), "/tmp/mockups/options.html");
        assert_eq!(label("fleet_page_show", Value::Null), "fleet_page_show");
        assert_eq!(get_tool("fleet_page_show").heading, "Show page");
        assert_ne!(get_tool("fleet_page_show").icon, get_tool("unknown-tool").icon);
    }

    #[test]
    fn labels_a_walkthrough_by_its_title() {
        assert_eq!(
            label("fleet_walkthrough_show", json!({"title": "Orders ship from the nearest warehouse", "guide": {}})),
            "Orders ship from the nearest warehouse"
        );
        assert_eq!(label("fleet_walkthrough_show", Value::Null), "fleet_walkthrough_show");
        assert_eq!(get_tool("fleet_walkthrough_show").heading, "Walkthrough");
        assert_ne!(get_tool("fleet_walkthrough_show").icon, get_tool("unknown-tool").icon);
    }

    #[test]
    fn names_reading_an_at_referenced_session() {
        assert_eq!(get_tool("fleet_session_read").heading, "Read session");
        assert_ne!(get_tool("fleet_session_read").icon, get_tool("unknown-tool").icon);
    }

    #[test]
    fn falls_back_to_the_tools_name_when_the_input_says_nothing() {
        for name in ["websearch", "subagent", "question", "execute"] {
            assert_eq!(label(name, Value::Null), name);
        }
    }

    #[test]
    fn names_opencode_2s_tools_and_gives_them_the_icon_of_the_opencode_tool_they_replace() {
        assert_eq!(get_tool("shell").heading, "Shell");
        assert_eq!(get_tool("subagent").heading, "Subagent");
        assert_eq!(get_tool("websearch").heading, "Web Search");
        assert_eq!(get_tool("execute").heading, "Code");
        assert_eq!(get_tool("shell").icon, get_tool("bash").icon);
        assert_eq!(get_tool("subagent").icon, get_tool("task").icon);
        assert_ne!(get_tool("execute").icon, get_tool("unknown-tool").icon);
    }

    /// Fleet's own tools, from the rows in the checklist that the Vue tests only reach through the card.
    #[test]
    fn labels_the_rest_of_fleets_tools() {
        assert_eq!(label("fleet_app_start", json!({"title": "Shop", "url": "http://localhost:5173/"})), "Shop · http://localhost:5173/");
        assert_eq!(label("fleet_browser_screenshot", json!({"viewport": "phone", "path": "/settings"})), "screenshot /settings (phone)");
        assert_eq!(label("fleet_browser_screenshot", json!({})), "screenshot (desktop)");
        assert_eq!(label("fleet_browser_read", json!({"what": "find", "text": "Reconnect"})), "find “Reconnect”");
        assert_eq!(label("fleet_browser_act", json!({"action": "fill", "url": "", "ref": "", "text": "hello"})), "fill hello");
        assert_eq!(label("fleet_browser_act", json!({"action": "click", "ref": "@e3"})), "click @e3");
        assert_eq!(label("fleet_memory_save", json!({})), "remember");
        assert_eq!(label("fleet_memory_search", json!({"query": "scratch"})), "scratch");
        assert_eq!(label("fleet_memory_forget", json!({"id": "note-7"})), "note-7");
        assert_eq!(label("fleet_mod_check", json!({})), "check mod");
        assert_eq!(label("fleet_mod_keep", json!({"name": "chips"})), "chips");
    }
}

mod how_the_client_classifies_tools {
    use super::*;

    #[test]
    fn edit_like_tools_share_the_pencil_and_label_by_their_file() {
        for name in ["notebookedit", "strreplaceeditor", "multiedit", "patch", "apply_patch"] {
            assert_eq!(get_tool(name).category, Category::Edit, "{name}");
            assert_eq!(get_tool(name).icon, ToolIcon::Pencil, "{name}");
            assert_eq!(label(name, json!({"filePath": "src/widgets/panel.ts"})), "src/widgets/panel.ts", "{name}");
        }
        assert_eq!(get_tool("apply_patch").heading, "Patch");
        assert_eq!(label("notebookedit", json!({"notebook_path": "notebooks/demo.ipynb"})), "notebooks/demo.ipynb");
    }

    #[test]
    fn task_has_no_label_builder_though_its_icon_and_heading_exist_subagent_has_all_three() {
        assert_eq!(label("task", json!({"description": "Check the panel", "agent": "explore"})), "task");
        assert_eq!(label("subagent", json!({"description": "Check the panel", "agent": "explore"})), "explore · Check the panel");
        assert_eq!(get_tool("task").icon, ToolIcon::GitBranch);
        assert_eq!(get_tool("task").heading, "Task");
    }

    #[test]
    fn shell_like_tools_terminal_is_a_shell_and_execute_keeps_the_code_icon() {
        assert_eq!(get_tool("terminal").icon, ToolIcon::Terminal);
        assert_eq!(get_tool("terminal").category, Category::Shell);
        assert_eq!(get_tool("execute").icon, ToolIcon::Code);
        assert!(is_shell_row("terminal") && is_shell_row("bash") && is_shell_row("shell"));
        assert!(!is_shell_row("execute"));
    }

    #[test]
    fn read_search_and_web_tools_list_and_ls_get_a_folder_codesearch_a_search() {
        assert_eq!(get_tool("list").icon, ToolIcon::Folder);
        assert_eq!(get_tool("LS").icon, ToolIcon::Folder);
        assert_eq!(get_tool("websearch").icon, ToolIcon::Search);
        assert_eq!(get_tool("codesearch").icon, ToolIcon::Search);
        assert_eq!(get_tool("lsp").category, Category::Read);
    }

    #[test]
    fn case_does_not_matter_anywhere() {
        assert_eq!(get_tool("Read").icon, ToolIcon::FileText);
        assert_eq!(label("Bash", json!({"description": "Check the panel"})), "Check the panel");
        assert_eq!(get_tool("Bash").icon, ToolIcon::Terminal);
        assert_eq!(get_tool("SHELL").heading, "Shell");
        // The unknown name keeps its own casing when nothing is known.
        assert_eq!(get_tool("frobnicate").heading, "Frobnicate");
    }

    #[test]
    fn agent_is_a_subagent_drawn_as_a_task() {
        let agent = get_tool("Agent");
        assert_eq!(agent.category, Category::Subagent);
        assert_eq!(agent.icon, ToolIcon::GitBranch);
        assert_eq!(agent.heading, "Task");
    }

    #[test]
    fn the_subagent_set_is_task_agent_and_subagent_either_case() {
        for name in ["task", "subagent", "Task", "Subagent", "Agent"] {
            assert_eq!(get_tool(name).category, Category::Subagent, "{name}");
        }
    }

    #[test]
    fn the_pattern_pill_is_glob_and_grep_in_any_case() {
        assert!(get_tool("glob").pattern_label && get_tool("Grep").pattern_label);
        assert!(!get_tool("websearch").pattern_label);
    }

    #[test]
    fn fleets_titled_tools_take_their_title_from_fleets_answer_the_memory_and_browser_read_act_tools_do_not() {
        for name in ["fleet_app_start", "fleet_browser_open", "fleet_browser_screenshot", "fleet_page_show", "fleet_walkthrough_show"] {
            assert!(get_tool(name).titled_from_answer, "{name}");
        }
        for name in ["fleet_browser_read", "fleet_browser_act", "fleet_memory_save", "fleet_step_done"] {
            assert!(!get_tool(name).titled_from_answer, "{name}");
        }
    }

    #[test]
    fn mcp_fleet_x_is_fleets_tool_x_everywhere_another_servers_tool_is_unknown_and_shown_as_its_own_name() {
        let shown = get_tool("mcp__fleet__fleet_page_show");
        assert_eq!(shown.display_name, "Fleet_page_show");
        assert_eq!(shown.icon, ToolIcon::PanelsTopLeft);
        assert_eq!(shown.heading, "Show page");
        assert!(shown.titled_from_answer);
        let other = get_tool("mcp__acme__lookup_order");
        assert_eq!(other.display_name, "Lookup_order");
        assert_eq!(other.icon, ToolIcon::Wrench);
        assert_eq!(other.heading, "Mcp__acme__lookup_order");
    }

    #[test]
    fn the_question_checks_match_question_exactly() {
        assert_eq!(get_tool("question").category, Category::Question);
        assert_ne!(get_tool("Task").category, Category::Question);
    }
}
