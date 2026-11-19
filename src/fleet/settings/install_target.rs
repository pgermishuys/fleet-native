//! Where a skill or tool is installed (`lib/install-target.ts`, `InstallTargetPicker.vue`'s two lines): Global goes into the
//! user's OpenCode config and every session sees it; a repository's goes into that repository's `.opencode` folder and only
//! its sessions see it. Plain data: Skills and Tools, Catalog and Custom, all share it.
use crate::conv::export_history::encode_component;
use serde_json::{Value, json};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum InstallTarget {
    #[default]
    Global,
    Project(String),
}

/// What an install writes, so the picker can say where (`kind` of the picker).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Skills,
    Tools,
    /// An MCP server: a block in `opencode.json`.
    Config,
}

/// `toInstallTarget`: anything but a project with a path is global.
pub fn to_install_target(scope: Option<&str>, project_path: Option<&str>) -> InstallTarget {
    match (scope, project_path) {
        (Some("project"), Some(path)) if !path.is_empty() => InstallTarget::Project(path.to_string()),
        _ => InstallTarget::Global,
    }
}

impl InstallTarget {
    /// `isSameTarget`: global equals global, repositories by path.
    pub fn is_same(&self, other: &InstallTarget) -> bool {
        self == other
    }

    /// `targetKey`: a stable string for keys and select values.
    pub fn key(&self) -> String {
        match self {
            InstallTarget::Global => "global".into(),
            InstallTarget::Project(path) => format!("project:{path}"),
        }
    }

    /// `targetFromKey`.
    pub fn from_key(key: &str) -> InstallTarget {
        key.strip_prefix("project:").map_or(InstallTarget::Global, |path| InstallTarget::Project(path.to_string()))
    }

    /// `targetBody`: the fields an install request carries.
    pub fn body(&self) -> Value {
        match self {
            InstallTarget::Global => json!({ "scope": "global", "projectPath": null }),
            InstallTarget::Project(path) => json!({ "scope": "project", "projectPath": path }),
        }
    }

    /// `targetQuery`: the query that names an existing install on update and remove calls, already encoded.
    pub fn query(&self) -> String {
        match self {
            InstallTarget::Global => "scope=global".into(),
            InstallTarget::Project(path) => format!("scope=project&projectPath={}", encode_component(path)),
        }
    }

    /// `targetLabel`: "Global", or the repository's folder name, whichever way its path is written.
    pub fn label(&self) -> String {
        match self {
            InstallTarget::Global => "Global".into(),
            InstallTarget::Project(path) => path.split(['/', '\\']).rfind(|p| !p.is_empty()).unwrap_or(path).to_string(),
        }
    }

    /// The badge's tooltip: the repository's path, or who sees a global install.
    pub fn tooltip(&self) -> String {
        match self {
            InstallTarget::Global => "Every session".into(),
            InstallTarget::Project(path) => path.clone(),
        }
    }

    /// "Every session" or "Sessions in this repository" (the line under the picker, before the dot).
    pub fn audience(&self) -> &'static str {
        match self {
            InstallTarget::Global => "Every session",
            InstallTarget::Project(_) => "Sessions in this repository",
        }
    }

    /// Where the install lands, as the picker's line says it (a project's MCP servers sit in the `opencode.json` at the
    /// repository's root).
    pub fn destination(&self, kind: Kind) -> String {
        match (self, kind) {
            (InstallTarget::Global, Kind::Config) => "~/.config/opencode/opencode.json".into(),
            (InstallTarget::Global, Kind::Skills) => "~/.config/opencode/skills/".into(),
            (InstallTarget::Global, Kind::Tools) => "~/.config/opencode/tools/".into(),
            (InstallTarget::Project(path), Kind::Config) => format!("{path}/opencode.json"),
            (InstallTarget::Project(path), Kind::Skills) => format!("{path}/.opencode/skills/"),
            (InstallTarget::Project(path), Kind::Tools) => format!("{path}/.opencode/tools/"),
        }
    }
}
