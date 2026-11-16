//! The summary bar at the top of the home page (`SummaryBar.vue`): five counts side by side and the last fourteen days'
//! spend, a button that opens Analytics. Sizes are the component's own CSS (padding 12 16, value 20/600, 6 px bars 3 apart).
//! The words follow weave-fleet's `main` ("Est. spend"); v0.72.0's client says "Total spend".
use crate::composer::format::{format_count, format_money, format_tokens};
use crate::fleet::summary::{Day, SPARK_DAYS, SPARK_H, bar_height, totals};
use crate::icons::CHEVRON_RIGHT;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::{Rail, State};
use rune::{Cursor, El, col, el, icon, row, text};

/// A label in its column, with the value under it (`.summary-bar__metric`).
fn metric(t: &Theme, key: &str, label: &str, value: String, first: bool) -> El {
    col()
        .grow(1.0)
        .min_w(0.0)
        .gap(2.0)
        .px(16.0)
        .py(12.0)
        .border_sides([0.0, 0.0, 0.0, if first { 0.0 } else { 1.0 }], t.border)
        .child(text(label.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted))
        .child(text(value).key(key.to_string()).font(sans(20.0, 600.0, 1.5).tracking(-0.2).features("'tnum' 1")).color(t.text))
}

fn spark(t: &Theme, days: &[Day], max: f64) -> El {
    row().items_end().gap(3.0).h(SPARK_H).children(days.iter().enumerate().map(|(i, day)| {
        let ink = if i + 1 == days.len() { t.accent } else { t.accent.alpha(0.45) };
        el().key(format!("summary-bar-{i}")).w(6.0).h(bar_height(day.cost, max)).radius(2.0).fill(ink)
    }))
}

fn spend(s: &State, t: &Theme) -> El {
    let days = s.summary_days();
    let (total, max) = totals(&days);
    let label = format!("Estimated spend over the last {SPARK_DAYS} days: {}. Open Analytics", format_money(Some(total)));
    row()
        .key("summary-spend")
        .role("button")
        .label(label)
        .center_y()
        .gap(14.0)
        .pl(16.0)
        .pr(12.0)
        .py(12.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.border)
        .radius(RADIUS_CARD)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .hover(t.wash(4.0))
        .on_press(|s: &mut State| s.press_rail(Rail::Analytics))
        .child(
            col().gap(2.0).child(text(format!("Last {SPARK_DAYS} days")).font(sans(12.0, 400.0, 1.5)).color(t.muted)).child(
                text(format_money(Some(total)))
                    .key("summary-spend-total")
                    .font(sans(20.0, 600.0, 1.5).tracking(-0.2).features("'tnum' 1"))
                    .color(t.text),
            ),
        )
        .child(spark(t, &days, max))
        .child(icon(&CHEVRON_RIGHT, 14.0).color(t.muted))
}

pub fn bar(s: &State, t: &Theme) -> El {
    let (active, idle) = s.summary_counts();
    let (queued, tokens, cost) = s.bar.summary.as_ref().map_or((0, 0.0, 0.0), |m| (m.queued, m.tokens, m.cost));
    let cells = [
        ("summary-active-count", "Working", format_count(Some(f64::from(active)))),
        ("summary-idle-count", "Idle", format_count(Some(f64::from(idle)))),
        ("summary-queued-count", "Queued", format_count(Some(f64::from(queued)))),
        ("summary-tokens-count", "Total tokens", format_tokens(Some(tokens))),
        ("summary-cost-count", "Est. spend", format_money(Some(cost))),
    ];
    let metrics = row().grow(1.0).min_w(0.0).children(cells.into_iter().enumerate().map(|(i, (k, l, v))| metric(t, k, l, v, i == 0)));
    row()
        .key("summary-bar")
        .role("region")
        .label("Fleet summary")
        .w_full()
        .items_stretch()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(metrics)
        .child(spend(s, t))
}
