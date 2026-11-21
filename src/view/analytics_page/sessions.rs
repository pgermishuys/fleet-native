//! The Sessions section (`components/analytics/tabs/SessionsTab.vue`): a table of sessions in a bordered box, each column head a
//! sort button, 820 wide at least and scrolling sideways under that. Loading comes first (so a sort change shows the words until
//! the refetch answers, as Fleet does), then the failure with no rows, then the empty words.
use super::models::loading_box;
use crate::State;
use crate::composer::format::{format_count, format_money};
use crate::conv::working::format_duration;
use crate::fleet::analytics::SESSIONS_EMPTY;
use crate::fleet::analytics::SessionRow;
use crate::fleet::analytics::sessions_sort::{COLUMNS, SortBy, SortDir, pressed, project_label, sorted, title_label};
use crate::icons::{ARROW_DOWN, ARROW_UP, ARROW_UP_DOWN};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::age;
use crate::ui::error_note::error_note;
use rune::{Align, Cursor, El, col, el, icon, row, text};

pub fn sessions(s: &State, t: &Theme) -> El {
    let a = &s.analytics;
    let has = !a.sessions.data.is_empty();
    let body = if a.sessions_loading() {
        loading_box(t, "Loading session analytics…")
    } else if !has && a.sessions.error.is_some() {
        error_note(t, "analytics-error", "Unable to load session analytics.", a.sessions.error.as_deref().unwrap_or_default())
    } else if !has {
        // `EmptyState` without `bordered`: the same box and words, no outline.
        col()
            .key("analytics-empty")
            .w_full()
            .min_h(245.0)
            .center()
            .padding(24.0)
            .child(text(SESSIONS_EMPTY).font(sans(16.0, 600.0, 1.5)).color(t.text).wrap().align(Align::Center))
    } else {
        table(s, t, &sorted(&a.sessions.data, a.sort.0, a.sort.1))
    };
    col().key("analytics-sessions").label("Sessions analytics table").w_full().min_w(0.0).child(body)
}

/// A column head: the label and its sort icon, the sorted one in `--text`, the rest `--muted` until hovered.
fn head(s: &State, t: &Theme, c: usize, by: SortBy) -> El {
    let (cur, dir) = s.analytics.sort;
    let on = cur == by;
    let glyph = if !on {
        &ARROW_UP_DOWN
    } else if dir == SortDir::Asc {
        &ARROW_UP
    } else {
        &ARROW_DOWN
    };
    let ink = if on { t.text } else { t.muted };
    let mut label = text(by.label()).key(format!("analytics-sessions-head-{c}")).font(sans(12.0, 500.0, 1.5)).color(ink);
    let mut mark = icon(glyph, 12.0).color(ink);
    if !on {
        label = label.group_color(t.text);
        mark = mark.group_color(t.text);
    }
    let mut b = row()
        .key(format!("analytics-sessions-sort-{}", by.key()))
        .role("button")
        .label(format!(
            "{}, {}",
            by.label(),
            if !on {
                "not sorted"
            } else if dir == SortDir::Asc {
                "ascending"
            } else {
                "descending"
            }
        ))
        .w_full()
        .px(16.0)
        .py(8.0)
        .gap(8.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .hover_group();
    if by.align_right() {
        b = b.justify_end();
    }
    b.child(label).child(mark).on_press(move |s: &mut State| {
        let (by, dir) = pressed(s.analytics.sort, by);
        s.analytics_set_sort(by, dir);
    })
}

/// A column's share of the row: Fleet's table sizes columns by their content, here each gets a width near what its widest cell
/// takes at 1280 and the spare room is shared in that proportion, so every row has the same columns and one height.
const WIDTHS: [f32; 8] = [120.0, 105.0, 95.0, 70.0, 125.0, 105.0, 125.0, 90.0];

fn sized(c: usize, el: El) -> El {
    el.w(WIDTHS[c]).grow(WIDTHS[c]).shrink().min_w(0.0)
}

/// One body cell: padding 10/16, 13 px, top-aligned.
fn cell(c: usize, right: bool, content: El) -> El {
    let mut b = col().px(16.0).py(10.0).items_start();
    if right {
        b = b.items_end();
    }
    sized(c, b.child(content))
}

fn words(t: &Theme, key: String, w: &str, strong: bool, num: bool) -> El {
    let mut font = sans(13.0, if strong { 500.0 } else { 400.0 }, 1.54);
    if num {
        font = font.features("'tnum' 1");
    }
    let label = text(w.to_string()).key(key).font(font).color(t.text);
    // A number stays on one line; a title, a name and a time wrap.
    if num { label } else { label.wrap() }
}

fn chip(t: &Theme, key: String, name: &str, muted: bool) -> El {
    row()
        .min_h(22.0)
        .px(8.0)
        .py(2.0)
        .center_y()
        .radius(6.0)
        .fill(if muted { t.text.alpha(0.06) } else { t.accent_dim })
        .child(text(name.to_string()).key(key).font(sans(11.0, 400.0, 1.2)).color(if muted { t.muted } else { t.text }))
}

fn body_cell(s: &State, t: &Theme, r: usize, by: SortBy, c: usize, x: &SessionRow) -> El {
    let k = format!("analytics-sessions-{r}-{}", by.key());
    let right = by.align_right();
    let content = match by {
        SortBy::Title => col()
            .child(words(t, k, &title_label(x), true, false))
            .child(el().h(2.0))
            .child(text(x.session_id.clone()).key(format!("analytics-sessions-{r}-id")).font(mono(11.0, 400.0, 1.4)).color(t.muted).wrap()),
        SortBy::Project => words(t, k, &project_label(x), true, false),
        SortBy::Tokens => words(t, k, &format_count(Some(x.tokens)), false, true),
        SortBy::Cost => words(t, k, &format_money(Some(x.cost)), false, true),
        SortBy::EstimatedCost => words(t, k, &format_money(Some(x.estimated_cost)), false, true),
        SortBy::DurationSeconds => {
            let d = x.duration_seconds.map_or_else(|| "—".to_string(), |d| format_duration((d * 1000.0) as i64));
            words(t, k, &d, false, true)
        }
        SortBy::Models => {
            let mut chips = row().flex_wrap().gap(6.0);
            if x.models.is_empty() {
                chips = chips.child(chip(t, k.clone(), "Unknown model", true));
            }
            for (i, m) in x.models.iter().enumerate() {
                chips = chips.child(chip(t, format!("{k}-{i}"), m, false));
            }
            chips
        }
        SortBy::CreatedAt => {
            let now = crate::view::message::now_ms(s);
            let offset = crate::conv::time::local_offset_secs();
            let when = age::compact(&x.created_at, now, offset);
            words(t, k, &when, true, false).help(age::exact(&x.created_at, now, offset))
        }
    };
    cell(c, right, content)
}

/// The table as rows of eight columns: the head ruled, a rule under each row but the last, a 3 % tint under the pointer.
fn table(s: &State, t: &Theme, rows: &[SessionRow]) -> El {
    let n = rows.len();
    let heads = COLUMNS.iter().enumerate().map(|(c, &by)| sized(c, head(s, t, c, by)));
    let mut lines = col().min_w(820.0).grow(1.0).child(row().w_full().border_sides([0.0, 0.0, 1.0, 0.0], t.border).children(heads));
    for (r, x) in rows.iter().enumerate() {
        let cells = COLUMNS.iter().enumerate().map(|(c, &by)| body_cell(s, t, r, by, c, x));
        let line = row().key(format!("analytics-sessions-row-{r}")).w_full().hover(t.text.alpha(0.03)).children(cells);
        lines = lines.child(if r + 1 < n { line.border_sides([0.0, 0.0, 1.0, 0.0], t.border) } else { line });
    }
    col()
        .key("analytics-sessions-box")
        .w_full()
        .radius(RADIUS_CARD)
        .clip_rounded(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(row().key("analytics-sessions-table").role("table").label("Session analytics table").w_full().scroll_x().child(lines))
}
