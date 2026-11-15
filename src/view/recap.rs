//! Fleet's own voice between the conversation and the composer: what it wrote for the session while nobody looked. Ports
//! `components/session/RecapLine.vue`. Hidden without text.
use super::conversation::COLUMN;
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{El, Span, col, el, icon, rich, row};

pub fn recap_line(t: &Theme, recap: Option<&str>) -> Option<El> {
    let said = recap.filter(|r| !r.is_empty())?;
    // "recap:" is the text colour at weight 500 in Fleet; Rune styles it with a span of the same line.
    let line = format!("recap: {said}");
    let font = sans(13.0, 400.0, 1.5);
    Some(
        col().key("recap-line").w_full().px(24.0).py(6.0).label("Recap").child(
            row().w_full().justify_center().child(
                row()
                    .w_full()
                    .max_w(COLUMN)
                    .px(4.0)
                    .gap(6.0)
                    .items_start()
                    .child(el().size(13.0, 13.0).mt(3.0).child(icon(&HISTORY, 13.0).color(t.accent)))
                    .child(
                        rich(line, vec![Span::new(0, 6).color(t.text).weight(500.0)])
                            .key("recap-text")
                            .font(font)
                            .color(t.muted)
                            .wrap()
                            .selectable(true)
                            .grow(1.0)
                            .shrink(),
                    ),
            ),
        ),
    )
}
