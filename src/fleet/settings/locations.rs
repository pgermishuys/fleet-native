//! Settings → Folders → Locations as plain data (`WorkspaceSection.vue`): the roots Fleet scans for repositories, what the
//! page keeps while it is open, the folder browser behind the Browse button, and the calls with the order they run in. No Rune
//! and no I/O here but [`run`] and [`browse`], which a thread calls. What touches the app is `locations_actions.rs`.
use crate::fleet::http;
use crate::fleet::settings::memory::memory_error;
use rune::TextEdit;
use serde_json::Value;

/// One location (`WorkspaceRootItem`). `id` is null for a root that came from the environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub id: Option<String>,
    pub path: String,
    /// `source === "env"`: set where Fleet starts, so it can't be removed here.
    pub env: bool,
    pub exists: bool,
}

impl Root {
    /// The Remove button: an added root with an id.
    pub fn removable(&self) -> Option<&str> {
        if self.env { None } else { self.id.as_deref() }
    }

    /// The chip beside the path.
    pub fn source_word(&self) -> &'static str {
        if self.env { "From the environment" } else { "Added" }
    }
}

/// One folder in the browser (`DirectoryEntry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub git: bool,
}

/// What the popover shows (`useDirectoryBrowser`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Browse {
    pub current: Option<String>,
    pub entries: Vec<Entry>,
    pub roots: Vec<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub search: TextEdit,
    /// Answers to an older ask are dropped.
    pub request: u64,
}

impl Eq for Browse {}

/// What the Folders page keeps about Locations while it is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationsPage {
    /// The machine key it was read from; leaving the page or moving the switch reads again.
    pub loaded_for: Option<String>,
    pub request: u64,
    pub loading: bool,
    /// Refresh was pressed and the answer hasn't come (the button shows the spinner).
    pub refreshing: bool,
    pub roots: Vec<Root>,
    /// Why the list isn't shown, in the server's words; replaces the list.
    pub error: Option<String>,
    /// The id of the root being removed.
    pub removing: Option<String>,
    pub adding: bool,
    pub add_error: Option<String>,
    pub path: TextEdit,
    pub browse: Option<Browse>,
}

impl Eq for LocationsPage {}

pub const LOAD_FALLBACK: &str = "Couldn't load your locations.";
pub const ADD_FALLBACK: &str = "Couldn't add that location.";
pub const REMOVE_FALLBACK: &str = "Couldn't remove that location.";
pub const EMPTY_PATH: &str = "Enter the folder to add.";

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `{roots: [{id, path, source, exists}]}`. A root with no path is dropped; `exists` reads as true unless it says false.
pub fn parse_roots(v: &Value) -> Option<Vec<Root>> {
    let list = v.get("roots")?.as_array()?;
    Some(
        list.iter()
            .filter_map(|r| {
                Some(Root {
                    id: text(r, "id"),
                    path: text(r, "path")?,
                    env: text(r, "source").as_deref() == Some("env"),
                    exists: r.get("exists").and_then(Value::as_bool) != Some(false),
                })
            })
            .collect(),
    )
}

/// What one browse answer holds (`DirectoryListResponse`).
pub fn parse_browse(v: &Value) -> Browse {
    let entries = v.get("entries").and_then(Value::as_array).map_or_else(Vec::new, |a| {
        a.iter()
            .filter_map(|e| {
                let path = text(e, "path")?;
                let name = text(e, "name").unwrap_or_else(|| path.rsplit(['/', '\\']).next().unwrap_or_default().to_string());
                Some(Entry { name, path, git: e.get("isGitRepo").and_then(Value::as_bool) == Some(true) })
            })
            .collect()
    });
    let roots = v
        .get("roots")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| a.iter().filter_map(|r| r.as_str().map(str::to_string)).collect());
    Browse { current: text(v, "currentPath"), entries, roots, ..Browse::default() }
}

/// The path split into the breadcrumbs under the root that holds it (`breadcrumbs`): each name with the path it goes to.
pub fn breadcrumbs(current: &str, roots: &[String]) -> Vec<(String, String)> {
    let sep = if current.contains('\\') { '\\' } else { '/' };
    let name = |p: &str| p.split(sep).rfind(|s| !s.is_empty()).unwrap_or(p).to_string();
    let Some(root) = roots.iter().find(|r| current == r.as_str() || current.starts_with(&format!("{r}{sep}"))) else {
        return vec![(current.to_string(), current.to_string())];
    };
    let mut out = vec![(name(root), root.clone())];
    if current != root.as_str() {
        let mut at = root.clone();
        for seg in current[root.len() + 1..].split(sep) {
            at = format!("{at}{sep}{seg}");
            out.push((seg.to_string(), at.clone()));
        }
    }
    out
}

/// What a thread did and found: the list as it stands now, or why not.
pub struct Outcome {
    /// The add or remove itself was refused (an add leaves the list alone; a remove puts the words where the list was).
    pub refused: Option<String>,
    pub roots: Option<Result<Vec<Root>, String>>,
}

/// One thing the page asks the machine to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// On mount, and Refresh: only re-reads the list.
    Load,
    Add(String),
    Remove(String),
}

fn load(base: &str) -> Result<Vec<Root>, String> {
    let (status, body) = http::workspace_roots(base)?;
    if !(200..300).contains(&status) {
        return Err(memory_error(&body, LOAD_FALLBACK));
    }
    let v: Value = serde_json::from_str(&body).map_err(|_| "No data returned".to_string())?;
    parse_roots(&v).ok_or_else(|| "No data returned".to_string())
}

/// Runs one op in the order Fleet does: the change, then the list, then a repository scan whose answer nobody reads.
pub fn run(base: &str, op: &Op) -> Outcome {
    let step = match op {
        Op::Load => None,
        Op::Add(path) => Some((http::add_workspace_root(base, path), ADD_FALLBACK)),
        Op::Remove(id) => Some((http::remove_workspace_root(base, id), REMOVE_FALLBACK)),
    };
    let changed = step.is_some();
    if let Some((answer, fallback)) = step {
        match answer {
            Ok((status, _)) if (200..300).contains(&status) => {}
            Ok((_, body)) => return Outcome { refused: Some(memory_error(&body, fallback)), roots: None },
            Err(e) => return Outcome { refused: Some(e), roots: None },
        }
    }
    let roots = load(base);
    if changed {
        drop(http::repositories_refresh(base));
    }
    Outcome { refused: None, roots: Some(roots) }
}

/// `GET /api/directories` for the popover.
pub fn browse(base: &str, path: Option<&str>) -> Result<Browse, String> {
    let (status, body) = http::browse_directory(base, path)?;
    if !(200..300).contains(&status) {
        return Err(memory_error(&body, "Failed to browse directories"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|_| "No data returned".to_string())?;
    Ok(parse_browse(&v))
}
