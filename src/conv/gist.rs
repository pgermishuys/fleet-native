//! The one line a folded thinking block shows, and the setting that says how thinking is drawn. Ports
//! `lib/reasoning-gist.ts` and the `thinking` choice of `stores/theme.ts`. No Rune in here.

/// Longest gist kept; the line is cut to its width anyway.
const MAX_GIST: usize = 240;

/// How reasoning is drawn (`weave:thinking`): one line per block, every block in full, or nothing. Settings owns the
/// choice; until it is built the default is the only one a user reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Thinking {
    #[default]
    Folded,
    Open,
    Hidden,
}

/// A line with its Markdown line marker (`#`, `>`, `-`, `1.`) taken off.
fn strip_marker(line: &str) -> &str {
    fn after_space(rest: &str) -> Option<&str> {
        let trimmed = rest.trim_start();
        (trimmed.len() < rest.len()).then_some(trimmed)
    }
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes)
        && let Some(rest) = after_space(&line[hashes..])
    {
        return rest;
    }
    if let Some(rest) = line.strip_prefix('>') {
        return rest.trim_start();
    }
    if let Some(rest) = line.strip_prefix(['-', '*', '+']).and_then(after_space) {
        return rest;
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && let Some(rest) = line[digits..].strip_prefix(['.', ')']).and_then(after_space)
    {
        return rest;
    }
    line
}

/// The lines of `text` with their markers off and the empty ones dropped.
fn lines(text: &str) -> Vec<&str> {
    text.lines().map(|l| strip_marker(l.trim()).trim()).filter(|l| !l.is_empty()).collect()
}

/// `line.split(/(?<=[.!?])\s+/)`: cut where white space follows a full stop, question or exclamation mark.
fn sentences(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev = ' ';
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_whitespace() && matches!(prev, '.' | '!' | '?') {
            let mut end = i + c.len_utf8();
            while let Some(&(j, w)) = chars.peek().filter(|(_, w)| w.is_whitespace()) {
                end = j + w.len_utf8();
                chars.next();
            }
            out.push(&line[start..i]);
            start = end;
            prev = ' ';
            continue;
        }
        prev = c;
    }
    if start < line.len() {
        out.push(&line[start..]);
    }
    out
}

/// A finished block shows the summary the harness sent, else its first sentence. A block still being written shows its
/// newest sentence, so the line follows along.
pub fn reasoning_gist(text: &str, summary: Option<&str>, live: bool) -> String {
    let gist = if live {
        lines(text).last().and_then(|l| sentences(l).last().copied()).unwrap_or("")
    } else {
        lines(summary.unwrap_or(""))
            .first()
            .copied()
            .or_else(|| lines(text).first().and_then(|l| sentences(l).first().copied()))
            .unwrap_or("")
    };
    if gist.chars().count() > MAX_GIST { format!("{}\u{2026}", gist.chars().take(MAX_GIST).collect::<String>()) } else { gist.to_string() }
}

/// Inline Markdown boiled down to what a one-line label can show: emphasis and code marks off, links as their text.
/// Fleet renders the gist as inline Markdown; Rune's `Markdown` is block level, so the marks are dropped.
pub fn plain_inline(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut chars = md.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '`' | '~' => {
                // A mark is dropped; a doubled one too.
                while chars.peek() == Some(&c) {
                    chars.next();
                }
            }
            '\\' => out.extend(chars.next()),
            _ => out.push(c),
        }
    }
    out
}
