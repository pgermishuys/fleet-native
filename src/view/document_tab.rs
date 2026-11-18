//! The Document tab (component 14, piece E): `components/canvas/VisualCanvas.vue`. A 40 px bar with the title and what it is,
//! then the content: markdown drawn by Rune, HTML in a web view that runs no scripts (decision 11).
use super::prose;
use crate::State;
use crate::conv::visual::kind_label;
use crate::fleet::canvases::{VisualPayload, VisualType, visual_title};
use crate::theme::{Theme, sans};
use rune::{ScrollSpec, col, row, text};

pub fn document_tab(s: &State, t: &Theme, payload: &VisualPayload) -> rune::El {
    let title = visual_title(payload);
    let bar = row()
        .key("document-bar")
        .w_full()
        .h(40.0)
        .px(14.0)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .border_layout()
        .child(
            text(title.clone())
                .key("document-title")
                .font(sans(12.5, 600.0, 1.4))
                .color(t.text)
                .truncate()
                .min_w(0.0)
                .shrink()
                .help_when_truncated(title),
        )
        .child(text(kind_label(payload.kind)).key("document-kind").font(sans(12.0, 400.0, 1.4)).color(t.muted).min_w(0.0));
    let content = match payload.kind {
        VisualType::Markdown => col().key("document-content").w_full().grow(1.0).min_h(0.0).child(
            col()
                .w_full()
                .px(14.0)
                .py(12.0)
                .child(prose::prose(&prose::style(t), "document-markdown", &payload.content, false))
                .scroll(ScrollSpec { stick_bottom: false })
                .key("document-scroll")
                .grow(1.0)
                .min_h(0.0),
        ),
        VisualType::Html => col().key("document-content").w_full().grow(1.0).min_h(0.0).child(crate::web::html_body(
            s,
            t,
            "document-html",
            &payload.content,
        )),
    };
    col().key("document-tab").w_full().h_full().min_h(0.0).child(bar).child(content)
}
