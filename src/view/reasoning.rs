//! Thinking: one quiet line per block (the gist of what the model reasoned), opened on click. Ports
//! `components/session/ReasoningBlock.vue`; the gist is `conv::gist`.
use super::prose;
use crate::State;
use crate::conv::gist::{Thinking, plain_inline, reasoning_gist};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Cursor, Cycle, El, col, el, icon, row, text};
use rune_kit::markdown::MarkdownStyle;

/// Whether a reasoning part draws at all: not when thinking is hidden, nor when it has nothing to say.
pub fn is_drawn(thinking: Thinking, text: &str, summary: Option<&str>) -> bool {
    thinking != Thinking::Hidden && (!text.trim().is_empty() || summary.is_some_and(|s| !s.trim().is_empty()))
}

/// The block's own look of Markdown: 13px at 1.5, in the text colour (Fleet asks for `--text-secondary`, which
/// no stylesheet defines, so the colour falls back to the inherited `--text`), italic, headings plain.
pub fn style(t: &Theme) -> MarkdownStyle {
    prose::style(t)
        .body_font(sans(13.0, 400.0, 1.5).italic())
        .heading_font(1, sans(13.0, 600.0, 1.5).italic())
        .heading_font(2, sans(13.0, 600.0, 1.5).italic())
        .heading_font(3, sans(13.0, 600.0, 1.5).italic())
        .heading_font(4, sans(13.0, 600.0, 1.5).italic())
}

/// The label "Thinking…" while the model writes: Fleet sweeps a highlight through it; Rune has no gradient text, so the
/// whole word breathes between the muted and the text colour instead.
fn live_label(t: &Theme) -> El {
    static BREATHE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)];
    let font = sans(13.0, 600.0, 1.5);
    el().child(text("Thinking\u{2026}").font(font).color(t.muted)).child(
        text("Thinking\u{2026}").font(font).color(t.text).absolute(0.0, 0.0).cycle(Cycle {
            period: 2.2,
            phase: 0.0,
            stops: &BREATHE,
            still: 0.0,
            fps: 30.0,
        }),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn reasoning_row(t: &Theme, s: &State, md: &MarkdownStyle, id: &str, body: &str, summary: Option<&str>, live: bool) -> El {
    let folded = s.thinking == Thinking::Folded;
    let open = s.reasoning_open.contains(id);
    let mut content = col().grow(1.0).shrink().min_w(0.0);
    if folded {
        let mut fold = row()
            .key(format!("reasoning-{id}-fold"))
            .gap(6.0)
            .center_y()
            .ml(-4.0)
            .pl(4.0)
            .pr(6.0)
            .py(2.0)
            .radius(RADIUS_BTN)
            .hover(t.wash(5.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .label(if open { "Hide thinking" } else { "Show thinking" })
            .on_press({
                let id = id.to_string();
                move |s: &mut State| {
                    if !s.reasoning_open.remove(&id) {
                        s.reasoning_open.insert(id.clone());
                    }
                }
            })
            .child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted).group_color(t.text).rotate(if open {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            }));
        if live {
            fold = fold.child(live_label(t).key(format!("reasoning-{id}-label")));
        } else if open {
            fold = fold.child(
                text("Thinking").key(format!("reasoning-{id}-label")).font(sans(13.0, 600.0, 1.5)).color(t.muted).group_color(t.text),
            );
        }
        if !open {
            let gist = plain_inline(&reasoning_gist(body, summary, live));
            fold = fold.child(
                text(gist)
                    .key(format!("reasoning-{id}-gist"))
                    .font(sans(13.0, 400.0, 1.5).italic())
                    .color(t.muted)
                    .group_color(t.text)
                    .truncate()
                    .shrink(),
            );
        }
        content = content.child(fold);
    }
    if !folded || open {
        let mut part = col().key(format!("reasoning-{id}-body")).w_full().mt(if folded { 6.0 } else { 0.0 });
        if let Some(summary) = summary.filter(|s| !s.is_empty()) {
            part = part.child(
                text(plain_inline(summary))
                    .key(format!("reasoning-{id}-summary"))
                    .font(sans(13.0, 500.0, 1.5))
                    .color(t.text)
                    .wrap()
                    .w_full()
                    .mb(4.0),
            );
        }
        content = content.child(part.child(prose::prose(md, &format!("{id}-text"), body, live)));
    }
    let mut line = row()
        .key(format!("reasoning-{id}"))
        .w_full()
        .px(8.0)
        .gap(9.0)
        .items_start()
        .child(el().size(14.0, 14.0).mt(8.0).center().child(icon(&BRAIN, 14.0).color(t.muted.alpha(0.75))));
    line = if folded { line.min_h(30.0) } else { line.py(3.0) };
    line.child(content)
}
