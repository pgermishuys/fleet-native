//! A mod's Markdown and Code: `components/mods/ModMarkdown.vue`, `mod-markdown.ts` and `ModCode.vue` (component 22, piece C).
//! Both draw as the conversation does (`view::prose::style`, its diff view); what a mod's text may reach out to is cut first
//! (`fleet::mods::markdown`).
use super::leaves::{flag, str_prop};
use crate::State;
use crate::conv::diff::parse_unified_diff;
use crate::fleet::mods::markdown;
use crate::theme::{Theme, mono, sans};
use crate::ui::text_menu;
use crate::view::{diff, prose};
use rune::{El, Tx, col, text};
use rune_kit::markdown::Markdown;
use serde_json::{Map, Value};

/// `ModMarkdown`: the text through the conversation's look, muted on `dimColor`. An image is its alt text, and a link is live only
/// for `http(s):` and `mailto:` (it opens outside the app); the rest is plain text.
pub fn markdown(t: &Theme, key: &str, props: &Map<String, Value>, size: f32) -> El {
    let style = at_size(prose::style(t), size);
    let style = if flag(props, "dimColor") { style.text(t.muted) } else { style };
    let src = markdown::source(str_prop(props, "text").unwrap_or_default());
    let md =
        Markdown::new(src).selectable(true).on_link_secondary(text_menu::on_link).on_link_tx(|_: &mut State, url: &str, tx: &mut Tx| {
            if markdown::is_live(url) {
                let _ = tx.open_url(url);
            }
        });
    text_menu::selectable(md.render(&style).key(key.to_string())).min_w(0.0)
}

/// The conversation's look at the size the site's text has: a message sets 15 px on its bubble, and `.md-content` takes the rest
/// in `em` (headings 1.4, 1.2, 1.075 and 1; code blocks stay 12.5 px), so in a 14 px pane the whole scale is a little smaller.
fn at_size(style: rune_kit::markdown::MarkdownStyle, size: f32) -> rune_kit::markdown::MarkdownStyle {
    style
        .body_font(sans(size, 400.0, 1.65))
        .heading_font(1, sans(size * 1.4, 600.0, 1.3).tracking(-0.015 * size * 1.4))
        .heading_font(2, sans(size * 1.2, 600.0, 1.3).tracking(-0.01 * size * 1.2))
        .heading_font(3, sans(size * 1.075, 600.0, 1.3))
        .heading_font(4, sans(size, 600.0, 1.3))
}

/// `ModCode`: a header (mono 11.5 px, muted) with the path and `:startLine`, then the source as a fenced block the conversation's way
/// (highlighted by `language`; it wraps, or with `wrap: "truncate"` keeps lines whole and scrolls sideways inside the block), or
/// with `format: "diff"` the tool cards' diff view.
pub fn code(s: &State, t: &Theme, key: &str, props: &Map<String, Value>) -> El {
    let source = str_prop(props, "source").unwrap_or_default();
    let mut c = col().key(key.to_string()).w_full().min_w(0.0).items_stretch();
    if let Some(head) = markdown::code_head(str_prop(props, "path"), props.get("startLine").and_then(Value::as_f64)) {
        c = c.child(
            text(head)
                .key(format!("{key}.head"))
                .font(mono(11.5, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .wrap_anywhere()
                .px(2.0)
                .pb(4.0)
                .min_w(0.0),
        );
    }
    if str_prop(props, "format") == Some("diff") {
        return c.child(diff::diff_view(t, s, &format!("{key}.diff"), &parse_unified_diff(source).lines));
    }
    let fenced = markdown::fenced(source, str_prop(props, "language").unwrap_or_default());
    let truncate = str_prop(props, "wrap") == Some("truncate");
    c.child(text_menu::selectable(
        Markdown::new(fenced).selectable(true).scroll_wide(truncate).render(&prose::style(t)).key(format!("{key}.body")),
    ))
}
