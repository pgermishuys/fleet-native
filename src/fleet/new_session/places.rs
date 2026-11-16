//! The place the GitHub plugin fills in the New session page (`docs/PORT.md`, "GitHub boundary"; rows A6.1, A6.11, A6.12, A7.9,
//! A19.1, A19.2): the `#` trigger in the message, the list of items it offers, and the preset an item attaches. Core defines the
//! `ItemPicker` trait and the registry; it names no host. Component 21 registers GitHub's, and until something is registered the
//! `#` stays text, as Fleet's picker does with the plugin off. Plain data and plain functions: no Rune, no I/O.
use super::feeds::Repository;
use super::request::GitHubPreset;
use std::sync::Arc;

/// What a pull request or an issue is, for the glyph of its row and the preset it makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Issue,
    PullRequest,
}

/// One row of the `#` list: what to show, and the preset picking it attaches (`itemSessionPreset`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerItem {
    pub kind: ItemKind,
    pub number: u64,
    pub title: String,
    pub preset: GitHubPreset,
}

/// A host's items for the `#` list. Called from a thread of the host's own, never from a view or a handler.
pub trait ItemPicker: Send + Sync {
    /// The items for what was typed after `#`, in the repository of `folder` (a path on the target machine). Nothing when the
    /// folder's repository isn't this host's.
    fn search(&self, folder: &str, query: &str) -> Vec<PickerItem>;
}

/// The pickers registered at start-up.
#[derive(Clone, Default)]
pub struct ItemPickers(Vec<Arc<dyn ItemPicker>>);

impl ItemPickers {
    pub fn register(&mut self, picker: Arc<dyn ItemPicker>) {
        self.0.push(picker);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every registered host's items, in the order they were registered.
    pub fn search(&self, folder: &str, query: &str) -> Vec<PickerItem> {
        self.0.iter().flat_map(|p| p.search(folder, query)).collect()
    }
}

/// A `#query` the caret is typing: where the `#` is, and what follows it up to the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashTrigger {
    pub start: usize,
    pub end: usize,
    pub query: String,
}

/// `findHashTrigger`: `(^|\s)#([^\s#]*)$` on the text before the caret. A `#` inside a word ("C#") doesn't open it, and with no
/// caret there is no trigger.
pub fn find_hash_trigger(text: &str, caret: usize) -> Option<HashTrigger> {
    if caret == 0 || caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    let before = &text[..caret];
    let hash = before.rfind('#')?;
    if before[hash + 1..].contains(char::is_whitespace) || before[..hash].chars().next_back().is_some_and(|c| !c.is_whitespace()) {
        return None;
    }
    Some(HashTrigger { start: hash, end: caret, query: before[hash + 1..].to_string() })
}

/// `removeTrigger`: the message without the `#query` that picked an item, and where the caret goes. No two spaces are left where it
/// was, and at the very start the text loses its leading space.
pub fn remove_trigger(text: &str, trigger: &HashTrigger) -> (String, usize) {
    let before = &text[..trigger.start];
    let mut after = &text[trigger.end..];
    if before.ends_with(' ') && after.starts_with(' ') {
        after = &after[1..];
    }
    let joined = format!("{before}{after}");
    if before.is_empty() { (joined.trim_start().to_string(), 0) } else { (joined, before.len()) }
}

/// `findRepositoryForGitHubPreset`: the scanned repository named for the item's repository, exactly first, then ignoring case.
pub fn repository_for_preset<'a>(preset: &GitHubPreset, repositories: &'a [Repository]) -> Option<&'a Repository> {
    repositories
        .iter()
        .find(|r| r.name == preset.repo)
        .or_else(|| repositories.iter().find(|r| r.name.to_lowercase() == preset.repo.to_lowercase()))
}

/// The attachment chip's aria label (`Remove pull request #N`, `Remove issue #N`).
pub fn remove_label(preset: &GitHubPreset) -> String {
    let what = if preset.source_type == "github-pull-request" { "pull request" } else { "issue" };
    format!("Remove {what} #{}", preset.number)
}
