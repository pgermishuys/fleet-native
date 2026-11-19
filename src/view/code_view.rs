//! The code a file tab shows (`FileCanvas.vue` with CodeMirror's `editor-extensions.ts` and `theme.ts`, rows 5.1 to 5.4, 5.7, 5.8,
//! 5.9 and 5.11 of docs/parity/11-files.md): the lines in monospace 12.5 px at 20 px, an opaque number gutter that holds still while
//! long lines scroll sideways, colours by language, and text that can be selected and copied. Only the rows near the window are
//! built (`virtual_rows`), so a file of twenty thousand lines costs what a screenful does; what is worked out from the whole text
//! (the colours) is cached by the buffer's revision.
use crate::State;
use crate::conv::changed_lines::{base_lines, changed_lines};
use crate::conv::file_language::language_for;
use crate::fleet::file_buffers::Buffer;
use crate::fleet::file_lines::{LineRange, SelectedText, text_line};
use crate::theme::{Theme, mono};
use rune::{Color, Drag, El, GrowthSpec, Phase, ScrollSpec, Span, TextHighlight, col, el, row, text};
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
pub(super) fn line_highlights(c: &Coloured, i: usize, len: usize) -> Vec<(Range<usize>, Paint)> {
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

/// What a line is drawn with besides its text: the tint behind it, the words marked in it, and the thin bars in the gutters after
/// its number.
#[derive(Default)]
pub struct Decor {
    /// The line's number; none for a removed line in Diff.
    pub number: Option<usize>,
    /// The tint of the whole line (a changed or removed line in Diff, the flash).
    pub fill: Option<Color>,
    /// Words marked inside the line, as byte ranges of the line's own text.
    pub words: Vec<(Range<usize>, Color)>,
    /// Edit's stripe gutter (`.cm-agent-gutter`, 3 px and 2 px of margin): drawn when it is `Some`, with the bar when `true`.
    pub stripe: Option<bool>,
    /// Diff's change gutter (`.cm-changeGutter`, 3 px): drawn when it is `Some`, with this colour (none for an unchanged line).
    pub change: Option<Option<Color>>,
    /// The line a file was opened at: its number in `--text` and a 3 % wash over the line.
    pub active: bool,
}

/// The keys of one line's parts, so a test or a picture can find them.
pub struct LineKeys {
    pub number: String,
    pub text: String,
    pub hang: String,
}

/// A byte range of `line` as the same range of the line as drawn, where tabs became spaces and special characters their placeholder.
pub fn shown_range(line: &str, r: Range<usize>) -> Range<usize> {
    if !needs_rewrite(line) {
        return r;
    }
    let (mut at, mut col, mut out) = (Vec::with_capacity(line.len() + 1), 0, 0);
    for (i, c) in line.char_indices() {
        at.resize(i, 0);
        let n = if c == '\t' { TAB - col % TAB } else { placeholder(c).map_or(c.len_utf8(), char::len_utf8) };
        at.extend(std::iter::repeat_n(out, c.len_utf8()));
        out += n;
        col += if c == '\t' { n } else { 1 };
    }
    at.push(out);
    at[r.start]..at[r.end]
}

/// One line of code: the gutters (its number, and the bars), then its text, wrapped or on one line.
pub fn code_line(t: &Theme, keys: LineKeys, gutter: f32, shown: String, spans: Vec<Span>, wrap: bool, d: &Decor) -> El {
    let font = mono(FONT, 400.0, 1.6);
    let stripe_key = format!("{}-stripe", keys.number);
    let number_key = keys.number.clone();
    let mut number_col = row().key(keys.number).w(gutter).h(LINE_H).pl(8.0).pr(10.0).justify_end().items_center().child(
        text(d.number.map(|n| n.to_string()).unwrap_or_default())
            .key(format!("{number_key}-t"))
            .font(font)
            .color(if d.active { t.text } else { t.muted.alpha(0.7) })
            .single_line(),
    );
    number_col = number_col.fill(t.panel_bg);
    // The bars share the gutter's opaque ground, so code scrolled sideways doesn't show through them either.
    let bar = |w: f32, margin: f32, color: Option<Color>, key: String| {
        let mut b = row().w(w + margin).h(LINE_H).items_stretch().fill(t.panel_bg);
        if let Some(color) = color {
            b = b.child(el().key(key).w(w).h(LINE_H).radius(2.0).fill(color));
        }
        b
    };
    let mut gutters = row().items_center().child(number_col);
    if let Some(marked) = d.stripe {
        gutters = gutters.child(bar(3.0, 2.0, marked.then_some(t.accent), stripe_key));
    }
    if let Some(color) = d.change {
        // The change gutter's bar is square: Fleet's `.cm-changedLineGutter` has no radius.
        let mut b = row().w(3.0).h(LINE_H).fill(t.panel_bg);
        if let Some(c) = color {
            b = b.fill(c);
        }
        gutters = gutters.child(b);
    }
    if !wrap {
        gutters = gutters.sticky_left(0.0);
    }
    // `.cm-line`'s `padding: 0 2px 0 6px`. Only the code is selectable: a copy has no line numbers in it.
    let code = |shown: String, spans: Vec<Span>, words: Vec<TextHighlight>| {
        text(shown).selectable(true).selection_color(t.accent.alpha(0.3)).font(font).color(t.text).spans(spans).text_highlights(words)
    };
    let words = |from: usize| -> Vec<TextHighlight> {
        d.words
            .iter()
            .filter(|(r, _)| r.end > from)
            .map(|(r, c)| TextHighlight::new(r.start.max(from) - from..r.end - from, *c).pad(0.0))
            .collect()
    };
    // `.cm-activeLine`: 3 % of `--text` over the code (the number's own fill stays opaque); a tint (Diff, the flash) goes over it.
    let tint = |r: El| match d.fill {
        Some(f) => r.fill(f),
        None if d.active => r.fill(t.wash(3.0)),
        None => r,
    };
    if !wrap {
        let code = code(shown, spans, words(0)).key(keys.text).single_line().pl(6.0).pr(2.0);
        return tint(row().w_full().h(LINE_H).items_center()).child(gutters).child(code);
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
        body = body.child(el().key(keys.hang).w(hang as f32 * DIGIT_W).h(LINE_H));
    }
    let wrapped = code(rest, spans, words(hang)).key(keys.text).wrap_anywhere().grow(1.0).shrink().min_w(0.0);
    tint(row().w_full().min_h(LINE_H).items_start()).child(gutters).child(body.child(wrapped))
}

/// The tint of the flash on a line: `--accent` 28 % easing out to nothing (`cm-agent-flash`, 1.6 s `ease-out`).
pub fn flash_tint(t: &Theme, progress: f64) -> Color {
    let left = 1.0 - progress;
    // `ease-out` is cubic-bezier(0, 0, 0.58, 1): close to this.
    let eased = 1.0 - left * left;
    t.accent.alpha(0.28 * (1.0 - eased as f32))
}

/// What the view marks besides the text: the lines of the stripe and the lines flashing.
#[derive(Default, Clone)]
pub struct Marks {
    pub stripe: Rc<Vec<usize>>,
    pub flash: Option<crate::fleet::file_live::Flash>,
    /// Find in a file's matches on the line, as ranges of the line as drawn, and whether each is the current one.
    pub finds: Vec<(Range<usize>, bool)>,
}

type StripeKey = (String, String, String, u64, u64);

thread_local! {
    /// The stripe of the last few files: worked out once per revision of the text and of its base.
    static STRIPES: RefCell<Vec<(StripeKey, Rc<Vec<usize>>)>> = const { RefCell::new(Vec::new()) };
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// The lines that differ from the git base, for the stripe (`markStripe`): from the base when the file is among the changes and its
/// base has loaded, else from the text as last read or saved, which only differs once the buffer is edited (slice 2).
pub fn stripe_lines(s: &State, machine: &str, session: &str, path: &str, buffer: &Buffer) -> Rc<Vec<usize>> {
    let base = s.file_base(session, path);
    if base.is_none() && !buffer.is_dirty() {
        return Rc::default();
    }
    let key: StripeKey = (machine.into(), session.into(), path.into(), buffer.rev(), base.map_or(0, hash_of));
    if let Some(hit) = STRIPES.with(|c| c.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return hit;
    }
    let reference = match base {
        Some(before) => base_lines(before),
        None => buffer.saved_lines().to_vec(),
    };
    let made = Rc::new(changed_lines(&reference, buffer.lines()));
    STRIPES.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|(k, _)| !(k.0 == key.0 && k.1 == key.1 && k.2 == key.2));
        c.push((key, made.clone()));
        if c.len() > 4 {
            c.remove(0);
        }
    });
    made
}

/// One line: its number, which picks it (a press, Shift, a drag down the numbers), and its code. `picked` lines are filled `--accent`
/// 14 % across, their number `--accent` 600 (`.cm-picked-line`, `.cm-picked-gutter`); `active` is the line a file was opened at.
#[allow(clippy::too_many_arguments)]
fn line_row(
    t: &Theme,
    c: &Coloured,
    buffer: &Buffer,
    path: &str,
    i: usize,
    gutter: f32,
    wrap: bool,
    active: bool,
    picked: Option<LineRange>,
    marks: &Marks,
) -> El {
    let lines = buffer.lines();
    let font = mono(FONT, 400.0, 1.6);
    let Some(line) = lines.get(i) else { return row().w_full().h(LINE_H) };
    let (shown, spans) = draw_line(t, line, line_highlights(c, i, line.len()));
    let n = i as u32 + 1;
    let is_picked = picked.is_some_and(|r| r.contains(n));
    let (number_fill, number_ink, number_weight) = if is_picked {
        (t.mix(t.accent, t.panel_bg, 0.14), t.accent, 600.0)
    } else if active {
        (t.panel_bg, t.text, 400.0)
    } else {
        (t.panel_bg, t.muted.alpha(0.7), 400.0)
    };
    let owner = path.to_string();
    let number = row()
        .key(format!("code-no-{}", i + 1))
        .w(gutter)
        .h(LINE_H)
        .pl(8.0)
        .pr(10.0)
        .justify_end()
        .items_center()
        // Opaque, so code scrolled sideways doesn't show through the numbers.
        .fill(number_fill)
        .child(text(n.to_string()).font(mono(FONT, number_weight, 1.6)).color(number_ink).single_line())
        // `onNumberMouseDown`: the press picks, and while the button is down the line under the pointer extends the pick. The pointer is
        // placed by whole lines from the pressed one; wrapped lines are taller, so there the count runs ahead (docs/RUNE.md).
        .on_drag(move |s: &mut State, d: &mut Drag, tx| match d.phase {
            Phase::Down => d.data = s.press_line_number(&owner, n, tx.mods().shift, tx) as f32,
            Phase::Move => {
                let over = (n as f32 + (d.y / LINE_H).floor()).max(1.0) as u32;
                s.drag_line_number(&owner, d.data as u32, over);
            }
            Phase::Up => {}
        });
    // Edit's stripe gutter: a 3 px bar in `--accent` on the numbers' own ground, 2 px from them (`.cm-agent-gutter`).
    let mut gutters = row().items_center().child(number);
    gutters = gutters.child(
        row().w(5.0).h(LINE_H).items_stretch().fill(t.panel_bg).children(
            marks
                .stripe
                .binary_search(&(n as usize))
                .is_ok()
                .then(|| el().key(format!("code-no-{n}-stripe")).w(3.0).h(LINE_H).radius(2.0).fill(t.accent)),
        ),
    );
    let number = if wrap { gutters } else { gutters.sticky_left(0.0) };
    // `.cm-line`'s `padding: 0 2px 0 6px`. Only the code is selectable: a copy has no line numbers in it.
    let code = |shown: String, spans: Vec<Span>, from: usize| {
        let found = marks
            .finds
            .iter()
            .filter(|(r, _)| r.end > from)
            .map(|(r, now)| super::file_find::highlight(t, r.start.max(from) - from..r.end - from, *now));
        text(shown)
            .key(format!("code-text-{}", i + 1))
            .selectable(true)
            .selection_color(t.accent.alpha(0.3))
            .font(font)
            .color(t.text)
            .spans(spans)
            .text_highlights(found)
    };
    // `.cm-activeLine`: the line a file was opened at, 3 % of `--text` over the code (the number's own fill stays opaque); a picked
    // line takes the pick's fill instead.
    let line_fill = if is_picked {
        t.accent.alpha(0.14)
    } else if let Some(f) = marks.flash.as_ref().filter(|f| (f.lines.0..=f.lines.1).contains(&(n as usize))) {
        flash_tint(t, f.progress)
    } else if active {
        t.wash(3.0)
    } else {
        Color::TRANSPARENT
    };
    if !wrap {
        return row()
            .w_full()
            .h(LINE_H)
            .fill(line_fill)
            .items_center()
            .child(number)
            .child(code(shown, spans, 0).single_line().pl(6.0).pr(2.0));
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
    let wrapped = code(rest, spans, hang).wrap_anywhere().grow(1.0).shrink().min_w(0.0);
    row().w_full().min_h(LINE_H).fill(line_fill).items_start().child(number).child(body.child(wrapped))
}

/// The viewport of lines for the buffer at (`machine`, `session`, `path`): scrolls both ways, builds the rows near the window only.
pub fn code_view(s: &State, t: &Theme, machine: &str, session: &str, path: &str, wrap: bool) -> El {
    let Some(buffer) = s.file_buffers.buffer(machine, session, path) else { return el() };
    let count = buffer.lines().len();
    let key: Key = (machine.into(), session.into(), path.into(), buffer.rev());
    let colours = coloured(key, path, buffer.lines());
    let gutter = gutter_width(count);
    let (theme, machine_id, session_id, file) = (*t, machine.to_string(), session.to_string(), path.to_string());
    let picked = s.file_pick(machine, session, path);
    let stripe = stripe_lines(s, machine, session, path, buffer);
    // The rows near the window are kept while this stays the same: a flash changes it a hundred times over its second and a half.
    let flash = s.file_live.flashing(machine, session, path, s.clock).map(|f| (f.progress * 100.0) as u32);
    let version = (path, buffer.rev(), wrap, picked, hash_of(&format!("{stripe:?}")), flash, s.file_find.stamp(session, path));
    let owner = path.to_string();
    let rows = col()
        .key("file-code-lines")
        .w_full()
        .py(PAD_Y)
        .selection_group(true)
        // A selection counts as the lines it spans for the lines bar, and selecting text takes the pick away (rows 8.3, 8.4).
        .on_selection_change(move |s: &mut State, change, _| match change.selection() {
            Some(sel) => {
                let (texts, _) = crate::fleet::review_pick::picked_texts(sel);
                let texts: Vec<SelectedText> =
                    texts.iter().filter_map(|t| Some(SelectedText { line: text_line(&t.key)?, start: t.start, end: t.end })).collect();
                s.read_file_selection(&owner, &texts);
            }
            None => s.clear_file_selection_state(&owner),
        })
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
                    let line = buffer.lines().get(i).map_or("", String::as_str);
                    let finds = super::file_find::finds(s, &session_id, &file, i, &|r| shown_range(line, r));
                    let marks =
                        Marks { stripe: stripe.clone(), flash: s.file_live.flashing(&machine_id, &session_id, &file, s.clock), finds };
                    line_row(&theme, &colours, buffer, &file, i, gutter, wrap, marked, s.file_pick(&machine_id, &session_id, &file), &marks)
                }
                None => row().w_full().h(LINE_H),
            },
        )
        .virtual_rows_version(version);
    // Focusable, so a press on the numbers can give it the keyboard and Esc reaches it; no ring (CodeMirror's has none).
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
