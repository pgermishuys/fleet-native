//! The Document tab (component 14, piece E): `components/canvas/VisualCanvas.vue`. A 40 px bar with the title and what it is,
//! then the content: markdown drawn by Rune, HTML in a web view that runs no scripts (decision 11).
use super::prose;
use crate::State;
use crate::Tx;
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
                .help_when_truncated(title.clone()),
        )
        .child(text(kind_label(payload.kind)).key("document-kind").font(sans(12.0, 400.0, 1.4)).color(t.muted).min_w(0.0));
    let content = match payload.kind {
        VisualType::Markdown => {
            // `VisualCanvas.vue` `:annotatable="isMarkdown"`: a drag over the text asks the agent about it when it lets go, with the
            // document's source file (else its title) as the prompt's file (row 14.85).
            let owner = payload.source_path.clone().filter(|p| !p.trim().is_empty()).unwrap_or_else(|| title.clone());
            let text = col()
                .w_full()
                .selection_group(true)
                .on_selection_change(move |s: &mut State, change, tx| {
                    s.read_rendered_selection(&owner, change.selection(), change.dragging(), tx)
                })
                .child(prose::prose(&prose::style(t), "document-markdown", &payload.content, false));
            col().key("document-content").w_full().grow(1.0).min_h(0.0).child(
                col()
                    .w_full()
                    .px(14.0)
                    .py(12.0)
                    .on_pointer_move_local(|s: &mut State, p: rune::PointerEvent, tx: &mut Tx| {
                        s.annotation.pointer = (p.window_x, p.window_y);
                        tx.live_only();
                    })
                    .child(text)
                    .scroll(ScrollSpec { stick_bottom: false })
                    .key("document-scroll")
                    .grow(1.0)
                    .min_h(0.0),
            )
        }
        VisualType::Html => col().key("document-content").w_full().grow(1.0).min_h(0.0).child(crate::web::html_body(
            s,
            t,
            "document-html",
            &payload.content,
        )),
    };
    col().key("document-tab").w_full().h_full().min_h(0.0).child(bar).child(content)
}
