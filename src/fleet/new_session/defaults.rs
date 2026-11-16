//! What the New session page remembers between runs (`composables/use-new-session-defaults.ts`; rows A16.1-A16.11): the
//! last folder, five recent folders, the workspace per repository, the agent and model per folder and harness, the root
//! the last new folder went into, and thirty plain folders. One set per machine, because folders are the machine's own.
//! Pure: no Rune, no I/O. The settings file holds it (`settings::Sidebar`), read and written on a thread.
use super::request::{Folder, Workspace};
use crate::composer::choice::AgentModelChoice;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub const MAX_RECENT_FOLDERS: usize = 5;
pub const MAX_PLAIN_FOLDERS: usize = 30;

/// Where Fleet keeps the home machine's defaults (`NEW_SESSION_DEFAULTS_KEY`); another machine's adds `:<key>`.
pub const DEFAULTS_KEY: &str = "weave:new-session:defaults";

/// Where a machine's defaults are kept (`newSessionDefaultsKey`).
pub fn defaults_key(machine_key: &str) -> String {
    if machine_key == crate::fleet::settings::HOME_MACHINE_KEY { DEFAULTS_KEY.to_string() } else { format!("{DEFAULTS_KEY}:{machine_key}") }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewSessionDefaults {
    last_folder: Option<Folder>,
    /// Repositories and folders, most recent first.
    recent_folders: Vec<Folder>,
    /// `current`, `new`, or the path of an existing worktree.
    workspace_by_repository: BTreeMap<String, String>,
    /// By folder (its path, or `none` for a quick chat), then harness.
    choice_by_folder: BTreeMap<String, BTreeMap<String, AgentModelChoice>>,
    last_new_folder_root: Option<String>,
    /// Plain (non-git) folders sessions ran in, most recent first: the repository scan never finds them.
    plain_folders: Vec<String>,
}

fn folder_key(folder: &Folder) -> &str {
    match folder {
        Folder::None => "none",
        Folder::Repository(p) | Folder::Directory(p) => p,
    }
}

fn folder_json(folder: &Folder) -> Value {
    match folder {
        Folder::None => json!({ "kind": "none" }),
        Folder::Repository(path) => json!({ "kind": "repository", "path": path }),
        Folder::Directory(path) => json!({ "kind": "directory", "path": path }),
    }
}

/// A folder as `isFolder` reads it: `none`, or a repository or directory with a path that isn't empty.
fn folder_from(v: &Value) -> Option<Folder> {
    let path = || v.get("path").and_then(Value::as_str).filter(|p| !p.is_empty()).map(str::to_string);
    match v.get("kind").and_then(Value::as_str)? {
        "none" => Some(Folder::None),
        "repository" => path().map(Folder::Repository),
        "directory" => path().map(Folder::Directory),
        _ => None,
    }
}

fn still_exists(folder: &Folder, repositories: &[String]) -> bool {
    match folder {
        Folder::Repository(path) => repositories.contains(path),
        _ => true,
    }
}

fn remembered(workspace: &Workspace) -> String {
    match workspace {
        Workspace::Current => "current".into(),
        Workspace::New => "new".into(),
        Workspace::Existing(path) => path.clone(),
    }
}

impl NewSessionDefaults {
    /// Whatever storage held, made safe to read (`normalize`): entries of the wrong kind are dropped, nothing throws.
    pub fn parse(stored: &Value) -> NewSessionDefaults {
        let Some(root) = stored.as_object() else { return NewSessionDefaults::default() };
        let strings = |v: Option<&Value>| -> Vec<String> {
            v.and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        };
        let workspace_by_repository = root
            .get("workspaceByRepository")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
            .collect();
        let choice_by_folder = root
            .get("choiceByFolder")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(folder, by_harness)| {
                let by_harness = by_harness.as_object()?;
                let choices = by_harness
                    .iter()
                    .filter_map(|(harness, c)| {
                        Some((
                            harness.clone(),
                            AgentModelChoice { agent: c.get("agent")?.as_str()?.into(), model: c.get("model")?.as_str()?.into() },
                        ))
                    })
                    .collect();
                Some((folder.clone(), choices))
            })
            .collect();
        NewSessionDefaults {
            last_folder: root.get("lastFolder").and_then(folder_from),
            recent_folders: root.get("recentFolders").and_then(Value::as_array).into_iter().flatten().filter_map(folder_from).collect(),
            workspace_by_repository,
            choice_by_folder,
            last_new_folder_root: root.get("lastNewFolderRoot").and_then(Value::as_str).map(str::to_string),
            plain_folders: strings(root.get("plainFolders")),
        }
    }

    /// The object Fleet writes: the same field names, `null` for a folder or root never set.
    pub fn to_json(&self) -> Value {
        let choices: Map<String, Value> = self
            .choice_by_folder
            .iter()
            .map(|(folder, by_harness)| {
                let inner: Map<String, Value> =
                    by_harness.iter().map(|(h, c)| (h.clone(), json!({ "agent": c.agent, "model": c.model }))).collect();
                (folder.clone(), Value::Object(inner))
            })
            .collect();
        json!({
            "lastFolder": self.last_folder.as_ref().map(folder_json),
            "recentFolders": self.recent_folders.iter().map(folder_json).collect::<Vec<_>>(),
            "workspaceByRepository": self.workspace_by_repository,
            "choiceByFolder": choices,
            "lastNewFolderRoot": self.last_new_folder_root,
            "plainFolders": self.plain_folders,
        })
    }

    /// The folder to start with: the last one used if it still exists, else the most recent repository still there, else none
    /// (`initialFolder`). `repositories` are the scanned repositories' paths.
    pub fn initial_folder(&self, repositories: &[String]) -> Option<Folder> {
        if let Some(last) = self.last_folder.as_ref().filter(|f| still_exists(f, repositories)) {
            return Some(last.clone());
        }
        self.recent_folders.iter().find(|f| matches!(f, Folder::Repository(_)) && still_exists(f, repositories)).cloned()
    }

    /// Recently used repositories and folders, without repositories that are gone.
    pub fn recent_folders(&self, repositories: &[String]) -> Vec<Folder> {
        self.recent_folders.iter().filter(|f| still_exists(f, repositories)).cloned().collect()
    }

    /// The workspace last used in a repository; a new worktree the first time. A remembered worktree that isn't in `worktrees`
    /// falls back to a new one; pass `None` while worktrees are loading (`workspaceFor`).
    pub fn workspace_for(&self, repository: &str, worktrees: Option<&[String]>) -> Workspace {
        match self.workspace_by_repository.get(repository).map(String::as_str) {
            Some("current") => Workspace::Current,
            None | Some("") | Some("new") => Workspace::New,
            Some(path) if worktrees.is_some_and(|w| !w.iter().any(|x| x == path)) => Workspace::New,
            Some(path) => Workspace::Existing(path.to_string()),
        }
    }

    /// The worktree path last used in a repository, so menus can list it first.
    pub fn last_worktree_for(&self, repository: &str) -> Option<String> {
        self.workspace_by_repository.get(repository).filter(|w| !matches!(w.as_str(), "" | "current" | "new")).cloned()
    }

    /// The agent and model last started with in a folder on a harness; Default for both the first time.
    pub fn choice_for(&self, folder: &Folder, harness_type: &str) -> AgentModelChoice {
        self.choice_by_folder.get(folder_key(folder)).and_then(|h| h.get(harness_type)).cloned().unwrap_or_default()
    }

    pub fn plain_folders(&self) -> &[String] {
        &self.plain_folders
    }

    pub fn last_new_folder_root(&self) -> Option<&str> {
        self.last_new_folder_root.as_deref()
    }

    pub fn remember_new_folder_root(&mut self, root: &str) {
        self.last_new_folder_root = Some(root.to_string());
    }

    /// Records what a session was just created with (`remember`). `choice` is the harness and the agent and model sent, if any.
    pub fn remember(&mut self, folder: &Folder, workspace: &Workspace, choice: Option<(&str, &AgentModelChoice)>) {
        if let Some((harness, c)) = choice {
            self.choice_by_folder.entry(folder_key(folder).to_string()).or_default().insert(harness.to_string(), c.clone());
        }
        if *folder != Folder::None {
            self.recent_folders.retain(|f| f != folder);
            self.recent_folders.insert(0, folder.clone());
            self.recent_folders.truncate(MAX_RECENT_FOLDERS);
        }
        if let Folder::Repository(path) = folder {
            self.workspace_by_repository.insert(path.clone(), remembered(workspace));
        }
        if let Folder::Directory(path) = folder {
            self.plain_folders.retain(|p| p != path);
            self.plain_folders.insert(0, path.clone());
            self.plain_folders.truncate(MAX_PLAIN_FOLDERS);
        }
        self.last_folder = Some(folder.clone());
    }
}
