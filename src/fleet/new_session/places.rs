//! The place the GitHub plugin fills in the New session page (`docs/PORT.md`, "GitHub boundary"; rows A6.1, A6.11, A6.12, A7.9,
//! A19.1, A19.2): the `#` trigger in the message, the list of items it offers, and the preset an item attaches. Core defines the
//! `ItemPicker` trait and the registry; it names no host. Component 21 registers GitHub's, and until something is registered the
//! `#` stays text, as Fleet's picker does with the plugin off. Plain data and plain functions: no Rune, no I/O.
use super::feeds::Repository;
use super::request::GitHubPreset;
use crate::State;
use crate::theme::Theme;
use rune::{El, KeyEvent, Tx};
use std::sync::Arc;

/// A host's `#` popup. The host keeps its own list in its own state and asks its own server on its own threads; core only tells it
/// what is typed (`keep`), draws what it returns above the box (`view`) and hands it the keys while the popup is open (`key`).
/// Called from the UI thread, so none of them waits for anything.
pub trait ItemPicker: Send + Sync {
    /// Whether the host is on (a plugin that is switched off adds nothing, and the `#` stays text).
    fn on(&self, s: &State) -> bool;
    /// Every tick: the `#query` the caret is typing, or `None` while the popup is shut. True when something drawn changed.
    fn keep(&self, s: &mut State, open: Option<&HashTrigger>, now: f64, tx: &mut Tx) -> bool;
    /// A key while the popup is open, before the box sees it. True when the popup took it.
    fn key(&self, s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool;
    /// The popup, 8 px above the box's frame and as wide as it; `None` while the host has nothing to draw.
    fn view(&self, s: &State, t: &Theme) -> Option<El>;
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

    /// The first registered host that is on.
    pub fn active(&self, s: &State) -> Option<Arc<dyn ItemPicker>> {
        self.0.iter().find(|p| p.on(s)).cloned()
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
