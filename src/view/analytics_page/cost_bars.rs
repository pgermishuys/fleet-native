//! `HorizontalCostBars` (`components/analytics/charts/HorizontalCostBars.vue`): a ranking of costs, each a name, a line of detail,
//! the cost and a track with a fill. Bare: it sits inside a panel that draws the box. Shared with Models (piece I).
use crate::fleet::analytics::overview::{BarItem, bar_label, rank};
use crate::theme::{Theme, sans};
use crate::ui::error_note::error_note;
use crate::ui::{empty_state, rem};
use crate::view::files_tab::spinner;
use rune::{Align, El, col, el, row, text};

/// The list; else the first answer's wait; else the failure; else the empty words. `key` names this ranking in tests.
pub fn bars(t: &Theme, key: &str, items: &[BarItem], empty: &str, error: Option<&str>, loading: bool) -> El {
    let ranked = rank(items);
    if !ranked.is_empty() {
        return col().key(key.to_string()).label("Cost ranking").w_full().gap(14.0).children(ranked.iter().enumerate().map(|(i, r)| {
            let track = row()
                .key(format!("{key}-track-{i}"))
                .role("img")
                .label(bar_label(&r.name, r.width))
                .w_full()
                .h(6.0)
                .radius(999.0)
                .clip_rounded(999.0)
                .fill(t.text.alpha(0.08))
                .child(el().grow(r.width as f32).w(0.0).h_full().radius(999.0).fill(t.accent))
                .child(el().grow((100.0 - r.width) as f32).w(0.0));
            let copy = col()
                .grow(1.0)
                .min_w(0.0)
                .child(text(r.name.clone()).font(sans(rem(0.92857), 500.0, 1.5)).color(t.text).truncate().min_w(0.0))
                .child(text(r.detail.clone()).font(sans(rem(0.85714), 400.0, 1.5)).color(t.muted));
            col()
                .key(format!("{key}-item-{i}"))
                .w_full()
                .gap(6.0)
                .child(
                    row()
                        .w_full()
                        .items_start()
                        .gap(16.0)
                        .child(copy)
                        .child(text(r.cost.clone()).font(sans(rem(0.92857), 600.0, 1.5).features("'tnum' 1")).color(t.text)),
                )
                .child(track)
        }));
    }
    if loading {
        return loading_row(t, &format!("{key}-loading"), "Loading…");
    }
    if let Some(reason) = error {
        return error_note(t, &format!("{key}-error"), "Unable to load analytics.", reason);
    }
    empty_state::compact_bare(t, &format!("{key}-empty"), empty)
}

/// `flex items-center justify-center gap-2 p-6 text-sm text-muted`: a spinner and what it waits for.
pub fn loading_row(t: &Theme, key: &str, words: &str) -> El {
    row()
        .key(key.to_string())
        .label(words.to_string())
        .w_full()
        .center()
        .gap(8.0)
        .padding(rem(1.5))
        .child(spinner(t.muted, 14.0))
        .child(text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).align(Align::Center))
}
