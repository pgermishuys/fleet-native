//! Component 16, piece AJ: Settings → Skills & tools → Tools → Custom (rows 8.49 to 8.54). Fleet has no test for the tab, so these
//! are written from the rows: the switch between the forms, what each form asks for before it sends anything, the body each
//! sends, the failures, what is cleared and what is kept. The parsing itself is tested beside it in `tool_custom.rs`.
use crate::fleet::settings::tool_custom::Mode;
use crate::test_wait;
use crate::tests::app_tall;
use crate::tests_settings_skills::{Rig, rig_in};
use rune::TextEdit;
use serde_json::{Value, json};

fn custom_open() -> Rig {
    custom_open_with(|_| {})
}

fn custom_open_with(setup: impl FnOnce(&mut crate::tests_settings_skills::Fake)) -> Rig {
    let mut r = rig_in(app_tall(), None, setup);
    r.read();
    r.press("skills-tools-tab-tools");
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the tools to be read",
        |s| s.settings.tools.active && !s.settings.tools.loading && !s.settings.tools.repos_loading,
        |s| format!("{:?}", s.settings.tools),
    );
    r.press("tools-tab-custom");
    r.settle();
    r
}

fn page(r: &Rig) -> &crate::fleet::settings::tools::ToolsPage {
    &r.rt.state.settings.tools
}

fn type_in(r: &mut Rig, field: &'static str, words: &str) {
    let words = words.to_string();
    r.rt.update(move |s, _| {
        let p = &mut s.settings.tools;
        let edit = match field {
            "url" => &mut p.native_url,
            "name" => &mut p.mcp_name,
            "command" => &mut p.mcp_command,
            "args" => &mut p.mcp_args,
            _ => &mut p.mcp_env,
        };
        *edit = TextEdit::new(words);
    });
    r.settle();
}

fn installs(r: &Rig) -> Vec<Value> {
    r.fake.lock().unwrap().tool_installs.clone()
}

fn wait_installed(r: &mut Rig) {
    test_wait::drive(&mut r.rt, &r.queue, "the install", |s| !s.settings.tools.installing, |s| format!("{:?}", s.settings.tools));
    r.settle();
}

#[test]
fn opens_on_native_with_its_form_and_examples() {
    let r = custom_open();
    assert_eq!(page(&r).mode, Mode::Native);
    assert!(r.text("tools-custom-form").contains("Install from URL or path"));
    assert!(r.text("tools-custom-form").contains("Install a native tool from a GitHub URL or local file path."));
    assert_eq!(r.text("tools-native-url"), "");
    assert_eq!(r.text("tools-custom-submit"), "Install tool");
    assert_eq!(r.text("tools-target-line"), "Every session \u{b7} ~/.config/opencode/tools/");
    let examples = r.text("tools-custom-examples");
    for line in [
        "Native tools:",
        "https://github.com/username/repo/blob/main/tools/my-tool.ts",
        "https://github.com/username/my-tool",
        "/Users/username/tools/my-tool.ts",
    ] {
        assert!(examples.contains(line), "{line:?} in {examples:?}");
    }
    assert!(!r.exists("tools-mcp-name"));
}

#[test]
fn the_mcp_form_names_opencode_json_and_has_its_four_fields_and_example() {
    let mut r = custom_open();
    r.press("tools-mode-mcp");
    r.settle();
    assert_eq!(page(&r).mode, Mode::Mcp);
    let form = r.text("tools-custom-form");
    for words in [
        "Configure MCP server",
        "Install an MCP server with custom command, arguments, and environment variables.",
        "Arguments (one per line)",
        "Environment variables (KEY=VALUE per line)",
    ] {
        assert!(form.to_lowercase().contains(&words.to_lowercase()), "{words:?} in {form:?}");
    }
    assert_eq!(r.text("tools-custom-submit"), "Install MCP server");
    assert_eq!(r.text("tools-target-line"), "Every session \u{b7} ~/.config/opencode/opencode.json");
    let examples = r.text("tools-custom-examples");
    for line in [
        "MCP server example:",
        "Name:",
        "filesystem",
        "Command:",
        "npx",
        "-y @modelcontextprotocol/server-filesystem /path/to/allowed/files",
    ] {
        assert!(examples.contains(line), "{line:?} in {examples:?}");
    }
    // (`text` would still read the Native lines while they fade out, so ask whether they are there.)
    assert!(!r.exists("tools-custom-examples-native") && r.exists("tools-custom-examples-mcp"));
}

#[test]
fn the_mcp_picker_follows_the_repository_chosen_for_tools() {
    let mut r = custom_open();
    r.press("tools-mode-mcp");
    r.settle();
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.settle();
    assert_eq!(r.text("tools-target-line"), "Sessions in this repository \u{b7} /src/app/opencode.json");
    r.press("tools-mode-native");
    r.settle();
    assert_eq!(r.text("tools-target-line"), "Sessions in this repository \u{b7} /src/app/.opencode/tools/");
}

#[test]
fn switching_keeps_what_was_typed_in_each_form_and_clears_the_error() {
    let mut r = custom_open();
    type_in(&mut r, "url", "https://github.com/example");
    r.press("tools-custom-submit");
    assert!(r.exists("tools-custom-error"));
    r.press("tools-mode-mcp");
    r.settle();
    assert!(!r.exists("tools-custom-error"), "the error goes with the switch");
    type_in(&mut r, "name", "fs");
    r.press("tools-mode-native");
    r.settle();
    assert_eq!(page(&r).native_url.text, "https://github.com/example");
    r.press("tools-mode-mcp");
    r.settle();
    assert_eq!(page(&r).mcp_name.text, "fs");
}

#[test]
fn asks_for_an_address_before_sending_anything() {
    let mut r = custom_open();
    type_in(&mut r, "url", "   ");
    r.press("tools-custom-submit");
    assert_eq!(r.text("tools-custom-error"), "Tool URL or path is required.");
    assert!(installs(&r).is_empty());
    assert!(!page(&r).installing);
}

#[test]
fn refuses_a_github_link_it_cannot_read_without_asking_the_server() {
    let mut r = custom_open();
    type_in(&mut r, "url", "https://github.com/example");
    r.press("tools-custom-submit");
    assert_eq!(r.text("tools-custom-error"), "Use a GitHub link to a repository, a folder, or a .ts/.js file.");
    assert!(installs(&r).is_empty());
    assert!(!page(&r).installing, "nothing was installing");
    assert_eq!(page(&r).native_url.text, "https://github.com/example", "what was typed stays");
}

#[test]
fn installs_a_github_file_as_its_folder_named_after_the_file() {
    let mut r = custom_open();
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    type_in(&mut r, "url", "https://github.com/example/kit/blob/main/tools/csv-peek.ts");
    r.press("tools-custom-submit");
    wait_installed(&mut r);
    let sent = installs(&r);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0],
        json!({
            "name": "csv-peek", "toolType": "native", "source": 1, "command": null, "args": null, "env": null,
            "repoUrl": "https://github.com/example/kit.git", "ref": "main", "subPath": "tools", "localPath": null,
            "scope": "project", "projectPath": "/src/app",
        })
    );
    assert_eq!(page(&r).native_url.text, "", "success clears the field");
    assert!(page(&r).tools.iter().any(|t| t.name == "csv-peek"), "the list was read again");
}

#[test]
fn installs_a_local_file_named_without_its_extension() {
    let mut r = custom_open();
    type_in(&mut r, "url", "/work/tools/csv-peek.ts");
    r.press("tools-custom-submit");
    wait_installed(&mut r);
    let sent = installs(&r);
    assert_eq!(
        (sent[0]["name"].clone(), sent[0]["source"].clone(), sent[0]["localPath"].clone(), sent[0]["repoUrl"].clone()),
        (json!("csv-peek"), json!(2), json!("/work/tools/csv-peek.ts"), Value::Null)
    );
}

#[test]
fn installs_an_mcp_server_with_its_arguments_and_variables_and_clears_all_four_fields() {
    let mut r = custom_open();
    r.press("tools-mode-mcp");
    r.settle();
    type_in(&mut r, "name", "fs");
    type_in(&mut r, "command", "node");
    type_in(&mut r, "args", "/path/to/server.js\n --port \n\n3000");
    type_in(&mut r, "env", "DEBUG=true\nTOKEN = a=b\nnope");
    r.press("tools-custom-submit");
    wait_installed(&mut r);
    let sent = installs(&r);
    assert_eq!(
        sent[0],
        json!({
            "name": "fs", "toolType": "mcp", "source": 2, "command": "node", "args": ["/path/to/server.js", "--port", "3000"],
            "env": { "DEBUG": "true", "TOKEN": "a=b" }, "repoUrl": null, "ref": null, "subPath": null, "localPath": null,
            "scope": "global", "projectPath": null,
        })
    );
    let p = page(&r);
    assert_eq!([&p.mcp_name.text, &p.mcp_command.text, &p.mcp_args.text, &p.mcp_env.text], ["", "", "", ""]);
    assert!(p.tools.iter().any(|t| t.name == "fs"));
}

#[test]
fn mcp_asks_for_the_name_and_then_the_command() {
    let mut r = custom_open();
    r.press("tools-mode-mcp");
    r.settle();
    r.press("tools-custom-submit");
    assert_eq!(r.text("tools-custom-error"), "Name is required.");
    type_in(&mut r, "name", "fs");
    r.press("tools-custom-submit");
    assert_eq!(r.text("tools-custom-error"), "Command is required.");
    assert!(installs(&r).is_empty());
}

#[test]
fn sends_null_for_no_arguments_and_no_variables() {
    let mut r = custom_open();
    r.press("tools-mode-mcp");
    r.settle();
    type_in(&mut r, "name", "fs");
    type_in(&mut r, "command", "node");
    type_in(&mut r, "env", "no equals sign here");
    r.press("tools-custom-submit");
    wait_installed(&mut r);
    let sent = installs(&r);
    assert!(sent[0]["args"].is_null() && sent[0]["env"].is_null(), "{:?}", sent[0]);
}

#[test]
fn shows_the_servers_reason_in_the_form_and_keeps_what_was_typed() {
    let mut r = custom_open_with(|f| f.tool_install = Some((409, json!({ "detail": "Tool already exists" }).to_string())));
    r.press("tools-mode-mcp");
    r.settle();
    type_in(&mut r, "name", "fs");
    type_in(&mut r, "command", "node");
    r.press("tools-custom-submit");
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the failure",
        |s| s.settings.tools.custom_error.is_some(),
        |s| format!("{:?}", s.settings.tools),
    );
    r.settle();
    assert_eq!(r.text("tools-custom-error"), "Tool already exists");
    assert_eq!(page(&r).mcp_name.text, "fs");
    assert!(!page(&r).installing);
    assert_eq!(r.text("tools-custom-submit"), "Install MCP server");
}

#[test]
fn takes_an_install_answer_with_nothing_in_it_for_a_failure() {
    let mut r = custom_open_with(|f| f.tool_install = Some((200, String::new())));
    type_in(&mut r, "url", "/work/tools/csv-peek.ts");
    r.press("tools-custom-submit");
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the failure",
        |s| s.settings.tools.custom_error.is_some(),
        |s| format!("{:?}", s.settings.tools),
    );
    assert_eq!(page(&r).custom_error.as_deref(), Some("No data returned"));
    assert_eq!(page(&r).native_url.text, "/work/tools/csv-peek.ts");
}

#[test]
fn the_other_form_keeps_its_words_after_an_install() {
    let mut r = custom_open();
    type_in(&mut r, "name", "fs");
    type_in(&mut r, "url", "/work/tools/csv-peek.ts");
    r.press("tools-custom-submit");
    wait_installed(&mut r);
    assert_eq!(page(&r).native_url.text, "");
    assert_eq!(page(&r).mcp_name.text, "fs");
}

#[test]
fn forgets_the_forms_when_the_tab_is_left() {
    let mut r = custom_open();
    r.press("tools-mode-mcp");
    r.settle();
    type_in(&mut r, "name", "fs");
    r.press("tools-tab-installed");
    r.press("tools-tab-custom");
    r.settle();
    assert_eq!(page(&r).mode, Mode::Native);
    assert_eq!(page(&r).mcp_name.text, "");
    assert!(!r.exists("tools-custom-error"));
}

#[test]
fn keeps_the_install_target_between_catalog_and_custom() {
    let mut r = custom_open();
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.press("tools-tab-catalog");
    r.press("tools-tab-custom");
    r.settle();
    assert_eq!(r.text("tools-target-line"), "Sessions in this repository \u{b7} /src/app/.opencode/tools/");
}

#[test]
fn installing_into_a_picked_repository_sends_its_path() {
    let mut r = custom_open();
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.press("tools-mode-mcp");
    r.settle();
    type_in(&mut r, "name", "fs");
    type_in(&mut r, "command", "node");
    r.press("tools-custom-submit");
    wait_installed(&mut r);
    let sent = installs(&r);
    assert_eq!((sent[0]["scope"].clone(), sent[0]["projectPath"].clone()), (json!("project"), json!("/src/app")));
}

#[test]
fn does_not_change_the_form_while_an_install_runs() {
    let mut r = custom_open();
    r.rt.update(|s, _| s.settings.tools.installing = true);
    r.settle();
    r.rt.update(|s, _| s.set_tool_mode(Mode::Mcp));
    assert_eq!(page(&r).mode, Mode::Native);
    assert_eq!(r.text("tools-custom-submit"), "Installing\u{2026}");
}
