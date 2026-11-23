//! `BoardSummaryPanel.vue`: four cards over the cards the left panel leaves visible: the total, the status breakdown, spend by agent,
//! and the cost breakdown with the live fleet summary's footnote. Cards carry no cost or tokens, so the money rows read "$0.00"
//! and the stacked bar is by session count (FB5).
use crate::State;
use crate::composer::format::{format_money, format_tokens};
use crate::fleet::board::cards::Status;
use crate::fleet::board::panel::{self as model, StatusStat};
use crate::theme::{Theme, sans};
use rune::{Align, El, col, el, row, text};

/// `.summary-card`: padding 12, border, radius 10, fill white at .02 (Fleet's literal, the same in both themes).
fn card(t: &Theme, key: &str) -> El {
    col().key(key.to_string()).w_full().padding(12.0).border(1.0, t.border).border_layout().radius(10.0).fill(t.board_faint_fill())
}

fn muted(t: &Theme, words: String) -> El {
    text(words).font(sans(10.0, 400.0, 1.5)).color(t.muted).wrap()
}

fn title(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(11.0, 600.0, 1.5)).color(t.text).mb(12.0)
}

fn total(s: &State, t: &Theme, visible: usize) -> El {
    card(t, "board-summary-total")
        .child(text(s.board.sessions().len().to_string()).key("board-summary-total-n").font(sans(28.0, 700.0, 1.5)).color(t.text).mb(4.0))
        .child(text("TOTAL SESSIONS").font(sans(10.0, 400.0, 1.5).tracking(0.5)).color(t.muted).mb(14.0))
        .child(muted(t, format!("{visible} visible with current board filters")))
}

/// One row of the breakdown: label (46), count (30, right), the track and its fill, percent (30, right).
fn status_row(t: &Theme, st: &StatusStat, last: bool) -> El {
    let key = st.status.value();
    let cell = |w: f32, words: String, align: Align| el().w(w).child(text(words).font(sans(10.0, 400.0, 1.5)).color(t.muted).align(align));
    row()
        .key(format!("board-status-{key}"))
        .w_full()
        .items_center()
        .gap(8.0)
        .mb(if last { 0.0 } else { 6.0 })
        .child(cell(46.0, st.status.label().to_string(), Align::Start))
        .child(cell(30.0, st.count.to_string(), Align::End))
        .child(row().grow(1.0).min_w(0.0).h(8.0).radius(4.0).clip_rounded(4.0).fill(t.board_track()).child(
            el().key(format!("board-status-{key}-fill")).w_frac((st.width / 100.0) as f32).h_full().radius(4.0).fill(st.status.color(t)),
        ))
        .child(cell(30.0, format!("{}%", (st.share * 100.0).round()), Align::End))
}

fn breakdown(t: &Theme, visible: &[crate::fleet::board::cards::CardSession]) -> El {
    card(t, "board-summary-status")
        .child(title(t, "Status breakdown"))
        .children(model::status_stats(visible).iter().enumerate().map(|(n, st)| status_row(t, st, n + 1 == Status::ALL.len())))
}

fn agents(t: &Theme, visible: &[crate::fleet::board::cards::CardSession]) -> El {
    let segments = model::spend_segments(visible);
    let label = model::spend_label(visible);
    let bar =
        row().key("board-spend-bar").label(label).w_full().h(12.0).radius(6.0).clip_rounded(6.0).fill(t.board_track()).mb(14.0).children(
            segments.iter().enumerate().map(|(n, (a, w))| {
                el().key(format!("board-spend-{}", a.agent))
                    .help(format!("{}: {}", a.agent, format_money(Some(a.cost))))
                    .w_frac((*w / 100.0) as f32)
                    .h_full()
                    .fill(t.board_segment(n))
            }),
        );
    let rows = col().gap(8.0).children(model::agent_stats(visible).iter().map(|a| {
        row()
            .key(format!("board-agent-{}", a.agent))
            .w_full()
            .items_center()
            .justify_between()
            .gap(12.0)
            .child(
                col()
                    .min_w(0.0)
                    .child(text(a.agent.clone()).font(sans(11.0, 600.0, 1.5)).color(t.text))
                    .child(muted(t, format!("{} tokens", format_tokens(Some(a.tokens))))),
            )
            .child(
                col()
                    .min_w(0.0)
                    .items_end()
                    .child(muted(t, format!("{} sessions", a.sessions)))
                    .child(text(format_money(Some(a.cost))).font(sans(10.0, 400.0, 1.5)).color(t.text)),
            )
    }));
    card(t, "board-summary-spend").child(title(t, "Spend by agent")).child(bar).child(rows)
}

fn metric(t: &Theme, key: &str, label: &str, value: String, last: bool) -> El {
    let mut r = row()
        .key(key.to_string())
        .w_full()
        .items_center()
        .justify_between()
        .gap(12.0)
        .py(6.0)
        .child(muted(t, label.to_string()))
        .child(text(value).font(sans(11.0, 600.0, 1.5)).color(t.text));
    if !last {
        r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.board_track()).border_layout();
    }
    r
}

fn costs(s: &State, t: &Theme, visible: &[crate::fleet::board::cards::CardSession]) -> El {
    let (cost, tokens, average) = model::totals(visible);
    let right = &s.board.right;
    let queued = right.summary.as_ref().map(|f| f.queued);
    let mut c = card(t, "board-summary-costs")
        .child(title(t, "Cost breakdown"))
        .child(metric(t, "board-metric-cost", "Visible session cost", format_money(Some(cost)), false))
        .child(metric(t, "board-metric-average", "Average per visible session", format_money(Some(average)), false))
        .child(metric(t, "board-metric-tokens", "Visible session tokens", format_tokens(Some(tokens)), queued.is_none()));
    if let Some(n) = queued {
        c = c.child(metric(t, "board-metric-queued", "Queued tasks", n.to_string(), true));
    }
    let foot = if let Some(f) = &right.summary {
        Some((model::live_label(f), t.muted))
    } else if right.loading {
        Some(("Loading live fleet summary…".to_string(), t.muted))
    } else {
        right.error.as_ref().map(|e| (format!("Live fleet summary unavailable: {e}"), t.board_error_note()))
    };
    match foot {
        Some((words, ink)) => c.child(text(words).key("board-summary-footnote").font(sans(10.0, 400.0, 1.5)).color(ink).wrap().mt(10.0)),
        None => c,
    }
}

/// `BoardSummaryPanel`: the cards, 12 apart.
pub fn view(s: &State, t: &Theme) -> El {
    let visible = s.board.filtered_sessions();
    col()
        .key("board-summary")
        .label("Board summary panel")
        .w_full()
        .gap(12.0)
        .child(total(s, t, visible.len()))
        .child(breakdown(t, &visible))
        .child(agents(t, &visible))
        .child(costs(s, t, &visible))
}
