//! The plan line under the message box: one sentence saying what Enter will do (`lib/new-session-plan.ts`; rows
//! A12.5-A12.11, A17). Pure: it returns runs of text, and a view decides how a `code` or `warn` run is drawn.
use super::request::{Folder, Workspace};

/// A run of plan-line text: `code` runs are paths and branch names, `warn` runs a caution (`PlanPart`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanPart {
    pub text: String,
    pub code: bool,
    pub warn: bool,
}

fn plain(text: &str) -> PlanPart {
    PlanPart { text: text.to_string(), ..PlanPart::default() }
}

fn code(text: &str) -> PlanPart {
    PlanPart { text: text.to_string(), code: true, ..PlanPart::default() }
}

fn warn(text: String) -> PlanPart {
    PlanPart { text, warn: true, ..PlanPart::default() }
}

/// What the line is made from (`NewSessionPlanInput`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanInput {
    /// `None` until a folder is chosen.
    pub folder: Option<Folder>,
    pub workspace: Workspace,
    /// Branch the main checkout has checked out, when known.
    pub current_branch: Option<String>,
    /// Branch a new worktree will get; `None` when the server will name it.
    pub new_branch: Option<String>,
    /// Branch of the chosen existing worktree, when known.
    pub existing_branch: Option<String>,
    /// The repository's default branch (`main`), when known.
    pub default_branch: Option<String>,
    /// Where a new worktree starts (`origin/main`), when known.
    pub base: Option<String>,
    /// Whether an `origin/…` base is fetched first (default true).
    pub fetch_origin: bool,
    /// Where the new worktree will be, as the naming templates resolve it; `None` until they've been read, when the label
    /// falls back to Fleet's default layout.
    pub worktree_path: Option<String>,
}

impl PlanInput {
    pub fn new(folder: Option<Folder>, workspace: Workspace) -> Self {
        PlanInput {
            folder,
            workspace,
            current_branch: None,
            new_branch: None,
            existing_branch: None,
            default_branch: None,
            base: None,
            fetch_origin: true,
            worktree_path: None,
        }
    }
}

/// How many bytes of `path` a home folder takes up, if it starts with one: `/home/x`, `/Users/x`, `C:\Users\x`, and only
/// when the next character ends the name (`/home/me-too` is a home folder named `me-too`, which is all of it).
fn home_len(path: &str) -> Option<usize> {
    for prefix in ["/home/", "/Users/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let name = rest.find('/').unwrap_or(rest.len());
            return (name > 0).then_some(prefix.len() + name);
        }
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\' {
        let rest = path[3..].strip_prefix("Users\\")?;
        let name = rest.find('\\').unwrap_or(rest.len());
        return (name > 0).then_some(3 + 6 + name);
    }
    None
}

/// `/home/me/src/x` → `~/src/x` (also macOS and Windows home folders) (`tildePath`).
pub fn tilde_path(path: &str) -> String {
    match home_len(path) {
        Some(n) => format!("~{}", &path[n..]),
        None => path.to_string(),
    }
}

fn base_name(path: &str) -> &str {
    path.split(['/', '\\']).rfind(|s| !s.is_empty()).unwrap_or(path)
}

/// Where the server will put a new worktree (before any `-2` for a taken folder), when the naming templates haven't
/// been read yet (`worktreeFolderLabel`).
pub fn worktree_folder_label(repository_path: &str, branch: &str) -> String {
    format!("{}-worktrees/{}", base_name(repository_path), branch.replace(['/', '\\'], "-"))
}

/// One sentence saying what pressing Enter will do (`describeNewSession`).
pub fn describe_new_session(input: &PlanInput) -> Vec<PlanPart> {
    let Some(folder) = &input.folder else { return vec![plain("Machine and folder")] };
    let path = match folder {
        Folder::None => return vec![plain("Chat only. No folder.")],
        Folder::Directory(path) => return vec![plain("Runs in "), code(&tilde_path(path)), plain(" as it is.")],
        Folder::Repository(path) => path,
    };

    match &input.workspace {
        Workspace::Current => {
            let off_default =
                matches!((&input.current_branch, &input.default_branch), (Some(c), Some(d)) if !c.is_empty() && !d.is_empty() && c != d);
            let mut parts = vec![plain("Works directly in "), code(&tilde_path(path))];
            if let Some(branch) = input.current_branch.as_deref().filter(|b| !b.is_empty()) {
                parts.extend([plain(" on "), code(branch)]);
            }
            parts.push(match (&input.default_branch, off_default) {
                (Some(default), true) => warn(format!(". That's not {default}.")),
                _ => plain(". Edits land in your checkout."),
            });
            parts
        }
        Workspace::Existing(worktree) => {
            let mut parts = vec![plain("Continues in "), code(&tilde_path(worktree))];
            if let Some(branch) = input.existing_branch.as_deref().filter(|b| !b.is_empty()) {
                parts.extend([plain(" on "), code(branch)]);
            }
            parts.push(plain("."));
            parts
        }
        Workspace::New => {
            let base: Vec<PlanPart> = match input.base.as_deref().filter(|b| !b.is_empty()) {
                Some(base) => {
                    let mut parts = vec![code(base)];
                    if !input.fetch_origin && base.starts_with("origin/") {
                        parts.push(plain(" as last fetched"));
                    }
                    parts
                }
                None => vec![plain("the default branch")],
            };
            let Some(branch) = input.new_branch.as_deref().filter(|b| !b.is_empty()) else {
                let mut parts = vec![plain("New worktree from ")];
                parts.extend(base);
                parts.push(plain(". The branch is named from your message, or the session id without one."));
                return parts;
            };
            let place = match input.worktree_path.as_deref().filter(|p| !p.is_empty()) {
                Some(resolved) => tilde_path(resolved),
                None => worktree_folder_label(path, branch),
            };
            let mut parts = vec![plain("New worktree "), code(&place), plain(" on "), code(branch), plain(", from ")];
            parts.extend(base);
            parts.push(plain("."));
            parts
        }
    }
}

/// The line as one string, as Fleet's tests read it.
pub fn sentence(parts: &[PlanPart]) -> String {
    parts.iter().map(|p| p.text.as_str()).collect()
}
