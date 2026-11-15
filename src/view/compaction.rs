//! Where the harness compacted the conversation: a hairline across the column with what happened on it, and the summary
//! the model goes on from behind "Show summary". Ports `components/session/CompactionDivider.vue` (`conv::compaction`
//! says what it reads).
use super::prose;
use crate::State;
use crate::conv::compaction::{CompactionView, compaction_title, describe_compaction};
use crate::theme::{Theme, sans};
use rune::{Cursor, El, ScrollSpec, col, el, row, text};
use rune_kit::markdown::MarkdownStyle;

/// The summary's Markdown: 13px at 1.55 in the muted colour.
pub fn style(t: &Theme) -> MarkdownStyle {
    prose::style(t).text(t.muted).body_font(sans(13.0, 400.0, 1.55))
}

pub fn divider(t: &Theme, s: &State, md: &MarkdownStyle, id: &str, c: &CompactionView) -> El {
    let open = s.summary_open.contains(id);
    let font = sans(12.5, 400.0, 1.4);
    let hairline = || el().h(1.0).grow(1.0).fill(t.border);
    let mut label = row()
        .shrink()
        .center_y()
        .justify_center()
        .help(compaction_title(c.trigger.as_deref()))
        .child(text(describe_compaction(c)).key(format!("compaction-{id}-label")).font(font).color(t.muted).wrap());
    let has_summary = c.summary.as_ref().is_some_and(|s| !s.is_empty());
    if has_summary {
        let key = id.to_string();
        let words = if open { "Hide summary" } else { "Show summary" };
        label = label.child(text(" \u{b7} ").font(font).color(t.muted)).child(
            text(words)
                .key(format!("compaction-{id}-toggle"))
                .font(font)
                .color(t.accent_text)
                .cursor(Cursor::Pointer)
                .label(words)
                .on_press(move |s: &mut State| {
                    if !s.summary_open.remove(&key) {
                        s.summary_open.insert(key.clone());
                    }
                }),
        );
    }
    let mut out = col()
        .key(format!("compaction-{id}"))
        .w_full()
        .my(10.0)
        .label("Context compacted")
        .child(row().w_full().gap(12.0).center_y().child(hairline()).child(label).child(hairline()));
    if let (true, Some(summary)) = (open, c.summary.as_deref()) {
        out = out.child(
            row().w_full().justify_center().mt(10.0).child(
                el().key(format!("compaction-{id}-summary"))
                    .w_full()
                    .max_w(680.0)
                    .px(14.0)
                    .py(10.0)
                    .radius(8.0)
                    .border(1.0, t.border)
                    .border_layout()
                    .max_h(360.0)
                    .scroll(ScrollSpec { stick_bottom: false })
                    .child(prose::prose(md, &format!("{id}-summary"), summary, false)),
            ),
        );
    }
    out
}
