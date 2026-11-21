//! Overview's numbers (`tabs/OverviewTab.vue`, `charts/HorizontalCostBars.vue`): the four cards' values and details, and the
//! items of the two cost rankings with their fallbacks. Plain Rust; the views only draw what comes out.
use super::{Daily, ModelRow, Summary, TopItem};
use crate::composer::format::{format_count, format_money, format_tokens};

/// One `StatCard`.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: &'static str,
    pub label: &'static str,
    pub value: String,
    /// The muted text beside the value ("Est. $101.20").
    pub secondary: Option<String>,
    pub detail: String,
}

/// `formatCurrency`: money, with anything that isn't a positive number written as $0.00.
pub fn currency(amount: f64) -> String {
    format_money(Some(if amount.is_finite() && amount > 0.0 { amount } else { 0.0 }))
}

/// The totals the cards show: the summary's, or with no summary (loading or failed) summed from the daily rows.
fn totals(summary: Option<&Summary>, daily: &[Daily]) -> (f64, f64, f64, f64, f64) {
    if let Some(s) = summary {
        return (s.total_tokens, s.total_cost, s.total_estimated_cost, s.session_count, s.message_count);
    }
    let sum = |f: fn(&Daily) -> f64| daily.iter().map(f).sum::<f64>();
    (sum(|d| d.tokens), sum(|d| d.cost), sum(|d| d.estimated_cost), sum(|d| d.sessions), sum(|d| d.messages))
}

/// The estimated total beside the chart panel's title.
pub fn estimated_total(summary: Option<&Summary>, daily: &[Daily]) -> f64 {
    totals(summary, daily).2
}

/// `summaryCards`: Tokens, Cost, Sessions, Messages.
pub fn cards(summary: Option<&Summary>, daily: &[Daily]) -> Vec<Card> {
    let (tokens, cost, estimated, sessions, messages) = totals(summary, daily);
    let days = daily.len() as f64;
    let average = |total: f64| if daily.is_empty() { 0.0 } else { total / days };
    let derived = "Derived from the daily activity trend".to_string();
    vec![
        Card {
            id: "tokens",
            label: "Tokens",
            value: format_tokens(Some(tokens)),
            secondary: None,
            detail: if daily.is_empty() {
                "No daily token breakdown available".into()
            } else {
                format!("{} avg / day", format_tokens(Some(average(tokens))))
            },
        },
        Card {
            id: "cost",
            label: "Cost",
            value: currency(cost),
            secondary: Some(format!("Est. {}", currency(estimated))),
            detail: if daily.is_empty() {
                "No daily cost breakdown available".into()
            } else {
                format!("{} avg / day", currency(average(cost)))
            },
        },
        Card {
            id: "sessions",
            label: "Sessions",
            value: format_count(Some(sessions)),
            secondary: None,
            detail: if summary.is_some() { format!("{} total messages", format_count(Some(messages))) } else { derived.clone() },
        },
        Card {
            id: "messages",
            label: "Messages",
            value: format_count(Some(messages)),
            secondary: None,
            detail: if summary.is_some() { format!("{} active sessions", format_count(Some(sessions))) } else { derived },
        },
    ]
}

/// `HorizontalCostBarItem`.
#[derive(Clone, Debug, PartialEq)]
pub struct BarItem {
    pub name: String,
    pub cost: f64,
    pub max_cost: f64,
    /// Under the name; none says "<cost> of <max>".
    pub detail: Option<String>,
}

/// `modelBarItems`: the models read, or with none the summary's top models (no detail).
pub fn model_items(summary: Option<&Summary>, models: &[ModelRow]) -> Vec<BarItem> {
    if !models.is_empty() {
        let max = largest(models.iter().map(|m| m.cost));
        return models
            .iter()
            .map(|m| BarItem {
                name: m.model_id.clone(),
                cost: m.cost,
                max_cost: max,
                detail: Some(format!("Est. {}", currency(m.estimated_cost))),
            })
            .collect();
    }
    let top = summary.map(|s| s.top_models.as_slice()).unwrap_or_default();
    let max = largest(top.iter().map(|m| m.cost));
    top.iter().map(|m| BarItem { name: m.name.clone(), cost: m.cost, max_cost: max, detail: None }).collect()
}

/// `projectBarItems`: the page's project list, or with none the summary's top projects.
pub fn project_items(summary: Option<&Summary>, projects: &[TopItem]) -> Vec<BarItem> {
    let fallback: &[TopItem] = summary.map(|s| s.top_projects.as_slice()).unwrap_or_default();
    let source = if projects.is_empty() { fallback } else { projects };
    let max = largest(source.iter().map(|p| p.cost));
    source
        .iter()
        .map(|p| BarItem {
            name: p.name.clone(),
            cost: p.cost,
            max_cost: max,
            detail: Some(format!("{} tokens", format_tokens(Some(p.tokens)))),
        })
        .collect()
}

/// `getMaxCost`: the largest cost, 0 for none.
fn largest(costs: impl Iterator<Item = f64>) -> f64 {
    let costs: Vec<f64> = costs.collect();
    if costs.is_empty() { 0.0 } else { costs.into_iter().fold(f64::NEG_INFINITY, f64::max) }
}

/// A ranked row: what `HorizontalCostBars` draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub name: String,
    pub detail: String,
    pub cost: String,
    /// Percent of the track the fill covers.
    pub width: f64,
}

/// `getRelativeWidth`: 0 for a zero, negative or not-a-number cost or maximum, at most 100.
pub fn relative_width(cost: f64, max_cost: f64) -> f64 {
    if !cost.is_finite() || !max_cost.is_finite() || max_cost <= 0.0 || cost <= 0.0 {
        return 0.0;
    }
    (cost / max_cost * 100.0).min(100.0)
}

fn money(amount: f64) -> String {
    format_money(Some(if amount.is_finite() { amount } else { 0.0 }))
}

/// `rankedItems`: by cost, largest first, then by name.
pub fn rank(items: &[BarItem]) -> Vec<Ranked> {
    let mut sorted: Vec<&BarItem> = items.iter().collect();
    sorted.sort_by(|a, b| {
        let by_cost = (b.cost - a.cost).partial_cmp(&0.0).filter(|o| o.is_ne());
        by_cost.unwrap_or_else(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)))
    });
    sorted
        .into_iter()
        .map(|i| Ranked {
            name: i.name.clone(),
            detail: i.detail.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| format!("{} of {}", money(i.cost), money(i.max_cost))),
            cost: money(i.cost),
            width: relative_width(i.cost, i.max_cost),
        })
        .collect()
}

/// A bar's name for a screen reader.
pub fn bar_label(name: &str, width: f64) -> String {
    format!("{name} cost bar at {width:.0} percent")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(tokens: f64, cost: f64) -> Daily {
        Daily { date: "2026-11-20".into(), tokens, cost, estimated_cost: cost * 2.0, sessions: 2.0, messages: 10.0 }
    }

    #[test]
    fn the_card_details_with_daily_rows_and_a_summary() {
        let summary = Summary {
            total_tokens: 42_300_000.0,
            total_cost: 96.9,
            total_estimated_cost: 101.2,
            session_count: 18.0,
            message_count: 1765.0,
            ..Default::default()
        };
        let cards = cards(Some(&summary), &[day(1.0, 1.0), day(1.0, 1.0)]);
        assert_eq!(cards[0].value, "42.3M");
        assert_eq!(cards[0].detail, "21.1M avg / day");
        assert_eq!((cards[1].value.as_str(), cards[1].secondary.as_deref()), ("$96.90", Some("Est. $101.20")));
        assert_eq!(cards[1].detail, "$48.45 avg / day");
        assert_eq!((cards[2].value.as_str(), cards[2].detail.as_str()), ("18", "1,765 total messages"));
        assert_eq!((cards[3].value.as_str(), cards[3].detail.as_str()), ("1,765", "18 active sessions"));
    }

    #[test]
    fn the_card_details_with_no_daily_rows() {
        let cards = cards(Some(&Summary::default()), &[]);
        assert_eq!(cards[0].detail, "No daily token breakdown available");
        assert_eq!(cards[1].detail, "No daily cost breakdown available");
        assert_eq!(cards[0].value, "0");
    }

    #[test]
    fn with_no_summary_the_totals_are_summed_from_the_daily_rows() {
        let cards = cards(None, &[day(1_000_000.0, 2.0), day(500_000.0, 3.0)]);
        assert_eq!(cards[0].value, "1.5M");
        assert_eq!(cards[1].value, "$5.00");
        assert_eq!(cards[1].secondary.as_deref(), Some("Est. $10.00"));
        assert_eq!((cards[2].value.as_str(), cards[2].detail.as_str()), ("4", "Derived from the daily activity trend"));
        assert_eq!((cards[3].value.as_str(), cards[3].detail.as_str()), ("20", "Derived from the daily activity trend"));
    }

    #[test]
    fn a_negative_or_missing_cost_is_written_as_zero_dollars() {
        assert_eq!(currency(-3.0), "$0.00");
        assert_eq!(currency(f64::NAN), "$0.00");
    }

    fn item(name: &str, cost: f64, max: f64) -> BarItem {
        BarItem { name: name.into(), cost, max_cost: max, detail: None }
    }

    #[test]
    fn bars_are_sorted_by_cost_then_name() {
        let ranked = rank(&[item("b", 1.0, 5.0), item("c", 5.0, 5.0), item("a", 1.0, 5.0)]);
        assert_eq!(ranked.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["c", "a", "b"]);
        assert_eq!(ranked[0].detail, "$5.00 of $5.00");
    }

    #[test]
    fn widths_are_clamped() {
        assert_eq!(relative_width(5.0, 10.0), 50.0);
        assert_eq!(relative_width(20.0, 10.0), 100.0);
        for (cost, max) in [(0.0, 10.0), (-1.0, 10.0), (f64::NAN, 10.0), (1.0, 0.0), (1.0, f64::NAN), (1.0, f64::INFINITY)] {
            assert_eq!(relative_width(cost, max), 0.0, "{cost} of {max}");
        }
        assert_eq!(bar_label("harbor-api", 49.6), "harbor-api cost bar at 50 percent");
    }

    #[test]
    fn the_model_bars_fall_back_to_the_summarys_top_models() {
        let summary = Summary { top_models: vec![TopItem { name: "m1".into(), tokens: 1.0, cost: 4.0 }], ..Default::default() };
        let bars = model_items(Some(&summary), &[]);
        assert_eq!(bars, vec![BarItem { name: "m1".into(), cost: 4.0, max_cost: 4.0, detail: None }]);
        let models = [ModelRow { model_id: "m2".into(), cost: 2.0, estimated_cost: 2.5, ..Default::default() }];
        assert_eq!(model_items(Some(&summary), &models)[0].detail.as_deref(), Some("Est. $2.50"));
        assert!(model_items(None, &[]).is_empty());
    }

    #[test]
    fn the_project_bars_fall_back_to_the_summarys_top_projects() {
        let top = TopItem { name: "p1".into(), tokens: 2_100_000.0, cost: 3.0 };
        let summary = Summary { top_projects: vec![top.clone()], ..Default::default() };
        assert_eq!(project_items(Some(&summary), &[])[0].detail.as_deref(), Some("2.1M tokens"));
        let own = [TopItem { name: "p2".into(), tokens: 10.0, cost: 1.0 }];
        assert_eq!(project_items(Some(&summary), &own)[0].name, "p2");
    }
}
