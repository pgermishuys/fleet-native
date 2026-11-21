//! What Daily's table and Projects' cards show (`tabs/DailyTab.vue`, `tabs/ProjectsTab.vue`): the rows in their order, with
//! every number already written the way Fleet writes it. Plain Rust, no Rune.
use super::overview::relative_width;
use super::{Daily, TopItem};
use crate::composer::format::{format_count, format_money};

/// One line of the Daily table.
#[derive(Clone, Debug, PartialEq)]
pub struct DayRow {
    pub date: String,
    pub tokens: String,
    pub cost: String,
    pub sessions: String,
    pub messages: String,
}

/// The days newest first (`right.date.localeCompare(left.date)`: the day you came to check is on top).
pub fn day_rows(days: &[Daily]) -> Vec<DayRow> {
    let mut sorted: Vec<&Daily> = days.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));
    sorted
        .into_iter()
        .map(|d| DayRow {
            date: d.date.clone(),
            tokens: format_count(Some(d.tokens)),
            cost: format_money(Some(d.cost)),
            sessions: format_count(Some(d.sessions)),
            messages: format_count(Some(d.messages)),
        })
        .collect()
}

/// One card of the Projects grid.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectCard {
    pub name: String,
    pub tokens: String,
    pub cost: String,
    /// Percent of the largest cost, 0 to 100.
    pub width: f64,
    /// `toFixed(0)` of the width, as the card says it.
    pub percent: String,
}

/// JavaScript's `toFixed(0)`: halves round up (`(2.5).toFixed(0)` is "3", where Rust's `{:.0}` says "2").
pub fn to_fixed_0(x: f64) -> String {
    format!("{}", (x + 0.5).floor())
}

/// The cards by cost, largest first, then by name (`localeCompare`, so case is looked at second).
pub fn project_cards(projects: &[TopItem]) -> Vec<ProjectCard> {
    let max = projects.iter().fold(0.0_f64, |m, p| m.max(p.cost));
    let mut sorted: Vec<&TopItem> = projects.iter().collect();
    sorted.sort_by(|a, b| {
        let by_cost = (b.cost - a.cost).partial_cmp(&0.0).filter(|o| o.is_ne());
        by_cost.unwrap_or_else(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)))
    });
    sorted
        .into_iter()
        .map(|p| {
            let width = relative_width(p.cost, max);
            ProjectCard {
                name: p.name.clone(),
                tokens: format_count(Some(p.tokens)),
                cost: format_money(Some(p.cost)),
                width,
                percent: to_fixed_0(width),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(date: &str, tokens: f64) -> Daily {
        Daily { date: date.into(), tokens, cost: 1.5, estimated_cost: 1.5, sessions: 2.0, messages: 1234.0 }
    }

    fn item(name: &str, cost: f64) -> TopItem {
        TopItem { name: name.into(), tokens: 12_500.0, cost }
    }

    #[test]
    fn days_come_newest_first_with_fleets_number_words() {
        let rows = day_rows(&[day("2026-11-18", 1.0), day("2026-11-20", 12_500.0), day("2026-11-19", 3.0)]);
        let dates: Vec<&str> = rows.iter().map(|r| r.date.as_str()).collect();
        assert_eq!(dates, ["2026-11-20", "2026-11-19", "2026-11-18"]);
        assert_eq!(rows[0].tokens, "12,500");
        assert_eq!(rows[0].cost, "$1.50");
        assert_eq!(rows[0].messages, "1,234");
    }

    #[test]
    fn project_cards_sort_by_cost_then_name_with_the_percent_as_to_fixed() {
        let cards = project_cards(&[item("beta", 5.0), item("alpha", 5.0), item("gamma", 20.0), item("delta", 0.0), item("eps", 2.5)]);
        let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["gamma", "alpha", "beta", "eps", "delta"]);
        // 5 / 20 = 25 %; 2.5 / 20 = 12.5 % rounds up to 13 as toFixed(0) does; a zero cost is 0 %.
        assert_eq!(cards[1].percent, "25");
        assert_eq!(cards[3].percent, "13");
        assert_eq!(cards[4].percent, "0");
        assert_eq!(cards[0].percent, "100");
        assert_eq!(cards[0].tokens, "12,500");
    }

    #[test]
    fn no_projects_make_no_cards() {
        assert!(project_cards(&[]).is_empty());
    }
}
