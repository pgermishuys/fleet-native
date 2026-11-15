//! The files a sent message names in its trailing note, and the chip each one gets (`lib/attached-files.ts`).
//! No Rune in here: the note is plain text the agent reads, so the parse and the write must agree to the letter.

/// What the server saved for an uploaded file; `path` is from the session's folder.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// Lines in a text file, `None` for anything else.
    pub lines: Option<u64>,
}

/// A file named in a sent message's note.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedFileRef {
    pub path: String,
    pub name: String,
    pub details: String,
}

/// `B` under 1 KB, whole `KB` under 1 MB, then `MB` with one decimal (`formatFileSize`).
pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        // JavaScript's Math.round rounds halves up.
        format!("{} KB", (bytes as f64 / 1024.0 + 0.5).floor())
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

/// The extension of a name: what follows the last dot, unless the dot is first or last.
fn extension(name: &str) -> Option<&str> {
    let dot = name.rfind('.')?;
    (dot > 0 && dot < name.len() - 1).then(|| &name[dot + 1..])
}

/// The label a chip shows in its box: LOG, PDF, FILE when the name has no extension (`fileExtensionLabel`).
pub fn file_extension_label(name: &str) -> String {
    extension(name).map_or_else(|| "FILE".to_string(), |e| e.to_uppercase().chars().take(4).collect())
}

/// `2031` as `2,031` (`toLocaleString("en-US")`).
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn describe(file: &AttachedFile) -> String {
    let size = format_file_size(file.size);
    if let Some(lines) = file.lines {
        return format!("{} {}, {size}", thousands(lines), if lines == 1 { "line" } else { "lines" });
    }
    format!("{}, {size}", extension(&file.name).map_or_else(|| "File".to_string(), str::to_uppercase))
}

/// The lines the agent gets after the typed text (`formatAttachedNote`).
pub fn format_attached_note(files: &[AttachedFile]) -> String {
    let mut lines: Vec<String> = files.iter().map(|f| format!("Attached: {} ({})", f.path, describe(f))).collect();
    lines.push(if files.len() == 1 { "Read it as needed." } else { "Read them as needed." }.to_string());
    lines.join("\n")
}

pub fn with_attached_note(text: &str, files: &[AttachedFile]) -> String {
    if files.is_empty() {
        return text.to_string();
    }
    let note = format_attached_note(files);
    let typed = text.trim_end();
    if typed.is_empty() { note } else { format!("{typed}\n\n{note}") }
}

/// `Attached: <path> (<details>)`: a path with no spaces, details to the last `)` of the line (`NOTE_LINE`).
fn note_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("Attached: ")?;
    let (path, details) = rest.split_once(' ')?;
    if path.is_empty() || path.contains(char::is_whitespace) {
        return None;
    }
    let details = details.strip_prefix('(')?.strip_suffix(')')?;
    Some((path, details))
}

/// Splits a sent message into what was typed and the files its trailing note names (`parseAttachedNote`). The note
/// is the last block: one or more `Attached:` lines, then "Read it as needed." or "Read them as needed.".
pub fn parse_attached_note(body: &str) -> (String, Vec<AttachedFileRef>) {
    let plain = || (body.to_string(), Vec::new());
    let trimmed = body.trim_end();
    let Some(without_end) = trimmed.strip_suffix("Read it as needed.").or_else(|| trimmed.strip_suffix("Read them as needed.")) else {
        return plain();
    };
    let Some(block) = without_end.strip_suffix('\n') else { return plain() };
    let mut lines: Vec<&str> = block.split('\n').collect();
    let mut files = Vec::new();
    while let Some((path, details)) = lines.last().and_then(|l| note_line(l)) {
        files.push(AttachedFileRef {
            path: path.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            details: details.to_string(),
        });
        lines.pop();
    }
    if files.is_empty() {
        return plain();
    }
    files.reverse();
    (lines.join("\n").trim_end().to_string(), files)
}
