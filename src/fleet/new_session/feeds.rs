//! What the New session page reads from a machine, as plain data (component 7, piece C): each call's answer parsed from the
//! status and body the server sent (`use-repositories.ts`, `use-repository-detail.ts`, `use-worktrees.ts`,
//! `lib/folder-access.ts`, `stores/harness-profiles.ts`, `stores/worktree-naming.ts`). No Rune and no I/O here; the threads
//! that make the calls are `feed_calls.rs`, and what is kept is `feed_state.rs`.
//!
//! Errors: Fleet's `String(apiError)` would print `[object Object]` for a refusal; here the server's own `error` text is
//! said, else `HTTP <status>` (Fleet issue 12 in the checklist).
use super::naming::Templates;
use crate::fleet::machines::read_error;
use serde_json::Value;

/// The refusal's words: the body's `error` (or `message`), else `HTTP <status>`.
pub fn api_error(status: u16, body: &str) -> String {
    read_error(body, format!("HTTP {status}"))
}

fn ok(status: u16) -> bool {
    (200..300).contains(&status)
}

fn json(body: &str) -> Option<Value> {
    serde_json::from_str(body).ok()
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A call's answer: the status and the body, or why nothing answered.
pub type Answer = Result<(u16, String), String>;

/// The body of a good answer, or the words of what went wrong: `fallback` for a transport failure is the error itself.
fn body_of(answer: Answer) -> Result<Value, String> {
    let (status, body) = answer?;
    if !ok(status) {
        return Err(api_error(status, &body));
    }
    json(&body).filter(|v| !v.is_null()).ok_or_else(|| "No data returned".to_string())
}

// ─── Repositories ───────────────────────────────────────────────────────────

/// One repository the machine found (`ScannedRepository`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    pub name: String,
    pub path: String,
    /// The location it was found under.
    pub parent_root: String,
}

/// `GET /api/repositories` and `POST /api/repositories/refresh`: `{repositories, scannedAt}`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub repositories: Vec<Repository>,
    pub scanned_at: Option<i64>,
}

impl Scan {
    pub fn from_json(v: &Value) -> Scan {
        let repositories = v
            .get("repositories")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some(Repository { name: text(r, "name")?, path: text(r, "path")?, parent_root: text(r, "parentRoot").unwrap_or_default() })
            })
            .collect();
        Scan { repositories, scanned_at: v.get("scannedAt").and_then(Value::as_f64).map(|n| n as i64) }
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "repositories": self.repositories.iter().map(|r| serde_json::json!({ "name": r.name, "path": r.path, "parentRoot": r.parent_root })).collect::<Vec<_>>(),
            "scannedAt": self.scanned_at,
        })
    }
}

pub fn parse_scan(answer: Answer) -> Result<Scan, String> {
    body_of(answer).map(|v| Scan::from_json(&v))
}

/// `groupByRoot`: the repositories under each location, the locations in the order first met, each group by name.
pub fn group_by_root(repositories: &[Repository]) -> Vec<(String, Vec<Repository>)> {
    let mut groups: Vec<(String, Vec<Repository>)> = Vec::new();
    for r in repositories {
        match groups.iter_mut().find(|(root, _)| *root == r.parent_root) {
            Some((_, group)) => group.push(r.clone()),
            None => groups.push((r.parent_root.clone(), vec![r.clone()])),
        }
    }
    for (_, group) in &mut groups {
        // `localeCompare`: case-blind first, so "api" sits beside "API".
        group.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
    }
    groups
}

/// One branch of a repository (`RepositoryBranch`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub is_current: bool,
    pub is_remote: bool,
}

/// A remote and, when it is on GitHub, whose repository it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub url: String,
    pub github: Option<(String, String)>,
}

/// `GET /api/repositories/detail`: the repository's branches and remotes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    pub name: String,
    pub path: String,
    /// The branch checked out.
    pub branch: String,
    /// Where a new worktree starts unless told otherwise.
    pub default_base: String,
    pub default_branch: String,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
}

pub fn parse_detail(answer: Answer) -> Result<Detail, String> {
    let body = body_of(answer)?;
    let r = body.get("repository").ok_or("No data returned")?;
    let flag = |b: &Value, k: &str| b.get(k).and_then(Value::as_bool) == Some(true);
    let get = |b: &Value, k: &str| text(b, k).unwrap_or_default();
    Ok(Detail {
        name: get(r, "name"),
        path: get(r, "path"),
        branch: get(r, "branch"),
        default_base: get(r, "defaultBase"),
        default_branch: get(r, "defaultBranch"),
        branches: r
            .get("branches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|b| b.get("name").is_some_and(Value::is_string))
            .map(|b| Branch {
                name: get(b, "name"),
                short_hash: get(b, "shortHash"),
                message: get(b, "message"),
                author: get(b, "author"),
                date: get(b, "date"),
                is_current: flag(b, "isCurrent"),
                is_remote: flag(b, "isRemote"),
            })
            .collect(),
        remotes: r
            .get("remotes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|m| Remote {
                name: get(m, "name"),
                url: get(m, "url"),
                github: m.get("github").and_then(|g| Some((text(g, "owner")?, text(g, "repo")?))),
            })
            .collect(),
    })
}

/// One worktree of a repository (`WorktreeInfo`); a detached one has no branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: String,
    pub branch: Option<String>,
    pub commit_hash: String,
}

pub fn parse_worktrees(answer: Answer) -> Result<Vec<Worktree>, String> {
    let body = body_of(answer)?;
    let list = body.get("worktrees").and_then(Value::as_array).ok_or("No data returned")?;
    Ok(list
        .iter()
        .filter_map(|w| {
            Some(Worktree { path: text(w, "path")?, branch: text(w, "branch"), commit_hash: text(w, "commitHash").unwrap_or_default() })
        })
        .collect())
}

// ─── Folders ────────────────────────────────────────────────────────────────

/// `GET /api/workspace-roots`: the locations that exist on disk, where new folders can go. Any failure is the words Fleet
/// gives, and the page shows no locations then.
pub fn parse_roots(answer: Answer) -> Result<Vec<String>, String> {
    const FAILED: &str = "Couldn't load your locations.";
    let (status, body) = answer.map_err(|_| FAILED.to_string())?;
    let v = json(&body).filter(|_| ok(status)).ok_or(FAILED)?;
    let roots = v.get("roots").and_then(Value::as_array).ok_or(FAILED)?;
    Ok(roots.iter().filter(|r| r.get("exists").and_then(Value::as_bool) == Some(true)).filter_map(|r| text(r, "path")).collect())
}

/// What a new folder gets on a machine unless told otherwise, and the home folder `~` stands for there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryDefaults {
    pub first_branch: String,
    pub home: Option<String>,
}

impl Default for DirectoryDefaults {
    /// What an unanswered or refused call leaves: `main`, home unknown (the display falls back to `tildePath`).
    fn default() -> Self {
        DirectoryDefaults { first_branch: "main".into(), home: None }
    }
}

pub fn parse_defaults(answer: Answer) -> DirectoryDefaults {
    let Ok(v) = body_of(answer) else { return DirectoryDefaults::default() };
    DirectoryDefaults { first_branch: text(&v, "firstBranch").unwrap_or_else(|| "main".into()), home: text(&v, "home") }
}

/// Whether a folder exists, is a git repository, and is inside the locations (`FolderInspection`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub path: String,
    pub exists: bool,
    pub is_git_repo: bool,
    pub is_within_roots: bool,
}

pub fn parse_inspection(answer: Answer) -> Result<Inspection, String> {
    const FAILED: &str = "Couldn't check that folder.";
    let (status, body) = answer?;
    let Some(v) = json(&body).filter(|v| !v.is_null()) else {
        return Err(if ok(status) { FAILED.into() } else { api_error(status, &body) });
    };
    if !ok(status) {
        return Err(read_error(&body, FAILED.into()));
    }
    let flag = |k: &str| v.get(k).and_then(Value::as_bool) == Some(true);
    Ok(Inspection {
        path: text(&v, "path").unwrap_or_default(),
        exists: flag("exists"),
        is_git_repo: flag("isGitRepo"),
        is_within_roots: flag("isWithinRoots"),
    })
}

/// `POST /api/workspace-roots`: nothing comes back; a refusal says why (`addFolderToFleet`).
pub fn parse_added_root(answer: Answer) -> Result<(), String> {
    let (status, body) = answer?;
    if ok(status) { Ok(()) } else { Err(read_error(&body, "Couldn't add that folder.".into())) }
}

/// One entry of a folder listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_git_repo: bool,
}

/// One folder as the folder box lists it: what is in it, or the nearest folder above it that is there (`FolderListing`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    /// The folder, as the machine writes it (`~` expanded).
    pub path: String,
    pub exists: bool,
    pub nearest_existing: Option<String>,
    pub entries: Vec<Entry>,
}

/// `listFolder`: any failure is "Couldn't list that folder." whatever the server said. `asked` stands in for a missing
/// `currentPath`.
pub fn parse_listing(asked: &str, answer: Answer) -> Result<Listing, String> {
    let v = body_of(answer).map_err(|_| "Couldn't list that folder.".to_string())?;
    Ok(Listing {
        path: text(&v, "currentPath").unwrap_or_else(|| asked.to_string()),
        exists: v.get("exists").and_then(Value::as_bool) == Some(true),
        nearest_existing: text(&v, "nearestExisting"),
        entries: v
            .get("entries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some(Entry {
                    name: text(e, "name")?,
                    path: text(e, "path")?,
                    is_git_repo: e.get("isGitRepo").and_then(Value::as_bool) == Some(true),
                })
            })
            .collect(),
    })
}

/// A folder Fleet just made (`NewFolder`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewFolder {
    pub path: String,
    pub is_git_repo: bool,
    /// It was outside the locations, so Fleet added it.
    pub added_to_fleet: bool,
    /// Something to tell the person, such as a missing first commit.
    pub warning: Option<String>,
}

/// Why a create failed: the server's words, and the folder that was already there when it answered 409 (`FolderExistsError`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakeFailure {
    pub message: String,
    pub existing: Option<String>,
}

/// `POST /api/directories` for `path`: the folder, or why not. A refusal says why in the body's `error`.
pub fn parse_created(path: &str, answer: Answer) -> Result<NewFolder, MakeFailure> {
    const FAILED: &str = "Couldn't create that folder.";
    let fail = |status: Option<u16>, message: String| MakeFailure { message, existing: (status == Some(409)).then(|| path.to_string()) };
    let (status, body) = answer.map_err(|_| fail(None, FAILED.into()))?;
    if !ok(status) {
        return Err(fail(Some(status), read_error(&body, FAILED.into())));
    }
    let v = json(&body).filter(|v| !v.is_null()).ok_or_else(|| fail(Some(status), FAILED.into()))?;
    Ok(folder_from_value(&v))
}

/// A `NewFolder` as the server writes it (a create's answer, and a clone's `done` line).
pub fn folder_from_value(v: &Value) -> NewFolder {
    NewFolder {
        path: text(v, "path").unwrap_or_default(),
        is_git_repo: v.get("isGitRepo").and_then(Value::as_bool) == Some(true),
        added_to_fleet: v.get("addedToFleet").and_then(Value::as_bool) == Some(true),
        warning: text(v, "warning").filter(|w| !w.is_empty()),
    }
}

// ─── Harnesses ──────────────────────────────────────────────────────────────

/// `GET /api/harnesses`: the entries, or the words of a failure (the body's `error`, else `HTTP n`; a call that never
/// answered says "Failed to fetch harnesses").
pub fn parse_harnesses(answer: Answer) -> Result<Vec<super::harness_choice::HarnessEntry>, String> {
    let (status, body) = answer.map_err(|_| "Failed to fetch harnesses".to_string())?;
    if !ok(status) {
        return Err(api_error(status, &body));
    }
    let v = json(&body).ok_or("Failed to fetch harnesses")?;
    Ok(super::harness_choice::parse_harness_entries(&v))
}

/// One profile of a harness (`HarnessProfile`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// The profile's settings, as the text the user wrote.
    pub content: String,
    pub is_default: bool,
    /// How many open sessions started with it (`openSessions`); they keep the version they started with.
    pub open_sessions: u32,
}

impl Profile {
    /// One profile as the machine words it (`HarnessProfile`); none without an id.
    pub fn parse(p: &Value) -> Option<Profile> {
        Some(Profile {
            id: text(p, "id")?,
            name: text(p, "name").unwrap_or_default(),
            content: text(p, "content").unwrap_or_default(),
            is_default: p.get("isDefault").and_then(Value::as_bool) == Some(true),
            open_sessions: p.get("openSessions").and_then(Value::as_u64).unwrap_or(0) as u32,
        })
    }
}

/// `GET /api/harnesses/{type}/profiles`: a list, or `None` when the machine couldn't say (the list stays as it was).
pub fn parse_profiles(answer: Answer) -> Option<Vec<Profile>> {
    let v = body_of(answer).ok()?;
    Some(v.as_array()?.iter().filter_map(Profile::parse).collect())
}

/// What a harness offers in a folder before a session exists (`HarnessCatalogResponse`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    /// Whether to offer a choice at all: the harness can list its agents and models here.
    pub supported: bool,
    pub agents: Vec<crate::composer::choice::AgentOption>,
    pub models: Vec<crate::composer::choice::ModelOption>,
    pub default_agent: Option<String>,
    pub default_model: Option<crate::composer::choice::ModelReference>,
}

/// `GET /api/harnesses/{type}/catalog`; a refusal says the body's `error`, else `HTTP n`.
pub fn parse_catalog(answer: Answer) -> Result<Catalog, String> {
    let (status, body) = answer?;
    if !ok(status) {
        return Err(api_error(status, &body));
    }
    let v = json(&body).filter(|v| !v.is_null()).ok_or_else(|| format!("HTTP {status}"))?;
    Ok(Catalog {
        supported: v.get("supported").and_then(Value::as_bool) == Some(true),
        agents: crate::fleet::catalog::parse_agents(&v),
        models: crate::fleet::catalog::parse_models(&v),
        default_agent: text(&v, "defaultAgent"),
        default_model: v.get("defaultModel").and_then(crate::fleet::catalog::model_reference),
    })
}

// ─── Worktree naming ────────────────────────────────────────────────────────

/// What `GET /api/worktrees/naming` says for a repository: the templates in force, Fleet's, and where each came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamingAnswer {
    pub effective: Templates,
    pub defaults: Templates,
    /// The user's own layer: a field is `None` where the user sets nothing.
    pub user: Layer,
    /// Which layer each field came from (`project`, `user`, `default`).
    pub layers: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layer {
    pub branch: Option<String>,
    pub root: Option<String>,
    pub folder: Option<String>,
    pub prefix: Option<String>,
}

fn templates(v: Option<&Value>) -> Templates {
    let d = Templates::default();
    let get = |k: &str, fallback: &str| v.and_then(|v| text(v, k)).unwrap_or_else(|| fallback.to_string());
    let mut capture: Vec<(String, String)> = v
        .and_then(|v| v.get("capture"))
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, p)| Some((k.clone(), p.as_str()?.to_string())))
        .collect();
    capture.sort();
    Templates {
        branch: get("branch", &d.branch),
        root: get("root", &d.root),
        folder: get("folder", &d.folder),
        capture,
        prefix: get("prefix", &d.prefix),
    }
}

/// `extractApiError`: `detail`, `error`, `title`, `message`, else `fallback`.
fn extract_error(body: &str, fallback: &str) -> String {
    let said = json(body).and_then(|b| ["detail", "error", "title", "message"].iter().find_map(|k| text(&b, k)));
    said.unwrap_or_else(|| fallback.to_string())
}

pub fn parse_naming(answer: Answer) -> Result<NamingAnswer, String> {
    const FAILED: &str = "Couldn't load worktree naming.";
    let (status, body) = answer.map_err(|_| FAILED.to_string())?;
    if !ok(status) {
        return Err(extract_error(&body, FAILED));
    }
    let v = json(&body).filter(|v| v.is_object()).ok_or(FAILED)?;
    let user = v.get("user");
    let own = |k: &str| user.and_then(|u| text(u, k));
    let mut layers: Vec<(String, String)> = v
        .get("layers")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, l)| Some((k.clone(), l.as_str()?.to_string())))
        .collect();
    layers.sort();
    Ok(NamingAnswer {
        effective: templates(v.get("effective")),
        defaults: Templates { capture: Vec::new(), ..templates(v.get("defaults")) },
        user: Layer { branch: own("branch"), root: own("root"), folder: own("folder"), prefix: own("prefix") },
        layers,
    })
}

impl NamingAnswer {
    /// The repository ships its own naming convention (`hasProjectConvention`).
    pub fn has_project_convention(&self) -> bool {
        self.layers.iter().any(|(_, l)| l == "project")
    }
}

// ─── Preferences the page reads ─────────────────────────────────────────────

/// Settings → General → Show starters: on unless turned off (`NewSessionStarters`).
pub const STARTERS_PREFERENCE_KEY: &str = "NewSessionStarters";
/// Settings → Labs: the Workflows switch.
pub const WORKFLOWS_PREFERENCE_KEY: &str = "Workflows";

/// Whether the starters show: preferences are known and the value isn't `"false"`. Until the first answer they stay away.
pub fn starters_shown(prefs: &crate::fleet::prefs::Prefs) -> bool {
    prefs.fetched && prefs.get(STARTERS_PREFERENCE_KEY) != Some("false")
}

/// `isWorkflowsEnabled`: off unless the preference is `"true"`.
pub fn workflows_enabled(prefs: &crate::fleet::prefs::Prefs) -> bool {
    prefs.get(WORKFLOWS_PREFERENCE_KEY) == Some("true")
}

// ─── A change the harness pushed ────────────────────────────────────────────

/// `harness.catalog_changed` on the `sessions` topic (`HarnessCatalogChange`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogChange {
    pub harness_type: String,
    pub directory: String,
    /// The folder is where quick chats run.
    pub quick_chat: bool,
    pub profile_ids: Vec<String>,
}

impl CatalogChange {
    /// `parseCatalogChange`: none without a harness and a folder.
    pub fn parse(payload: &Value) -> Option<CatalogChange> {
        let strings = |k: &str| {
            payload.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
        };
        Some(CatalogChange {
            harness_type: text(payload, "harnessType")?,
            directory: text(payload, "directory")?,
            quick_chat: payload.get("quickChat").and_then(Value::as_bool) == Some(true),
            profile_ids: strings("profileIds"),
        })
    }

    /// `isCatalogChangeFor`: whether this change is to the catalog asked for with `harness`, `directory` (None for a quick
    /// chat's) and `profile` (an id, `none`, or None for the harness's default, which the page may not know yet).
    pub fn is_for(&self, harness: &str, directory: Option<&str>, profile: Option<&str>) -> bool {
        let trim = |d: &str| if d.len() > 1 { d.trim_end_matches(['/', '\\']).to_string() } else { d.to_string() };
        if self.harness_type != harness {
            return false;
        }
        let same_folder = match directory.filter(|d| !d.is_empty()) {
            Some(d) => trim(d) == trim(&self.directory),
            None => self.quick_chat,
        };
        same_folder && profile.is_none_or(|p| self.profile_ids.iter().any(|i| i == p))
    }
}
