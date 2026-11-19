//! The code a file tab shows (`FileCanvas.vue` with CodeMirror's `editor-extensions.ts` and `theme.ts`, rows 5.1 to 5.4, 5.7, 5.8,
//! 5.9 and 5.11 of docs/parity/11-files.md): the lines in monospace 12.5 px at 20 px, an opaque number gutter that holds still while
//! long lines scroll sideways, colours by language, and text that can be selected and copied. Only the rows near the window are
//! built (`virtual_rows`), so a file of twenty thousand lines costs what a screenful does; what is worked out from the whole text
//! (the colours) is cached by the buffer's revision.
use crate::State;
use crate::conv::file_language::language_for;
use crate::fleet::file_buffers::Buffer;
use crate::theme::{Theme, mono};
use rune::{Color, El, GrowthSpec, ScrollSpec, Span, col, el, row, text};
use rune_kit::syntax::{Highlight, Paint, Syntax, highlight};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

/// A line: 12.5 px at CodeMirror's 1.6.
pub const LINE_H: f32 = 20.0;
/// The scrolling code: it takes the keyboard when a file is opened from Go to file or at a line (row 11.1).
pub const CODE: &str = "file-code";
const FONT: f32 = 12.5;
/// One mono digit at 12.5 px (JetBrains Mono advances 0.6 em).
const DIGIT_W: f32 = 7.5;
/// `.cm-content`'s `padding: 6px 0`.
const PAD_Y: f32 = 6.0;
/// CodeMirror's tab size.
const TAB: usize = 4;
/// `hangingIndents`: a wrapped line hangs under its indent, at most this many columns (Fleet's `min(indent + 2, 24)` less its two).
const MAX_HANG: usize = 22;

/// The colours of the whole text, as the highlighter gives them, with where each line starts in it.
pub struct Coloured {
    highlights: Vec<Highlight>,
    starts: Vec<usize>,
}

type Key = (String, String, String, u64);

thread_local! {
    /// The last few files coloured: tabs are drawn one at a time, and a view is rebuilt on every change of state.
    static CACHE: RefCell<Vec<(Key, Rc<Coloured>)>> = const { RefCell::new(Vec::new()) };
}

/// The colours of `lines`, worked out once per revision of the buffer. Rune colours the first 128 KiB of a source; the rest of a
/// larger file draws plain (docs/RUNE.md, "Code colours stop at 128 KiB").
pub fn coloured(key: Key, path: &str, lines: &[String]) -> Rc<Coloured> {
    if let Some(hit) = CACHE.with(|c| c.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return hit;
    }
    let mut starts = Vec::with_capacity(lines.len());
    let mut joined = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
    for l in lines {
        starts.push(joined.len());
        joined.push_str(l);
        joined.push('\n');
    }
    let mut highlights: Vec<Highlight> = match language_for(path) {
        Some(lang) => highlight(Some(lang), &joined).to_vec(),
        None => Vec::new(),
    };
    highlights.sort_by_key(|(r, _)| r.start);
    let made = Rc::new(Coloured { highlights, starts });
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(k, _)| !(k.0 == key.0 && k.1 == key.1 && k.2 == key.2));
        c.push((key, made.clone()));
        if c.len() > 4 {
            c.remove(0);
        }
    });
    made
}

/// How a kind of token is drawn (`fleetHighlight`): its colour, and whether italic, underlined or heavier.
struct Look {
    color: Color,
    italic: bool,
    underline: bool,
    weight: Option<f32>,
}

fn look(t: &Theme, kind: Syntax) -> Look {
    let [keyword, string, number, comment, function, attr, ty, literal] = t.syntax();
    let plain = |color| Look { color, italic: false, underline: false, weight: None };
    match kind {
        Syntax::Keyword => plain(keyword),
        Syntax::String | Syntax::Regex => plain(string),
        Syntax::Number => plain(number),
        Syntax::Comment => Look { italic: true, ..plain(comment) },
        Syntax::Function => plain(function),
        Syntax::Property | Syntax::Attribute => plain(attr),
        Syntax::Type => plain(ty),
        Syntax::Link => Look { underline: true, ..plain(literal) },
        // `--md-heading` is only defined inside a reasoning block, so in the editor a heading keeps the text colour.
        Syntax::Heading => Look { weight: Some(600.0), ..plain(t.text) },
        Syntax::Punctuation | Syntax::Operator => plain(t.muted),
        Syntax::Inserted => plain(t.running_text),
        Syntax::Deleted => plain(t.error_text),
        // CodeMirror leaves variable names and parameters in the text colour.
        Syntax::Variable | Syntax::Parameter => plain(t.text),
        _ => plain(t.text),
    }
}

/// The span of a highlight kind, in the line's own bytes.
fn span_of(t: &Theme, paint: Paint, range: Range<usize>) -> Span {
    let mut s = Span::new(range.start, range.end);
    match paint {
        Paint::Color(c) => s.color = Some(c),
        Paint::Kind(k) => {
            let l = look(t, k);
            s.color = Some(l.color);
            s.italic = l.italic;
            s.weight = l.weight;
            if l.underline {
                s.underline = Some(l.color);
            }
        }
    }
    s
}

/// What a character of the file is drawn as when CodeMirror draws a placeholder for it (`specialChars`): control characters as
/// their control picture, the invisible ones as a bullet, in `--error`.
fn placeholder(c: char) -> Option<char> {
    match c {
        '\t' => None,
        '\u{0}'..='\u{1f}' => char::from_u32(0x2400 + c as u32),
        '\u{7f}' => Some('\u{2421}'),
        '\u{80}'..='\u{9f}'
        | '\u{ad}'
        | '\u{61c}'
        | '\u{200b}'
        | '\u{200e}'
        | '\u{200f}'
        | '\u{2028}'
        | '\u{2029}'
        | '\u{202d}'
        | '\u{202e}'
        | '\u{2066}'
        | '\u{2067}'
        | '\u{2069}'
        | '\u{feff}'
        | '\u{fff9}'..='\u{fffc}' => Some('\u{2022}'),
        _ => None,
    }
}

fn needs_rewrite(line: &str) -> bool {
    line.chars().any(|c| c == '\t' || placeholder(c).is_some())
}

/// The text of one line as drawn, with the spans over it: tabs become spaces to the next tab stop and special characters their
/// placeholder, the spans moved to follow. A line with neither goes through untouched.
pub fn draw_line(t: &Theme, line: &str, base: Vec<(Range<usize>, Paint)>) -> (String, Vec<Span>) {
    if !needs_rewrite(line) {
        return (line.to_string(), base.into_iter().map(|(r, p)| span_of(t, p, r)).collect());
    }
    let mut out = String::with_capacity(line.len() + 8);
    // `at[old byte]` is where that byte of the line now starts; one past the end too.
    let mut at = vec![0usize; line.len() + 1];
    let mut col = 0;
    let mut specials: Vec<Range<usize>> = Vec::new();
    for (i, c) in line.char_indices() {
        for b in 0..c.len_utf8() {
            at[i + b] = out.len();
        }
        if c == '\t' {
            let n = TAB - col % TAB;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else if let Some(p) = placeholder(c) {
            let from = out.len();
            out.push(p);
            specials.push(from..out.len());
            col += 1;
        } else {
            out.push(c);
            col += 1;
        }
    }
    at[line.len()] = out.len();
    let mut spans: Vec<Span> = Vec::new();
    for (r, p) in base {
        let (a, b) = (at[r.start], at[r.end]);
        // A coloured run is cut around the placeholders inside it, which take `--error` instead.
        let mut from = a;
        for s in specials.iter().filter(|s| s.start >= a && s.end <= b) {
            if from < s.start {
                spans.push(span_of(t, p, from..s.start));
            }
            from = s.end;
        }
        if from < b {
            spans.push(span_of(t, p, from..b));
        }
    }
    for s in specials {
        let mut sp = Span::new(s.start, s.end);
        sp.color = Some(t.error);
        // A placeholder inside a coloured run was cut out of it above; one outside any run just takes its place.
        spans.retain(|o| !(o.start < sp.end && sp.start < o.end));
        spans.push(sp);
    }
    spans.sort_by_key(|s| s.start);
    (out, spans)
}

/// The highlights of line `i`, moved to start at its first byte, and cut at its end where a block comment runs on.
fn line_highlights(c: &Coloured, i: usize, len: usize) -> Vec<(Range<usize>, Paint)> {
    let (a, b) = (c.starts[i], c.starts[i] + len);
    let first = c.highlights.partition_point(|(r, _)| r.end <= a);
    c.highlights[first..]
        .iter()
        .take_while(|(r, _)| r.start < b)
        .map(|(r, p)| (r.start.max(a) - a..r.end.min(b) - a, *p))
        .filter(|(r, _)| r.start < r.end)
        .collect()
}

/// The width of the number gutter: CodeMirror sizes it to the widest number, never under 34 px (`min-width`), with its padding
/// of 8 and 10 inside.
pub fn gutter_width(count: usize) -> f32 {
    let digits = count.max(1).to_string().len() as f32;
    (digits * DIGIT_W + 18.0).max(34.0)
}

/// The leading spaces of a drawn line that a wrapped line hangs under, and how many columns they are (tabs are spaces by now).
pub fn hang_of(shown: &str) -> usize {
    shown.bytes().take_while(|b| *b == b' ').count().min(MAX_HANG)
}

fn line_row(t: &Theme, c: &Coloured, buffer: &Buffer, i: usize, gutter: f32, wrap: bool, active: bool) -> El {
    let lines = buffer.lines();
    let font = mono(FONT, 400.0, 1.6);
    let Some(line) = lines.get(i) else { return row().w_full().h(LINE_H) };
    let (shown, spans) = draw_line(t, line, line_highlights(c, i, line.len()));
    let mut number = row()
        .key(format!("code-no-{}", i + 1))
        .w(gutter)
        .h(LINE_H)
        .pl(8.0)
        .pr(10.0)
        .justify_end()
        .items_center()
        // Opaque, so code scrolled sideways doesn't show through the numbers.
        .fill(t.panel_bg)
        .child(text((i + 1).to_string()).font(font).color(if active { t.text } else { t.muted.alpha(0.7) }).single_line());
    if !wrap {
        number = number.sticky_left(0.0);
    }
    // `.cm-line`'s `padding: 0 2px 0 6px`. Only the code is selectable: a copy has no line numbers in it.
    let code = |shown: String, spans: Vec<Span>| {
        text(shown)
            .key(format!("code-text-{}", i + 1))
            .selectable(true)
            .selection_color(t.accent.alpha(0.3))
            .font(font)
            .color(t.text)
            .spans(spans)
    };
    // `.cm-activeLine`: the line a file was opened at, 3 % of `--text` over the code (the number's own fill stays opaque).
    let wash = if active { t.wash(3.0) } else { rune::Color::TRANSPARENT };
    if !wrap {
        return row().w_full().h(LINE_H).items_center().fill(wash).child(number).child(code(shown, spans).single_line().pl(6.0).pr(2.0));
    }
    // Rune has no negative text indent, so the indent is a column of its own and the text wraps beside it: the rows it wraps onto
    // start under the indent (CodeMirror's start two columns further in).
    let hang = hang_of(&shown);
    let rest = shown[hang..].to_string();
    let cut = hang as u32;
    let spans: Vec<Span> = spans
        .into_iter()
        .filter(|s| s.end > cut)
        .map(|mut s| {
            s.start = s.start.max(cut) - cut;
            s.end -= cut;
            s
        })
        .collect();
    let mut body = row().grow(1.0).shrink().min_w(0.0).items_start().pl(6.0).pr(2.0);
    if hang > 0 {
        body = body.child(el().key(format!("code-hang-{}", i + 1)).w(hang as f32 * DIGIT_W).h(LINE_H));
    }
    let wrapped = code(rest, spans).wrap_anywhere().grow(1.0).shrink().min_w(0.0);
    row().w_full().min_h(LINE_H).items_start().fill(wash).child(number).child(body.child(wrapped))
}

/// The viewport of lines for the buffer at (`machine`, `session`, `path`): scrolls both ways, builds the rows near the window only.
pub fn code_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, wrap: bool) -> El {
    let Some(buffer) = s.file_buffers.buffer(machine, session, path) else { return el() };
    let count = buffer.lines().len();
    let key: Key = (machine.into(), session.into(), path.into(), buffer.rev());
    let colours = coloured(key, path, buffer.lines());
    let gutter = gutter_width(count);
    let (theme, machine_id, session_id, file) = (*t, machine.to_string(), session.to_string(), path.to_string());
    let version = (path, buffer.rev(), wrap);
    let rows = col()
        .key("file-code-lines")
        .w_full()
        .py(PAD_Y)
        .selection_group(true)
        .growth(GrowthSpec::default())
        .virtual_rows(
            count,
            LINE_H,
            // A row keeps its measured height by key: a wrapped row is measured again, not taken from the unwrapped one.
            move |_: &State, i| std::borrow::Cow::Owned(format!("code-line-{}{}", i + 1, if wrap { "w" } else { "" })),
            move |s: &State, i| match s.file_buffers.buffer(&machine_id, &session_id, &file) {
                Some(buffer) => {
                    let marked = s
                        .go_to_file
                        .shown_line
                        .as_ref()
                        .is_some_and(|(se, pa, n)| *se == session_id && *pa == file && *n as usize == i + 1);
                    line_row(&theme, &colours, buffer, i, gutter, wrap, marked)
                }
                None => row().w_full().h(LINE_H),
            },
        )
        .virtual_rows_version(version);
    let view = col().key(CODE).focusable(true).w_full().grow(1.0).shrink().min_h(0.0);
    // Wrapped, there is nothing to scroll sideways.
    (if wrap { view.scroll(ScrollSpec { stick_bottom: false }) } else { view.scroll_xy() }).child(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn dark() -> Theme {
        Theme::of(true)
    }

    #[test]
    fn the_gutter_grows_with_the_widest_number() {
        assert_eq!(gutter_width(9), 34.0);
        assert_eq!(gutter_width(99), 34.0);
        assert_eq!(gutter_width(100), 40.5);
        assert_eq!(gutter_width(20_000), 55.5);
    }

    #[test]
    fn a_tab_goes_to_the_next_stop_and_the_colours_follow() {
        let t = dark();
        let (shown, spans) = draw_line(&t, "\treturn x", vec![(1..7, Paint::Kind(Syntax::Keyword))]);
        assert_eq!(shown, "    return x");
        assert_eq!((spans[0].start, spans[0].end), (4, 10));
    }

    #[test]
    fn control_characters_draw_as_error_placeholders() {
        let t = dark();
        let (shown, spans) = draw_line(&t, "a\u{1}b\u{200b}", Vec::new());
        assert_eq!(shown, "a\u{2401}b\u{2022}");
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().all(|s| s.color == Some(t.error)));
    }

    #[test]
    fn a_line_with_nothing_to_change_is_left_as_it_is() {
        let t = dark();
        let (shown, spans) = draw_line(&t, "let x = 1", vec![(0..3, Paint::Kind(Syntax::Keyword))]);
        assert_eq!(shown, "let x = 1");
        assert_eq!((spans[0].start, spans[0].end), (0, 3));
    }

    #[test]
    fn a_block_comment_is_cut_at_each_line() {
        let lines: Vec<String> = ["/* one", "two", "three */ x"].map(String::from).to_vec();
        let c = coloured(("m".into(), "s".into(), "a.c".into(), 1), "a.c", &lines);
        let covered = |i: usize| line_highlights(&c, i, lines[i].len()).iter().map(|(r, _)| r.end - r.start).sum::<usize>();
        assert_eq!(covered(0), 6);
        assert_eq!(covered(1), 3);
        assert!(covered(2) >= 8, "the comment's last line is coloured to its end");
    }

    #[test]
    fn colours_are_worked_out_once_per_revision() {
        let lines = vec!["let x = 1;".to_string()];
        let key = |rev| ("m".to_string(), "s".to_string(), "x.rs".to_string(), rev);
        let a = coloured(key(7), "x.rs", &lines);
        assert!(Rc::ptr_eq(&a, &coloured(key(7), "x.rs", &lines)));
        assert!(!Rc::ptr_eq(&a, &coloured(key(8), "x.rs", &lines)));
    }
}
