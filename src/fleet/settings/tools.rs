//! Settings → Skills & tools → Tools, as plain data (`composables/use-tools.ts`, `use-tool-catalog.ts`, `ToolsSection.vue`,
//! `tools/InstalledToolsTab.vue`, `tools/CatalogTab.vue`): the installed tools and the catalog, how the answers are read, the
//! calls under `/api/tools`, the bodies an install sends, and the page's own state. No Rune and no I/O here; what touches the
//! app is `tools_actions.rs`. The Custom tab is piece AJ.
use super::install_target::{InstallTarget, to_install_target};
use super::skills::{Inner, Request};
use crate::conv::export_history::encode_component;
use serde_json::{Map, Value, json};

/// `mapToolType`: "native" or "mcp" in any case; anything else is native.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolType {
    #[default]
    Native,
    Mcp,
}

impl ToolType {
    pub fn parse(s: &str) -> ToolType {
        if s.eq_ignore_ascii_case("mcp") { ToolType::Mcp } else { ToolType::Native }
    }
    /// What the badge says.
    pub fn label(self) -> &'static str {
        match self {
            ToolType::Native => "Native",
            ToolType::Mcp => "MCP",
        }
    }
    /// What a request carries.
    pub fn word(self) -> &'static str {
        match self {
            ToolType::Native => "native",
            ToolType::Mcp => "mcp",
        }
    }
}

/// One install of a tool (`InstalledTool`): the same tool can be global and in any number of repositories.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledTool {
    pub name: String,
    pub target: InstallTarget,
    pub tool_type: ToolType,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub repo_url: Option<String>,
    pub local_path: Option<String>,
    /// The tool file, or the `opencode.json` holding an MCP server. None when it isn't anywhere OpenCode looks.
    pub installed_path: Option<String>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

impl InstalledTool {
    /// The card's key (`name|targetKey`): busy and confirm states are per install.
    pub fn key(&self) -> String {
        format!("{}|{}", self.name, self.target.key())
    }

    /// The card's title: `displayName || name`.
    pub fn title(&self) -> &str {
        self.display_name.as_deref().filter(|d| !d.is_empty()).unwrap_or(&self.name)
    }

    /// `formatCommand`: the command and its arguments joined by spaces; none without a command.
    pub fn command_line(&self) -> String {
        match self.command.as_deref().filter(|c| !c.is_empty()) {
            None => String::new(),
            Some(c) if self.args.is_empty() => c.to_string(),
            Some(c) => format!("{c} {}", self.args.join(" ")),
        }
    }

    /// The mono line under the description: an MCP server's command, a native tool's local path or address.
    pub fn source_line(&self) -> Option<String> {
        match self.tool_type {
            ToolType::Mcp => Some(self.command_line()).filter(|c| !c.is_empty()).map(|c| format!("command: {c}")),
            ToolType::Native => [&self.local_path, &self.repo_url].into_iter().flatten().find(|p| !p.is_empty()).cloned(),
        }
    }
}

/// `GET /api/tools`: `{tools: [...]}`. A body with nothing in it is "No data returned"; a tool with no name is dropped.
pub fn parse_tools(body: &Value) -> Result<Vec<InstalledTool>, String> {
    if body.is_null() {
        return Err("No data returned".into());
    }
    let list = body.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|t| {
            Some(InstalledTool {
                name: text(t, "name").filter(|n| !n.is_empty())?,
                target: to_install_target(text(t, "scope").as_deref(), text(t, "projectPath").as_deref()),
                tool_type: ToolType::parse(&text(t, "toolType").unwrap_or_default()),
                display_name: text(t, "displayName"),
                description: text(t, "description"),
                command: text(t, "command"),
                args: strings(t, "args"),
                repo_url: text(t, "repoUrl"),
                local_path: text(t, "localPath"),
                installed_path: text(t, "installedPath"),
            })
        })
        .collect())
}

/// One tool the catalog offers (`ToolCatalogEntry`). `isStale` and `cachedAt` are read by Fleet and shown nowhere, so they
/// aren't kept. `source` is the number the API sends (1, GitHub, when it sends none) and goes back unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: String,
    pub tool_type: ToolType,
    pub source: i64,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<Map<String, Value>>,
    pub repo_url: Option<String>,
    pub ref_: Option<String>,
    pub sub_path: Option<String>,
    pub local_path: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
}

impl CatalogEntry {
    pub fn title(&self) -> &str {
        self.display_name.as_deref().filter(|d| !d.is_empty()).unwrap_or(&self.name)
    }

    /// `POST /api/tools/install`'s body for this card (`handleInstall`).
    pub fn install_body(&self, target: &InstallTarget) -> Value {
        let mut body = json!({
            "name": self.name,
            "toolType": self.tool_type.word(),
            "source": self.source,
            "command": self.command,
            "args": self.args,
            "env": self.env,
            "repoUrl": self.repo_url,
            "ref": self.ref_,
            "subPath": self.sub_path,
            "localPath": self.local_path,
        });
        if let (Some(into), Value::Object(more)) = (body.as_object_mut(), target.body()) {
            into.extend(more);
        }
        body
    }
}

/// `GET /api/tools/catalog`: `{entries: [...]}`. A body with nothing in it is "No data returned"; an entry with no name is dropped.
pub fn parse_catalog(body: &Value) -> Result<Vec<CatalogEntry>, String> {
    if body.is_null() {
        return Err("No data returned".into());
    }
    let list = body.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|e| {
            Some(CatalogEntry {
                name: text(e, "name").filter(|n| !n.is_empty())?,
                tool_type: ToolType::parse(&text(e, "toolType").unwrap_or_default()),
                source: e.get("source").and_then(Value::as_i64).unwrap_or(1),
                display_name: text(e, "displayName"),
                description: text(e, "description"),
                command: text(e, "command"),
                args: e.get("args").and_then(Value::as_array).map(|_| strings(e, "args")),
                env: e.get("env").and_then(Value::as_object).cloned(),
                repo_url: text(e, "repoUrl"),
                ref_: text(e, "ref"),
                sub_path: text(e, "subPath"),
                local_path: text(e, "localPath"),
                author: text(e, "author"),
                version: text(e, "version"),
                tags: strings(e, "tags"),
            })
        })
        .collect())
}

// ─── Requests ───────────────────────────────────────────────────────────────

pub fn list() -> Request {
    Request { method: "GET", path: "/api/tools".into(), fallback: "Failed to fetch tools" }
}
pub fn remove(name: &str, target: &InstallTarget) -> Request {
    Request {
        method: "DELETE",
        path: format!("/api/tools/{}?{}", encode_component(name), target.query()),
        fallback: "Failed to remove tool",
    }
}
pub fn catalog() -> Request {
    Request { method: "GET", path: "/api/tools/catalog".into(), fallback: "Failed to fetch tool catalog" }
}
pub fn install() -> Request {
    Request { method: "POST", path: "/api/tools/install".into(), fallback: "Failed to install tool" }
}

/// Whether `name` is installed at `target` (`isInstalled`): installed somewhere else doesn't count.
pub fn is_installed(installed: &[InstalledTool], name: &str, target: &InstallTarget) -> bool {
    installed.iter().any(|t| t.name == name && t.target.is_same(target))
}

// ─── The page ───────────────────────────────────────────────────────────────

/// What the Tools tab keeps while it is the one showing: the list, the install target Catalog and Custom share (separate from
/// Skills'), and the busy and confirm states (per install, as `InstalledToolsTab.vue` keeps them). Leaving the tab forgets it
/// all but the target, which `ToolsSection.vue` holds above its inner tabs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolsPage {
    /// Whether the tab has been opened and its first reads sent; `for_seq` is the Skills page read it was opened under.
    pub active: bool,
    pub for_seq: u64,
    pub inner: Inner,
    pub tools: Vec<InstalledTool>,
    pub loading: bool,
    pub error: Option<String>,
    /// Numbers the list's reads so an answer to one the tab has left, or one a newer read overtook, is dropped.
    pub reads: u64,
    pub target: InstallTarget,
    /// An install from Catalog or Custom is under way: the picker is disabled while it is.
    pub installing: bool,
    pub repositories: Vec<(String, String)>,
    pub repos_loading: bool,
    pub removing: Option<String>,
    pub confirm_remove: Option<String>,
    pub catalog: Vec<CatalogEntry>,
    pub catalog_loading: bool,
    pub catalog_error: Option<String>,
    pub catalog_reads: u64,
    pub installing_name: Option<String>,
    pub install_error: Option<String>,
}
