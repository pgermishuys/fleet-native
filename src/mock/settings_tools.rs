//! Invented tools for Settings → Skills & tools → Tools when the app has no Fleet to ask (and for its scenes and tests).
//! Nothing here is from a real machine.
use super::skills::REPOSITORY;
use crate::fleet::settings::install_target::InstallTarget;
use crate::fleet::settings::tools::{CatalogEntry, InstalledTool, ToolType};

/// A native tool from a local file, an MCP server with a command line, the same server in one repository, then (the scenes
/// that show the cards' other parts add these) a native tool from GitHub with a description, and one that is not anywhere
/// OpenCode looks. The first three are what the scratch Fleet the pictures are compared with has installed.
pub fn installed() -> Vec<InstalledTool> {
    let tool = |name: &str, target, tool_type| InstalledTool {
        name: name.into(),
        target,
        tool_type,
        display_name: None,
        description: None,
        command: None,
        args: Vec::new(),
        repo_url: None,
        local_path: None,
        installed_path: None,
    };
    let mut csv = tool("csv-peek", InstallTarget::Global, ToolType::Native);
    csv.local_path = Some("/work/tools/csv-peek.ts".into());
    csv.installed_path = Some("~/.config/opencode/tools/csv-peek.ts".into());
    let mut files = tool("filesystem", InstallTarget::Global, ToolType::Mcp);
    files.command = Some("node".into());
    files.args = vec!["/path/to/server.js".into(), "--port".into(), "3000".into()];
    files.installed_path = Some("~/.config/opencode/opencode.json".into());
    let mut scoped = tool("filesystem", InstallTarget::Project(REPOSITORY.into()), ToolType::Mcp);
    scoped.command = Some("npx".into());
    scoped.args = vec!["-y".into(), "@example/mcp-files".into()];
    scoped.installed_path = Some(format!("{REPOSITORY}/opencode.json"));
    let mut lint = tool("tidy-lint", InstallTarget::Global, ToolType::Native);
    lint.display_name = Some("Tidy lint".into());
    lint.description = Some("Reports lines the formatter would change.".into());
    lint.repo_url = Some("https://github.com/acme-labs/tidy-lint.git".into());
    lint.installed_path = Some("~/.config/opencode/tools/tidy-lint.ts".into());
    let lost = tool("harbor-probe", InstallTarget::Project(REPOSITORY.into()), ToolType::Native);
    vec![csv, files, scoped, lint, lost]
}

/// The tools the scratch Fleet has: the first three of [`installed`].
pub fn as_scratch() -> Vec<InstalledTool> {
    installed().into_iter().take(3).collect()
}

fn entry(name: &str, tool_type: ToolType, about: &str) -> CatalogEntry {
    CatalogEntry {
        name: name.into(),
        tool_type,
        source: 1,
        display_name: None,
        description: Some(about.into()),
        command: None,
        args: None,
        env: None,
        repo_url: Some(format!("https://github.com/acme-labs/{name}.git")),
        ref_: Some("main".into()),
        sub_path: None,
        local_path: None,
        author: None,
        version: None,
        tags: Vec::new(),
    }
}

/// What the catalog offers with no Fleet: a native tool, an MCP server, one already installed, a long description and tags.
pub fn catalog() -> Vec<CatalogEntry> {
    let mut peek = entry("csv-peek", ToolType::Native, "Prints the first rows of a CSV file as a table.");
    peek.author = Some("acme-labs".into());
    peek.version = Some("0.3.0".into());
    peek.tags = vec!["data".into()];
    let mut files = entry(
        "filesystem",
        ToolType::Mcp,
        "Gives agents read access to the folders you name, with a size limit on every read. Lists directories, reads text files \
         and reports what it skipped, and never writes anything, so it is safe to leave on for every session you start.",
    );
    files.display_name = Some("Filesystem".into());
    files.command = Some("npx".into());
    files.args = Some(vec!["-y".into(), "@example/mcp-files".into()]);
    files.version = Some("1.4.2".into());
    files.tags = vec!["files".into(), "read-only".into()];
    let mut clock = entry("clock", ToolType::Native, "Tells the agent the date and time in any zone.");
    clock.author = Some("Harbor team".into());
    let mut notes = entry("notes-search", ToolType::Mcp, "Searches your notes folder.");
    notes.command = Some("node".into());
    notes.args = Some(vec!["/work/mcp/notes.js".into()]);
    notes.repo_url = None;
    vec![peek, files, clock, notes]
}

/// The install a catalog card makes with no Fleet.
pub fn from_entry(entry: &CatalogEntry, target: InstallTarget) -> InstalledTool {
    let file = match (entry.tool_type, &target) {
        (ToolType::Mcp, InstallTarget::Global) => "~/.config/opencode/opencode.json".to_string(),
        (ToolType::Mcp, InstallTarget::Project(p)) => format!("{p}/opencode.json"),
        (ToolType::Native, InstallTarget::Global) => format!("~/.config/opencode/tools/{}.ts", entry.name),
        (ToolType::Native, InstallTarget::Project(p)) => format!("{p}/.opencode/tools/{}.ts", entry.name),
    };
    InstalledTool {
        name: entry.name.clone(),
        target,
        tool_type: entry.tool_type,
        display_name: entry.display_name.clone(),
        description: entry.description.clone(),
        command: entry.command.clone(),
        args: entry.args.clone().unwrap_or_default(),
        repo_url: entry.repo_url.clone(),
        local_path: entry.local_path.clone(),
        installed_path: Some(file),
    }
}
