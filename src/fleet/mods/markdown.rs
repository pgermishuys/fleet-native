//! What a mod's Markdown and Code say before they are drawn (`components/mods/mod-markdown.ts`, `ModCode.vue`; component 22,
//! piece C). A stage 1 mod must not reach the network, so an image is its alt text, never a picture; a link is live only when it is a
//! web or mail address, and anything else (a file path the conversation would open in a tab, `/api/…`, `#frag`, `//host`) is plain
//! text keeping the link's words. Rune draws the conversation's Markdown from source, so the rules are applied to the source: the
//! parts that reach out are rewritten into text, and what is left is what the conversation draws.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// `LIVE_LINK`: `/^(?:https?:|mailto:)/i` over the trimmed destination.
pub fn is_live(dest: &str) -> bool {
    let d = dest.trim().as_bytes();
    let starts = |p: &str| d.len() >= p.len() && d[..p.len()].eq_ignore_ascii_case(p.as_bytes());
    starts("http:") || starts("https:") || starts("mailto:")
}

/// The options the conversation's renderer parses with, so a link or image is found where it draws one.
fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS
}

/// `text` with every punctuation mark escaped, so it reads as the words it is (`\<b\>x\</b\>`).
fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The index of the event that ends the one starting at `at`.
fn end_of(events: &[(Event<'_>, Range<usize>)], at: usize) -> usize {
    let mut depth = 0usize;
    for (i, (event, _)) in events.iter().enumerate().skip(at) {
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    events.len() - 1
}

/// `src` with each image replaced by its alt text and each dead link by the words inside it. A live link stays as written, and
/// so does everything else, so the conversation's own look draws the rest.
pub fn source(src: &str) -> String {
    let events: Vec<_> = Parser::new_ext(src, options()).into_offset_iter().collect();
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        match &events[i].0 {
            Event::Start(Tag::Image { .. }) => {
                let end = end_of(&events, i);
                // markdown-it's alt is the source between the brackets, drawn as the characters it is.
                let alt = match (events.get(i + 1..end).and_then(<[_]>::first), events.get(i + 1..end).and_then(<[_]>::last)) {
                    (Some((_, first)), Some((_, last))) => &src[first.start..last.end],
                    _ => "",
                };
                edits.push((events[i].1.clone(), literal(alt)));
                i = end;
            }
            Event::Start(Tag::Link { dest_url, .. }) if !is_live(dest_url) => {
                let end = end_of(&events, i);
                let whole = events[i].1.clone();
                let inner = events[i + 1..end].iter().map(|(_, r)| r.clone()).fold(None::<Range<usize>>, |all, r| match all {
                    None => Some(r),
                    Some(a) => Some(a.start.min(r.start)..a.end.max(r.end)),
                });
                match inner {
                    // The link's own marks go, and the words inside stay where they are (an image inside is its own edit).
                    Some(inner) => {
                        edits.push((whole.start..inner.start, String::new()));
                        edits.push((inner.end..whole.end, String::new()));
                    }
                    None => edits.push((whole, String::new())),
                }
            }
            // Raw HTML is never HTML (markdown-it's `html: false`): it is drawn as the characters it is.
            Event::Html(_) | Event::InlineHtml(_) => edits.push((events[i].1.clone(), literal(&src[events[i].1.clone()]))),
            _ => {}
        }
        i += 1;
    }
    edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
    let mut out = src.to_string();
    for (range, with) in edits {
        out.replace_range(range, &with);
    }
    out
}

/// The destinations of the links a mod's Markdown draws live, in the order they are read (what a press can open).
pub fn live_links(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (event, _) in Parser::new_ext(&source(src), options()).into_offset_iter() {
        if let Event::Start(Tag::Link { dest_url, .. }) = event
            && is_live(&dest_url)
        {
            out.push(dest_url.to_string());
        }
    }
    out
}

/// Whether the drawn Markdown still holds an image (it never does: a test's way to say so).
pub fn has_image(src: &str) -> bool {
    Parser::new_ext(&source(src), options()).any(|e| matches!(e, Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image)))
}

/// A fenced block for `code`: a fence longer than any run of backticks in it, so the source can't end it early, and the language
/// with only the characters a language name has (`[^\w+#.-]` goes).
pub fn fenced(source: &str, language: &str) -> String {
    let (mut longest, mut run) = (0usize, 0usize);
    for c in source.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat((longest + 1).max(3));
    let language: String = language.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '#' | '.' | '-')).collect();
    format!("{fence}{language}\n{source}\n{fence}")
}

/// `ModCode`'s header: the path, and `:startLine` when a number is given.
pub fn code_head(path: Option<&str>, start_line: Option<f64>) -> Option<String> {
    let path = path.filter(|p| !p.is_empty())?;
    Some(match start_line {
        Some(n) => format!("{path}:{}", if n.fract() == 0.0 { format!("{}", n as i64) } else { n.to_string() }),
        None => path.to_string(),
    })
}
