//! Component 16, piece AI: Settings → Skills & tools → Tools, Installed and Catalog. Fleet has no test for any of it, so these
//! are written from the rows (8.41 to 8.48): what each tab reads and shows, the remove confirm, the body an install from a card
//! sends, the failures, that Installed means installed *here*, that Tools keeps its own install target, and that a tab that is
//! left forgets what it read.
use crate::fleet::settings::install_target::InstallTarget;
use crate::fleet::settings::skills::{Inner, Tab};
use crate::fleet::settings::tools::{self, ToolType, parse_catalog, parse_tools};
use crate::tests::{FRAME, app, click};
use crate::tests_settings_skills::{Rig, rig_at};
use crate::{Rail, test_wait};
use serde_json::{Value, json};

/// A tool as `GET /api/tools` sends it.
pub(crate) fn tool(name: &str, over: Value) -> Value {
    let mut v = json!({
        "name": name, "scope": "global", "projectPath": null, "toolType": "native", "displayName": null, "description": null,
        "command": null, "args": null, "env": null, "repoUrl": null, "localPath": format!("/work/tools/{name}.ts"),
        "installedPath": format!("/home/user/.config/opencode/tools/{name}.ts"),
        "installedAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
    });
    for (k, x) in over.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

fn mcp(name: &str, over: Value) -> Value {
    let mut v =
        tool(name, json!({ "toolType": "mcp", "localPath": null, "command": "node", "args": ["/path/to/server.js", "--port", "3000"] }));
    for (k, x) in over.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

/// A catalog entry as `GET /api/tools/catalog` sends it.
fn card(name: &str, over: Value) -> Value {
    let mut v = json!({
        "name": name, "toolType": "native", "source": 1, "displayName": null, "description": "Does a thing.", "command": null,
        "args": null, "env": null, "repoUrl": format!("https://github.com/example/{name}.git"), "ref": "main", "subPath": null,
        "localPath": null, "author": null, "version": null, "tags": [], "createdAt": null, "updatedAt": null,
    });
    for (k, x) in over.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

fn project(path: &str) -> InstallTarget {
    InstallTarget::Project(path.into())
}

fn wait_tools(r: &mut Rig, what: &str, done: impl FnMut(&crate::State) -> bool) {
    test_wait::drive(&mut r.rt, &r.queue, what, done, |s| format!("{:?}", s.settings.tools));
}

/// The Tools tab open and read: the list and the repositories are in.
fn tools_open(setup: impl FnOnce(&mut crate::tests_settings_skills::Fake)) -> Rig {
    let mut r = rig_at(None, setup);
    r.read();
    r.press("skills-tools-tab-tools");
    wait_tools(&mut r, "the tools to be read", |s| s.settings.tools.active && !s.settings.tools.loading && !s.settings.tools.repos_loading);
    r.settle();
    r
}

fn catalog_open(setup: impl FnOnce(&mut crate::tests_settings_skills::Fake)) -> Rig {
    let mut r = tools_open(setup);
    r.press("tools-tab-catalog");
    wait_tools(&mut r, "the catalog", |s| !s.settings.tools.catalog_loading && !s.settings.tools.loading);
    r.settle();
    r
}

fn heard(r: &Rig, method: &str, prefix: &str) -> usize {
    r.heard_count(method, prefix)
}

fn page(r: &Rig) -> &tools::ToolsPage {
    &r.rt.state.settings.tools
}

// ─── the data ───────────────────────────────────────────────────────────────────

#[test]
fn reads_a_tools_answer_with_the_type_in_any_case_and_drops_a_tool_with_no_name() {
    let body = json!({ "tools": [
        tool("a", json!({ "toolType": "Native" })),
        mcp("b", json!({ "toolType": "MCP", "scope": "project", "projectPath": "/src/app" })),
        tool("c", json!({ "toolType": "something-new", "installedPath": null })),
        { "toolType": "native" },
    ]});
    let list = parse_tools(&body).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!([list[0].tool_type, list[1].tool_type, list[2].tool_type], [ToolType::Native, ToolType::Mcp, ToolType::Native]);
    assert_eq!(list[1].target, project("/src/app"));
    assert_eq!(list[2].installed_path, None);
    assert_eq!(list[0].installed_path.as_deref(), Some("/home/user/.config/opencode/tools/a.ts"));
    assert_eq!(parse_tools(&Value::Null), Err("No data returned".to_string()));
    assert_eq!(parse_tools(&json!({})).unwrap(), vec![]);
}

#[test]
fn says_the_command_with_its_arguments_joined_by_spaces() {
    let list = parse_tools(&json!({ "tools": [
        mcp("a", json!({})),
        mcp("b", json!({ "args": null })),
        mcp("c", json!({ "command": null })),
        tool("d", json!({ "localPath": null, "repoUrl": "https://github.com/example/d.git" })),
        tool("e", json!({ "localPath": null })),
    ]}))
    .unwrap();
    assert_eq!(list[0].source_line().as_deref(), Some("command: node /path/to/server.js --port 3000"));
    assert_eq!(list[1].source_line().as_deref(), Some("command: node"));
    assert_eq!(list[2].source_line(), None, "no command, no line");
    assert_eq!(list[3].source_line().as_deref(), Some("https://github.com/example/d.git"));
    assert_eq!(list[4].source_line(), None);
    assert_eq!(list[0].command_line(), "node /path/to/server.js --port 3000");
}

#[test]
fn reads_a_catalog_answer_and_defaults_the_source_to_github() {
    let body = json!({ "entries": [
        card("a", json!({ "displayName": "A", "tags": ["x", "y"], "version": "1.0.0", "source": null })),
        card("b", json!({ "toolType": "mcp", "source": 2 })),
        { "description": "nameless" },
    ]});
    let list = parse_catalog(&body).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].title(), list[0].source, list[0].tags.clone()), ("A", 1, vec!["x".to_string(), "y".to_string()]));
    assert_eq!((list[1].tool_type, list[1].source), (ToolType::Mcp, 2));
    assert_eq!(parse_catalog(&Value::Null), Err("No data returned".to_string()));
}

#[test]
fn builds_a_catalog_install_with_everything_the_entry_has() {
    let list = parse_catalog(&json!({ "entries": [
        card("g", json!({ "subPath": "tools/g.ts" })),
        card("m", json!({ "toolType": "mcp", "source": 2, "repoUrl": null, "command": "npx", "args": ["-y", "pkg"], "env": { "KEY": "v" } })),
    ]}))
    .unwrap();
    let body = list[0].install_body(&project("/src/app"));
    assert_eq!((body["name"].clone(), body["toolType"].clone(), body["source"].clone()), (json!("g"), json!("native"), json!(1)));
    assert_eq!(
        (body["repoUrl"].clone(), body["ref"].clone(), body["subPath"].clone()),
        (json!("https://github.com/example/g.git"), json!("main"), json!("tools/g.ts"))
    );
    assert_eq!((body["scope"].clone(), body["projectPath"].clone()), (json!("project"), json!("/src/app")));
    assert_eq!((body["command"].clone(), body["args"].clone(), body["env"].clone()), (Value::Null, Value::Null, Value::Null));
    let body = list[1].install_body(&InstallTarget::Global);
    assert_eq!((body["toolType"].clone(), body["source"].clone(), body["command"].clone()), (json!("mcp"), json!(2), json!("npx")));
    assert_eq!((body["args"].clone(), body["env"].clone()), (json!(["-y", "pkg"]), json!({ "KEY": "v" })));
    assert_eq!((body["scope"].clone(), body["projectPath"].clone()), (json!("global"), Value::Null));
}

#[test]
fn asks_for_a_tool_to_be_removed_by_name_and_target() {
    let global = tools::remove("my tool", &InstallTarget::Global);
    assert_eq!((global.method, global.path.as_str()), ("DELETE", "/api/tools/my%20tool?scope=global"));
    assert_eq!(tools::remove("t", &project("/src/app")).path, "/api/tools/t?scope=project&projectPath=%2Fsrc%2Fapp");
}

// ─── Installed ──────────────────────────────────────────────────────────────────

#[test]
fn reads_the_tools_only_when_the_tab_is_opened_and_says_what_it_is_for() {
    let mut r = rig_at(None, |f| f.tools = vec![tool("one", json!({}))]);
    r.read();
    assert_eq!(heard(&r, "GET", "/api/tools"), 0, "the Skills tab doesn't read tools");
    r.press("skills-tools-tab-tools");
    wait_tools(&mut r, "the tools", |s| s.settings.tools.active && !s.settings.tools.loading);
    r.settle();
    assert_eq!(r.page().tab, Tab::Tools);
    assert_eq!(heard(&r, "GET", "/api/tools"), 1);
    assert_eq!(
        r.text("tools-section").split("Installed").next().unwrap(),
        "Manage installed tools, browse the catalog, or install from a custom source."
    );
    assert_eq!(page(&r).inner, Inner::Installed);
}

#[test]
fn draws_a_card_for_a_native_tool_and_one_for_an_mcp_server() {
    let r = tools_open(|f| {
        f.tools = vec![
            tool("csv-peek", json!({ "description": "Prints rows.", "displayName": "CSV peek" })),
            mcp("files", json!({ "scope": "project", "projectPath": "/src/app", "installedPath": "/src/app/opencode.json" })),
        ]
    });
    let native = r.text("tool-csv-peek|global");
    assert!(native.contains("CSV peek") && native.contains("Native") && native.contains("Global"), "{native}");
    assert!(native.contains("Prints rows."));
    assert!(native.contains("/work/tools/csv-peek.ts"), "a native tool's local path");
    assert!(native.contains("\u{2192} /home/user/.config/opencode/tools/csv-peek.ts"));
    let server = r.text("tool-files|project:/src/app");
    assert!(server.contains("MCP") && server.contains("app"), "{server}");
    assert!(server.contains("command: node /path/to/server.js --port 3000"));
    assert!(server.contains("\u{2192} /src/app/opencode.json"));
    assert!(!server.contains("Not anywhere OpenCode looks."));
}

#[test]
fn warns_about_a_tool_that_is_not_anywhere_opencode_looks() {
    let r = tools_open(|f| f.tools = vec![tool("lost", json!({ "installedPath": null }))]);
    assert_eq!(r.text("tool-no-file-lost|global"), "Not anywhere OpenCode looks. Remove it and install it again.");
    assert!(!r.text("tool-lost|global").contains('\u{2192}'));
    assert!(r.exists("tool-remove-lost|global"), "it can still be removed");
}

#[test]
fn says_so_when_nothing_is_installed() {
    let r = tools_open(|_| {});
    assert!(r.text("tools-empty").contains("No tools installed"));
    assert!(r.text("tools-empty").contains("Install a tool from the Catalog or Custom tab."));
    assert!(!r.exists("tools-list"));
}

#[test]
fn shows_a_failed_read_alone_when_there_is_nothing_to_list() {
    let mut r = rig_at(None, |f| f.tools_broken = Some((500, "Network error".into())));
    r.read();
    r.press("skills-tools-tab-tools");
    wait_tools(&mut r, "the failure", |s| s.settings.tools.error.is_some());
    r.settle();
    assert_eq!(r.text("tools-error"), "Network error");
    assert!(!r.exists("tools-empty") && !r.exists("tools-list"));
}

#[test]
fn removes_a_tool_after_a_second_press_and_reads_the_list_again() {
    let mut r = tools_open(|f| f.tools = vec![tool("one", json!({})), tool("two", json!({}))]);
    assert_eq!(r.text("tool-remove-one|global"), "Remove");
    r.press("tool-remove-one|global");
    assert_eq!(r.text("tool-remove-one|global"), "Confirm remove");
    assert!(r.exists("tool-cancel-one|global"));
    assert_eq!(heard(&r, "DELETE", "/api/tools/"), 0, "the first press only asks");
    let reads = heard(&r, "GET", "/api/tools");
    r.press("tool-remove-one|global");
    wait_tools(&mut r, "the removal and the read after it", |s| s.settings.tools.removing.is_none() && s.settings.tools.tools.len() == 1);
    r.settle();
    assert_eq!(heard(&r, "DELETE", "/api/tools/one?scope=global"), 1);
    assert_eq!(heard(&r, "GET", "/api/tools") - reads, 1);
    assert!(!r.exists("tool-one|global") && r.exists("tool-two|global"));
    assert_eq!(page(&r).confirm_remove, None);
}

#[test]
fn cancels_a_remove_and_asks_about_one_card_at_a_time() {
    let mut r = tools_open(|f| f.tools = vec![tool("one", json!({})), tool("two", json!({}))]);
    r.press("tool-remove-one|global");
    r.press("tool-remove-two|global");
    r.settle();
    assert_eq!(r.text("tool-remove-one|global"), "Remove", "the question moved to the other card");
    assert_eq!(r.text("tool-remove-two|global"), "Confirm remove");
    r.press("tool-cancel-two|global");
    assert_eq!(r.text("tool-remove-two|global"), "Remove");
    assert!(!r.exists("tool-cancel-two|global"));
    assert_eq!(heard(&r, "DELETE", "/api/tools/"), 0);
}

#[test]
fn removes_the_install_in_the_repository_not_the_global_one() {
    let mut r =
        tools_open(|f| f.tools = vec![tool("one", json!({})), tool("one", json!({ "scope": "project", "projectPath": "/src/app" }))]);
    r.press("tool-remove-one|project:/src/app");
    r.press("tool-remove-one|project:/src/app");
    wait_tools(&mut r, "the removal", |s| s.settings.tools.removing.is_none() && s.settings.tools.tools.len() == 1);
    assert_eq!(heard(&r, "DELETE", "/api/tools/one?scope=project&projectPath=%2Fsrc%2Fapp"), 1);
    assert!(r.exists("tool-one|global"));
}

#[test]
fn shows_a_failed_remove_under_the_list_and_keeps_the_question() {
    let mut r = tools_open(|f| {
        f.tools = vec![tool("one", json!({}))];
        f.tool_remove = Some((500, json!({ "detail": "Could not remove" }).to_string()));
    });
    r.press("tool-remove-one|global");
    r.press("tool-remove-one|global");
    wait_tools(&mut r, "the failure", |s| s.settings.tools.error.is_some());
    r.settle();
    assert_eq!(r.text("tools-error"), "Could not remove");
    assert!(r.exists("tool-one|global"), "the list stays");
    assert_eq!(r.text("tool-remove-one|global"), "Confirm remove");
}

#[test]
fn forgets_what_it_read_when_the_tab_is_left() {
    let mut r = tools_open(|f| f.tools = vec![tool("one", json!({}))]);
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.press("skills-tools-tab-skills");
    r.settle();
    assert_eq!(*page(&r), tools::ToolsPage::default());
    r.press("skills-tools-tab-tools");
    wait_tools(&mut r, "the second read", |s| s.settings.tools.active && !s.settings.tools.loading && !s.settings.tools.tools.is_empty());
    assert_eq!(heard(&r, "GET", "/api/tools"), 2);
    assert_eq!(page(&r).target, InstallTarget::Global, "Fleet's section holds the target, and it is gone with the section");
}

// ─── Catalog ────────────────────────────────────────────────────────────────────

#[test]
fn draws_a_card_per_entry_with_its_type_author_tags_and_version() {
    let r = catalog_open(|f| {
        f.tool_entries = vec![
            card("one", json!({ "displayName": "Number one", "author": "Ada", "tags": ["data", "csv"], "version": "1.2.0" })),
            card("two", json!({ "toolType": "mcp", "description": null })),
        ]
    });
    assert!(r.exists("tools-catalog-grid"));
    let one = r.text("tool-catalog-card-one");
    assert!(
        one.contains("Number one") && one.contains("by Ada") && one.contains("data") && one.contains("Native") && one.contains("v1.2.0")
    );
    assert!(r.text("tool-catalog-card-two").contains("MCP"));
    assert!(!r.text("tool-catalog-card-two").contains("Does a thing."));
    assert_eq!(r.text("tool-catalog-install-one"), "Install");
}

#[test]
fn says_installed_only_for_a_tool_installed_where_the_picker_points() {
    let mut r = catalog_open(|f| {
        f.tool_entries = vec![card("one", json!({})), card("two", json!({}))];
        f.tools = vec![tool("one", json!({}))];
    });
    assert_eq!(r.text("tool-catalog-installed-one"), "Installed");
    assert!(r.exists("tool-catalog-install-two"));
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.settle();
    assert!(r.exists("tool-catalog-install-one"), "installed elsewhere does not count");
    assert!(!r.exists("tool-catalog-installed-one"));
}

#[test]
fn installs_from_a_card_and_reads_the_list_again_so_the_button_turns_to_installed() {
    let mut r = catalog_open(|f| f.tool_entries = vec![card("one", json!({ "subPath": "tools/one.ts" }))]);
    let reads = heard(&r, "GET", "/api/tools") - heard(&r, "GET", "/api/tools/catalog");
    r.press("tool-catalog-install-one");
    wait_tools(&mut r, "the install and the read after it", |s| {
        s.settings.tools.installing_name.is_none() && s.settings.tools.tools.len() == 1
    });
    r.settle();
    let sent = r.fake.lock().unwrap().tool_installs.clone();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0]["name"].clone(), sent[0]["toolType"].clone()), (json!("one"), json!("native")));
    assert_eq!(
        (sent[0]["repoUrl"].clone(), sent[0]["subPath"].clone()),
        (json!("https://github.com/example/one.git"), json!("tools/one.ts"))
    );
    assert_eq!((sent[0]["scope"].clone(), sent[0]["projectPath"].clone()), (json!("global"), Value::Null));
    let after = heard(&r, "GET", "/api/tools") - heard(&r, "GET", "/api/tools/catalog");
    assert_eq!(after - reads, 1, "one read after the install");
    assert_eq!(r.text("tool-catalog-installed-one"), "Installed");
    assert_eq!(page(&r).install_error, None);
}

#[test]
fn installs_an_mcp_entry_with_its_command_into_the_repository_the_picker_names() {
    let mut r = catalog_open(|f| {
        f.tool_entries =
            vec![card("srv", json!({ "toolType": "mcp", "source": 2, "repoUrl": null, "command": "npx", "args": ["-y", "pkg"] }))]
    });
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.press("tool-catalog-install-srv");
    wait_tools(&mut r, "the install", |s| !s.settings.tools.installing && s.settings.tools.installing_name.is_none());
    let sent = r.fake.lock().unwrap().tool_installs.clone();
    assert_eq!(
        (sent[0]["toolType"].clone(), sent[0]["command"].clone(), sent[0]["args"].clone()),
        (json!("mcp"), json!("npx"), json!(["-y", "pkg"]))
    );
    assert_eq!((sent[0]["scope"].clone(), sent[0]["projectPath"].clone()), (json!("project"), json!("/src/app")));
}

#[test]
fn shows_an_install_failure_under_the_grid_and_keeps_the_button() {
    let mut r = catalog_open(|f| {
        f.tool_entries = vec![card("one", json!({}))];
        f.tool_install = Some((409, json!({ "detail": "Tool already exists" }).to_string()));
    });
    r.press("tool-catalog-install-one");
    wait_tools(&mut r, "the failure", |s| s.settings.tools.install_error.is_some());
    r.settle();
    assert_eq!(r.text("tools-install-error"), "Tool already exists");
    assert!(r.exists("tool-catalog-install-one"));
    assert!(!page(&r).installing);
}

#[test]
fn takes_an_install_answer_with_nothing_in_it_for_a_failure() {
    let mut r = catalog_open(|f| {
        f.tool_entries = vec![card("one", json!({}))];
        f.tool_install = Some((200, String::new()));
    });
    r.press("tool-catalog-install-one");
    wait_tools(&mut r, "the failure", |s| s.settings.tools.install_error.is_some());
    assert_eq!(page(&r).install_error.as_deref(), Some("No data returned"));
}

#[test]
fn shows_the_load_error_alone_when_there_is_no_catalog_and_the_empty_state_when_it_is_empty() {
    let broken = catalog_open(|f| f.tool_catalog = Some((500, "Catalog down".into())));
    assert_eq!(broken.text("tools-catalog-error"), "Catalog down");
    assert!(!broken.exists("tools-catalog-empty"));
    let empty = catalog_open(|_| {});
    assert!(empty.text("tools-catalog-empty").contains("No tools in catalog"));
    assert!(empty.text("tools-catalog-empty").contains("The tool catalog is empty or unavailable."));
}

#[test]
fn reads_the_catalog_each_time_the_tab_opens_and_keeps_its_own_target_between_catalog_and_custom() {
    let mut r = catalog_open(|f| f.tool_entries = vec![card("one", json!({}))]);
    assert_eq!(heard(&r, "GET", "/api/tools/catalog"), 1);
    r.rt.update(|s, _| s.choose_tools_target("project:/src/app"));
    r.press("tools-tab-custom");
    assert_eq!(page(&r).target, project("/src/app"), "the choice is kept");
    assert!(page(&r).catalog.is_empty(), "a tab that is left forgets what it read");
    r.press("tools-tab-catalog");
    wait_tools(&mut r, "the second read", |s| !s.settings.tools.catalog_loading && !s.settings.tools.catalog.is_empty());
    assert_eq!(heard(&r, "GET", "/api/tools/catalog"), 2);
    assert_eq!(r.rt.state.settings.skills.target, InstallTarget::Global, "Skills keeps its own");
}

#[test]
fn says_where_a_tool_lands_under_the_picker_and_lists_the_repositories() {
    let mut r = catalog_open(|f| f.repositories = vec![json!({ "path": "/src/app", "name": "app" })]);
    assert_eq!(r.text("tools-target-line"), "Every session \u{b7} ~/.config/opencode/tools/");
    r.press("tools-target");
    assert!(r.exists("tools-target-list-opt0") && r.exists("tools-target-list-opt1"));
    assert_eq!(r.text("tools-target-list-opt1"), "app");
    r.press("tools-target-list-opt1");
    r.settle();
    assert_eq!(page(&r).target, project("/src/app"));
    assert_eq!(r.text("tools-target-line"), "Sessions in this repository \u{b7} /src/app/.opencode/tools/");
}

// ─── with no Fleet to ask ───────────────────────────────────────────────────────

#[test]
fn draws_the_invented_tools_with_no_server_and_the_buttons_change_them() {
    let mut rt = app();
    rt.update(|s, _| {
        s.rail = Rail::Settings;
        s.open_settings("skills", Some("tools"));
    });
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    assert!(rt.state.settings.tools.tools.iter().any(|t| t.name == "csv-peek"));
    click(&mut rt, "tool-remove-harbor-probe|project:/work/lantern");
    click(&mut rt, "tool-remove-harbor-probe|project:/work/lantern");
    assert!(!rt.state.settings.tools.tools.iter().any(|t| t.name == "harbor-probe"));
    click(&mut rt, "tools-tab-catalog");
    rt.frame(FRAME);
    click(&mut rt, "tool-catalog-install-clock");
    assert!(rt.state.settings.tools.tools.iter().any(|t| t.name == "clock" && t.target == InstallTarget::Global));
}
