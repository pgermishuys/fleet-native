//! Settings → Skills & tools → Tools → Custom, as plain data (`tools/CustomInstallTab.vue`): what a typed address or an MCP
//! server's fields turn into, and the words the form says when they don't. No Rune and no I/O here; the buttons are in
//! `tools_actions.rs`. Both forms build a [`CatalogEntry`], so the body they send is the one a catalog card sends.
use super::skills::parse_github_url;
use super::tools::{CatalogEntry, ToolType};
use serde_json::{Map, Value};

/// The two forms (`InstallMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Native,
    Mcp,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Native, Mode::Mcp];
    pub fn id(self) -> &'static str {
        match self {
            Mode::Native => "native",
            Mode::Mcp => "mcp",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Native => "Native",
            Mode::Mcp => "MCP",
        }
    }
}

pub const URL_REQUIRED: &str = "Tool URL or path is required.";
pub const NOT_GITHUB_TOOL: &str = "Use a GitHub link to a repository, a folder, or a .ts/.js file.";
pub const NAME_REQUIRED: &str = "Name is required.";
pub const COMMAND_REQUIRED: &str = "Command is required.";

/// `TOOL_FILE`: `.ts` or `.js`, in any case.
fn is_tool_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".ts") || lower.ends_with(".js")
}

/// `toolNameFromPath`: OpenCode names a tool after its file, so `sample-tool.ts` installs the tool "sample-tool".
pub fn tool_name_from_path(path: &str) -> String {
    let last = path.split(['/', '\\']).rfind(|p| !p.is_empty()).unwrap_or("tool");
    if is_tool_file(last) { last[..last.len() - 3].to_string() } else { last.to_string() }
}

/// What a GitHub link to a tool says (`parseGitHubToolUrl`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubTool {
    pub repo_url: String,
    pub ref_: Option<String>,
    pub sub_path: Option<String>,
    pub name: String,
}

/// A repository, a folder (`/tree/…`) or one file (`/blob/…/sample-tool.ts`). For a file, the folder is fetched and the tool is
/// named after the file. None when the link isn't one of those.
pub fn parse_github_tool_url(url: &str) -> Option<GitHubTool> {
    let parsed = parse_github_url(url)?;
    if let Some(file) = parsed.sub_path.as_deref().filter(|p| is_tool_file(p)) {
        let folder = file.rsplit_once('/').map(|(f, _)| f).filter(|f| !f.is_empty());
        return Some(GitHubTool {
            repo_url: parsed.repo_url.clone(),
            ref_: parsed.ref_.clone(),
            sub_path: folder.map(str::to_string),
            name: tool_name_from_path(file),
        });
    }
    Some(GitHubTool { repo_url: parsed.repo_url, ref_: parsed.ref_, sub_path: parsed.sub_path, name: parsed.name })
}

/// `isGitHubUrl`: the Native form's own check, `startsWith`, so `https://www.github.com/…` is a local path here (Fleet's quirk;
/// the Skills form uses `includes`).
fn is_github(url: &str) -> bool {
    url.starts_with("https://github.com/") || url.starts_with("http://github.com/")
}

fn entry(name: String, tool_type: ToolType, source: i64) -> CatalogEntry {
    CatalogEntry {
        name,
        tool_type,
        source,
        display_name: None,
        description: None,
        command: None,
        args: None,
        env: None,
        repo_url: None,
        ref_: None,
        sub_path: None,
        local_path: None,
        author: None,
        version: None,
        tags: vec![],
    }
}

/// `submitNativeInstall`, up to the request: what the typed address installs, or the words the form shows.
pub fn native_install(typed: &str) -> Result<CatalogEntry, &'static str> {
    let url = typed.trim();
    if url.is_empty() {
        return Err(URL_REQUIRED);
    }
    if !is_github(url) {
        let mut e = entry(tool_name_from_path(url), ToolType::Native, 2);
        e.local_path = Some(url.to_string());
        return Ok(e);
    }
    let github = parse_github_tool_url(url).ok_or(NOT_GITHUB_TOOL)?;
    let mut e = entry(github.name, ToolType::Native, 1);
    (e.repo_url, e.ref_, e.sub_path) = (Some(github.repo_url), github.ref_, github.sub_path);
    Ok(e)
}

/// `parseArgs`: one per line, trimmed, blanks dropped.
pub fn parse_args(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// `parseEnv`: `KEY=VALUE` per line, split at the first `=`; a line with no `=`, or starting with it, is dropped; key and value
/// are trimmed.
pub fn parse_env(text: &str) -> Map<String, Value> {
    let mut env = Map::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(at) = line.find('=').filter(|&i| i > 0) else { continue };
        let key = line[..at].trim();
        if !key.is_empty() {
            env.insert(key.to_string(), Value::String(line[at + 1..].trim().to_string()));
        }
    }
    env
}

/// `submitMcpInstall`, up to the request: the name is checked first, then the command. Arguments and variables are null when empty.
pub fn mcp_install(name: &str, command: &str, args: &str, env: &str) -> Result<CatalogEntry, &'static str> {
    let (name, command) = (name.trim(), command.trim());
    if name.is_empty() {
        return Err(NAME_REQUIRED);
    }
    if command.is_empty() {
        return Err(COMMAND_REQUIRED);
    }
    let mut e = entry(name.to_string(), ToolType::Mcp, 2);
    e.command = Some(command.to_string());
    let args = parse_args(args);
    e.args = (!args.is_empty()).then_some(args);
    let env = parse_env(env);
    e.env = (!env.is_empty()).then_some(env);
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::settings::install_target::InstallTarget;
    use serde_json::json;

    #[test]
    fn names_a_tool_after_its_file() {
        assert_eq!(tool_name_from_path("/work/tools/sample-tool.ts"), "sample-tool");
        assert_eq!(tool_name_from_path("C:\\work\\tools\\Sample.JS"), "Sample");
        assert_eq!(tool_name_from_path("/work/tools/my-tool/"), "my-tool");
        assert_eq!(tool_name_from_path("/work/tools/notes.txt"), "notes.txt");
        assert_eq!(tool_name_from_path(""), "tool");
    }

    #[test]
    fn reads_a_github_link_to_a_file_as_its_folder() {
        let t = parse_github_tool_url("https://github.com/example/kit/blob/main/tools/csv-peek.ts").expect("a file link");
        assert_eq!(
            t,
            GitHubTool {
                repo_url: "https://github.com/example/kit.git".into(),
                ref_: Some("main".into()),
                sub_path: Some("tools".into()),
                name: "csv-peek".into()
            }
        );
        let top = parse_github_tool_url("https://github.com/example/kit/blob/main/csv-peek.js").expect("a file at the top");
        assert_eq!((top.sub_path, top.name.as_str()), (None, "csv-peek"));
        let folder = parse_github_tool_url("https://github.com/example/kit/tree/dev/tools/csv-peek").expect("a folder");
        assert_eq!((folder.sub_path.as_deref(), folder.name.as_str()), (Some("tools/csv-peek"), "csv-peek"));
        let repo = parse_github_tool_url("https://github.com/example/csv-peek").expect("a repository");
        assert_eq!((repo.ref_, repo.sub_path, repo.name.as_str()), (None, None, "csv-peek"));
        assert!(parse_github_tool_url("https://github.com/example").is_none());
    }

    #[test]
    fn native_needs_an_address_and_says_so() {
        assert_eq!(native_install("  ").unwrap_err(), "Tool URL or path is required.");
        assert_eq!(
            native_install("https://github.com/example").unwrap_err(),
            "Use a GitHub link to a repository, a folder, or a .ts/.js file."
        );
    }

    #[test]
    fn native_builds_the_github_and_the_local_body() {
        let target = InstallTarget::Project("/src/app".into());
        let github = native_install(" https://github.com/example/kit/blob/main/tools/csv-peek.ts ").expect("a link");
        assert_eq!(
            github.install_body(&target),
            json!({
                "name": "csv-peek", "toolType": "native", "source": 1, "command": null, "args": null, "env": null,
                "repoUrl": "https://github.com/example/kit.git", "ref": "main", "subPath": "tools", "localPath": null,
                "scope": "project", "projectPath": "/src/app",
            })
        );
        let local = native_install("/work/tools/csv-peek.ts").expect("a path");
        assert_eq!(
            local.install_body(&InstallTarget::Global),
            json!({
                "name": "csv-peek", "toolType": "native", "source": 2, "command": null, "args": null, "env": null,
                "repoUrl": null, "ref": null, "subPath": null, "localPath": "/work/tools/csv-peek.ts",
                "scope": "global", "projectPath": null,
            })
        );
    }

    #[test]
    fn a_www_link_is_a_local_path_for_a_tool() {
        let e = native_install("https://www.github.com/example/kit").expect("taken as a path");
        assert_eq!((e.source, e.local_path.as_deref(), e.name.as_str()), (2, Some("https://www.github.com/example/kit"), "kit"));
    }

    #[test]
    fn splits_arguments_by_line() {
        assert_eq!(parse_args("/path/to/server.js\n  --port \n\n3000\r\n"), ["/path/to/server.js", "--port", "3000"]);
        assert!(parse_args(" \n\n").is_empty());
    }

    #[test]
    fn splits_variables_at_the_first_equals() {
        let env = parse_env("API_KEY = abc=def \nDEBUG=true\nnoequals\n=novalue\n  \nEMPTY=");
        assert_eq!(Value::Object(env), json!({ "API_KEY": "abc=def", "DEBUG": "true", "EMPTY": "" }));
    }

    #[test]
    fn mcp_checks_the_name_first() {
        assert_eq!(mcp_install("", "", "", "").unwrap_err(), "Name is required.");
        assert_eq!(mcp_install("fs", " ", "", "").unwrap_err(), "Command is required.");
    }

    #[test]
    fn mcp_builds_its_body() {
        let e = mcp_install(" fs ", " node ", "/path/to/server.js\n--port\n3000", "DEBUG=true").expect("fields");
        assert_eq!(
            e.install_body(&InstallTarget::Global),
            json!({
                "name": "fs", "toolType": "mcp", "source": 2, "command": "node", "args": ["/path/to/server.js", "--port", "3000"],
                "env": { "DEBUG": "true" }, "repoUrl": null, "ref": null, "subPath": null, "localPath": null,
                "scope": "global", "projectPath": null,
            })
        );
        let bare = mcp_install("fs", "node", "\n", "junk").expect("no arguments, no variables");
        let body = bare.install_body(&InstallTarget::Global);
        assert!(body["args"].is_null() && body["env"].is_null());
    }
}
