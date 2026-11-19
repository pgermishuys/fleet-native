//! A message's markdown, styled as Fleet's `.md-content` (`client/src/assets/markdown.css`):
//! 15px body at 1.65, headings tinted toward the accent, a hairline card around code, inline code
//! on a 7% wash, tables with the same rules. Highlighting is Rune's built-in lexers painted with Fleet's eight
//! `--syntax-*` colours (`Theme::syntax`); highlight.js and Rune split tokens a little differently (see `docs/RUNE.md`).
use crate::conv::find::Marks;
use crate::fleet::find::MARK;
use crate::theme::{Theme, mono, sans};
use rune::El;
use rune_kit::markdown::{self, Case, Highlight, Markdown, MarkdownStyle};
use rune_kit::syntax::Syntax;

pub fn style(t: &Theme) -> MarkdownStyle {
    let [keyword, string, number, comment, function, attr, ty, literal] = t.syntax();
    // `muted` is only the list markers and h4+ headings in Rune's renderer; Fleet tints the markers (`--md-marker`).
    let mut s = MarkdownStyle::neutral(t.dark)
        .text(t.text)
        .muted(t.md_marker())
        .link(t.accent)
        .body_font(sans(15.0, 400.0, 1.65))
        .heading_font(1, sans(21.0, 600.0, 1.3).tracking(-0.3))
        .heading_font(2, sans(18.0, 600.0, 1.3).tracking(-0.18))
        .heading_font(3, sans(16.1, 600.0, 1.3))
        .heading_font(4, sans(15.0, 600.0, 1.3))
        .inline_code_font(0.86, 400.0)
        .inline_code_bg(t.wash(7.0))
        .code_block(t.wash(4.0), t.border)
        .code_block_padding(12.0, 10.0)
        .code_block_font(mono(12.5, 400.0, 1.55))
        .quote_rule(t.accent.alpha(0.55))
        .rule(t.border)
        .table(t.border, t.accent.alpha(if t.dark { 0.15 } else { 0.12 }))
        .table_header_text(t.text)
        .list(21.0, 4.0)
        .list_markers_outside(true)
        // Find's marks (`mark.find-hit`): `--status-waiting` at 38%, the current one solid with dark ink, no ring.
        .highlight_bg(t.idle.alpha(0.38))
        .highlight_fg(None)
        .highlight_ring(None)
        .highlight_current_bg(t.idle)
        .highlight_current_fg(Some(t.find_ink))
        .highlight_current_ring(None);
    for (kind, color) in [
        (Syntax::Keyword, keyword),
        (Syntax::Heading, keyword),
        (Syntax::String, string),
        (Syntax::Regex, string),
        (Syntax::Number, number),
        (Syntax::Comment, comment),
        (Syntax::Function, function),
        (Syntax::Property, attr),
        (Syntax::Attribute, attr),
        (Syntax::Type, ty),
        (Syntax::Variable, ty),
        (Syntax::Link, literal),
        // Fleet leaves these in the text colour.
        (Syntax::Punctuation, t.text),
        (Syntax::Operator, t.text),
        (Syntax::Parameter, t.text),
    ] {
        s = s.syntax(kind, color);
    }
    s
}

/// The same for a release's hand-written notes on Settings → Updates & about (`.whats-new__notes`): 13.5 px, a heading a level
/// down from a message's.
pub fn notes_style(t: &Theme) -> MarkdownStyle {
    style(t).body_font(sans(13.5, 400.0, 1.65)).heading_font(3, sans(14.5, 600.0, 1.3)).heading_font(4, sans(13.5, 600.0, 1.3))
}

/// One text part. `streaming` is true for the part still being written, which keeps its earlier
/// blocks mounted and its last table row from resizing the columns.
pub fn prose(style: &MarkdownStyle, id: &str, text: &str, streaming: bool) -> El {
    Markdown::new(text).selectable(true).streaming(streaming).render(style).key(id)
}

/// The same with find's marks over every match in what the text draws (`Markdown` maps them across bold, code, lists and
/// tables), the current one keyed so a scroll can find it.
pub fn prose_marked(style: &MarkdownStyle, id: &str, text: &str, streaming: bool, marks: Option<&Marks>) -> El {
    let Some(marks) = marks else { return prose(style, id, text, streaming) };
    let found = markdown::find(text, &marks.query, Case::Insensitive);
    let highlights = found.into_iter().enumerate().map(|(i, ranges)| {
        let on = Some(i) == marks.current;
        (ranges, if on { Highlight::current().key(MARK) } else { Highlight::new() })
    });
    Markdown::new(text).selectable(true).streaming(streaming).highlights(highlights).render(style).key(id)
}
