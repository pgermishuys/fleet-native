//! The folder menu's list view as plain data (`NSD/FolderPicker.vue`'s `options`, `groupLabel`, `startsOther`, the empty notes;
//! `lib/new-folder.ts`; rows B2.60-B2.86, B2.95). Nothing here draws or asks the machine: the view reads `rows` and the state
//! changes in `folder_menu.rs` act on a row's `Act`.
use super::feeds::Repository;
use super::plan::{base_name, tilde_path};
use super::request::Folder;

// ─── new-folder.ts ──────────────────────────────────────────────────────────────

/// A repository to clone, as typed, and the folder it gets by default (`CloneSource`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneSource {
    /// What goes to the server: `owner/repo`, or the address as typed.
    pub repository: String,
    /// `owner/repo` for GitHub, else the address.
    pub label: String,
    /// The repository's name, which the clone's folder is called.
    pub name: String,
}

fn is_owner(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// `[A-Za-z0-9._-]+?(?:\.git)?` ending the string: the repository and one `.git` stripped.
fn repo_name(s: &str) -> Option<&str> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return None;
    }
    let name = s.strip_suffix(".git").unwrap_or(s);
    (!name.is_empty()).then_some(name)
}

fn github(owner: &str, repo: &str) -> Option<CloneSource> {
    if !is_owner(owner) {
        return None;
    }
    let name = repo_name(repo)?;
    let both = format!("{owner}/{name}");
    Some(CloneSource { repository: both.clone(), label: both, name: name.to_string() })
}

/// Case-insensitive `strip_prefix`.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len()).filter(|p| p.eq_ignore_ascii_case(prefix)).map(|_| &s[prefix.len()..])
}

fn address_name(address: &str) -> String {
    let last = address.trim_end_matches('/').rsplit(['/', ':']).next().unwrap_or(address);
    let cut = last.len().checked_sub(4).filter(|&n| last.is_char_boundary(n) && last[n..].eq_ignore_ascii_case(".git"));
    cut.map_or(last, |n| &last[..n]).to_string()
}

/// A GitHub `owner/repo`, a GitHub URL, or an https/ssh address of any git host; `None` for anything else (`parseCloneSource`).
pub fn parse_clone_source(text: &str) -> Option<CloneSource> {
    let value = text.trim().trim_end_matches('/');
    // A repository's page on github.com, or a page inside it (`/tree/main`, `/pull/12`).
    let page = strip_prefix_ci(value, "https://").unwrap_or(value);
    let page = strip_prefix_ci(page, "www.").unwrap_or(page);
    if let Some(rest) = strip_prefix_ci(page, "github.com/") {
        let mut parts = rest.splitn(3, '/');
        if let (Some(owner), Some(repo)) = (parts.next(), parts.next())
            && let Some(found) = github(owner, repo)
        {
            return Some(found);
        }
    }
    if let Some((owner, repo)) = value.split_once('/')
        && let Some(found) = github(owner, repo)
    {
        return Some(found);
    }
    let other_url = ["https://", "ssh://"].iter().find_map(|p| value.strip_prefix(p)).is_some_and(|rest| {
        !rest.contains(char::is_whitespace) && rest.split_once('/').is_some_and(|(host, path)| !host.is_empty() && !path.is_empty())
    });
    let scp_like = value.split_once('@').is_some_and(|(user, rest)| {
        let ok = |s: &str, extra: &[char]| s.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(&c));
        rest.split_once(':').is_some_and(|(host, path)| {
            !user.is_empty()
                && ok(user, &['.', '_', '-'])
                && !host.is_empty()
                && ok(host, &['.', '-'])
                && !path.is_empty()
                && ok(path, &['.', '_', '/', '-'])
        })
    });
    (other_url || scp_like).then(|| CloneSource { repository: value.to_string(), label: value.to_string(), name: address_name(value) })
}

/// A name or path typed for a new folder: the folders it makes, or what is wrong with it (`TypedFolderPath`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedFolderPath {
    /// The folders to make, outermost first: `clients\acme portal` is `["clients", "acme-portal"]`.
    pub segments: Vec<String>,
    /// A character no folder name can have, when one was typed. Nothing is dropped quietly.
    pub invalid: Option<char>,
}

/// A character no folder name may have on Windows, macOS or Linux, and control characters.
pub fn invalid_name_character(name: &str) -> Option<char> {
    name.chars().find(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (*c as u32) < 0x20)
}

/// `folderPathFrom`: `/` and `\` separate folders, spaces become dashes, case is kept, `.` and `..` go.
pub fn folder_path_from(text: &str) -> TypedFolderPath {
    let segments = text
        .split(['/', '\\'])
        .map(|segment| {
            let dashed = segment.split_whitespace().collect::<Vec<_>>().join("-");
            dashed.trim_start_matches(['.', '-']).trim_end_matches('.').to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    TypedFolderPath { segments, invalid: invalid_name_character(text) }
}

/// The separator a machine's paths use, judged from one of its paths (`separatorOf`).
pub fn separator_of(path: Option<&str>) -> char {
    let Some(path) = path else { return '/' };
    let b = path.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    if drive || path.starts_with("\\\\") || (path.contains('\\') && !path.contains('/')) { '\\' } else { '/' }
}

/// Whether typed text names a place by itself (`~`, `/`, `C:\`, `\\server`) rather than one inside a location.
pub fn is_rooted_path(text: &str) -> bool {
    let b = text.as_bytes();
    text.starts_with(['~', '/'])
        || text.starts_with("\\\\")
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || matches!(b[2], b'/' | b'\\')))
}

/// The folders between `base`, which is there, and `target` inside it: the ones a create would make.
pub fn folders_between(base: &str, target: &str) -> Vec<String> {
    let trim = |v: &str| v.trim_end_matches(['/', '\\']).to_string();
    let (base, target) = (trim(base), trim(target));
    target.get(base.len()..).unwrap_or_default().split(['/', '\\']).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// `name` inside `parent`, with the separator the parent already uses.
pub fn join_path(parent: &str, name: &str) -> String {
    let sep = if parent.contains('\\') && !parent.contains('/') { '\\' } else { '/' };
    if parent.ends_with(sep) { format!("{parent}{name}") } else { format!("{parent}{sep}{name}") }
}

fn is_inside(path: &str, root: &str) -> bool {
    let norm = |v: &str| v.trim_end_matches(['/', '\\']).to_lowercase();
    let (child, parent) = (norm(path), norm(root));
    child == parent || child.starts_with(&format!("{parent}/")) || child.starts_with(&format!("{parent}\\"))
}

/// The workspace root `path` is in, if any (`rootContaining`).
pub fn root_containing<'a>(path: &str, roots: &'a [String]) -> Option<&'a String> {
    roots.iter().find(|r| is_inside(path, r))
}

/// Where a new folder goes: the root last used for one, else the root of the folder in use, else the first root
/// (`defaultNewFolderRoot`).
pub fn default_new_folder_root<'a>(roots: &'a [String], last: Option<&str>, current: Option<&str>) -> Option<&'a String> {
    if let Some(r) = last.and_then(|l| roots.iter().find(|r| r.as_str() == l)) {
        return Some(r);
    }
    current.and_then(|c| root_containing(c, roots)).or_else(|| roots.first())
}

// ─── the rows ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Recent,
    Repositories,
    Folders,
    New,
    Other,
}

/// Which mark leads a row (`option.icon`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Repository,
    Directory,
    Browse,
    Create,
    Clone,
    Chat,
}

/// What a row does when it is run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Choose(Folder),
    /// The path view (piece G), with this text typed in it.
    Path(Option<String>),
    /// The clone view (piece H).
    CloneForm,
    Create {
        target: String,
        root: String,
    },
    Clone {
        source: CloneSource,
        target: String,
        root: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderOption {
    pub id: String,
    pub group: Group,
    pub title: String,
    /// The bold part of the title (`Create` **`clients/portal`**).
    pub title_name: Option<String>,
    pub detail: String,
    pub mono: bool,
    pub mark: Mark,
    pub selected: bool,
    pub act: Act,
}

/// What the rows depend on.
pub struct Inputs<'a> {
    pub repositories: &'a [Repository],
    /// The recent folders, repositories no longer scanned already dropped.
    pub recent: &'a [Folder],
    pub plain: &'a [String],
    pub current: Option<&'a Folder>,
    pub query: &'a str,
    pub allow_browse: bool,
    pub allow_create: bool,
    pub allow_none: bool,
    /// Where a Create or Clone would go; `None` while the machine hasn't said, or has no location.
    pub root: Option<&'a str>,
    pub first_branch: &'a str,
}

fn is_current(current: Option<&Folder>, candidate: &Folder) -> bool {
    current == Some(candidate)
}

fn folder_option(group: Group, folder: Folder, title: String, mark: Mark, current: Option<&Folder>) -> FolderOption {
    let (Folder::Repository(path) | Folder::Directory(path)) = &folder else { unreachable!("only folders with a path") };
    let tag = if group == Group::Recent {
        "Recent"
    } else if group == Group::Repositories {
        "Repositories"
    } else {
        "Folders"
    };
    FolderOption {
        id: format!("{tag}:{}{path}", if mark == Mark::Directory { "dir:" } else { "" }),
        group,
        title,
        title_name: None,
        detail: tilde_path(path),
        mono: true,
        mark,
        selected: is_current(current, &folder),
        act: Act::Choose(folder),
    }
}

/// `options`: Recent, Repositories, Folders, at most one New row, then the Other rows (B2.60-B2.77).
pub fn rows(i: &Inputs) -> Vec<FolderOption> {
    let needle = i.query.trim().to_lowercase();
    let clone = if i.allow_create { parse_clone_source(i.query) } else { None };
    // A pasted repository address never matches a local repository by text.
    let matches = |text: &str| needle.is_empty() || (clone.is_none() && text.to_lowercase().contains(&needle));
    let repo_match = |r: &Repository| matches(&format!("{} {}", r.name, r.path));
    let repo_option =
        |group, r: &Repository| folder_option(group, Folder::Repository(r.path.clone()), r.name.clone(), Mark::Repository, i.current);
    let dir_option =
        |group, p: &str| folder_option(group, Folder::Directory(p.to_string()), base_name(p).to_string(), Mark::Directory, i.current);

    let (mut recent, mut recent_paths) = (Vec::new(), Vec::new());
    for folder in i.recent {
        match folder {
            Folder::Repository(path) => {
                if let Some(r) = i.repositories.iter().find(|r| &r.path == path).filter(|r| repo_match(r)) {
                    recent.push(repo_option(Group::Recent, r));
                }
                recent_paths.push(path.as_str());
            }
            Folder::Directory(path) if i.allow_browse => {
                if matches(path) {
                    recent.push(dir_option(Group::Recent, path));
                }
                recent_paths.push(path.as_str());
            }
            _ => {}
        }
    }
    let mut others: Vec<&Repository> =
        i.repositories.iter().filter(|r| !recent_paths.contains(&r.path.as_str()) && repo_match(r)).collect();
    // `localeCompare`: case-insensitive first, so `Aurora-docs` sits among the a's.
    others.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
    let plain = i.plain.iter().filter(|p| i.allow_browse && !recent_paths.contains(&p.as_str()) && matches(p));

    let mut out = recent;
    out.extend(others.into_iter().map(|r| repo_option(Group::Repositories, r)));
    out.extend(plain.map(|p| dir_option(Group::Folders, p)));

    let query = i.query.trim();
    let rooted = i.allow_browse && clone.is_none() && is_rooted_path(query);
    let typed = (i.allow_create && clone.is_none() && !rooted && !query.is_empty()).then(|| folder_path_from(i.query));
    let create = typed.as_ref().zip(i.root).and_then(|(t, root)| {
        let taken = t.segments.len() == 1 && i.repositories.iter().any(|r| r.name.eq_ignore_ascii_case(&t.segments[0]));
        (t.invalid.is_none() && !t.segments.is_empty() && !taken)
            .then(|| (t.segments.iter().fold(root.to_string(), |p, s| join_path(&p, s)), root))
    });
    let new_row = |id: &str, title: &str, name: String, detail: String, mark, act| FolderOption {
        id: id.into(),
        group: Group::New,
        title: title.into(),
        title_name: Some(name),
        detail,
        mono: false,
        mark,
        selected: false,
        act,
    };
    if let (Some(root), Some(source)) = (i.root, &clone) {
        let target = join_path(root, &source.name);
        out.push(new_row(
            "clone",
            "Clone",
            source.label.clone(),
            format!("into {}", tilde_path(&target)),
            Mark::Clone,
            Act::Clone { source: source.clone(), target, root: root.into() },
        ));
    } else if let (Some((target, root)), Some(base)) = (create, i.root) {
        let branch = if i.first_branch.is_empty() { "git repository".to_string() } else { format!("git repository on {}", i.first_branch) };
        out.push(new_row(
            "create",
            "Create",
            folders_between(base, &target).join(&separator_of(Some(root)).to_string()),
            format!("{} · {branch}", tilde_path(&target)),
            Mark::Create,
            Act::Create { target, root: root.into() },
        ));
    } else if rooted {
        let detail = if i.allow_create { "Open it, or create it if it isn't there" } else { "Open it" };
        out.push(new_row("go", "Go to", query.to_string(), detail.into(), Mark::Browse, Act::Path(Some(query.to_string()))));
    }

    let other = |id: &str, title: &str, detail: &str, mark, act| FolderOption {
        id: id.into(),
        group: Group::Other,
        title: title.into(),
        title_name: None,
        detail: detail.into(),
        mono: false,
        mark,
        selected: false,
        act,
    };
    if i.allow_browse {
        let (title, detail) = if i.allow_create {
            ("Open or create a folder…", "Type a path; Fleet lists what's there")
        } else {
            ("Open a folder…", "Any repository or folder on this machine")
        };
        out.push(other("path", title, detail, Mark::Browse, Act::Path(None)));
    }
    if i.allow_create {
        out.push(other("clone-form", "Clone a repository…", "From GitHub or any git address", Mark::Clone, Act::CloneForm));
    }
    if i.allow_none {
        let mut row = other("none", "No folder, just chat", "Ask anything without a repository", Mark::Chat, Act::Choose(Folder::None));
        row.selected = i.current == Some(&Folder::None);
        out.push(row);
    }
    out
}

/// `hasFolderMatches`: some folder row is there.
pub fn has_folder_matches(rows: &[FolderOption]) -> bool {
    rows.iter().any(|r| matches!(r.mark, Mark::Repository | Mark::Directory))
}

/// `offersToMake`: a Create or Clone row is there.
pub fn offers_to_make(rows: &[FolderOption]) -> bool {
    rows.iter().any(|r| r.id == "create" || r.id == "clone")
}

/// `groupLabel`: the label shown above row `index`, if it starts a group.
pub fn group_label(rows: &[FolderOption], index: usize) -> Option<&'static str> {
    let row = rows.get(index)?;
    if index > 0 && rows[index - 1].group == row.group {
        return None;
    }
    match row.group {
        Group::New if row.mark == Mark::Clone => Some("Repository to clone"),
        Group::New if has_folder_matches(rows) => Some("Or start something new"),
        Group::New => Some("No folder matches"),
        Group::Other => None,
        Group::Recent => Some("Recent"),
        Group::Repositories => Some("Repositories"),
        Group::Folders => Some("Folders"),
    }
}

/// `startsOther`: a rule goes above the first Other row unless it is the first row of all.
pub fn starts_other(rows: &[FolderOption], index: usize) -> bool {
    index > 0 && rows[index].group == Group::Other && rows[index - 1].group != Group::Other
}

/// The note under an empty list (B2.80-B2.83), if the list wants one.
pub fn empty_note(i: &Inputs, rows: &[FolderOption]) -> Option<String> {
    let query = i.query.trim();
    let rooted = i.allow_browse && parse_clone_source(i.query).is_none_or(|_| !i.allow_create) && is_rooted_path(query);
    let invalid = invalid_in_query(i);
    if has_folder_matches(rows) || offers_to_make(rows) || rooted || invalid.is_some() {
        return None;
    }
    Some(if !query.is_empty() {
        format!("No folder matches “{query}”.")
    } else if i.allow_create {
        "No repositories yet. Type a name to create one.".into()
    } else if i.allow_browse {
        "No repositories yet. Open a folder to add one.".into()
    } else {
        "No repositories found in your locations.".into()
    })
}

/// `invalidInQuery`: a character typed in the search box that no folder name can have (create mode, not rooted, not a clone
/// address).
pub fn invalid_in_query(i: &Inputs) -> Option<char> {
    let query = i.query.trim();
    if !i.allow_create || query.is_empty() || parse_clone_source(i.query).is_some() || (i.allow_browse && is_rooted_path(query)) {
        return None;
    }
    folder_path_from(i.query).invalid
}

/// The search box's words (B2.38-B2.40).
pub fn placeholder(allow_create: bool) -> &'static str {
    if allow_create { "Search, name a new folder, or paste a repository…" } else { "Search repositories…" }
}
