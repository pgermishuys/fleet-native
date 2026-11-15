//! A conversation as a Markdown file a person can read, for an issue, a PR or a doc (`lib/export-markdown.ts`). Built from
//! the normalised messages, so every harness reads the same; it can't be imported back. No Rune in here.
//!
//! Fleet does not remove secrets. Tool output is left out by default; [`find_key_lines`] only counts lines that look like
//! keys so the dialog can say so.
//!
//! Differences from Fleet, all small: lengths count `char`s where Fleet counts UTF-16 units (they differ only outside the
//! basic plane); an object a tool returns is printed with its keys in alphabetical order (the JSON map keeps no order);
//! times are written in the zone the caller names (`offset_secs`), because the standard library can't read the zone.
use super::compaction::{CompactionView, describe_compaction};
use super::diff::tool_diff_text;
use super::time::civil;
use super::tools::{Category, as_record, get_tool, parse_tool_input, string_field};
use crate::fleet::model::{Msg, PartData};
use crate::view::conversation::Role;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportToolMode {
    /// One line per call.
    Line,
    /// The line with its output.
    Output,
    /// No tool calls at all.
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    pub tools: ExportToolMode,
    /// The model's reasoning, as quotes.
    pub thinking: bool,
    /// Cut the session's folder off the front of paths.
    pub shorten_paths: bool,
    /// Times, model and the export date.
    pub meta: bool,
}

pub const DEFAULT_EXPORT_OPTIONS: ExportOptions =
    ExportOptions { tools: ExportToolMode::Line, thinking: false, shorten_paths: true, meta: true };

#[derive(Clone, Debug, Default)]
pub struct ExportContext {
    pub title: String,
    pub harness: Option<String>,
    pub model: Option<String>,
    /// The session's folder, cut off paths when `shorten_paths` is on.
    pub directory: Option<String>,
    /// Unix milliseconds.
    pub exported_at: i64,
    /// Seconds east of UTC of the zone times are written in.
    pub offset_secs: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportResult {
    pub markdown: String,
    /// Lines that look like keys or tokens. A hint only: Fleet has not looked at everything.
    pub key_lines: usize,
    /// Messages that made it into the file.
    pub turns: usize,
}

/// A tool's output past this is cut, with a line saying so; a build log can run to megabytes.
const MAX_OUTPUT_CHARS: usize = 20_000;

// ── Key shapes ────────────────────────────────────────────────────────────────

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Length of the run of `pred` characters starting at `at`.
fn run(chars: &[char], at: usize, pred: impl Fn(char) -> bool) -> usize {
    chars.get(at..).map_or(0, |rest| rest.iter().take_while(|c| pred(**c)).count())
}

fn starts_with(chars: &[char], at: usize, prefix: &str) -> bool {
    prefix.chars().enumerate().all(|(n, p)| chars.get(at + n) == Some(&p))
}

/// Regex `\b` before a position whose character is a word character.
fn boundary_before(chars: &[char], at: usize) -> bool {
    at == 0 || !is_word(chars[at - 1])
}

/// `\b<prefix>[class]{min,}` somewhere in the line.
fn prefixed_run(chars: &[char], prefixes: &[&str], class: impl Fn(char) -> bool + Copy, min: usize) -> bool {
    (0..chars.len())
        .filter(|&i| boundary_before(chars, i))
        .any(|i| prefixes.iter().any(|p| starts_with(chars, i, p) && run(chars, i + p.chars().count(), class) >= min))
}

fn private_key_header(chars: &[char]) -> bool {
    const OPEN: &str = "-----BEGIN ";
    (0..chars.len()).filter(|&i| starts_with(chars, i, OPEN)).any(|i| {
        let from = i + OPEN.len();
        let end = from + run(chars, from, |c| c.is_ascii_uppercase() || c == ' ');
        let name = "PRIVATE KEY";
        let tail = end >= from + name.len() && chars[end - name.len()..end].iter().copied().eq(name.chars());
        tail && starts_with(chars, end, "-----")
    })
}

fn bearer(chars: &[char]) -> bool {
    (0..chars.len()).filter(|&i| boundary_before(chars, i) && starts_with(chars, i, "Bearer")).any(|i| {
        let spaces = run(chars, i + 6, char::is_whitespace);
        spaces > 0 && run(chars, i + 6 + spaces, |c| c.is_ascii_alphanumeric() || "._~+/=-".contains(c)) >= 20
    })
}

/// `NAME_WITH_KEY=value`: a word that holds API_KEY, SECRET, TOKEN or PASSWORD (any case), then `=` or `:`, then 8 or
/// more characters that aren't space or a quote.
fn named_secret(chars: &[char]) -> bool {
    let mut i = 0;
    while i < chars.len() {
        if !is_word(chars[i]) {
            i += 1;
            continue;
        }
        let end = i + run(chars, i, is_word);
        let word: String = chars[i..end].iter().collect::<String>().to_ascii_lowercase();
        if ["apikey", "api_key", "secret", "token", "password"].iter().any(|k| word.contains(k)) {
            let mut j = end + run(chars, end, char::is_whitespace);
            if matches!(chars.get(j), Some('=' | ':')) {
                j += 1;
                j += run(chars, j, char::is_whitespace);
                if matches!(chars.get(j), Some('"' | '\'')) {
                    j += 1;
                }
                if run(chars, j, |c| !c.is_whitespace() && c != '"' && c != '\'') >= 8 {
                    return true;
                }
            }
        }
        i = end;
    }
    false
}

/// A line that looks like it holds a key or a token (`looksLikeKey`).
pub fn looks_like_key(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let alnum = |c: char| c.is_ascii_alphanumeric();
    prefixed_run(&chars, &["sk_live_", "sk_test_"], alnum, 8)
        || prefixed_run(&chars, &["sk-"], |c| alnum(c) || c == '_' || c == '-', 20)
        || prefixed_run(&chars, &["AKIA"], |c| c.is_ascii_digit() || c.is_ascii_uppercase(), 12)
        || prefixed_run(&chars, &["ghp_", "gho_", "ghu_", "ghs_", "ghr_"], alnum, 20)
        || prefixed_run(&chars, &["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"], |c| alnum(c) || c == '-', 10)
        || private_key_header(&chars)
        || bearer(&chars)
        || named_secret(&chars)
}

/// How many lines of the file look like keys (`findKeyLines`). Nothing is redacted.
pub fn find_key_lines(markdown: &str) -> usize {
    markdown.split('\n').filter(|l| looks_like_key(l)).count()
}

/// The session has reasoning to include, so the Thinking option has something to do (`hasReasoning`).
pub fn has_reasoning(messages: &[Msg]) -> bool {
    messages.iter().any(|m| m.parts.iter().any(|p| matches!(p, PartData::Reasoning { text, .. } if !text.trim().is_empty())))
}

/// The title as a file name, "Add upload retries" → "add-upload-retries.md" (`exportFileName`).
pub fn export_file_name(title: &str, fallback: &str) -> String {
    let mut stem = String::new();
    let mut in_other = false;
    for c in title.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' {
            stem.push(c);
            in_other = false;
        } else if !in_other {
            // A run of anything else is one dash.
            stem.push('-');
            in_other = true;
        }
    }
    let stem: String = stem.trim_matches('-').chars().take(80).collect();
    let stem = stem.trim_end_matches('-');
    format!("{}.md", if stem.is_empty() { fallback } else { stem })
}

// ── Text helpers ──────────────────────────────────────────────────────────────

fn clock(ms: i64, offset_secs: i64) -> String {
    let (.., h, m) = civil(ms, offset_secs);
    format!("{h:02}:{m:02}")
}

fn stamp(ms: i64, offset_secs: i64) -> String {
    let (y, mo, d, h, mi) = civil(ms, offset_secs);
    format!("{y:04}-{:02}-{d:02} {h:02}:{mi:02}", mo + 1)
}

fn longest_backticks(text: &str) -> usize {
    let (mut longest, mut now) = (0, 0);
    for c in text.chars() {
        now = if c == '`' { now + 1 } else { 0 };
        longest = longest.max(now);
    }
    longest
}

/// A code fence longer than any run of backticks inside, so the content can't close it early.
fn fenced(body: &str, language: &str) -> String {
    let fence = "`".repeat(3.max(longest_backticks(body) + 1));
    format!("{fence}{language}\n{body}\n{fence}")
}

fn quoted(text: &str) -> String {
    text.split('\n').map(|l| if l.is_empty() { ">".to_string() } else { format!("> {l}") }).collect::<Vec<_>>().join("\n")
}

fn inline_code(text: &str) -> String {
    let tick = "`".repeat(longest_backticks(text) + 1);
    format!("{tick}{text}{tick}")
}

/// Every newline, with the space around it, becomes one space (`singleLine`).
fn single_line(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            out.truncate(out.trim_end().len());
            while chars.next_if(|n| n.is_whitespace()).is_some() {}
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// Cut a terminal escape sequence (`ESC [ params intermediates final`) out of the text.
fn strip_ansi(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\u{1b}' && chars.get(i + 1) == Some(&'[') {
            let params = run(&chars, i + 2, |c| c.is_ascii_digit() || c == ';' || c == '?');
            let mid = run(&chars, i + 2 + params, |c| (' '..='/').contains(&c));
            if chars.get(i + 2 + params + mid).is_some_and(|c| ('@'..='~').contains(c)) {
                i += 3 + params + mid;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The text a call returned: the first of the usual fields that is there, as text, without terminal escapes.
fn tool_output(state: &Value) -> Option<String> {
    let record = as_record(state)?;
    for key in ["output", "result", "content", "error", "message", "stdout", "stderr"] {
        match record.get(key) {
            None | Some(Value::Null) => continue,
            Some(Value::String(s)) => return Some(strip_ansi(s)),
            Some(other) => return Some(strip_ansi(&serde_json::to_string_pretty(other).unwrap_or_default())),
        }
    }
    None
}

/// The change an edit made, as a diff: the unified diff the harness kept, or the patches of the files it changed
/// (OpenCode 2 keeps them in `metadata.files`), or else the strings the call swapped (`editDiff`).
fn edit_diff(state: &Value) -> Option<String> {
    if let Some(kept) = tool_diff_text(state) {
        return Some(kept);
    }
    if let Some(files) = state.pointer("/metadata/files").and_then(Value::as_array) {
        let patches: Vec<&str> =
            files.iter().filter_map(|f| f.get("patch")?.as_str()).filter(|p| !p.trim().is_empty()).map(str::trim_end).collect();
        if !patches.is_empty() {
            return Some(patches.join("\n"));
        }
    }
    let input = parse_tool_input(state.get("input").unwrap_or(&Value::Null));
    let (old, new) = (input.old_string?, input.new_string?);
    let side = |prefix: &str, text: &str| text.split('\n').map(|l| format!("{prefix}{l}")).collect::<Vec<_>>().join("\n");
    Some(format!("{}\n{}", side("-", &old), side("+", &new)))
}

fn cut(output: &str) -> String {
    let trimmed = output.trim_end();
    let total = trimmed.chars().count();
    if total <= MAX_OUTPUT_CHARS {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(MAX_OUTPUT_CHARS).collect();
    format!("{kept}\n\u{2026} {} more characters left out", total - MAX_OUTPUT_CHARS)
}

// ── The writer ────────────────────────────────────────────────────────────────

struct Writer<'a> {
    options: ExportOptions,
    context: &'a ExportContext,
    /// The folder to cut off paths, empty for none.
    folder: String,
    /// Whether the last thing written was an assistant turn, so its next step doesn't repeat it.
    open_turn: bool,
}

impl Writer<'_> {
    fn shorten(&self, text: &str) -> String {
        if self.folder.is_empty() {
            return text.to_string();
        }
        text.replace(&format!("{}/", self.folder), "").replace(&format!("{}\\", self.folder), "")
    }

    fn tool_block(&self, tool_name: &str, state: &Value) -> Option<String> {
        let tool = get_tool(tool_name);
        let input_value = state.get("input").unwrap_or(&Value::Null);
        let input = parse_tool_input(input_value);

        // A page the agent showed is part of the story, whatever the tool setting says.
        if tool.name == "fleet_page_show" {
            let title = string_field(as_record(state), &["title"]).or_else(|| string_field(Some(&input.raw), &["title"]));
            let name = title.or_else(|| string_field(Some(&input.raw), &["path"])).unwrap_or_else(|| "page".to_string());
            return Some(quoted(&format!("Showed page: \"{}\"", single_line(&self.shorten(&name)))));
        }
        if self.options.tools == ExportToolMode::Off {
            return None;
        }

        // A file tool names its whole path (the label cuts long ones short); the shortening below is the user's choice.
        let file_tool = matches!(tool.category, Category::Read | Category::Edit);
        let label = single_line(&match (&input.file_path, file_tool) {
            (Some(path), true) => path.clone(),
            _ => tool.label(input_value),
        });
        let heading = if tool.heading.is_empty() { tool_name.to_string() } else { tool.heading.clone() };
        // A label that only repeats the tool's name adds nothing.
        let lower = label.to_lowercase();
        let subject = if !label.is_empty() && lower != tool_name.to_lowercase() && lower != heading.to_lowercase() {
            let shown = match (&input.command, tool.category == Category::Shell) {
                (Some(command), true) => single_line(command).chars().take(120).collect(),
                _ => label,
            };
            format!(" {}", inline_code(&self.shorten(&shown)))
        } else {
            String::new()
        };
        let status = state.get("status").and_then(Value::as_str).unwrap_or("").to_lowercase();
        let note = match status.as_str() {
            "error" => " (failed)",
            "pending" | "running" => " (did not finish)",
            _ => "",
        };
        let line = format!("- {heading}{subject}{note}");
        if self.options.tools != ExportToolMode::Output {
            return Some(line);
        }

        if tool.category == Category::Edit
            && let Some(diff) = edit_diff(state)
        {
            return Some(format!("{line}\n\n{}", fenced(&self.shorten(&cut(&diff)), "diff")));
        }
        match tool_output(state) {
            Some(output) if !output.trim().is_empty() => {
                let language = if tool.category == Category::Shell { "text" } else { "" };
                Some(format!("{line}\n\n{}", fenced(&self.shorten(&cut(&output)), language)))
            }
            _ => Some(line),
        }
    }

    fn message(&mut self, message: &Msg) -> Vec<String> {
        if message.compaction_summary {
            return Vec::new();
        }
        let compaction = message.parts.iter().find_map(|p| match p {
            PartData::Compaction { trigger, tokens_before, tokens_after, summary, .. } => Some(CompactionView {
                trigger: trigger.clone(),
                tokens_before: *tokens_before,
                tokens_after: *tokens_after,
                summary: summary.clone(),
            }),
            _ => None,
        });
        let mut blocks = Vec::new();
        if let Some(c) = &compaction {
            self.open_turn = false;
            blocks.push("---".to_string());
            blocks.push(format!("_{}_", describe_compaction(c)));
        }

        let mut body: Vec<String> = Vec::new();
        for part in &message.parts {
            match part {
                PartData::Text { text, .. } => {
                    if !text.trim().is_empty() {
                        body.push(text.trim().to_string());
                    }
                }
                PartData::Reasoning { text, .. } => {
                    if self.options.thinking && !text.trim().is_empty() {
                        body.push(quoted(&format!("**Thinking**\n\n{}", text.trim())));
                    }
                }
                // Never the bytes: a data address can be megabytes.
                PartData::File { mime, filename, .. } => {
                    let kind = if mime.starts_with("image/") { "image" } else { "file" };
                    let name = filename.as_deref().filter(|n| !n.is_empty()).or(Some(mime.as_str()).filter(|m| !m.is_empty()));
                    body.push(format!("[{kind}: {}]", name.unwrap_or("attachment")));
                }
                PartData::Tool { tool, state, .. } => body.extend(self.tool_block(tool, state)),
                PartData::Compaction { .. } => {}
            }
        }
        if body.is_empty() {
            return blocks;
        }

        if let Some(command) = &message.command {
            let args = command.arguments.as_deref().filter(|a| !a.is_empty()).map(|a| format!(" {a}")).unwrap_or_default();
            body = vec![inline_code(&format!("/{}{args}", command.name))];
        }

        let who = match message.role {
            Role::Assistant => "Assistant",
            Role::Shell => "You ran a command",
            Role::User => "You",
        };
        let meta = if self.options.meta {
            let time = message.created_at.map(|ms| clock(ms, self.context.offset_secs));
            let model = message.model_id.clone().filter(|_| message.role == Role::Assistant);
            [time, model].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" \u{b7} ")
        } else {
            String::new()
        };
        // An agent's steps arrive as messages of their own; one reply reads as one turn.
        let heading = if meta.is_empty() { format!("## {who}") } else { format!("## {who} \u{b7} {meta}") };
        let same_turn = message.role == Role::Assistant && self.open_turn;
        self.open_turn = message.role == Role::Assistant;
        if !same_turn {
            blocks.push(heading);
        }
        blocks.extend(body);
        blocks
    }

    fn header(&self) -> Vec<String> {
        let title = single_line(&self.context.title);
        let mut blocks = vec![format!("# {}", if title.is_empty() { "Conversation" } else { &title })];
        if self.options.meta {
            let exported = format!("exported {}", stamp(self.context.exported_at, self.context.offset_secs));
            let line = [self.context.harness.clone(), self.context.model.clone(), Some(exported)]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" \u{b7} ");
            blocks.push(format!("_{line}_"));
        }
        blocks
    }
}

/// The conversation, oldest message first, as Markdown (`buildMarkdown`).
pub fn build_markdown(messages: &[Msg], options: &ExportOptions, context: &ExportContext) -> ExportResult {
    let folder = if options.shorten_paths {
        context.directory.as_deref().unwrap_or("").trim_end_matches(['/', '\\']).to_string()
    } else {
        String::new()
    };
    let mut writer = Writer { options: *options, context, folder, open_turn: false };
    let mut blocks = writer.header();
    for message in messages {
        blocks.extend(writer.message(message));
    }
    let turns = blocks.iter().filter(|b| b.starts_with("## ")).count();

    // Tool lines sit tight in a list; everything else is a paragraph apart.
    let bullet = |b: &String| b.starts_with("- ") && !b.contains('\n');
    let mut markdown = String::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            markdown.push_str(if bullet(block) && bullet(&blocks[i - 1]) { "\n" } else { "\n\n" });
        }
        markdown.push_str(block);
    }
    markdown.push('\n');
    let key_lines = find_key_lines(&markdown);
    ExportResult { markdown, key_lines, turns }
}
