//! Links in the output (13.75, 13.76): the web-links addon's URLs, found on the whole logical line (rows the terminal wrapped
//! are joined), and OSC 8 hyperlinks. The view underlines the one under the pointer and a click opens it.
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;

/// What the pointer is over: where to open, and the cells to underline as (view row, first column, one past the last).
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub url: String,
    pub spans: Vec<(usize, u16, u16)>,
}

/// The web-links addon's pattern `(https?|HTTPS?):[/]{2}[^\s"'!*(){}|\\\^<>`]*[^\s"':,.!?{}|\\\^~\[\]`()<>]`, by hand: as
/// `(start, end)` char ranges of `text`, left to right.
pub fn url_ranges(text: &[char]) -> Vec<(usize, usize)> {
    let inside = |c: char| !c.is_whitespace() && !"\"'!*(){}|\\^<>`".contains(c);
    let last = |c: char| !c.is_whitespace() && !"\"':,.!?{}|\\^~[]`()<>".contains(c);
    let starts = |i: usize| {
        ["http://", "https://", "HTTP://", "HTTPS://"].iter().find_map(|p| {
            let p: Vec<char> = p.chars().collect();
            (text.len() >= i + p.len() && text[i..i + p.len()] == p[..]).then_some(p.len())
        })
    };
    let mut found = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let Some(head) = starts(i) else {
            i += 1;
            continue;
        };
        // The body is greedy and gives back what the last character may not be (the regex backtracks the same way).
        let mut end = i + head;
        while end < text.len() && inside(text[end]) {
            end += 1;
        }
        while end > i + head && !last(text[end - 1]) {
            end -= 1;
        }
        if end > i + head {
            found.push((i, end));
            i = end;
        } else {
            i += head;
        }
    }
    found
}

/// Whether an OSC 8 target may be opened without asking. xterm asks with a browser `confirm`; the native app has no such
/// dialog, so it opens only the schemes a click can't do harm with.
pub fn safe_target(uri: &str) -> bool {
    let scheme = uri.split(':').next().unwrap_or("").to_ascii_lowercase();
    matches!(scheme.as_str(), "http" | "https" | "mailto") && uri.contains(':')
}

/// The link at `col` of view row `row`, if there is one: an OSC 8 one first, then a URL in the text.
pub fn link_at<T: EventListener>(term: &Term<T>, row: usize, col: usize) -> Option<Link> {
    let grid = term.grid();
    let (lines, cols) = (term.screen_lines(), term.columns());
    if row >= lines || col >= cols {
        return None;
    }
    let offset = grid.display_offset() as i32;
    let line_of = |v: i32| Line(v - offset);
    let cell = &grid[line_of(row as i32)][Column(col)];
    // A wide character's second cell belongs to the first.
    let col = if cell.flags.contains(Flags::WIDE_CHAR_SPACER) { col.saturating_sub(1) } else { col };
    if let Some(h) = grid[line_of(row as i32)][Column(col)].hyperlink() {
        let same = |c: usize| grid[line_of(row as i32)][Column(c)].hyperlink().is_some_and(|o| o.uri() == h.uri());
        let (mut a, mut b) = (col, col + 1);
        while a > 0 && same(a - 1) {
            a -= 1;
        }
        while b < cols && same(b) {
            b += 1;
        }
        return safe_target(h.uri()).then(|| Link { url: h.uri().to_string(), spans: vec![(row, a as u16, b as u16)] });
    }
    // The logical line: the rows run into each other while one wraps. History above the view counts.
    let top = -(grid.history_size() as i32);
    let wraps = |v: i32| grid[Line(v)][Column(cols - 1)].flags.contains(Flags::WRAPLINE);
    let here = row as i32 - offset;
    let (mut first, mut last) = (here, here);
    while first > top && here - first < 20 && wraps(first - 1) {
        first -= 1;
    }
    while last < lines as i32 - 1 && last - here < 20 && wraps(last) {
        last += 1;
    }
    let mut text = Vec::new();
    // For each character: its grid line and column.
    let mut at = Vec::new();
    for v in first..=last {
        for c in 0..cols {
            let cell = &grid[Line(v)][Column(c)];
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            text.push(cell.c);
            at.push((v, c));
        }
    }
    let me = text.iter().enumerate().position(|(i, _)| at[i] == (here, col))?;
    let (s, e) = url_ranges(&text).into_iter().find(|&(s, e)| (s..e).contains(&me))?;
    let mut spans: Vec<(usize, u16, u16)> = Vec::new();
    for &(v, c) in &at[s..e] {
        let view = (v + offset) as usize;
        match spans.last_mut() {
            Some(last) if last.0 == view => last.2 = c as u16 + 1,
            _ => spans.push((view, c as u16, c as u16 + 1)),
        }
    }
    Some(Link { url: text[s..e].iter().collect(), spans })
}
