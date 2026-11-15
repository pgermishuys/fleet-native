//! Files, large pastes and terminal lines in a draft (`lib/attached-files.ts`, `lib/format-terminal-context.ts`,
//! `use-draft-files.ts`, `use-draft-terminal-context.ts`). No Rune and no I/O: reading and uploading a file is
//! `fleet::uploads`, on a thread; this file decides and writes the text.
use super::format::{format_bytes, format_count};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// `LARGE_PASTES_PREFERENCE_KEY`: what a big paste does.
pub const LARGE_PASTES_KEY: &str = "LargePastes";

/// A text file under this goes into the prompt instead of being uploaded (`INLINE_MAX_BYTES`).
pub const INLINE_MAX_BYTES: u64 = 4096;
/// A file over this is refused (`MAX_FILE_BYTES`).
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
const BIG_PASTE_CHARACTERS: usize = 2000;
const BIG_PASTE_LINES: usize = 25;
/// A text file over this isn't read for its line count.
pub const LINE_COUNT_MAX_BYTES: u64 = 5 * 1024 * 1024;

const TEXT_EXTENSIONS: [&str; 19] =
    ["txt", "log", "md", "json", "xml", "yaml", "yml", "csv", "tsv", "toml", "ini", "sql", "html", "css", "js", "ts", "py", "cs", "sh"];

/// What the Large pastes setting says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LargePastes {
    Ask,
    Attach,
    Text,
}

impl LargePastes {
    /// An unset or unknown value is "ask", the default.
    pub fn from_pref(value: Option<&str>) -> LargePastes {
        match value {
            Some("attach") => LargePastes::Attach,
            Some("text") => LargePastes::Text,
            _ => LargePastes::Ask,
        }
    }
}

/// What the server saved for an uploaded file; `path` is from the session's folder (`AttachedFile`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachedFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// Lines in a text file, `None` for anything else.
    pub lines: Option<usize>,
    pub mime: String,
}

/// A file named in a sent message's note (`AttachedFileRef`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachedFileRef {
    pub path: String,
    pub name: String,
    pub details: String,
}

fn count(n: usize) -> String {
    format_count(Some(n as f64))
}

pub fn format_file_size(bytes: u64) -> String {
    format_bytes(Some(bytes as f64))
}

/// The extension after the last dot, when the dot isn't first or last.
fn extension(name: &str) -> Option<&str> {
    let dot = name.rfind('.')?;
    (dot > 0 && dot + 1 < name.len()).then(|| &name[dot + 1..])
}

/// The label a chip shows in its box: LOG, PDF, FILE when the name has no extension (`fileExtensionLabel`).
pub fn file_extension_label(name: &str) -> String {
    extension(name).map_or_else(|| "FILE".into(), |e| e.to_uppercase().chars().take(4).collect())
}

fn describe(file: &AttachedFile) -> String {
    let size = format_file_size(file.size);
    if let Some(lines) = file.lines {
        return format!("{} {}, {size}", count(lines), if lines == 1 { "line" } else { "lines" });
    }
    format!("{}, {size}", extension(&file.name).map_or_else(|| "File".into(), str::to_uppercase))
}

/// The lines the agent gets after the typed text (`formatAttachedNote`).
pub fn format_attached_note(files: &[AttachedFile]) -> String {
    let mut lines: Vec<String> = files.iter().map(|f| format!("Attached: {} ({})", f.path, describe(f))).collect();
    lines.push(if files.len() == 1 { "Read it as needed." } else { "Read them as needed." }.into());
    lines.join("\n")
}

/// `withAttachedNote`: the note after the typed text, a blank line between.
pub fn with_attached_note(text: &str, files: &[AttachedFile]) -> String {
    if files.is_empty() {
        return text.to_string();
    }
    let note = format_attached_note(files);
    let typed = text.trim_end();
    if typed.is_empty() { note } else { format!("{typed}\n\n{note}") }
}

/// `parseAttachedNote`: a sent message split into what was typed and the files its trailing note names.
pub fn parse_attached_note(body: &str) -> (String, Vec<AttachedFileRef>) {
    let plain = || (body.to_string(), Vec::new());
    let end = body.trim_end();
    let Some(before) = end.strip_suffix("Read it as needed.").or_else(|| end.strip_suffix("Read them as needed.")) else {
        return plain();
    };
    // The note's lines each end in a newline, so what is left ends in one.
    let Some(before) = before.strip_suffix('\n') else { return plain() };
    let lines: Vec<&str> = before.split('\n').collect();
    let files: Vec<AttachedFileRef> = lines.iter().rev().map_while(|line| parse_note_line(line)).collect();
    if files.is_empty() {
        return plain();
    }
    let typed = lines[..lines.len() - files.len()].join("\n");
    (typed.trim_end().to_string(), files.into_iter().rev().collect())
}

/// `NOTE_LINE`: "Attached: <path> (<details>)", the path with no space in it.
fn parse_note_line(line: &str) -> Option<AttachedFileRef> {
    let rest = line.strip_prefix("Attached: ")?;
    let (path, details) = rest.split_once(" (")?;
    if path.is_empty() || path.contains(char::is_whitespace) {
        return None;
    }
    let details = details.strip_suffix(')')?;
    Some(AttachedFileRef { path: path.into(), name: path.rsplit('/').next().unwrap_or(path).into(), details: details.into() })
}

/// JavaScript's `string.length`: UTF-16 units.
fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `isBigPaste`: 2,000 characters or 25 lines.
pub fn is_big_paste(text: &str) -> bool {
    js_len(text) >= BIG_PASTE_CHARACTERS || text.split('\n').count() >= BIG_PASTE_LINES
}

/// `describePaste`: "2,031 lines pasted", or the characters when it's one long line.
pub fn describe_paste(text: &str) -> String {
    let lines = count_lines(text);
    if lines >= 2 { format!("{} lines pasted", count(lines)) } else { format!("{} characters pasted", count(js_len(text))) }
}

/// `pastedFileName`: `pasted-text.txt`, then `pasted-text-2.txt`, the first name the draft doesn't have.
pub fn pasted_file_name(taken: &[&str]) -> String {
    (1..)
        .map(|n| if n == 1 { "pasted-text.txt".to_string() } else { format!("pasted-text-{n}.txt") })
        .find(|name| !taken.contains(&name.as_str()))
        .unwrap_or_default()
}

/// `countLines`, the way the server counts: a last line with no newline still counts, an empty text has none.
pub fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.split('\n').count() - usize::from(text.ends_with('\n'))
}

/// `isTextFile`: by its type, or by one of 19 extensions.
pub fn is_text_file(name: &str, mime: &str) -> bool {
    if mime.starts_with("text/") || mime.ends_with("json") || mime.ends_with("xml") {
        return true;
    }
    extension(name).is_some_and(|e| TEXT_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

/// `describeDraftFile`: "2,031 lines · 186 KB", or just the size when the lines aren't known.
pub fn describe_draft_file(size: u64, lines: Option<usize>) -> String {
    match lines {
        None => format_file_size(size),
        Some(n) => format!("{} {} · {}", count(n), if n == 1 { "line" } else { "lines" }, format_file_size(size)),
    }
}

/// The longest run of backticks in `text`, so a fence can be longer.
fn fence_for(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat((longest + 1).max(3))
}

/// `inlineFileBlock`: a small file as a fenced block headed by its name, the fence longer than any backtick run inside.
pub fn inline_file_block(name: &str, content: &str) -> String {
    let fence = fence_for(content);
    format!("{name}:\n{fence}\n{}\n{fence}", content.strip_suffix('\n').unwrap_or(content))
}

/// How a file's bytes come: from the disk when it is sent, or from memory (a paste).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSource {
    Path(PathBuf),
    Text(Arc<str>),
}

/// A file held for the next prompt (`DraftFile`). Nothing is uploaded until Send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftFile {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// Lines in a text file, counted when it was read.
    pub lines: Option<usize>,
    /// Text the person pasted and chose to attach: it always uploads.
    pub pasted: bool,
    pub source: FileSource,
}

impl DraftFile {
    /// Whether the file goes into the prompt as a block instead of being uploaded.
    pub fn goes_inline(&self) -> bool {
        !self.pasted && is_text_file(&self.name, &self.mime) && self.size < INLINE_MAX_BYTES
    }
}

/// `TerminalContextDraft`: lines of a terminal's scrollback the person added to the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalDraft {
    pub id: String,
    pub terminal_id: String,
    /// The terminal's tab title, "zsh" or "zsh 2".
    pub label: String,
    /// First and last line, counted from the top of the scrollback, starting at 1.
    pub from: usize,
    pub to: usize,
    pub text: String,
}

/// `terminalLineRange`: "line 9" or "lines 9–11".
pub fn terminal_line_range(from: usize, to: usize) -> String {
    if from == to { format!("line {from}") } else { format!("lines {from}–{to}") }
}

/// `formatTerminalContext`: each attachment as a fenced block with its label, then what was typed.
pub fn format_terminal_context(contexts: &[TerminalDraft], message: &str) -> String {
    if contexts.is_empty() {
        return message.to_string();
    }
    let blocks: Vec<String> = contexts
        .iter()
        .map(|c| {
            let fence = fence_for(&c.text);
            format!("Terminal {}, {}:\n{fence}text\n{}\n{fence}", c.label, terminal_line_range(c.from, c.to), c.text)
        })
        .collect();
    let typed = message.trim();
    if typed.is_empty() { blocks.join("\n\n") } else { format!("{}\n\n{typed}", blocks.join("\n\n")) }
}

/// What a draft keeps for the prompt besides its text and pictures: files (by owner, the session), terminal lines, and
/// the terminal lines of prompts that went and may come back.
#[derive(Default)]
pub struct Held {
    files: HashMap<String, Vec<DraftFile>>,
    terminals: HashMap<String, Vec<TerminalDraft>>,
    /// By the prompt's correlation id: whose, the lines, what was typed and what went.
    sent: HashMap<String, Sent>,
    serial: u64,
}

struct Sent {
    owner: String,
    contexts: Vec<TerminalDraft>,
    typed: String,
}

impl Held {
    pub fn next_id(&mut self) -> String {
        self.serial += 1;
        format!("file{}", self.serial)
    }

    pub fn files(&self, owner: &str) -> &[DraftFile] {
        self.files.get(owner).map_or(&[], Vec::as_slice)
    }

    /// `addFile`.
    pub fn add_file(&mut self, owner: &str, file: DraftFile) {
        self.files.entry(owner.to_string()).or_default().push(file);
    }

    /// `removeFile`.
    pub fn remove_file(&mut self, owner: &str, id: &str) {
        if let Some(list) = self.files.get_mut(owner) {
            list.retain(|f| f.id != id);
        }
    }

    /// `clearFiles`.
    pub fn clear_files(&mut self, owner: &str) {
        self.files.remove(owner);
    }

    /// The name a pasted text gets in this draft (`pastedFileName`).
    pub fn pasted_name(&self, owner: &str) -> String {
        let taken: Vec<&str> = self.files(owner).iter().map(|f| f.name.as_str()).collect();
        pasted_file_name(&taken)
    }

    pub fn terminals(&self, owner: &str) -> &[TerminalDraft] {
        self.terminals.get(owner).map_or(&[], Vec::as_slice)
    }

    /// `addDraftTerminalContext`: the same lines of the same terminal are only added once.
    pub fn add_terminal(&mut self, owner: &str, context: TerminalDraft) {
        let list = self.terminals.entry(owner.to_string()).or_default();
        if list.iter().any(|c| c.terminal_id == context.terminal_id && c.from == context.from && c.to == context.to) {
            return;
        }
        list.push(context);
    }

    /// `removeContext`.
    pub fn remove_terminal(&mut self, owner: &str, id: &str) {
        if let Some(list) = self.terminals.get_mut(owner) {
            list.retain(|c| c.id != id);
        }
    }

    /// `clearDraftTerminalContext`.
    pub fn clear_terminals(&mut self, owner: &str) {
        self.terminals.remove(owner);
    }

    /// The prompt goes: the lines leave the draft and wait, by the prompt's id, in case the server refuses it.
    pub fn take_terminals_for_send(&mut self, owner: &str, correlation_id: &str, typed: &str) -> Vec<TerminalDraft> {
        let contexts = self.terminals.remove(owner).unwrap_or_default();
        if !contexts.is_empty() {
            self.sent.insert(correlation_id.to_string(), Sent { owner: owner.into(), contexts: contexts.clone(), typed: typed.into() });
        }
        contexts
    }

    /// The server took the prompt: nothing to put back.
    pub fn delivered(&mut self, correlation_id: &str) {
        self.sent.remove(correlation_id);
    }

    /// The server refused it (`putBackRefused`): the lines are back in their draft, and the text typed is given so the
    /// draft can show it without the blocks in front.
    pub fn refused(&mut self, correlation_id: &str) -> Option<(String, String)> {
        let sent = self.sent.remove(correlation_id)?;
        for context in sent.contexts {
            self.add_terminal(&sent.owner, context);
        }
        Some((sent.owner, sent.typed))
    }
}
