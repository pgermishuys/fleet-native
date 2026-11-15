//! Diffs as plain logic with no Rune in it: the line list a card draws, from the three places a harness can put one
//! (`lib/diff-parser.ts`, `cardDiffLines` in `activity-stream-tool-card.ts`, `fileEditsOf` in `lib/turns.ts`), and the
//! rows `DiffView.vue` makes of it (context runs collapsed past 10 lines, 3 kept at each edge).
use super::tools::{FileWrite, as_record, get_tool, parse_tool_input};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Add,
    Remove,
    Context,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: Kind,
    pub content: String,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

impl DiffLine {
    fn context(content: &str, old: u32, new: u32) -> Self {
        DiffLine { kind: Kind::Context, content: content.to_string(), old: Some(old), new: Some(new) }
    }
    fn remove(content: &str, old: u32) -> Self {
        DiffLine { kind: Kind::Remove, content: content.to_string(), old: Some(old), new: None }
    }
    fn add(content: &str, new: u32) -> Self {
        DiffLine { kind: Kind::Add, content: content.to_string(), old: None, new: Some(new) }
    }
}

/// How many lines a diff added and removed.
pub fn count_changes(lines: &[DiffLine]) -> (u32, u32) {
    let n = |k: Kind| lines.iter().filter(|l| l.kind == k).count() as u32;
    (n(Kind::Add), n(Kind::Remove))
}

// ── Two texts to lines ──────────────────────────────────────────────────────────────────────────────────────────────

/// Lines of a text: CRLF and CR read as line ends, and the empty line a final line end leaves is dropped.
fn split_content_lines(content: &str) -> Vec<String> {
    if content.is_empty() {
        return Vec::new();
    }
    let normal = content.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = normal.split('\n').map(str::to_string).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// The lines that turn `before` into `after`, with every line numbered on the side(s) it is on (`parseDiffLines`).
pub fn parse_diff_lines(before: &str, after: &str) -> Vec<DiffLine> {
    let a = split_content_lines(before);
    let b = split_content_lines(after);
    let (mut i, mut j) = (0usize, 0usize);
    let mut out = Vec::new();
    for step in diff_script(&a, &b) {
        match step {
            0 => {
                out.push(DiffLine::context(&a[i], i as u32 + 1, j as u32 + 1));
                i += 1;
                j += 1;
            }
            -1 => {
                out.push(DiffLine::remove(&a[i], i as u32 + 1));
                i += 1;
            }
            _ => {
                out.push(DiffLine::add(&b[j], j as u32 + 1));
                j += 1;
            }
        }
    }
    out
}

/// The edit script between two line lists: 0 keeps a line, -1 removes one from `a`, 1 adds one from `b`. Lines the
/// two share at the start and end are taken first, then Myers' O(ND) walk finds the shortest script for the middle.
fn diff_script(a: &[String], b: &[String]) -> Vec<i8> {
    let mut head = 0;
    while head < a.len() && head < b.len() && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < a.len() - head && tail < b.len() - head && a[a.len() - 1 - tail] == b[b.len() - 1 - tail] {
        tail += 1;
    }
    let mut script: Vec<i8> = vec![0; head];
    // Lines become numbers so the walk compares integers, not strings.
    let mut ids: HashMap<&str, i32> = HashMap::new();
    fn ids_of<'a>(ids: &mut HashMap<&'a str, i32>, lines: &'a [String]) -> Vec<i32> {
        lines
            .iter()
            .map(|line| {
                let next = ids.len() as i32;
                *ids.entry(line.as_str()).or_insert(next)
            })
            .collect()
    }
    let mid_a = ids_of(&mut ids, &a[head..a.len() - tail]);
    let mid_b = ids_of(&mut ids, &b[head..b.len() - tail]);

    // A line only one side has can only be a removal or an addition, so the walk skips it.
    let in_b: HashSet<i32> = mid_b.iter().copied().collect();
    let in_a: HashSet<i32> = mid_a.iter().copied().collect();
    let kept_a: Vec<usize> = (0..mid_a.len()).filter(|&i| in_b.contains(&mid_a[i])).collect();
    let kept_b: Vec<usize> = (0..mid_b.len()).filter(|&i| in_a.contains(&mid_b[i])).collect();
    let steps = myers(&kept_a.iter().map(|&i| mid_a[i]).collect::<Vec<_>>(), &kept_b.iter().map(|&i| mid_b[i]).collect::<Vec<_>>());

    let (mut pa, mut pb, mut i, mut j) = (0usize, 0usize, 0usize, 0usize);
    let removes_up_to = |script: &mut Vec<i8>, pa: &mut usize, end: usize| {
        while *pa < end {
            script.push(-1);
            *pa += 1;
        }
    };
    let adds_up_to = |script: &mut Vec<i8>, pb: &mut usize, end: usize| {
        while *pb < end {
            script.push(1);
            *pb += 1;
        }
    };
    for step in steps {
        match step {
            0 => {
                removes_up_to(&mut script, &mut pa, kept_a[i]);
                adds_up_to(&mut script, &mut pb, kept_b[j]);
                script.push(0);
                pa += 1;
                pb += 1;
                i += 1;
                j += 1;
            }
            -1 => {
                removes_up_to(&mut script, &mut pa, kept_a[i] + 1);
                i += 1;
            }
            _ => {
                removes_up_to(&mut script, &mut pa, kept_a.get(i).copied().unwrap_or(mid_a.len()));
                adds_up_to(&mut script, &mut pb, kept_b[j] + 1);
                j += 1;
            }
        }
    }
    removes_up_to(&mut script, &mut pa, mid_a.len());
    adds_up_to(&mut script, &mut pb, mid_b.len());
    script.extend(std::iter::repeat_n(0, tail));
    script
}

/// Past this many changed lines the middle is shown as all removed then all added rather than searched for.
const MAX_EDIT_DISTANCE: usize = 6000;

fn myers(a: &[i32], b: &[i32]) -> Vec<i8> {
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return vec![1; m];
    }
    if m == 0 {
        return vec![-1; n];
    }
    let max = (n + m).min(MAX_EDIT_DISTANCE);
    let offset = max as isize + 1;
    let mut v = vec![0i32; 2 * max + 3];
    // trace[d] holds the furthest x on diagonals -d..d (index k + d) after round d.
    let mut trace: Vec<Vec<i32>> = Vec::new();
    let mut found: Option<usize> = None;
    let at = |v: &[i32], k: isize| v[(offset + k) as usize];
    let mut d = 0usize;
    while d <= max && found.is_none() {
        let di = d as isize;
        let mut k = -di;
        while k <= di {
            let mut x = if k == -di || (k != di && at(&v, k - 1) < at(&v, k + 1)) { at(&v, k + 1) } else { at(&v, k - 1) + 1 } as isize;
            let mut y = x - k;
            while (x as usize) < n && (y as usize) < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(offset + k) as usize] = x as i32;
            if x as usize >= n && y as usize >= m {
                found = Some(d);
                break;
            }
            k += 2;
        }
        trace.push(v[(offset - di) as usize..=(offset + di) as usize].to_vec());
        d += 1;
    }
    let Some(found) = found else {
        let mut all = vec![-1; n];
        all.extend(std::iter::repeat_n(1, m));
        return all;
    };
    let mut reversed: Vec<i8> = Vec::new();
    let (mut x, mut y) = (n as isize, m as isize);
    for d in (1..=found).rev() {
        let prev = &trace[d - 1]; // diagonals -(d-1)..(d-1), index k + d - 1
        let di = d as isize;
        let k = x - y;
        let get = |diag: isize| prev[(diag + di - 1) as usize] as isize;
        let down = k == -di || (k != di && get(k - 1) < get(k + 1));
        let prev_k = if down { k + 1 } else { k - 1 };
        let prev_x = get(prev_k);
        let prev_y = prev_x - prev_k;
        while x > (if down { prev_x } else { prev_x + 1 }) && y > (if down { prev_y + 1 } else { prev_y }) {
            reversed.push(0);
            x -= 1;
            y -= 1;
        }
        reversed.push(if down { 1 } else { -1 });
        if down {
            y -= 1;
        } else {
            x -= 1;
        }
    }
    while x > 0 && y > 0 {
        reversed.push(0);
        x -= 1;
        y -= 1;
    }
    reversed.reverse();
    reversed
}

// ── A unified diff to lines ─────────────────────────────────────────────────────────────────────────────────────────

/// Follows a unified diff's hunks by the line counts in each `@@ -a,b +c,d @@` header, so a hunk's own lines are never
/// mistaken for something else: a removed `-- x` arrives as `--- x` and an added `++ x` as `+++ x` (both look like
/// file headers), and with git's `diff.suppressBlankEmpty` a blank context line is an empty line, not a space.
#[derive(Default)]
pub struct HunkTracker {
    old_left: u32,
    new_left: u32,
}

impl HunkTracker {
    /// True while the hunk still owes lines: whatever comes next is that hunk's content, not a header.
    pub fn inside(&self) -> bool {
        self.old_left > 0 || self.new_left > 0
    }

    /// Reads a hunk header; returns its starting line numbers, or `None` when the line isn't one.
    pub fn start(&mut self, raw: &str) -> Option<(u32, u32)> {
        let rest = raw.strip_prefix("@@ -")?;
        let (old_start, rest) = number(rest)?;
        let (old_len, rest) = match rest.strip_prefix(',') {
            Some(r) => {
                let (n, r) = number(r)?;
                (Some(n), r)
            }
            None => (None, rest),
        };
        let rest = rest.strip_prefix(" +")?;
        let (new_start, rest) = number(rest)?;
        let (new_len, rest) = match rest.strip_prefix(',') {
            Some(r) => {
                let (n, r) = number(r)?;
                (Some(n), r)
            }
            None => (None, rest),
        };
        rest.strip_prefix(" @@")?;
        self.old_left = old_len.unwrap_or(1);
        self.new_left = new_len.unwrap_or(1);
        Some((old_start, new_start))
    }

    /// Counts a line of the hunk off; call it for every line while `inside`.
    pub fn consume(&mut self, kind: Kind) {
        if kind != Kind::Add {
            self.old_left = self.old_left.saturating_sub(1);
        }
        if kind != Kind::Remove {
            self.new_left = self.new_left.saturating_sub(1);
        }
    }

    /// A line ending the hunk early (a new file's `diff --git`): the counts were wrong, stop trusting them.
    pub fn reset(&mut self) {
        self.old_left = 0;
        self.new_left = 0;
    }
}

fn number(s: &str) -> Option<(u32, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (end > 0).then_some(())?;
    Some((s[..end].parse().ok()?, &s[end..]))
}

fn kind_of(raw: &str) -> Kind {
    if raw.starts_with('+') {
        Kind::Add
    } else if raw.starts_with('-') {
        Kind::Remove
    } else {
        Kind::Context
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub lines: Vec<DiffLine>,
    pub created: bool,
    pub paths: Vec<String>,
}

/// `s` without its first character when that is a one-byte prefix (`a/`).
fn skip1(raw: &str) -> &str {
    raw.get(1..).unwrap_or("")
}

/// Parses a unified diff (what most harnesses attach to an edit) into lines. Hunk headers give the line numbers;
/// `--- /dev/null` means the file is new.
pub fn parse_unified_diff(patch: &str) -> Parsed {
    let mut out = Parsed::default();
    let (mut old_header, mut old_line, mut new_line) = (String::new(), 0u32, 0u32);
    let mut hunks = HunkTracker::default();
    let rows: Vec<&str> = patch.split('\n').collect();
    for (index, raw) in rows.iter().copied().enumerate() {
        if raw.starts_with("diff ") {
            hunks.reset();
        }
        // The newline ending the patch isn't a blank context line, even when a harness's hunk header counts too many.
        if raw.is_empty() && index == rows.len() - 1 {
            continue;
        }
        if hunks.inside() && !raw.starts_with("\\ ") {
            let kind = kind_of(raw);
            hunks.consume(kind);
            push_line(&mut out.lines, kind, skip1(raw), &mut old_line, &mut new_line);
            continue;
        }
        if let Some((o, n)) = hunks.start(raw) {
            old_line = o;
            new_line = n;
            continue;
        }
        if let Some(rest) = raw.strip_prefix("--- ") {
            old_header = rest.trim().to_string();
            if old_header == "/dev/null" {
                out.created = true;
            }
            continue;
        }
        if let Some(rest) = raw.strip_prefix("+++ ") {
            let path = strip_patch_path_prefix(rest.trim(), &old_header);
            if !path.is_empty() && path != "/dev/null" {
                out.paths.push(path);
            }
            continue;
        }
        // OpenCode's apply_patch names its files in its own header.
        if let Some(rest) = raw.strip_prefix("*** ") {
            let named = [("Add File: ", true), ("Update File: ", false), ("Delete File: ", false)];
            if let Some((path, add)) = named.iter().find_map(|(p, add)| rest.strip_prefix(p).filter(|s| !s.is_empty()).map(|s| (s, *add))) {
                out.created |= add;
                out.paths.push(path.trim().to_string());
                continue;
            }
        }
        if raw.starts_with("diff ") || raw.starts_with("index ") || raw.starts_with("\\ ") {
            continue;
        }
        if raw.starts_with('+') || raw.starts_with('-') || raw.starts_with(' ') {
            push_line(&mut out.lines, kind_of(raw), skip1(raw), &mut old_line, &mut new_line);
        }
    }
    out
}

fn push_line(lines: &mut Vec<DiffLine>, kind: Kind, content: &str, old_line: &mut u32, new_line: &mut u32) {
    lines.push(match kind {
        Kind::Add => DiffLine::add(content, *new_line),
        Kind::Remove => DiffLine::remove(content, *old_line),
        Kind::Context => DiffLine::context(content, *old_line, *new_line),
    });
    if kind != Kind::Add {
        *old_line += 1;
    }
    if kind != Kind::Remove {
        *new_line += 1;
    }
}

/// `x/rest` when `x` is one of `set`.
fn prefixed<'a>(path: &'a str, set: &str) -> Option<&'a str> {
    let mut chars = path.chars();
    let first = chars.next()?;
    let rest = chars.as_str().strip_prefix('/')?;
    (set.contains(first) && !rest.is_empty()).then_some(rest)
}

/// The path a `+++` line names, without git's prefix. Git writes `a/` and `b/`, or with `diff.mnemonicPrefix` `c/`
/// (commit), `i/` (index), `w/` (worktree) and `o/` (object); `diff.noprefix` writes none. A prefix is only taken off
/// when the `---` line before it carries a matching one (or the file is new), so a real folder called `w` survives
/// noprefix.
fn strip_patch_path_prefix(path: &str, old_header: &str) -> String {
    let without_tab = path.split('\t').next().unwrap_or(path);
    let before = old_header.split('\t').next().unwrap_or("").trim();
    let Some(rest) = prefixed(without_tab, "bwio2") else { return without_tab.to_string() };
    let matches_old = prefixed(before, "acio1") == Some(rest);
    if before == "/dev/null" || before.is_empty() || matches_old { rest.to_string() } else { without_tab.to_string() }
}

// ── What the harness attached to a call ─────────────────────────────────────────────────────────────────────────────

fn str_of(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// An array of line entries: `{type, content|text|line, oldLineNumber, newLineNumber}`, the type read from a leading
/// `+` or `-` when absent (`normalizeDiffLines`). An entry without text or with another type is left out.
fn normalize_diff_lines(value: Option<&Value>) -> Vec<DiffLine> {
    let Some(Value::Array(entries)) = value else { return Vec::new() };
    entries
        .iter()
        .filter_map(|entry| {
            let record = as_record(entry);
            let get = |k: &str| record.and_then(|r| r.get(k));
            let content = str_of(get("content")).or_else(|| str_of(get("text"))).or_else(|| str_of(get("line")))?;
            let kind = match str_of(get("type")) {
                Some("add") => Kind::Add,
                Some("remove") => Kind::Remove,
                Some("context") => Kind::Context,
                Some(_) => return None,
                None => kind_of(content),
            };
            let n = |k: &str| get(k).and_then(Value::as_f64).map(|n| n as u32);
            Some(DiffLine { kind, content: content.to_string(), old: n("oldLineNumber"), new: n("newLineNumber") })
        })
        .collect()
}

/// The diff lines a harness attached to the call as an array; empty when it attached none (`toolDiffLines`).
pub fn tool_diff_lines(state: &Value) -> Vec<DiffLine> {
    ["diffLines", "diff", "patch"].iter().map(|k| normalize_diff_lines(state.get(k))).find(|l| !l.is_empty()).unwrap_or_default()
}

/// A unified diff kept on the call as text (`toolDiffText`).
pub fn tool_diff_text(state: &Value) -> Option<String> {
    let meta = |k: &str| state.pointer(&format!("/metadata/{k}"));
    [state.get("diff"), state.get("patch"), meta("diff"), meta("patch")].into_iter().find_map(|c| str_of(c).map(str::to_string))
}

/// What the card shows as the call's diff: the lines a harness attached, or else the unified diff it kept on the call
/// (`cardDiffLines`).
pub fn card_diff_lines(state: &Value) -> Vec<DiffLine> {
    let attached = tool_diff_lines(state);
    if !attached.is_empty() {
        return attached;
    }
    tool_diff_text(state).map(|t| parse_unified_diff(&t).lines).unwrap_or_default()
}

// ── What a write call changed ───────────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct FileEdit {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub created: bool,
    pub diff: Vec<DiffLine>,
    /// The call carried only the file's new contents, so `diff` is every line written as an addition.
    pub whole_file: bool,
}

fn lines_of(content: &str) -> Vec<&str> {
    let mut split: Vec<&str> = content.split('\n').collect();
    if split.last() == Some(&"") {
        split.pop();
    }
    split
}

fn edit(path: String, created: bool, diff: Vec<DiffLine>, whole_file: bool) -> FileEdit {
    let (additions, deletions) = count_changes(&diff);
    FileEdit { path, additions, deletions, created, diff, whole_file }
}

/// Every file a completed write call touched, with the lines it wrote to each (`fileEditsOf`). A call that failed or
/// hasn't finished wrote nothing.
pub fn file_edits_of(tool: &str, state: &Value) -> Vec<FileEdit> {
    let Some(kind) = get_tool(tool).file_write else { return Vec::new() };
    if state.get("status").and_then(Value::as_str).map(str::to_lowercase).as_deref() != Some("completed") {
        return Vec::new();
    }
    let input_value = state.get("input").unwrap_or(&Value::Null);
    let input = parse_tool_input(input_value);
    let declared = input.file_path.clone();

    // 1. Diff lines the harness sent as an array.
    let attached = tool_diff_lines(state);
    if let (false, Some(path)) = (attached.is_empty(), declared.clone()) {
        let (_, deletions) = count_changes(&attached);
        return vec![edit(path, kind == FileWrite::Create && deletions == 0, attached, false)];
    }

    // 2. A unified diff as text: one call can carry several files.
    let text = tool_diff_text(state).or_else(|| {
        let rec = as_record(input_value);
        ["patchText", "patch", "diff"].iter().find_map(|k| str_of(rec.and_then(|r| r.get(*k))).map(str::to_string))
    });
    if let Some(text) = text {
        let edits = split_patch_by_file(&text, declared.as_deref());
        if !edits.is_empty() {
            return edits;
        }
    }

    // 3. Nothing but the input: work the lines out from what was asked for.
    let Some(path) = declared else { return Vec::new() };
    let rec = as_record(input_value);
    let field = |keys: &[&str]| keys.iter().find_map(|k| rec.and_then(|r| r.get(*k)).and_then(Value::as_str)).map(str::to_string);
    let (new_string, old_string) = (field(&["newString", "new_string"]), field(&["oldString", "old_string"]));
    if new_string.is_some() || old_string.is_some() {
        let diff = parse_diff_lines(old_string.as_deref().unwrap_or(""), new_string.as_deref().unwrap_or(""))
            .into_iter()
            .filter(|l| l.kind != Kind::Context)
            .collect();
        return vec![edit(path, false, diff, false)];
    }
    if let Some(content) = field(&["content", "text", "newContent", "new_content"]) {
        let diff = lines_of(&content).iter().enumerate().map(|(i, l)| DiffLine::add(l, i as u32 + 1)).collect();
        return vec![edit(path, kind == FileWrite::Create, diff, true)];
    }
    // A write we can see happened but can't read: the file is still worth naming.
    vec![edit(path, false, Vec::new(), false)]
}

/// Splits a multi-file patch into one edit per file.
fn split_patch_by_file(patch: &str, declared: Option<&str>) -> Vec<FileEdit> {
    let mut sections: Vec<Vec<&str>> = Vec::new();
    let mut hunks = HunkTracker::default();
    for raw in patch.split('\n') {
        if raw.starts_with("diff ") {
            hunks.reset();
        }
        // Inside a hunk a removed `-- x` reads `--- x`: that's a line of the hunk, not the next file.
        let in_hunk = hunks.inside() && !raw.starts_with("\\ ");
        if in_hunk {
            hunks.consume(kind_of(raw));
        } else {
            hunks.start(raw);
        }
        let is_header = !in_hunk
            && (raw.starts_with("--- ")
                || ["Add", "Update", "Delete"]
                    .iter()
                    .any(|w| raw.strip_prefix("*** ").and_then(|r| r.strip_prefix(w)).is_some_and(|r| r.starts_with(" File: "))));
        if is_header && sections.last().is_some_and(|s| !s.is_empty()) {
            sections.push(Vec::new());
        }
        if sections.is_empty() {
            sections.push(Vec::new());
        }
        sections.last_mut().expect("a section was just pushed").push(raw);
    }
    sections
        .into_iter()
        .filter_map(|body| {
            let parsed = parse_unified_diff(&body.join("\n"));
            if parsed.lines.is_empty() && parsed.paths.is_empty() {
                return None;
            }
            let path = parsed.paths.first().cloned().or_else(|| declared.map(str::to_string))?;
            Some(edit(path, parsed.created, parsed.lines, false))
        })
        .collect()
}

// ── The rows DiffView draws ─────────────────────────────────────────────────────────────────────────────────────────

/// A run of context lines longer than this is collapsed (`COLLAPSE_CONTEXT_AFTER_LINE_COUNT`).
pub const COLLAPSE_CONTEXT_AFTER: usize = 10;
/// How many lines of a collapsed run stay at each edge (`CONTEXT_EDGE_LINE_COUNT`).
pub const CONTEXT_EDGE: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Line {
        index: usize,
    },
    /// The one row standing in for a collapsed run: its id (`start-end`) and how many lines it hides.
    Expand {
        id: String,
        hidden: usize,
    },
}

/// The rows to draw: every line, with each context run past 10 lines cut to 3 at each edge and an expander between,
/// unless its id is in `expanded`.
pub fn rows(lines: &[DiffLine], expanded: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind != Kind::Context {
            rows.push(Row::Line { index: i });
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && lines[i].kind == Kind::Context {
            i += 1;
        }
        let id = format!("{start}-{i}");
        if i - start <= COLLAPSE_CONTEXT_AFTER || expanded.contains(&id) {
            rows.extend((start..i).map(|index| Row::Line { index }));
            continue;
        }
        rows.extend((start..start + CONTEXT_EDGE).map(|index| Row::Line { index }));
        rows.push(Row::Expand { id, hidden: i - start - 2 * CONTEXT_EDGE });
        rows.extend((i - CONTEXT_EDGE..i).map(|index| Row::Line { index }));
    }
    rows
}

/// "Show 6 unchanged lines", "Show 1 unchanged line".
pub fn expand_label(hidden: usize) -> String {
    format!("Show {hidden} unchanged {}", if hidden == 1 { "line" } else { "lines" })
}
