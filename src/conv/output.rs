//! A tool's output as Fleet draws it: terminal colour codes (ANSI SGR) become styled runs in Fleet's palette, every
//! other escape sequence is dropped, and a line rewritten with `\r` (a progress bar) keeps only its last frame. Ports
//! `lib/tool-output.ts` and the line windowing of `ToolOutputBody.vue`. No Rune in here: the view maps a colour name
//! to a theme colour (`view::toolcard`).
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// The eight palette names, in SGR order.
const PALETTE: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];

/// A colour a code asked for: one of Fleet's sixteen (by class name, `red`, `bright-green`), or a number triple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    /// `PALETTE[i]`, bright or not.
    Named {
        index: u8,
        bright: bool,
    },
    Rgb(u8, u8, u8),
}

impl Ink {
    /// Fleet's class suffix: `red`, `bright-red`. `None` for a true colour.
    pub fn name(self) -> Option<String> {
        match self {
            Ink::Named { index, bright } => Some(format!("{}{}", if bright { "bright-" } else { "" }, PALETTE[index as usize])),
            Ink::Rgb(..) => None,
        }
    }
}

/// How a run looks. All off and no colour is plain text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub fg: Option<Ink>,
    pub bg: Option<Ink>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
}

/// Text that looks one way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputSpan {
    pub text: String,
    pub look: Look,
}

impl OutputSpan {
    /// Fleet's `className`, space-separated (`ansi-fg-red ansi-bold`); empty for none. A true colour is a style instead.
    pub fn class_name(&self) -> String {
        let l = &self.look;
        let mut classes = Vec::new();
        if let Some(name) = l.fg.and_then(Ink::name) {
            classes.push(format!("ansi-fg-{name}"));
        }
        if let Some(name) = l.bg.and_then(Ink::name) {
            classes.push(format!("ansi-bg-{name}"));
        }
        for (on, class) in [(l.bold, "ansi-bold"), (l.dim, "ansi-dim"), (l.italic, "ansi-italic"), (l.underline, "ansi-underline")] {
            if on {
                classes.push(class.to_string());
            }
        }
        classes.join(" ")
    }

    /// Fleet's inline `style` for true colours: `color:rgb(1,2,3)`; empty for none.
    pub fn style(&self) -> String {
        let mut styles = Vec::new();
        if let Some(Ink::Rgb(r, g, b)) = self.look.fg {
            styles.push(format!("color:rgb({r},{g},{b})"));
        }
        if let Some(Ink::Rgb(r, g, b)) = self.look.bg {
            styles.push(format!("background-color:rgb({r},{g},{b})"));
        }
        styles.join(";")
    }
}

pub type OutputLine = Vec<OutputSpan>;

pub struct ParsedOutput {
    pub lines: Vec<OutputLine>,
    /// The output with no codes and only the last frame of each `\r` line, as Copy gives it.
    pub plain: String,
}

/// A 256-colour index: 0-15 are the palette, the rest the 6x6x6 cube and the grey ramp.
fn color256(index: u8) -> Ink {
    if index < 16 {
        return Ink::Named { index: index % 8, bright: index >= 8 };
    }
    if index < 232 {
        let n = index - 16;
        let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        return Ink::Rgb(level(n / 36), level(n / 6 % 6), level(n % 6));
    }
    let grey = 8 + (index - 232) * 10;
    Ink::Rgb(grey, grey, grey)
}

fn byte(value: Option<i64>) -> u8 {
    value.unwrap_or(0).clamp(0, 255) as u8
}

/// Applies one SGR sequence's parameters (`1;31`, `38;5;208`, ...) to the running look.
fn apply_sgr(look: Look, params: &str) -> Look {
    let codes: Vec<i64> = if params.is_empty() {
        vec![0]
    } else {
        params.split([';', ':']).map(|p| if p.is_empty() { 0 } else { p.parse().unwrap_or(i64::MAX) }).collect()
    };
    let mut next = look;
    let mut i = 0;
    while i < codes.len() {
        let code = codes[i];
        let named = |base: i64, bright: bool| Some(Ink::Named { index: (code - base) as u8, bright });
        match code {
            0 => next = Look::default(),
            1 => next.bold = true,
            2 => next.dim = true,
            3 => next.italic = true,
            4 => next.underline = true,
            22 => (next.bold, next.dim) = (false, false),
            23 => next.italic = false,
            24 => next.underline = false,
            30..=37 => next.fg = named(30, false),
            39 => next.fg = None,
            40..=47 => next.bg = named(40, false),
            49 => next.bg = None,
            90..=97 => next.fg = named(90, true),
            100..=107 => next.bg = named(100, true),
            38 | 48 => {
                let at = |k: usize| codes.get(i + k).copied();
                let picked = match at(1) {
                    Some(5) => {
                        let ink = color256(byte(at(2)));
                        i += 2;
                        ink
                    }
                    Some(2) => {
                        let ink = Ink::Rgb(byte(at(2)), byte(at(3)), byte(at(4)));
                        i += 4;
                        ink
                    }
                    _ => {
                        // An extended colour with no mode says nothing more; the rest of the sequence is still read.
                        i += 2;
                        continue;
                    }
                };
                if code == 38 { next.fg = Some(picked) } else { next.bg = Some(picked) }
            }
            // Inverse, blink, strikethrough, fonts and the rest draw nothing in Fleet.
            _ => {}
        }
        i += 1;
    }
    next
}

/// A control character Fleet acts on or drops: everything below space except tab, plus DEL.
fn is_control(b: u8) -> bool {
    (b < 0x20 && b != b'\t') || b == 0x7f
}

/// From `at` (just after an opener), the end of a string that runs to `ESC \`, to BEL when `bel` says so, and, broken,
/// to just before the next escape or line end.
fn string_end(raw: &[u8], mut at: usize, bel: bool) -> usize {
    while at < raw.len() && !(raw[at] == 0x1b || raw[at] == b'\n' || (bel && raw[at] == 0x07)) {
        at += 1;
    }
    match raw.get(at) {
        Some(0x07) => at + 1,
        Some(0x1b) if raw.get(at + 1) == Some(&b'\\') => at + 2,
        _ => at,
    }
}

/// Reads the escape sequence at `at`, calls `on_sgr` for a colour sequence, and returns where the text resumes.
fn skip_escape(raw: &[u8], at: usize, on_sgr: &mut dyn FnMut(&str)) -> usize {
    // CSI: `ESC [`, parameter bytes, intermediate bytes, a final byte.
    if raw.get(at + 1) == Some(&b'[') {
        let mut i = at + 2;
        while raw.get(i).is_some_and(|b| (0x30..=0x3f).contains(b)) {
            i += 1;
        }
        let params = &raw[at + 2..i];
        while raw.get(i).is_some_and(|b| (0x20..=0x2f).contains(b)) {
            i += 1;
        }
        if let Some(&fin) = raw.get(i).filter(|b| (0x40..=0x7e).contains(*b)) {
            // Only plain SGR (`ESC [ 1;31 m`) colours text; private ones (`ESC [ ? 25 h`) and cursor moves are dropped.
            if fin == b'm' && params.iter().all(|b| b.is_ascii_digit() || *b == b';' || *b == b':') {
                on_sgr(std::str::from_utf8(params).unwrap_or(""));
            }
            return i + 1;
        }
    }
    // OSC (titles, hyperlinks), then DCS, SOS, PM and APC strings.
    match raw.get(at + 1) {
        Some(b']') => return string_end(raw, at + 2, true),
        Some(b'P' | b'X' | b'^' | b'_') => return string_end(raw, at + 2, false),
        _ => {}
    }
    // Character-set picks (`ESC ( B`) and the other two- and three-byte sequences.
    let mut i = at + 1;
    while raw.get(i).is_some_and(|b| (0x20..=0x2f).contains(b)) {
        i += 1;
    }
    if raw.get(i).is_some_and(|b| (0x30..=0x7e).contains(b)) {
        return i + 1;
    }
    // A broken sequence (an `ESC` at the end, or with nothing after it): drop the `ESC` and keep the rest as text.
    at + 1
}

fn is_blank(line: &OutputLine) -> bool {
    line.iter().all(|s| s.text.trim().is_empty())
}

pub fn line_text(line: &OutputLine) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}

/// Parses a tool's output for drawing. Trailing blank lines are dropped; an empty output has no lines.
pub fn parse_tool_output(raw: &str) -> ParsedOutput {
    let bytes = raw.as_bytes();
    let mut lines: Vec<OutputLine> = Vec::new();
    let mut line: OutputLine = Vec::new();
    let mut look = Look::default();
    // A `\r` starts the next frame of the line; the old frame goes when the new one prints something, so a line that
    // ends in `\r` keeps what it showed.
    let mut rewind = false;

    fn push(line: &mut OutputLine, rewind: &mut bool, look: Look, text: &str) {
        if text.is_empty() {
            return;
        }
        if *rewind {
            line.clear();
            *rewind = false;
        }
        match line.last_mut() {
            Some(last) if last.look == look => last.text.push_str(text),
            _ => line.push(OutputSpan { text: text.to_string(), look }),
        }
    }

    let mut at = 0;
    while at < bytes.len() {
        let end = bytes[at..].iter().position(|&b| is_control(b)).map_or(bytes.len(), |n| at + n);
        push(&mut line, &mut rewind, look, &raw[at..end]);
        at = end;
        if at >= bytes.len() {
            break;
        }
        match bytes[at] {
            b'\n' => {
                lines.push(std::mem::take(&mut line));
                rewind = false;
                at += 1;
            }
            b'\r' => {
                // `\r\n` is a plain line end.
                if bytes.get(at + 1) != Some(&b'\n') {
                    rewind = true;
                }
                at += 1;
            }
            0x1b => {
                let mut sgr = |params: &str| look = apply_sgr(look, params);
                at = skip_escape(bytes, at, &mut sgr);
            }
            // Bell, backspace and the other controls draw nothing.
            _ => at += 1,
        }
    }
    lines.push(line);

    while lines.last().is_some_and(is_blank) {
        lines.pop();
    }
    let plain = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
    ParsedOutput { lines, plain }
}

const CACHE_SIZE: usize = 32;

thread_local! {
    static CACHE: RefCell<VecDeque<(String, Rc<ParsedOutput>)>> = const { RefCell::new(VecDeque::new()) };
}

/// `parse_tool_output`, remembered for the last few outputs: a row that's closed and opened again, or drawn again when
/// the conversation re-renders, doesn't parse its output twice.
pub fn parse_tool_output_cached(raw: &str) -> Rc<ParsedOutput> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(i) = cache.iter().position(|(k, _)| k == raw) {
            let hit = cache.remove(i).expect("position is in range");
            cache.push_back(hit.clone());
            return hit.1;
        }
        let parsed = Rc::new(parse_tool_output(raw));
        cache.push_back((raw.to_string(), parsed.clone()));
        if cache.len() > CACHE_SIZE {
            cache.pop_front();
        }
        parsed
    })
}

/// Lines shown until the user asks for the rest, as the `!` block does.
pub const COLLAPSED_LINES: usize = 12;
/// Lines drawn at most when expanded; past this the box says so, and Copy still takes everything.
pub const MAX_EXPANDED_LINES: usize = 2000;

/// Which lines of an output the box draws (`ToolOutputBody.vue` `shownCount`, `hiddenLines`, `isCapped`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub total: usize,
    pub shown: usize,
    pub is_long: bool,
    pub capped: bool,
}

impl Window {
    pub fn of(total: usize, expanded: bool) -> Window {
        let is_long = total > COLLAPSED_LINES;
        let shown = if !is_long {
            total
        } else if expanded {
            total.min(MAX_EXPANDED_LINES)
        } else {
            COLLAPSED_LINES
        };
        Window { total, shown, is_long, capped: expanded && total > MAX_EXPANDED_LINES }
    }

    /// Lines above the box that the toggle offers.
    pub fn hidden(&self) -> usize {
        self.total - self.shown
    }
}

/// `1,234`, as `toLocaleString("en-US")`.
pub fn thousands(n: usize) -> String {
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

/// `188 earlier lines`, `1 earlier line`.
pub fn count(n: usize, noun: &str) -> String {
    format!("{} {noun}{}", thousands(n), if n == 1 { "" } else { "s" })
}
