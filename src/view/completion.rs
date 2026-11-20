//! The completion popup under the caret (CodeMirror's `.cm-tooltip-autocomplete` as `lib/code-editor/theme.ts` dresses it, row 13.11 of
//! docs/parity/11-files.md): a card of `--card-bg` with a 1 px `--border`, radius 8 and the tooltip shadow; inside, a list in the mono
//! face at 12 px, at most 14 em high, padding 4; each row radius 5, padding `2px 8px 2px 4px`, line height 1.7, the chosen row
//! `--accent-dim`; the letters that matched in `--accent` at 600, the detail in 11 px sans `--muted` 10 px from the label. The state is
//! `fleet::completion`. It never takes focus: the code keeps typing.
use crate::State;
use crate::conv::completion::{Kind, Ranked};
use crate::theme::{Theme, mono, sans};
use rune::{Align, Color, El, Shadow, Side, Span, col, el, rich, row, text};

/// `max-height: 14em` of the list's 12 px.
const MAX_H: f32 = 14.0 * 12.0;
const FONT: f32 = 12.0;

/// `.cm-completionIcon`: 1.1 em wide and 0.9 em of padding, at 70 % opacity. CodeMirror draws a glyph by type: `abc` (small) for a word,
/// a square for a namespace (a folder), nothing for a file.
fn icon(t: &Theme, kind: Kind) -> El {
    let slot = el().w(2.0 * FONT).h(20.4).items_center().opacity(0.7);
    match kind {
        Kind::Word => slot.center_y().child(text("abc").font(mono(FONT * 0.5, 400.0, 1.0)).color(t.text).single_line()),
        Kind::Folder => slot.center_y().child(text("\u{25a2}").font(mono(FONT, 400.0, 1.0)).color(t.text).single_line()),
        Kind::File => slot,
    }
}

fn label(t: &Theme, r: &Ranked) -> El {
    let chars: Vec<(usize, char)> = r.item.label.char_indices().collect();
    let spans: Vec<Span> = r
        .matched
        .iter()
        .filter_map(|&i| chars.get(i).map(|&(b, c)| Span::new(b, b + c.len_utf8()).color(t.accent).weight(600.0)))
        .collect();
    rich(r.item.label.clone(), spans).font(mono(FONT, 400.0, 1.7)).color(t.text).single_line()
}

fn option(t: &Theme, index: usize, r: &Ranked, chosen: bool) -> El {
    let mut line = row()
        .key(if chosen { "completion-chosen".to_string() } else { format!("completion-option-{index}") })
        .role("option")
        .label(r.item.label.clone())
        .checked(chosen)
        .items_center()
        .pl(4.0)
        .pr(8.0)
        .py(2.0)
        .radius(5.0)
        .child(icon(t, r.item.kind))
        .child(label(t, r));
    if let Some(detail) = &r.item.detail {
        line = line.child(el().w(10.0)).child(text(detail.clone()).font(sans(11.0, 400.0, 1.7)).color(t.muted).single_line());
    }
    if chosen { line.fill(t.accent_dim) } else { line }
}

/// The popup, to anchor under the caret; `None` while it is shut.
pub fn popup(s: &State, t: &Theme) -> Option<El> {
    let c = &s.completion;
    if !c.open || c.items.is_empty() {
        return None;
    }
    let mut list = col().key("completion-list").items_stretch().px(4.0).py(4.0);
    for (i, r) in c.items.iter().enumerate() {
        list = list.child(option(t, i, r, i == c.selected));
    }
    let list = list.scroll(rune::ScrollSpec { stick_bottom: false }).key("completion-scroll").shrink().min_h(0.0).max_h(MAX_H + 8.0);
    Some(
        col()
            .key("completion")
            .role("listbox")
            .label("Completions")
            .min_w(120.0)
            .items_stretch()
            .radius(8.0)
            .fill(t.card_bg)
            .border(1.0, t.border)
            .border_layout()
            .clip_corners([8.0; 4])
            .box_shadow(Shadow { x: 0.0, y: 16.0, blur: 40.0, spread: -12.0, color: Color::BLACK.alpha(0.45), inset: false })
            .child(list),
    )
}

/// Where the popup hangs: under the caret, its left edge at the text it replaces.
pub fn anchor(popup: El) -> El {
    popup.anchored_fit("code-caret", Side::Bottom, Align::Start, 4.0)
}
