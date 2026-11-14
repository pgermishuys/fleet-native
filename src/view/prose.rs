//! A message's markdown, styled as Fleet's `.md-content` (`client/src/assets/markdown.css`):
//! 15px body at 1.65, headings tinted toward the accent, a hairline card around code, inline code
//! on a 7% wash, tables with the same rules. Highlighting is Rune's built-in lexers; Fleet's
//! highlight.js colours come from its code themes, which land with Settings.
use crate::theme::{Theme, mono, sans};
use rune::El;
use rune_kit::markdown::{Markdown, MarkdownStyle};

pub fn style(t: &Theme) -> MarkdownStyle {
    MarkdownStyle::neutral(t.dark)
        .text(t.text)
        .muted(t.muted)
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
}

/// One text part. `streaming` is true for the part still being written, which keeps its earlier
/// blocks mounted and its last table row from resizing the columns.
pub fn prose(style: &MarkdownStyle, id: &str, text: &str, streaming: bool) -> El {
    Markdown::new(text).selectable(true).streaming(streaming).render(style).key(id)
}
