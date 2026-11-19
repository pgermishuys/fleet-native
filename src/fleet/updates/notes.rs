//! A release's notes the way What's new shows them (`lib/release-notes.ts`): GitHub's generated list split into what's new and
//! what's fixed, behind-the-scenes changes counted, the hand-written intro kept, and the summary the "What's new" card says
//! after an update. Pure; the page that draws them is piece X's.
use super::{compare_versions, release_notes_url};
use serde_json::Value;
use std::cmp::Ordering;

/// One Fleet release, as `GET /api/update/releases` lists it (newest first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseNote {
    pub version: String,
    pub published_at: Option<String>,
    pub body: String,
    pub url: String,
}

impl ReleaseNote {
    pub fn from_json(v: &Value) -> Option<ReleaseNote> {
        let version = v.get("version").and_then(Value::as_str)?.to_string();
        let url = v.get("url").and_then(Value::as_str).map_or_else(|| release_notes_url(&version), str::to_string);
        Some(ReleaseNote {
            published_at: v.get("publishedAt").and_then(Value::as_str).map(str::to_string),
            body: v.get("body").and_then(Value::as_str).unwrap_or_default().to_string(),
            version,
            url,
        })
    }
}

/// What `GET /api/update/releases` answers: `{releases, fetchedAt, error}`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Releases {
    pub releases: Vec<ReleaseNote>,
    pub fetched_at: Option<String>,
    pub error: Option<String>,
}

impl Releases {
    pub fn from_json(v: &Value) -> Option<Releases> {
        let releases = v.get("releases")?.as_array()?.iter().filter_map(ReleaseNote::from_json).collect();
        Some(Releases {
            releases,
            fetched_at: v.get("fetchedAt").and_then(Value::as_str).map(str::to_string),
            error: v.get("error").and_then(Value::as_str).map(str::to_string),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pr {
    pub number: u64,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseChange {
    pub title: String,
    /// The part of Fleet it's about, from the conventional-commit scope ("Claude Code" from `fix(claude-code):`).
    pub scope: Option<String>,
    /// The pull request, when the line names one.
    pub pr: Option<Pr>,
}

/// A release's notes split the way What's new shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedReleaseNotes {
    /// What's written above the change list: highlights, known issues. Markdown.
    pub intro: String,
    pub added: Vec<ReleaseChange>,
    pub fixed: Vec<ReleaseChange>,
    /// How many changes were left out as behind the scenes: tests, refactors, docs, CI.
    pub internal: usize,
}

/// How many versions older than the installed one What's new keeps, folded.
pub const OLDER_VERSIONS_SHOWN: usize = 5;
/// How many lines the What's new card lists.
pub const WHATS_NEW_ITEMS: usize = 3;

// Changes nobody using Fleet would notice. They're counted, not listed.
const INTERNAL_TYPES: [&str; 11] = ["test", "tests", "refactor", "docs", "doc", "chore", "ci", "build", "style", "prototype", "wip"];
// Scopes that name a layer of Fleet rather than something you'd recognise in it, so they get no label.
const UNLABELLED_SCOPES: [&str; 6] = ["client", "api", "server", "ui", "release", "app"];
const SCOPE_LABELS: [(&str, &str); 8] = [
    ("claude-code", "Claude Code"),
    ("opencode", "OpenCode"),
    ("opencode2", "OpenCode 2"),
    ("github", "GitHub"),
    ("signalr", "SignalR"),
    ("pwa", "PWA"),
    ("mcp", "MCP"),
    ("btw", "/btw"),
];

/// GitHub's generated line: "* <title> by @<user> in https://github.com/<owner>/<repo>/pull/<n>" (`CHANGE_LINE`).
fn change_line(line: &str) -> Option<(String, Pr)> {
    let rest = line.trim_start().strip_prefix(['*', '-'])?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    let (before, url) = rest.rsplit_once(char::is_whitespace)?;
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let (owner, repo, pull, number) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let name = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if !name(owner)
        || !name(repo)
        || pull != "pull"
        || parts.next().is_some()
        || number.is_empty()
        || !number.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let before = before.trim_end().strip_suffix("in")?;
    if !before.ends_with(char::is_whitespace) {
        return None;
    }
    let title = strip_by(before.trim_end());
    if title.is_empty() {
        return None;
    }
    Some((title.to_string(), Pr { number: number.parse().ok()?, url: url.to_string() }))
}

/// Drops a trailing " by @user" (or "@user[bot]").
fn strip_by(title: &str) -> &str {
    let Some((head, user)) = title.rsplit_once(char::is_whitespace) else { return title };
    let Some(user) = user.strip_prefix('@') else { return title };
    let user = user.strip_suffix("[bot]").unwrap_or(user);
    let Some(head) = head.trim_end().strip_suffix("by") else { return title };
    if user.is_empty() || !user.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-')) || !head.ends_with(char::is_whitespace) {
        return title;
    }
    head.trim_end()
}

/// `^(\w+)(?:\(([^)]*)\))?!?:\s*`: the type, the scope, and how much of the line they take.
fn conventional_prefix(raw: &str) -> Option<(String, Option<String>, usize)> {
    let word = raw.chars().take_while(|c| c.is_alphanumeric() || *c == '_').count();
    if word == 0 {
        return None;
    }
    let ty = raw[..raw.char_indices().nth(word).map_or(raw.len(), |(i, _)| i)].to_string();
    let mut at = ty.len();
    let mut scope = None;
    if raw[at..].starts_with('(') {
        let close = raw[at..].find(')')?;
        scope = Some(raw[at + 1..at + close].to_string());
        at += close + 1;
    }
    if raw[at..].starts_with('!') {
        at += 1;
    }
    raw[at..].starts_with(':').then(|| {
        at += 1;
        at += raw[at..].len() - raw[at..].trim_start().len();
        (ty, scope, at)
    })
}

/// "Claude Code" from `claude-code`, "Status bar" from `status-bar`; none for a scope that names a layer (`client`).
fn scope_label(scope: &str) -> Option<String> {
    let key = scope.trim().to_lowercase();
    if key.is_empty() || UNLABELLED_SCOPES.contains(&key.as_str()) || key.contains(',') {
        return None;
    }
    let label = SCOPE_LABELS.iter().find(|(k, _)| *k == key).map_or_else(
        || {
            let mut out = String::new();
            let mut gap = false;
            for c in key.chars() {
                if matches!(c, '-' | '_') {
                    gap = true;
                } else {
                    if gap {
                        out.push(' ');
                        gap = false;
                    }
                    out.push(c);
                }
            }
            if gap {
                out.push(' ');
            }
            out
        },
        |(_, l)| (*l).to_string(),
    );
    Some(capitalise(&label))
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

/// "Plan pages" from "plan pages"; a title that opens with an identifier (fleet_page_show) keeps its case.
fn sentence_case(title: &str) -> String {
    let first = title.split_whitespace().next().unwrap_or("");
    if first.contains(['_', '.', '/']) { title.to_string() } else { capitalise(title) }
}

#[derive(PartialEq)]
enum Section {
    Intro,
    Changes,
    Other,
}

/// `^#{1,6}\s+(.*)$`: the text of a heading line.
fn heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    let rest = &line[hashes..];
    ((1..=6).contains(&hashes) && rest.starts_with(char::is_whitespace)).then(|| rest.trim_start())
}

/// Splits GitHub's release notes into what was written by hand (above "What's Changed") and the pull requests, grouped into
/// new and fixed by their conventional-commit prefix, which is dropped; its scope becomes a label (`parseReleaseNotes`).
pub fn parse_release_notes(body: &str) -> ParsedReleaseNotes {
    let mut notes = ParsedReleaseNotes::default();
    let mut intro: Vec<&str> = Vec::new();
    let mut section = Section::Intro;
    let text = body.replace("\r\n", "\n");
    for line in text.lines() {
        if line.trim().starts_with("**Full Changelog**") {
            continue;
        }
        if let Some(text) = heading(line) {
            let lower = text.to_lowercase();
            if lower.contains("what's changed") || lower.contains("whats changed") {
                section = Section::Changes;
                continue;
            }
            // A heading after the change list (New Contributors) ends it; one before it is part of the intro.
            if section == Section::Changes {
                section = Section::Other;
                continue;
            }
        }
        match section {
            Section::Intro => intro.push(line),
            Section::Other => {}
            Section::Changes if line.trim().is_empty() => {}
            Section::Changes => {
                let found = change_line(line);
                let raw = found.as_ref().map_or_else(|| strip_bullet(line), |(t, _)| t.clone());
                let prefix = conventional_prefix(&raw);
                let ty = prefix.as_ref().map(|(t, ..)| t.to_lowercase());
                if ty.as_deref().is_some_and(|t| INTERNAL_TYPES.contains(&t)) {
                    notes.internal += 1;
                    continue;
                }
                let title = sentence_case(prefix.as_ref().map_or(raw.as_str(), |(.., end)| &raw[*end..]));
                let scope = prefix.as_ref().and_then(|(_, s, _)| s.as_deref()).and_then(scope_label);
                let entry = ReleaseChange { title, scope, pr: found.map(|(_, pr)| pr) };
                if ty.as_deref() == Some("fix") { notes.fixed.push(entry) } else { notes.added.push(entry) }
            }
        }
    }
    notes.intro = intro.join("\n").trim().to_string();
    notes
}

fn strip_bullet(line: &str) -> String {
    let t = line.trim_start();
    match t.strip_prefix(['*', '-']) {
        Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim_start().to_string(),
        _ => line.to_string(),
    }
}

/// "3 new, 4 fixed", for a link to the notes; empty when there's nothing to count (`changeSummary`).
pub fn change_summary(notes: &ParsedReleaseNotes) -> String {
    [(notes.added.len(), "new"), (notes.fixed.len(), "fixed")]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| format!("{n} {w}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first thing written by hand above the change list, as plain text: a heading or a sentence. A known issue isn't a
/// highlight, so it's skipped (`introHighlight`).
pub fn intro_highlight(intro: &str) -> Option<String> {
    intro.lines().map(plain_line).find(|t| !t.is_empty() && !t.to_lowercase().starts_with("known issue"))
}

fn plain_line(line: &str) -> String {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    let t = if (1..=6).contains(&hashes) && t[hashes..].starts_with(char::is_whitespace) {
        t[hashes..].trim_start()
    } else if let Some(rest) = t.strip_prefix(['*', '-']).filter(|r| r.starts_with(char::is_whitespace)) {
        rest.trim_start()
    } else {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        match t[digits..].strip_prefix('.') {
            Some(rest) if digits > 0 && rest.starts_with(char::is_whitespace) => rest.trim_start(),
            _ => t,
        }
    };
    // [text](url) keeps its text; then the emphasis and code marks go.
    let mut out = String::new();
    let mut rest = t;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        match after.find("](").and_then(|mid| after[mid + 2..].find(')').map(|end| (mid, end))) {
            Some((mid, end)) if !after[..mid].contains(']') => {
                out.push_str(&rest[..open]);
                out.push_str(&after[..mid]);
                rest = &after[mid + 2 + end + 1..];
            }
            _ => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.retain(|c| !matches!(c, '*' | '_' | '`'));
    out.trim().to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Highlight,
    New,
    Fixed,
}

/// One line of the What's new card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhatsNewItem {
    pub kind: ItemKind,
    pub text: String,
    pub scope: Option<String>,
}

/// What the What's new card says after an update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhatsNewSummary {
    /// Every release newer than the one Fleet ran before, up to the one it runs now, newest first.
    pub releases: Vec<ReleaseNote>,
    pub items: Vec<WhatsNewItem>,
    /// Changes in those releases the card leaves out.
    pub more: usize,
    /// Whether everything left out is a fix.
    pub more_are_fixes: bool,
}

/// What's new between the version Fleet ran before and the one it runs now: what was written by hand first, then what's new,
/// then what's fixed, newest release first. None when the notes for the running version aren't there (offline, or not
/// published yet) or have nothing worth a card, so the caller can say less (`whatsNewSummary`).
pub fn whats_new_summary(releases: &[ReleaseNote], previous: &str, current: &str, max: usize) -> Option<WhatsNewSummary> {
    releases.iter().any(|r| compare_versions(&r.version, current) == Ordering::Equal).then_some(())?;
    let covered: Vec<ReleaseNote> = releases
        .iter()
        .filter(|r| {
            compare_versions(&r.version, previous) == Ordering::Greater && compare_versions(&r.version, current) != Ordering::Greater
        })
        .cloned()
        .collect();
    let parsed: Vec<ParsedReleaseNotes> = covered.iter().map(|r| parse_release_notes(&r.body)).collect();
    let highlights =
        parsed.iter().filter_map(|n| intro_highlight(&n.intro)).map(|text| WhatsNewItem { kind: ItemKind::Highlight, text, scope: None });
    let item = |kind: ItemKind| move |c: &ReleaseChange| WhatsNewItem { kind, text: c.title.clone(), scope: c.scope.clone() };
    let added: Vec<WhatsNewItem> = parsed.iter().flat_map(|n| n.added.iter().map(item(ItemKind::New))).collect();
    let fixed: Vec<WhatsNewItem> = parsed.iter().flat_map(|n| n.fixed.iter().map(item(ItemKind::Fixed))).collect();
    let items: Vec<WhatsNewItem> = highlights.chain(added.iter().cloned()).chain(fixed.iter().cloned()).take(max).collect();
    if items.is_empty() {
        return None;
    }
    let shown = |k: ItemKind| items.iter().filter(|i| i.kind == k).count();
    let (shown_added, shown_fixed) = (shown(ItemKind::New), shown(ItemKind::Fixed));
    Some(WhatsNewSummary {
        releases: covered,
        more: added.len() - shown_added + (fixed.len() - shown_fixed),
        more_are_fixes: added.len() == shown_added,
        items,
    })
}

/// The hand-written intro as Markdown for the page, its `#` and `##` headings pushed down to `###` so they sit under the version's
/// own row.
pub fn intro_markdown(notes: &ParsedReleaseNotes) -> String {
    // Headings in the intro sit under the version's own row, so they start a level down.
    let intro: Vec<String> = notes
        .intro
        .split('\n')
        .map(|line| {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            match line[hashes..].chars().next() {
                Some(c) if (1..=2).contains(&hashes) && c.is_whitespace() => format!("### {}", &line[hashes + c.len_utf8()..]),
                _ => line.to_string(),
            }
        })
        .collect();
    intro.join("\n")
}

/// The notes as Markdown for Fleet's renderer: the intro, then New and Fixed lists with their pull requests
/// (`releaseNotesMarkdown`).
pub fn release_notes_markdown(notes: &ParsedReleaseNotes) -> String {
    let list = |label: &str, changes: &[ReleaseChange]| -> String {
        if changes.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = changes
            .iter()
            .map(|c| {
                let scope = c.scope.as_ref().map_or(String::new(), |s| format!("*{s}* "));
                let pr = c.pr.as_ref().map_or(String::new(), |p| format!(" [#{}]({})", p.number, p.url));
                format!("- {scope}{}{pr}", c.title)
            })
            .collect();
        format!("#### {label}\n\n{}\n", lines.join("\n"))
    };
    let intro = intro_markdown(notes);
    [intro, list("New", &notes.added), list("Fixed", &notes.fixed)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhatsNewRange {
    pub open: Vec<ReleaseNote>,
    pub folded: Vec<ReleaseNote>,
}

/// Which releases What's new shows: every version newer than the installed one and the installed one itself, open, then up to
/// five older ones, folded. When none is that new, the newest is open (`whatsNewRange`).
pub fn whats_new_range(releases: &[ReleaseNote], installed: Option<&str>) -> WhatsNewRange {
    let installed = installed.filter(|i| !i.is_empty());
    let (open, older): (Vec<_>, Vec<_>) = match installed {
        Some(i) => releases.iter().cloned().partition(|r| compare_versions(&r.version, i) != Ordering::Less),
        None => (Vec::new(), releases.to_vec()),
    };
    if open.is_empty() {
        // Nothing at or above the installed version (one built from source, or not known): the newest is open.
        return WhatsNewRange {
            open: older.iter().take(1).cloned().collect(),
            folded: older.iter().skip(1).take(OLDER_VERSIONS_SHOWN).cloned().collect(),
        };
    }
    WhatsNewRange { open, folded: older.into_iter().take(OLDER_VERSIONS_SHOWN).collect() }
}
