//! The Daily section (`components/analytics/tabs/DailyTab.vue`): a table of the days, newest first, in a bordered box; before
//! the first answer the wait, then the failure, then the empty words.
use super::cost_bars::loading_row;
use crate::State;
use crate::fleet::analytics::daily_empty;
use crate::fleet::analytics::tables::{DayRow, day_rows};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::empty_state;
use crate::ui::error_note::error_note;
use rune::{El, col, row, text};

const HEAD: [&str; 5] = ["Day", "Tokens", "Cost", "Sessions", "Messages"];

pub fn daily(s: &State, t: &Theme) -> El {
    let a = &s.analytics;
    let rows = day_rows(&a.daily.data);
    let body = if !rows.is_empty() {
        table(t, &rows)
    } else if a.daily_loading() {
        loading_row(t, "analytics-loading", "Loading daily analytics…")
    } else if let Some(reason) = &a.daily.error {
        error_note(t, "analytics-error", "Unable to load analytics.", reason)
    } else {
        empty_state::bordered(t, "analytics-empty", daily_empty(!a.filters.project_id.trim().is_empty()), 0.0)
    };
    col().key("analytics-daily").label("Daily analytics").w_full().min_w(0.0).child(body)
}

/// The table as five columns side by side, so a column is as wide as its widest cell. Each cell has padding 8/12 and a rule
/// under it except in the last row.
fn table(t: &Theme, rows: &[DayRow]) -> El {
    let cell = |key: String, words: &str, last: bool, head: bool, num: bool| {
        let font = if head { sans(11.0, 500.0, 1.5) } else { sans(12.5, 400.0, 1.5).features("'tnum' 1") };
        let mut c = col().px(12.0).py(8.0).border_sides([0.0, 0.0, if last { 0.0 } else { 1.0 }, 0.0], t.border);
        c = if num { c.items_end() } else { c.items_start() };
        c.child(text(words.to_string()).key(key).font(font).color(if head { t.muted } else { t.text }))
    };
    let n = rows.len();
    let columns = (0..5).map(|c| {
        let mut column = col().grow(1.0).min_w(0.0).child(cell(format!("analytics-daily-head-{c}"), HEAD[c], false, true, c > 0));
        for (r, day) in rows.iter().enumerate() {
            let words = [&day.date, &day.tokens, &day.cost, &day.sessions, &day.messages][c];
            column = column.child(cell(format!("analytics-daily-{r}-{c}"), words, r + 1 == n, false, c > 0));
        }
        column
    });
    col()
        .key("analytics-daily-table")
        .role("table")
        .label("Daily analytics table")
        .w_full()
        .radius(RADIUS_BTN)
        .clip_rounded(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(row().w_full().children(columns))
}
