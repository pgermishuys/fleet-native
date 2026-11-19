//! How a file's text is laid out on disk, so a buffer can give back exactly the bytes it was given
//! (`lib/code-editor/text-format.ts`). A leading BOM is taken off the text and remembered; the file's own most common line
//! break is the only one lines are split on, so a file with mixed breaks keeps the odd ones inside its lines.

/// The break between lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineSeparator {
    Lf,
    Crlf,
    Cr,
}

impl LineSeparator {
    pub fn as_str(self) -> &'static str {
        match self {
            LineSeparator::Lf => "\n",
            LineSeparator::Crlf => "\r\n",
            LineSeparator::Cr => "\r",
        }
    }
}

/// `TextFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextFormat {
    pub line_separator: LineSeparator,
    /// Whether the file starts with a UTF-8 byte order mark.
    pub bom: bool,
}

const BOM: char = '\u{FEFF}';

/// The file's most common break. Ties and files without breaks use `\n`.
fn dominant_separator(text: &str) -> LineSeparator {
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {
                crlf += 1;
                chars.next();
            }
            '\r' => cr += 1,
            '\n' => lf += 1,
            _ => {}
        }
    }
    if crlf > lf && crlf >= cr {
        LineSeparator::Crlf
    } else if cr > lf && cr > crlf {
        LineSeparator::Cr
    } else {
        LineSeparator::Lf
    }
}

/// Splits text as read from disk into the buffer's text and its format (`readTextFormat`).
pub fn read_text_format(raw: &str) -> (String, TextFormat) {
    let bom = raw.starts_with(BOM);
    let text = if bom { &raw[BOM.len_utf8()..] } else { raw };
    (text.to_string(), TextFormat { line_separator: dominant_separator(text), bom })
}

/// The text to write to disk for a buffer's text (`toDiskText`): a BOM goes back as U+FEFF.
pub fn to_disk_text(text: &str, format: &TextFormat) -> String {
    if format.bom { format!("{BOM}{text}") } else { text.to_string() }
}

/// Pasted or dropped text as the editor takes it (`clipboardInputFilter`): every break becomes the one the lines are split on, `\n`,
/// so it makes new lines instead of landing inside one; [`join_lines`] puts the file's own break back between them when it is saved.
pub fn paste_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn same_text_format(a: &TextFormat, b: &TextFormat) -> bool {
    a == b
}

/// The lines of a text, split on the file's own break only and numbered from 1 by position (`a\n` is two lines, the last empty).
pub fn split_lines(text: &str, format: &TextFormat) -> Vec<String> {
    text.split(format.line_separator.as_str()).map(str::to_string).collect()
}

/// The inverse of [`split_lines`].
pub fn join_lines(lines: &[String], format: &TextFormat) -> String {
    lines.join(format.line_separator.as_str())
}
