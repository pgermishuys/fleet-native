//! The `@` list's files and folders (`lib/file-matches.ts` and the file rows of `use-autocomplete.ts`): the last
//! answer narrowed while the server's is on its way, in the server's order, and a path's row.
//!
//! Folders end in "/". An empty query or one ending in "/" lists that folder, folders first; anything else ranks by
//! name, then path, then in-order matches, then shallower paths.

/// `narrowFileMatches`.
pub fn narrow_file_matches(entries: &[String], query: &str) -> Vec<String> {
    let normalized = query.trim().replace('\\', "/");
    let normalized = normalized.trim_start_matches('/');

    if normalized.is_empty() || normalized.ends_with('/') {
        let folder = normalized.to_lowercase();
        let mut listed: Vec<&String> = entries
            .iter()
            .filter(|entry| {
                let lower = entry.to_lowercase();
                if lower.len() <= folder.len() || !lower.starts_with(&folder) {
                    return false;
                }
                match lower[folder.len()..].find('/') {
                    None => true,
                    Some(slash) => folder.len() + slash == lower.len() - 1,
                }
            })
            .collect();
        listed.sort_by(|a, b| is_folder(b).cmp(&is_folder(a)).then_with(|| compare_text(a, b)));
        return listed.into_iter().cloned().collect();
    }

    let lower = normalized.to_lowercase();
    let path_only = lower.contains('/');
    let mut ranked: Vec<(&String, i8)> = entries.iter().map(|e| (e, rank(e, &lower, path_only))).filter(|(_, rank)| *rank >= 0).collect();
    ranked.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| depth(a.0).cmp(&depth(b.0)))
            .then_with(|| is_folder(b.0).cmp(&is_folder(a.0)))
            .then_with(|| compare_text(a.0, b.0))
    });
    ranked.into_iter().map(|(e, _)| e.clone()).collect()
}

fn is_folder(entry: &str) -> bool {
    entry.ends_with('/')
}

/// Lower is better; -1 means no match.
fn rank(entry: &str, query: &str, path_only: bool) -> i8 {
    let path = entry.strip_suffix('/').unwrap_or(entry).to_lowercase();
    if path_only {
        return if path.starts_with(query) {
            0
        } else if path.contains(query) {
            3
        } else if in_order(&path, query) {
            5
        } else {
            -1
        };
    }
    let name = &path[path.rfind('/').map_or(0, |i| i + 1)..];
    if name == query {
        0
    } else if name.starts_with(query) {
        1
    } else if name.contains(query) {
        2
    } else if path.contains(query) {
        3
    } else if in_order(name, query) {
        4
    } else if in_order(&path, query) {
        5
    } else {
        -1
    }
}

/// Whether every character of `query` appears in `text`, in order.
fn in_order(text: &str, query: &str) -> bool {
    let mut wanted = query.chars().peekable();
    for c in text.chars() {
        if wanted.peek() == Some(&c) {
            wanted.next();
        }
    }
    wanted.peek().is_none()
}

fn depth(entry: &str) -> usize {
    entry.strip_suffix('/').unwrap_or(entry).matches('/').count()
}

fn compare_text(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// A file's row in the `@` list: the last segment (a folder keeps its "/"), the parent path (or "…" and its last 47
/// characters when it is longer than 48), whether it is a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub label: String,
    pub description: Option<String>,
    pub is_directory: bool,
}

pub fn file_row(path: &str) -> FileRow {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    let name_start = trimmed.rfind('/').map_or(0, |i| i + 1);
    let parent = &trimmed[..name_start];
    let length = parent.chars().count();
    let description = if length > 48 {
        let tail: String = parent.chars().skip(length - 47).collect();
        Some(format!("…{tail}"))
    } else if parent.is_empty() {
        None
    } else {
        Some(parent.to_string())
    };
    FileRow { label: path[name_start..].to_string(), description, is_directory: path.ends_with('/') }
}
