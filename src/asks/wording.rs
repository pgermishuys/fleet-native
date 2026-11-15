//! How an ask is worded, in one place. Ports `lib/tools/permission-wording.ts` (`permissionHeading`,
//! `dontAskAgainWording`, `askPreviewWording`, `allowedRuleWording`) and the diff splitter of `PermissionCard.vue`.
use super::permission::PermissionAsk;
use crate::conv::tools::{Category, PermissionKind, get_tool};

/// "Run a command", "Edit a file", …: what the agent wants, as the card's heading (rows 3.3, 4.1).
pub fn permission_heading(ask: &PermissionAsk) -> String {
    if let Some(handoff) = &ask.handoff {
        return ask.title.clone().unwrap_or_else(|| format!("Send work to {}", handoff.machine));
    }
    match ask.kind {
        PermissionKind::Shell => "Run a command".into(),
        PermissionKind::Edit => "Edit a file".into(),
        // The server calls todowrite, task, skill and fleet_* asks reads too; those keep the tool's own name.
        PermissionKind::Read => match get_tool(&ask.tool).category {
            Category::Plan | Category::Subagent | Category::Skill | Category::Fleet | Category::Question => format!("Use {}", ask.tool),
            _ => "Read a file".into(),
        },
        PermissionKind::Web => "Go online".into(),
        PermissionKind::Other if ask.tool == "external_directory" => "Work outside the folder".into(),
        PermissionKind::Other => format!("Use {}", ask.tool),
    }
}

/// What "Don't ask again" covers: words, then the code after them (row 3.9).
#[derive(Debug, PartialEq, Eq)]
pub struct DontAsk {
    pub lead: String,
    pub code: Option<String>,
}

impl DontAsk {
    fn new(lead: &str, code: Option<&str>) -> DontAsk {
        DontAsk { lead: lead.into(), code: code.map(str::to_string) }
    }
}

/// The harness's patterns, or the whole kind of thing when there are none.
pub fn dont_ask_again_wording(ask: &PermissionAsk) -> DontAsk {
    // Fleet's ask about another machine: "always" saves that machine's setting.
    if let Some(handoff) = &ask.handoff {
        return DontAsk { lead: format!("Always for {}", handoff.machine), code: None };
    }
    let patterns: Vec<&str> = ask.always.iter().map(String::as_str).filter(|p| *p != "*").collect();
    if !patterns.is_empty() {
        return DontAsk::new("Don't ask again for", Some(&patterns.join(", ")));
    }
    match ask.kind {
        PermissionKind::Edit => DontAsk::new("Don't ask again for file edits", None),
        PermissionKind::Shell => DontAsk::new("Don't ask again for commands", None),
        PermissionKind::Web => DontAsk::new("Don't ask again for web access", None),
        _ => DontAsk::new("Don't ask again for", Some(&ask.tool)),
    }
}

/// The one-line preview of a pending ask, "Wants to run `git push`". The phone's inbox and mission control use it
/// (row 3.18); the desktop card does not.
#[derive(Debug, PartialEq, Eq)]
pub struct Preview {
    pub lead: String,
    pub detail: Option<String>,
    pub code: bool,
}

pub fn ask_preview_wording(ask: &PermissionAsk) -> Preview {
    if let Some(handoff) = &ask.handoff {
        return Preview { lead: format!("Wants to send work to {}", handoff.machine), detail: handoff.task.clone(), code: false };
    }
    let what = ask.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| ask.tool.clone());
    let with = |lead: &str| Preview { lead: lead.into(), detail: Some(what.clone()), code: true };
    match ask.kind {
        PermissionKind::Shell => with("Wants to run"),
        PermissionKind::Edit => with("Wants to edit"),
        PermissionKind::Read => with("Wants to read"),
        PermissionKind::Web => with("Wants to open"),
        PermissionKind::Other => Preview { lead: format!("Wants to use {}", ask.tool), detail: ask.title.clone(), code: ask.title.is_some() },
    }
}

/// One row of the Allowed list in plain words: "Run `bun test*` without asking" (component 4, piece E draws it).
/// `broad` is set when the rule covers every use of the kind of thing, which the list tints as a warning.
#[derive(Debug, PartialEq, Eq)]
pub struct RuleWording {
    pub lead: String,
    pub code: Option<String>,
    pub tail: &'static str,
    pub broad: bool,
}

pub fn allowed_rule_wording(kind: PermissionKind, tool: &str, patterns: &[&str]) -> RuleWording {
    let patterns: Vec<&str> = patterns.iter().copied().filter(|p| *p != "*").collect();
    let broad = patterns.is_empty();
    let code = (!broad).then(|| patterns.join(", "));
    let tail = " without asking";
    let (lead, code) = match kind {
        PermissionKind::Shell => (if broad { "Run any command" } else { "Run" }, code),
        PermissionKind::Edit => (if broad { "Edit files" } else { "Edit" }, code),
        PermissionKind::Web => (if broad { "Go online" } else { "Go online:" }, code),
        // Another tool: named, with the patterns when there are any.
        _ => ("Use", Some(if broad { tool.to_string() } else { format!("{tool} on {}", patterns.join(", ")) })),
    };
    RuleWording { lead: lead.into(), code, tail, broad }
}

/// How a diff line is tinted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Add,
    Del,
    Hunk,
    Context,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub text: String,
    pub kind: LineKind,
}

/// The file headers a unified diff starts with: `Index: `, `===…`, `--- ` and `+++ `.
fn is_header(line: &str) -> bool {
    line.starts_with("Index: ") || line.starts_with("===") || line.starts_with("--- ") || line.starts_with("+++ ")
}

/// A diff's lines, with the file headers left out (rows 3.6, 3.7).
pub fn diff_lines(detail: Option<&str>) -> Vec<DiffLine> {
    let Some(detail) = detail else { return Vec::new() };
    detail
        .split('\n')
        .filter(|line| !is_header(line))
        .map(|line| DiffLine {
            text: line.to_string(),
            kind: if line.starts_with("@@") {
                LineKind::Hunk
            } else if line.starts_with('+') {
                LineKind::Add
            } else if line.starts_with('-') {
                LineKind::Del
            } else {
                LineKind::Context
            },
        })
        .collect()
}

/// The added and removed line counts of a diff; a write that leaves the file as it is has none (row 3.7).
pub fn counts(lines: &[DiffLine]) -> (usize, usize) {
    let n = |kind| lines.iter().filter(|l| l.kind == kind).count();
    (n(LineKind::Add), n(LineKind::Del))
}

/// An OS name as the card says it (`osName`): Linux, macOS, Windows, or as sent.
pub fn os_name(os: &str) -> String {
    match os.to_lowercase().as_str() {
        "linux" => "Linux".into(),
        "macos" => "macOS".into(),
        "windows" => "Windows".into(),
        _ => os.to_string(),
    }
}

/// `PERMISSION_LEVEL_NAMES` through `toPermissionLevel` (row 4.3): the level a hand-off runs under there.
pub fn level_name(level: Option<&str>) -> &'static str {
    match level {
        Some("ask") => "Ask before changes",
        Some("edits") => "Allow edits",
        Some("all") => "Allow everything",
        _ => "unknown (that Fleet doesn't say)",
    }
}

/// The rows of what goes to the other machine, as the ask has them (row 4.2).
pub fn handoff_rows(handoff: &super::permission::HandoffAsk) -> Vec<(&'static str, String)> {
    let mut rows = vec![("Machine", match &handoff.os {
        Some(os) => format!("{} · {}", handoff.machine, os_name(os)),
        None => handoff.machine.clone(),
    })];
    if let Some(session) = &handoff.session {
        rows.push(("Session", session.clone()));
    }
    if let Some(folder) = &handoff.folder {
        rows.push(("Folder", folder.clone()));
        rows.push(("Branch", handoff.branch.clone().unwrap_or_else(|| "the folder as it is".into())));
        rows.push(("Harness", handoff.harness.clone().unwrap_or_else(|| "its default".into())));
    }
    if let Some(task) = &handoff.task {
        rows.push((if handoff.session.is_some() { "Message" } else { "Task" }, task.clone()));
    }
    rows
}
