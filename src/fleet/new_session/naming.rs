//! The composer's copy of the worktree naming resolver, for the plan line as you type (`lib/worktree-naming.ts`; rows
//! A17.2-A17.8). The server does the real naming; both are asserted against the shared cases in Fleet's
//! `tests/contracts/worktree-naming-cases.json`, so a preview can't promise a name the worktree won't get. Pure.
use super::pattern::Pattern;
use super::request::slug_for_branch;
use std::collections::HashMap;

/// The naming templates of a repository (`WorktreeNamingTemplates`): `branch`, `root`, `folder`, the `capture` patterns by
/// name, and what `{prefix}` is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Templates {
    pub branch: String,
    pub root: String,
    pub folder: String,
    pub capture: Vec<(String, String)>,
    pub prefix: String,
}

impl Default for Templates {
    /// `defaultWorktreeNaming`.
    fn default() -> Self {
        Templates {
            branch: "{prefix}/{slug}".into(),
            root: "{repoParent}/{repo}-worktrees".into(),
            folder: "{branch}".into(),
            capture: Vec::new(),
            prefix: "fleet".into(),
        }
    }
}

/// What the templates can name besides the message (`WorktreeNamingContext`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub repository_path: String,
    pub user: String,
    /// ISO date, `2026-09-21`.
    pub date: String,
    pub short_id: String,
    pub home: String,
}

/// A worktree's name (`WorktreeName`). `branch` is `None` when the template wanted a slug this message couldn't give and
/// there's no short id to stand in for it: the server names it `<template>/session-<id>` then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub branch: Option<String>,
    pub root: String,
    pub folder: String,
}

const MAX_CAPTURE_LENGTH: usize = 200;

fn base_name(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    trimmed.split(['/', '\\']).rfind(|s| !s.is_empty()).unwrap_or(path).to_string()
}

fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let cut = trimmed.rfind('/').or_else(|| trimmed.rfind('\\'));
    match cut {
        Some(i) if i > 0 => trimmed[..i].to_string(),
        _ => trimmed.to_string(),
    }
}

/// `{name}` tokens replaced; one with no value is empty (`substitute`: `\w+` names).
fn substitute(template: &str, values: &HashMap<&str, String>) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if close > 0 && after[..close].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
                out.push_str(&rest[..open]);
                out.push_str(values.get(&after[..close]).map_or("", String::as_str));
                rest = &after[close + 1..];
            }
            _ => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Doubled separators, and separators left beside a slash by an empty token (`tidy`).
fn tidy(value: &str) -> String {
    // `([-_/])\1+` → `$1`
    let mut doubled = String::new();
    let mut last: Option<char> = None;
    for c in value.chars() {
        if matches!(c, '-' | '_' | '/') && last == Some(c) {
            continue;
        }
        doubled.push(c);
        last = Some(c);
    }
    // `[-_]?\/[-_]?` → `/`
    let chars: Vec<char> = doubled.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let mut j = i;
        if matches!(chars[j], '-' | '_') && chars.get(j + 1) == Some(&'/') {
            j += 1;
        }
        if chars.get(j) == Some(&'/') {
            j += 1;
            if matches!(chars.get(j), Some('-' | '_')) {
                j += 1;
            }
            out.push('/');
            i = j;
        } else {
            out.push(chars[start]);
            i = start + 1;
        }
    }
    out
}

fn collapse(value: &str) -> String {
    tidy(value).trim_matches(|c| matches!(c, '-' | '_' | '/' | '.' | ' ')).to_string()
}

/// The same tidying for a path, which keeps its leading separator (`collapsePath`).
fn collapse_path(value: &str) -> String {
    tidy(value).trim_end_matches(['-', '_', '/', '.', ' ']).to_string()
}

fn read_captures(capture: &[(String, String)], message: &str) -> Vec<(String, String)> {
    capture
        .iter()
        .map(|(name, source)| {
            let found = if message.is_empty() || source.is_empty() || source.chars().count() > MAX_CAPTURE_LENGTH {
                None
            } else {
                // A pattern Settings would have refused, or one this matcher doesn't read, carries on without it.
                Pattern::new(source).and_then(|p| p.capture(message))
            };
            (name.clone(), found.unwrap_or_default())
        })
        .collect()
}

/// Names a worktree for `message`. `branch_override` is a branch already settled by the caller (a typed name or a source's
/// suggestion), and the folder then follows it (`resolveWorktreeName`).
pub fn resolve_worktree_name(naming: &Templates, context: &Context, message: &str, branch_override: Option<&str>) -> Name {
    let captures = read_captures(&naming.capture, message);

    // What a capture consumed is no longer part of the slug, or "{ticket}-{slug}" says it twice
    // ("feature/PLAT-1841-plat-1841-add-rate-limiting").
    let mut slug_source = message.to_string();
    for (_, captured) in &captures {
        if !captured.is_empty() {
            slug_source = slug_source.replacen(captured.as_str(), " ", 1);
        }
    }

    // A message with no slug in it (or no message) still gets the template's branch, with the session id standing in
    // ("fleet/session-a1b2c3d4"). Without an id the branch stays unnamed rather than collapse to the prefix alone.
    let mut slug = slug_for_branch(&slug_source);
    if slug.is_empty() && !context.short_id.is_empty() {
        slug = format!("session-{}", context.short_id);
    }

    let mut values: HashMap<&str, String> = HashMap::new();
    values.insert("slug", slug.clone());
    values.insert("repo", base_name(&context.repository_path));
    values.insert("user", context.user.clone());
    values.insert("prefix", naming.prefix.clone());
    values.insert("date", context.date.clone());
    values.insert("shortid", context.short_id.clone());
    for (name, captured) in &captures {
        values.insert(name.as_str(), captured.clone());
    }

    let branch = match branch_override.filter(|b| !b.is_empty()) {
        Some(b) => Some(b.to_string()),
        // A template built around {slug} with nothing to slug would collapse to its prefix alone ("pg"), which git
        // accepts and which collides with the next one.
        None if naming.branch.contains("{slug}") && slug.is_empty() => None,
        None => Some(collapse(&substitute(&naming.branch, &values))).filter(|b| !b.is_empty()),
    };

    let mut root_values: HashMap<&str, String> = HashMap::new();
    root_values.insert("repo", base_name(&context.repository_path));
    root_values.insert("repoParent", parent_path(&context.repository_path));
    root_values.insert("home", context.home.clone());
    root_values.insert("user", context.user.clone());
    root_values.insert("prefix", naming.prefix.clone());
    root_values.insert("date", context.date.clone());
    root_values.insert("shortid", context.short_id.clone());
    let root = collapse_path(&substitute(&naming.root, &root_values));

    values.insert("branch", branch.clone().unwrap_or_default());
    let folder = collapse(&substitute(&naming.folder, &values)).replace(['/', '\\'], "-");

    Name { branch, root, folder }
}

/// `~/src/x` for a path under the home directory, as the plan line shows it (`tildeWorktreePath`).
pub fn tilde_worktree_path(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path.to_string(),
    }
}
