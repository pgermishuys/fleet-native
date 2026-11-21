//! The Models section (`components/analytics/tabs/ModelsTab.vue`): "Cost by model" (the cost bars) and "Model details" (a table),
//! two panels. Loading comes first, so a filter change blanks the section while it asks again; then the failure with no models;
//! then the empty words.
use super::cost_bars::bars;
use crate::State;
use crate::fleet::analytics::models_empty;
use crate::fleet::analytics::tables::{ModelLine, model_bars, model_lines};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::empty_state;
use crate::ui::error_note::error_note;
use rune::{Align, El, col, el, row, text};

const HEAD: [&str; 7] = ["Model", "Provider", "Tokens", "Cost", "Estimated cost", "Messages", "Avg cost/msg"];

pub fn models(s: &State, t: &Theme) -> El {
    let a = &s.analytics;
    let with_project = !a.filters.project_id.trim().is_empty();
    let has = !a.models.data.is_empty();
    let body = if a.models_loading() {
        loading_box(t, "Loading model analytics…")
    } else if !has && a.models.error.is_some() {
        error_note(t, "analytics-error", "Unable to load model analytics.", a.models.error.as_deref().unwrap_or_default())
    } else if !has {
        empty_state::bordered(t, "analytics-empty", models_empty(with_project), 245.0)
    } else {
        let items = model_bars(&a.models.data);
        col()
            .key("analytics-models-content")
            .w_full()
            .gap(12.0)
            .child(panel(
                t,
                "analytics-models-chart",
                "Cost by model",
                16.0,
                bars(t, "analytics-bars-models", &items, "No model cost data available for the selected filters.", None, false),
            ))
            .child(panel(t, "analytics-models-details", "Model details", 0.0, table(t, &model_lines(&a.models.data))))
    };
    col().key("analytics-models").label("Models analytics").w_full().min_w(0.0).child(body)
}

/// `.models-tab__panel`: padding 16 (the table's panel none at the sides, so the table runs edge to edge), gap 12, a boxed card.
fn panel(t: &Theme, key: &str, title: &str, side: f32, body: El) -> El {
    col()
        .key(key.to_string())
        .label(title.to_string())
        .w_full()
        .min_w(0.0)
        .gap(12.0)
        .pt(16.0)
        .pb(if side == 0.0 { 0.0 } else { 16.0 })
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(col().px(16.0).child(text(title.to_string()).key(format!("{key}-title")).font(sans(13.0, 600.0, 1.5)).color(t.text)))
        .child(col().w_full().px(side).child(body))
}

/// The table: seven columns side by side, each as wide as its widest cell, 720 wide at least, scrolling sideways under that.
/// The head has a rule under it and each row but the last has one; a row's cells share one hover group for the 3 % tint.
fn table(t: &Theme, lines: &[ModelLine]) -> El {
    let n = lines.len();
    let cell = |key: String, words: &str, head: bool, c: usize, r: usize| {
        let right = c >= 2;
        let font = if head {
            sans(12.0, 500.0, 1.5)
        } else if c < 2 {
            sans(13.0, 500.0, 1.54)
        } else {
            sans(13.0, 400.0, 1.54).features("'tnum' 1")
        };
        let mut b = col().px(16.0).items_start();
        b = if head { b.py(8.0) } else { b.py(10.0).hover_group_shared(format!("analytics-model-row-{r}")) };
        b = b.border_sides([0.0, 0.0, if head || r + 1 < n { 1.0 } else { 0.0 }, 0.0], t.border);
        if right {
            b = b.items_end();
        }
        if !head {
            b = b.child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(t.text.alpha(0.03)).group_opacity(0.0, 1.0));
        }
        // One line a cell: the columns are separate boxes, so a wrapped cell would push its row out of line with the others.
        b.child(text(words.to_string()).key(key).font(font).color(if head { t.muted } else { t.text }))
    };
    let columns = (0..7).map(|c| {
        let mut column = col().grow(1.0).child(cell(format!("analytics-models-head-{c}"), HEAD[c], true, c, 0));
        for (r, line) in lines.iter().enumerate() {
            let words = [&line.model, &line.provider, &line.tokens, &line.cost, &line.estimated, &line.messages, &line.average][c];
            column = column.child(cell(format!("analytics-models-{r}-{c}"), words, false, c, r));
        }
        column
    });
    row()
        .key("analytics-models-table")
        .role("table")
        .label("Model details table")
        .w_full()
        .scroll_x()
        .child(row().min_w(720.0).grow(1.0).children(columns))
}

/// `.models-tab__state--loading` (the same box in Sessions): a dashed box, 280 tall at least, the words in `--text` (no spinner).
pub fn loading_box(t: &Theme, words: &str) -> El {
    col()
        .key("analytics-loading")
        .label(words.to_string())
        .w_full()
        .min_h(280.0)
        .center()
        .padding(24.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_dash(4.0, 3.0)
        .border_layout()
        .child(text(words.to_string()).font(sans(13.0, 400.0, 1.5)).color(t.text).align(Align::Center))
}
