//! The folder menu's path view as plain data (`NSD/FolderPicker.vue`'s `typedPath`, `folderToList`, `pathCreate`, `pathRows`,
//! `pathNote`, `addsLocation`, `typedPathFor`, `goUp`; rows B2.112-B2.136, B2.140-B2.143). Nothing here draws or asks the
//! machine: the view reads the rows, and the state changes in `folder_path.rs` act on one.
use super::feeds::Listing;
use super::folder::{folders_between, invalid_name_character, is_rooted_path, join_path, root_containing, separator_of};
use super::plan::{base_name, tilde_path};

/// Text typed in the box: the folder to list (up to and with the last separator) and the name typed after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedPath<'a> {
    pub folder: &'a str,
    pub name: &'a str,
}

pub fn split_typed_path(text: &str) -> TypedPath<'_> {
    match text.rfind(['/', '\\']) {
        Some(i) => TypedPath { folder: &text[..=i], name: &text[i + 1..] },
        None => TypedPath { folder: "", name: text },
    }
}

/// Every `/` and `\` written the way the machine writes them (`withSeparator`).
pub fn with_separator(text: &str, sep: char) -> String {
    text.replace(['/', '\\'], &sep.to_string())
}

/// A root keeps its own separator: `/`, `C:\`.
pub fn without_trailing_separator(path: &str) -> String {
    let b = path.as_bytes();
    let root = (b.len() == 1 && matches!(b[0], b'/' | b'\\'))
        || (b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\'));
    if root { path.to_string() } else { path.trim_end_matches(['/', '\\']).to_string() }
}

/// The folder whose contents the box lists: what is typed up to the last separator, inside the location when it isn't a path
/// of its own (`folderToList`).
pub fn folder_to_list(text: &str, location: Option<&str>, sep: char) -> String {
    let folder = split_typed_path(text).folder;
    let location = location.unwrap_or("~");
    if folder.is_empty() {
        return location.to_string();
    }
    let typed = with_separator(folder, sep);
    without_trailing_separator(&if is_rooted_path(&typed) { typed } else { join_path(location, &typed) })
}

/// A folder as the box writes it: `~` for the machine's own home folder, which the server expands the same way. Anything else
/// stays whole, because `~` there would mean a different folder (`typedPathFor`).
pub fn typed_path_for(path: &str, home: Option<&str>) -> String {
    let Some(own) = home.filter(|h| !h.is_empty()) else { return path.to_string() };
    let same = path.get(..own.len()).is_some_and(|head| head.to_lowercase() == own.to_lowercase());
    if !same {
        return path.to_string();
    }
    let rest = &path[own.len()..];
    if rest.is_empty() || rest.starts_with(['/', '\\']) { format!("~{rest}") } else { path.to_string() }
}

/// A folder for the box's rows: the way the box writes it once the home folder is known (`shownPath`).
pub fn shown_path(path: &str, home: Option<&str>) -> String {
    if home.is_some() { typed_path_for(path, home) } else { tilde_path(path) }
}

/// What the box says when it opens: the location with `~` and a separator, or `~` and one (`showPath`).
pub fn initial_text(location: Option<&str>, home: Option<&str>, sep: char) -> String {
    match location {
        Some(l) => format!("{}{sep}", typed_path_for(l, home)),
        None => format!("~{sep}"),
    }
}

/// What the box says after going into `path`: the folder and a separator (a root already has one).
pub fn entered_text(path: &str, home: Option<&str>) -> String {
    let typed = typed_path_for(path, home);
    if typed.ends_with(['/', '\\']) { typed } else { format!("{typed}{}", separator_of(Some(path))) }
}

/// The folder above `here`: `/` stays `/`, `C:` becomes `C:\`, and a path with no separator has none (`goUp`).
pub fn parent_of(here: &str) -> Option<String> {
    let i = here.rfind(['/', '\\'])?;
    let parent = if i == 0 { &here[..1] } else { &here[..i] };
    let b = parent.as_bytes();
    Some(if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' { format!("{parent}\\") } else { parent.to_string() })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathAction {
    Open,
    Enter,
    Create,
}

/// A row in the box: open a folder, go into one, or create the one typed (`PathRow`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathRow {
    pub id: String,
    pub action: PathAction,
    pub title: String,
    pub title_name: Option<String>,
    pub detail: String,
    pub path: String,
    pub is_git_repo: bool,
}

/// The new folders a create would make, and the folder that's there above them (`pathCreate`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathCreate {
    pub base: String,
    pub target: String,
    pub folders: Vec<String>,
    pub invalid: Option<char>,
}

/// What the rows depend on. `listing` is the answer for exactly the folder typed, or none.
pub struct PathInputs<'a> {
    pub text: &'a str,
    pub listing: Option<&'a Listing>,
    pub allow_create: bool,
    pub home: Option<&'a str>,
}

pub fn path_create(i: &PathInputs) -> Option<PathCreate> {
    let result = i.listing.filter(|_| i.allow_create)?;
    let name = split_typed_path(i.text).name.trim();
    let base = if result.exists { Some(result.path.as_str()) } else { result.nearest_existing.as_deref() }?;
    let is_there = result.entries.iter().any(|e| e.name.to_lowercase() == name.to_lowercase());
    if result.exists && (name.is_empty() || is_there) {
        return None;
    }
    let target = if name.is_empty() { result.path.clone() } else { join_path(&result.path, name) };
    let folders = folders_between(base, &target);
    let invalid = folders.iter().find_map(|f| invalid_name_character(f));
    Some(PathCreate { base: base.to_string(), target, folders, invalid })
}

pub fn path_rows(i: &PathInputs) -> Vec<PathRow> {
    let Some(result) = i.listing else { return Vec::new() };
    let name = split_typed_path(i.text).name.trim();
    let plan = path_create(i).filter(|p| p.invalid.is_none());
    let create = plan.map(|p| PathRow {
        id: "create".into(),
        action: PathAction::Create,
        title: "Create".into(),
        title_name: Some(p.folders.join(&separator_of(Some(&result.path)).to_string())),
        detail: format!("in {}", shown_path(&p.base, i.home)),
        path: p.target,
        is_git_repo: false,
    });
    if !result.exists {
        return create.into_iter().collect();
    }
    let needle = name.to_lowercase();
    let starts: Vec<usize> = (0..result.entries.len()).filter(|&n| result.entries[n].name.to_lowercase().starts_with(&needle)).collect();
    let exact = result
        .entries
        .iter()
        .position(|e| e.name == name)
        .or_else(|| if needle.is_empty() { None } else { result.entries.iter().position(|e| e.name.to_lowercase() == needle) });
    let mut rows = Vec::new();
    if name.is_empty() {
        rows.push(PathRow {
            id: "open-here".into(),
            action: PathAction::Open,
            title: "Open".into(),
            title_name: Some(base_name(&result.path).to_string()),
            detail: shown_path(&result.path, i.home),
            path: result.path.clone(),
            is_git_repo: false,
        });
    } else if let Some(n) = exact {
        let e = &result.entries[n];
        rows.push(PathRow {
            id: "open-exact".into(),
            action: PathAction::Open,
            title: "Open".into(),
            title_name: Some(e.name.clone()),
            detail: shown_path(&e.path, i.home),
            path: e.path.clone(),
            is_git_repo: e.is_git_repo,
        });
    }
    // Create leads only when nothing starts with what was typed: otherwise Enter would make "cl" instead of opening "clients".
    let nothing_starts = starts.is_empty();
    if nothing_starts && let Some(c) = &create {
        rows.push(c.clone());
    }
    for n in starts {
        if Some(n) == exact {
            continue;
        }
        let e = &result.entries[n];
        rows.push(PathRow {
            id: format!("entry:{}", e.path),
            action: PathAction::Enter,
            title: e.name.clone(),
            title_name: None,
            detail: if e.is_git_repo { "Repository" } else { "Folder" }.into(),
            path: e.path.clone(),
            is_git_repo: e.is_git_repo,
        });
    }
    if !nothing_starts && let Some(c) = create {
        rows.push(c);
    }
    rows
}

/// The line under the rows: why the listing failed, or what is wrong with the name, or that a folder isn't there
/// (`pathNote`). A listing that failed shows its words until a later one succeeds.
pub fn path_note(i: &PathInputs, listing_error: Option<&str>) -> Option<String> {
    if let Some(e) = listing_error {
        return Some(e.to_string());
    }
    let result = i.listing?;
    if let Some(c) = path_create(i).and_then(|p| p.invalid) {
        return Some(format!("“{c}” can't be in a folder name."));
    }
    (!result.exists && !i.allow_create).then(|| format!("There's no folder at {}.", shown_path(&result.path, i.home)))
}

/// Whether the highlighted row would add a folder to the locations, for the line that says so (`addsLocation`).
/// `roots` is `None` until the machine has said.
pub fn adds_location(row: Option<&PathRow>, roots: Option<&[String]>) -> bool {
    match (row, roots) {
        (Some(r), Some(roots)) => r.action != PathAction::Enter && root_containing(&r.path, roots).is_none(),
        _ => false,
    }
}
