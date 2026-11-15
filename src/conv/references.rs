//! Text in a message that names a file (`lib/file-references.ts`) or a slash command
//! (`lib/slash-command-utils.ts`). No Rune in here.

/// The path and line in a piece of text that looks like a file.
#[derive(Clone, Debug, PartialEq)]
pub struct FileReference {
    pub path: String,
    pub line: Option<u32>,
}

const MAX_PATH_LENGTH: usize = 260;

/// `:42`, `:42:7`, `:42-50`, `#L42`, `#L42-L50` at the end: where it starts and the line (`LINE_SUFFIX`).
fn line_suffix(s: &str) -> Option<(usize, u32)> {
    let digits_end = |t: &str| t.chars().take_while(char::is_ascii_digit).count();
    // The leftmost `:` or `#` from which the suffix runs to the end of the text, as the pattern finds it.
    for (at, c) in s.char_indices() {
        let rest = &s[at + 1..];
        let ok = |line: &str, mut tail: &str, hash: bool| -> Option<u32> {
            let n: u32 = line.parse().ok()?;
            if hash {
                // `-L?\d+`
                if let Some(t) = tail.strip_prefix('-') {
                    let t = t.strip_prefix('L').unwrap_or(t);
                    return (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).then_some(n);
                }
                return tail.is_empty().then_some(n);
            }
            // `(?::\d+)?(?:-\d+)?`
            if let Some(t) = tail.strip_prefix(':') {
                let d = digits_end(t);
                if d == 0 {
                    return None;
                }
                tail = &t[d..];
            }
            if let Some(t) = tail.strip_prefix('-') {
                return (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).then_some(n);
            }
            tail.is_empty().then_some(n)
        };
        match c {
            ':' => {
                let d = digits_end(rest);
                if d > 0 && ok(&rest[..d], &rest[d..], false).is_some() {
                    return Some((at, rest[..d].parse().ok()?));
                }
            }
            '#' => {
                if let Some(r) = rest.strip_prefix('L') {
                    let d = digits_end(r);
                    if d > 0
                        && let Some(n) = ok(&r[..d], &r[d..], true)
                    {
                        return Some((at, n));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
fn is_name_char(c: char) -> bool {
    is_word(c) || matches!(c, '.' | '@' | '+' | '-')
}

/// `PATH_SHAPE`: an optional drive, home or `./` start, then names joined by `/` or `\`.
fn path_shape(path: &str) -> bool {
    let mut rest = path;
    let b = rest.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/') {
        rest = &rest[3..];
    } else if let Some(r) = rest.strip_prefix("~/").or_else(|| rest.strip_prefix("~\\")) {
        rest = r;
    } else if let Some(r) = rest
        .strip_prefix("../")
        .or_else(|| rest.strip_prefix("..\\"))
        .or_else(|| rest.strip_prefix("./"))
        .or_else(|| rest.strip_prefix(".\\"))
    {
        rest = r;
    } else if let Some(r) = rest.strip_prefix(['/', '\\']) {
        rest = r;
    }
    !rest.is_empty() && rest.split(['/', '\\']).all(|name| !name.is_empty() && name.chars().all(is_name_char))
}

/// `FILE_NAME`: the last name has a dot, and an extension with a letter in it (`tax.ts`, `.gitignore`, not `1.2.3`).
fn file_name(path: &str) -> bool {
    let last = path.rsplit(['/', '\\']).next().unwrap_or(path);
    last.rfind('.').is_some_and(|dot| {
        let ext = &last[dot + 1..];
        !ext.is_empty() && ext.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && ext.chars().any(|c| c.is_ascii_lowercase())
    })
}

/// The path and line in `text`, when it looks like a file: a name with an extension, or a path with a folder
/// (`parseFileReference`).
pub fn parse_file_reference(text: &str) -> Option<FileReference> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_PATH_LENGTH || trimmed.contains("://") {
        return None;
    }
    let suffix = line_suffix(trimmed);
    let path = suffix.map_or(trimmed, |(at, _)| &trimmed[..at]);
    if !path_shape(path) || path.chars().all(|c| c == '.') {
        return None;
    }
    // A folder separator that isn't the drive's or a trailing one.
    let body = path.strip_prefix(|c: char| c.is_ascii_alphabetic()).and_then(|r| r.strip_prefix(':')).unwrap_or(path);
    if !file_name(path) && !body.trim_end_matches(['/', '\\']).contains(['/', '\\']) {
        return None;
    }
    Some(FileReference { path: path.to_string(), line: suffix.map(|(_, n)| n).filter(|&n| n > 0) })
}

/// A slash command as the user typed it: `/name arguments` (`formatSlashCommand`).
pub fn format_slash_command(name: &str, arguments: Option<&str>) -> String {
    match arguments {
        Some(a) if !a.is_empty() => format!("/{name} {a}"),
        _ => format!("/{name}"),
    }
}
