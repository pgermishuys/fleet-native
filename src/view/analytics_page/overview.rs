//! The Overview section (`components/analytics/tabs/OverviewTab.vue`, `cards/StatCard.vue`): four stat cards, the chart's panel
//! and the two cost rankings. The chart in the panel is `chart.rs`.
use super::cost_bars::{bars, loading_row};
use crate::State;
use crate::fleet::analytics::overview::{Card, cards, currency, estimated_total, model_items, project_items};
use crate::fleet::analytics::{daily_empty, models_empty, projects_empty, with_selected_project};
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::error_note::error_note;
use crate::ui::{empty_state, rem};
use rune::{El, col, row, text};

/// `.overview-tab`: the cards, the chart panel and the rankings, 12 apart.
pub fn overview(s: &State, t: &Theme) -> El {
    let a = &s.analytics;
    let summary = a.summary.data.as_ref();
    let with_project = !a.filters.project_id.trim().is_empty();
    // `scopedTopProjects`: the summary's top projects, and the picked project after them when it isn't among them.
    let projects = with_selected_project(summary.map(|x| x.top_projects.as_slice()).unwrap_or_default(), a.filters.project_id.trim());
    let models = bars(
        t,
        "analytics-bars-models",
        &model_items(summary, &a.models.data),
        models_empty(with_project),
        a.models.error.as_deref(),
        a.models_bars_loading(),
    );
    let by_project = bars(
        t,
        "analytics-bars-projects",
        &project_items(summary, &projects),
        projects_empty(with_project),
        a.summary.error.as_deref(),
        a.projects_loading(),
    );
    col()
        .key("analytics-overview")
        .label("Overview analytics")
        .w_full()
        .min_w(0.0)
        .gap(12.0)
        .child(
            row().key("analytics-cards").w_full().flex_wrap().gap(12.0).children(cards(summary, &a.daily.data).iter().map(|c| card(t, c))),
        )
        .child(trend(s, t))
        .child(
            row()
                .key("analytics-rankings")
                .w_full()
                .flex_wrap()
                .gap(12.0)
                .child(panel(t, "analytics-panel-models", "Top models by cost", None, models))
                .child(panel(t, "analytics-panel-projects", "Top projects by cost", None, by_project)),
        )
}

/// `StatCard`: padding 14/16, gap 4; the label, the value with its secondary beside it, the detail.
fn card(t: &Theme, c: &Card) -> El {
    let mut value = row().w_full().flex_wrap().items_end().gaps(4.0, 8.0).child(
        text(c.value.clone())
            .key(format!("analytics-card-{}-value", c.id))
            .font(sans(24.0, 600.0, 1.2).tracking(-0.24).features("'tnum' 1"))
            .color(t.text),
    );
    if let Some(secondary) = &c.secondary {
        value = value.child(
            text(secondary.clone()).key(format!("analytics-card-{}-secondary", c.id)).font(sans(12.0, 500.0, 1.2)).color(t.muted).mb(3.0),
        );
    }
    let mut body = col()
        .key(format!("analytics-card-{}", c.id))
        .min_w(140.0)
        .grow(1.0)
        .gap(4.0)
        .px(16.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(text(c.label).font(sans(12.0, 500.0, 1.5)).color(t.muted))
        .child(value);
    if !c.detail.is_empty() {
        body = body.child(
            text(c.detail.clone()).key(format!("analytics-card-{}-detail", c.id)).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap(),
        );
    }
    body
}

/// `.overview-tab__panel`: padding 16, gap 12, a boxed card; the head has the title and optionally a note on the right.
fn panel(t: &Theme, key: &str, title: &str, meta: Option<(&str, String)>, body: El) -> El {
    let mut head =
        row().w_full().items_end().gap(12.0).child(
            text(title.to_string()).key(format!("{key}-title")).font(sans(rem(0.92857), 600.0, 1.5)).color(t.text).grow(1.0).min_w(0.0),
        );
    if let Some((meta_key, words)) = meta {
        head = head.child(text(words).key(meta_key.to_string()).font(sans(12.0, 400.0, 1.5).features("'tnum' 1")).color(t.muted));
    }
    col()
        .key(key.to_string())
        .label(title.to_string())
        .min_w(320.0)
        .grow(1.0)
        .gap(12.0)
        .padding(16.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(head)
        .child(body)
}

/// The chart's panel: with rows the 300 px chart slot, else the wait, the failure, or the empty words.
fn trend(s: &State, t: &Theme) -> El {
    let a = &s.analytics;
    let with_project = !a.filters.project_id.trim().is_empty();
    let body = if !a.daily.data.is_empty() {
        super::chart::chart(s, t, &a.daily.data)
    } else if a.daily_loading() {
        loading_row(t, "analytics-loading", "Loading daily analytics…")
    } else if let Some(reason) = &a.daily.error {
        error_note(t, "analytics-error", "Unable to load analytics.", reason)
    } else {
        empty_state::bordered(t, "analytics-empty", daily_empty(with_project), 210.0)
    };
    let total = format!("Est. total {}", currency(estimated_total(a.summary.data.as_ref(), &a.daily.data)));
    panel(t, "analytics-overview-daily-trend", "Daily tokens and cost", Some(("analytics-estimated-total", total)), body)
}
