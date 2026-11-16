//! Cloning a repository, as plain data (`lib/folder-access.ts` `cloneRepository`; `NSD/FolderPicker.vue`'s clone view logic:
//! `cloneSuggestions`, `cloneParentPath`, `cloneTarget`, `cloneDescription`, `canSubmitClone`; rows B2.161-B2.169) and the
//! `RepositorySources` place the Clone view's suggestions come from. No Rune and no I/O of its own: `follow` reads whatever
//! `Read` it is given, so a test feeds it chunks.
use super::feeds::{MakeFailure, NewFolder, folder_from_value};
use super::folder::{CloneSource, join_path, parse_clone_source};
use crate::fleet::machines::read_error;
use std::io::Read;
use std::sync::Arc;

/// How far a clone has got, in git's words (`CloneProgress`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloneProgress {
    pub phase: String,
    pub percent: u8,
}

const FAILED: &str = "Couldn't clone that repository.";
const STOPPED: &str = "The clone stopped before it finished.";

/// What one line of the server's stream says (`CloneLine`).
#[derive(Debug, PartialEq, Eq)]
enum Line {
    Progress(CloneProgress),
    Done(NewFolder),
    Failed(String),
}

fn parse_line(line: &str) -> Option<Line> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    match v.get("type")?.as_str()? {
        "progress" => Some(Line::Progress(CloneProgress {
            phase: v.get("phase")?.as_str()?.to_string(),
            percent: v.get("percent").and_then(serde_json::Value::as_f64).unwrap_or(0.0).clamp(0.0, 100.0) as u8,
        })),
        "done" => Some(Line::Done(folder_from_value(v.get("folder")?))),
        _ => Some(Line::Failed(v.get("error").and_then(serde_json::Value::as_str).unwrap_or(FAILED).to_string())),
    }
}

/// Bytes in, whole lines out: a line that arrives in two chunks is held until its newline comes.
#[derive(Default)]
pub struct Lines {
    held: Vec<u8>,
}

impl Lines {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.held.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(at) = self.held.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.held.drain(..=at).collect();
            out.push(String::from_utf8_lossy(&line[..line.len() - 1]).into_owned());
        }
        out
    }
}

/// `cloneRepository` from the answer on: a refusal says why (a 409 names the folder that was there), a good answer is a
/// stream of lines that ends in `done` or `error`. `on_progress` is called for each progress line.
pub fn follow(status: u16, mut body: impl Read, path: &str, mut on_progress: impl FnMut(CloneProgress)) -> Result<NewFolder, MakeFailure> {
    let fail = |message: String| MakeFailure { message, existing: None };
    if !(200..300).contains(&status) {
        let mut text = String::new();
        let _ = body.read_to_string(&mut text);
        let existing = (status == 409).then(|| path.to_string());
        return Err(MakeFailure { message: read_error(&text, FAILED.into()), existing });
    }
    let (mut lines, mut chunk) = (Lines::default(), [0u8; 4096]);
    loop {
        let n = body.read(&mut chunk).map_err(|_| fail(STOPPED.into()))?;
        // At the end a line with no newline after it still counts (Fleet reads what is left too).
        let data: &[u8] = if n == 0 { b"\n" } else { &chunk[..n] };
        for line in lines.push(data).iter().filter(|l| !l.trim().is_empty()) {
            match parse_line(line) {
                Some(Line::Progress(p)) => on_progress(p),
                Some(Line::Done(folder)) => return Ok(folder),
                Some(Line::Failed(message)) => return Err(fail(message)),
                None => return Err(fail(FAILED.into())),
            }
        }
        if n == 0 {
            return Err(fail(STOPPED.into()));
        }
    }
}

// ─── the Clone view ─────────────────────────────────────────────────────────────

/// A repository a source offers (`RepositorySource.useRepositories()`'s items).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteRepository {
    pub id: String,
    /// `owner/name`.
    pub full_name: String,
    pub name: String,
}

/// A place the Clone view's suggestions come from (`getRepositorySources`). Core defines it and knows no host: component 21
/// registers GitHub's. With none registered the view suggests nothing.
pub trait RepositorySource: Send + Sync {
    /// The glyph beside each suggestion, by name (the view knows the icons).
    fn icon(&self) -> &'static str;
    /// Asks for the list again; called each time the Clone view opens. Must not block.
    fn refresh(&self);
    /// The repositories known now.
    fn repositories(&self) -> Vec<RemoteRepository>;
}

/// The sources registered at start-up, read once when the folder menu is set up.
#[derive(Clone, Default)]
pub struct RepositorySources(Vec<Arc<dyn RepositorySource>>);

impl RepositorySources {
    pub fn register(&mut self, source: Arc<dyn RepositorySource>) {
        self.0.push(source);
    }

    pub fn refresh(&self) {
        self.0.iter().for_each(|s| s.refresh());
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One suggestion: what to write in the box and the glyph to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub full_name: String,
    pub icon: &'static str,
}

/// `cloneSuggestions`: up to five repositories not already here (by name, ignoring case) whose `owner/name` holds what is typed;
/// none once the text already reads as a repository.
pub fn suggestions(sources: &RepositorySources, local_names: &[String], typed: &str) -> Vec<Suggestion> {
    if parse_clone_source(typed).is_some() {
        return Vec::new();
    }
    let local: Vec<String> = local_names.iter().map(|n| n.to_lowercase()).collect();
    let needle = typed.trim().to_lowercase();
    sources
        .0
        .iter()
        .flat_map(|s| s.repositories().into_iter().map(|r| Suggestion { full_name: r.full_name, icon: s.icon() }).collect::<Vec<_>>())
        .filter(|r| !local.contains(&r.full_name.rsplit('/').next().unwrap_or("").to_lowercase()))
        .filter(|r| needle.is_empty() || r.full_name.to_lowercase().contains(&needle))
        .take(5)
        .collect()
}

/// Where the clone goes and what the preview says, from the three fields (`cloneSource`, `cloneParentPath`, `cloneTarget`).
#[derive(Debug, PartialEq, Eq)]
pub struct ClonePlan {
    pub source: Option<CloneSource>,
    /// The location picked, or the parent typed for "Another folder…".
    pub parent: String,
    pub target: Option<String>,
    /// "Another folder…": the clone is outside the locations, so Fleet adds it.
    pub other: bool,
}

/// `location` is a location root, or `None` for "Another folder…".
pub fn plan(text: &str, location: Option<&str>, parent_typed: &str) -> ClonePlan {
    let source = parse_clone_source(text);
    let parent = location.map_or_else(|| parent_typed.trim().to_string(), str::to_string);
    let target = source.as_ref().filter(|_| !parent.is_empty()).map(|s| join_path(&parent, &s.name));
    ClonePlan { source, parent, target, other: location.is_none() }
}

impl ClonePlan {
    /// The preview's second line (`cloneDescription`) when no clone runs.
    pub fn description(&self) -> String {
        let what = self.source.as_ref().map_or("A clone".to_string(), |s| format!("A clone of {}", s.label));
        if self.other { format!("{what}, added to your locations") } else { what }
    }
}
