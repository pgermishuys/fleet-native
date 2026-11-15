//! Splits a turn failure's message into the sentence to show and the stack trace to keep folded
//! (`lib/turn-error.ts`). OpenCode puts the whole trace in the message, which buried the one sentence the reader needs.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnErrorText {
    /// What went wrong, in the harness's words, without its stack trace.
    pub summary: String,
    /// The stack trace the harness put after the message, if any.
    pub details: Option<String>,
}

/// Where the first stack frame begins: whitespace, `at`, whitespace, then text on the same line that holds
/// `:<digits>:<digits>` (`STACK_FRAME`). Returns the byte index of the whitespace run before `at`.
fn frame_start(text: &str) -> Option<usize> {
    let mut previous_space = false;
    for (i, c) in text.char_indices() {
        let space = c.is_whitespace();
        // `\s+` is greedy, so a frame starts at the first whitespace of a run.
        if space && !previous_space {
            let after = text[i..].trim_start();
            if let Some(rest) = after.strip_prefix("at").filter(|r| r.starts_with(char::is_whitespace))
                && ends_in_position(rest.trim_start().split('\n').next().unwrap_or(""))
            {
                return Some(i);
            }
        }
        previous_space = space;
    }
    None
}

/// Whether `line` holds a `:<digits>:<digits>` anywhere (the lazy match takes the first one).
fn ends_in_position(line: &str) -> bool {
    let b = line.as_bytes();
    (0..b.len()).any(|i| {
        if b[i] != b':' {
            return false;
        }
        let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
        let first = digits(i + 1);
        first > 0 && b.get(i + 1 + first) == Some(&b':') && digits(i + 2 + first) > 0
    })
}

/// `^[A-Z][\w$]*Error:\s+` removed from the front, when it matches.
fn without_error_prefix(head: &str) -> &str {
    let mut chars = head.char_indices();
    if !chars.next().is_some_and(|(_, c)| c.is_ascii_uppercase()) {
        return head;
    }
    // The longest run of word characters and `$`, then back off to the last `Error:` followed by whitespace.
    let word_end = head.char_indices().find(|(_, c)| !(c.is_alphanumeric() || *c == '_' || *c == '$')).map_or(head.len(), |(i, _)| i);
    let word = &head[..word_end];
    if !word.ends_with("Error") || !head[word_end..].starts_with(':') {
        return head;
    }
    let after = &head[word_end + 1..];
    let trimmed = after.trim_start();
    if trimmed.len() == after.len() {
        return head;
    }
    trimmed
}

pub fn split_turn_error_message(message: &str) -> TurnErrorText {
    let text = message.trim();
    let frame = frame_start(text);
    let head = frame.map_or(text, |i| text[..i].trim());
    let details = frame.map(|i| text[i..].trim().to_string());
    let stripped = without_error_prefix(head);
    let summary = if !stripped.is_empty() {
        stripped
    } else if !head.is_empty() {
        head
    } else {
        text
    };
    TurnErrorText { summary: summary.to_string(), details }
}

/// The web addresses in `text` as byte ranges, the way `linkifyText` finds them for the failure paragraph: `http(s)://`
/// up to whitespace, without the sentence's closing punctuation (and a bracket the address never opened).
pub fn url_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("http") {
        let start = from + found;
        let scheme = ["https://", "http://"].iter().find(|s| text[start..].to_ascii_lowercase().starts_with(*s));
        let Some(scheme) = scheme else {
            from = start + 4;
            continue;
        };
        let mut end = start + text[start..].find(char::is_whitespace).unwrap_or(text.len() - start);
        while let Some(last) = text[start..end].chars().next_back() {
            let opened = text[start..end].matches('(').count() >= text[start..end].matches(')').count();
            if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"') || (last == ')' && !opened) {
                end -= last.len_utf8();
            } else {
                break;
            }
        }
        if end > start + scheme.len() {
            out.push(start..end);
        }
        from = end.max(start + 4);
    }
    out
}
