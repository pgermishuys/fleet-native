//! A page's own text shown in a web view with no scripts (decision 11; `HtmlRenderer.vue`'s `srcdoc` frame under
//! `sandbox="allow-same-origin"`), for the Document tab and, from component 11, a rendered `.html` file.
use crate::State;
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{El, col, icon, text};

/// The text with a policy that refuses every script put first in its head, so only the browser's own parsing of it is left
/// (the frame's missing `allow-scripts`).
pub fn html_document(content: &str) -> String {
    const POLICY: &str = r#"<meta http-equiv="Content-Security-Policy" content="script-src 'none'">"#;
    let lower = content.to_ascii_lowercase();
    // Right after the opening `<head …>` tag when there is one; else before everything, which makes the browser start a head.
    if let Some(at) = lower.find("<head")
        && let Some(end) = lower[at..].find('>')
        && !lower[at..at + end].contains("<headline")
    {
        let end = at + end + 1;
        return format!("{}{POLICY}{}", &content[..end], &content[end..]);
    }
    format!("<head>{POLICY}</head>{content}")
}

/// What `html_body` shows where a web view is made: a white frame (a page with no background of its own expects one) that the
/// web view covers, whose rectangle the host is told. `key` names it for the host and for tests.
pub fn html_body(s: &State, t: &Theme, key: &str, content: &str) -> El {
    if s.web.can_show() {
        return col()
            .key(key.to_string())
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .fill(t.page_frame())
            .on_layout(|s: &mut State, r| s.web.rect = Some(r))
            .on_screen_layout(|s: &mut State, r| s.web.screen = Some(r));
    }
    let _ = content;
    col()
        .key(format!("{key}-fallback"))
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .fill(t.panel_bg)
        .center()
        .gap(12.0)
        .padding(24.0)
        .child(icon(&APP_WINDOW, 24.0).color(t.muted))
        .child(text(crate::view::page_tab::FALLBACK_TEXT).font(sans(13.0, 400.0, 1.5)).color(t.text).wrap().align(rune::Align::Center))
}
