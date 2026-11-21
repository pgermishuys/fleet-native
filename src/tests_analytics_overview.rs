//! Component 20, piece F: Overview's cards, panels and cost bars. Fleet's `AnalyticsTabsErrorOrEmpty.test.ts` rows for
//! `HorizontalCostBars` and `AnalyticsContentLoading.test.ts`'s overview row, under their names; the card details and bar
//! rules are in `fleet::analytics::overview`'s own tests.
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app};
use crate::{Rail, State};
use rune::{ElementInfo, Runtime};

fn page(scene: Scene) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics(true, scene));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn info(rt: &Runtime<State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    info(rt, key).is_some()
}

fn text_of(rt: &Runtime<State>, key: &str) -> String {
    info(rt, key).and_then(|e| e.text).unwrap_or_else(|| panic!("no text keyed {key:?}"))
}

/// Whether some text on the page reads exactly `words`.
fn says(rt: &Runtime<State>, words: &str) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some(words))
}

/// `HorizontalCostBars`' two rankings, under the Overview scene that gives each state.
const RANKINGS: [&str; 2] = ["analytics-bars-models", "analytics-bars-projects"];

#[test]
fn horizontal_cost_bars_shows_the_load_error_not_the_empty_state_when_the_load_failed() {
    let rt = page(Scene::Failed);
    for bars in RANKINGS {
        assert!(shown(&rt, &format!("{bars}-error")), "{bars}");
        assert!(!shown(&rt, &format!("{bars}-empty")), "{bars}");
        assert_eq!(info(&rt, &format!("{bars}-error")).map(|e| e.role), Some("alert"));
    }
    // The empty words come back once the answer is just empty.
    let rt = page(Scene::Nothing);
    for bars in RANKINGS {
        assert!(shown(&rt, &format!("{bars}-empty")) && !shown(&rt, &format!("{bars}-error")), "{bars}");
    }
}

#[test]
fn horizontal_cost_bars_says_its_loading_not_that_nothing_matched() {
    let rt = page(Scene::Loading);
    for bars in RANKINGS {
        assert!(shown(&rt, &format!("{bars}-loading")), "{bars}");
        assert!(!shown(&rt, &format!("{bars}-empty")), "{bars}");
    }
}

#[test]
fn the_overview_section_says_its_loading_not_that_nothing_matched() {
    let rt = page(Scene::Loading);
    for key in ["analytics-loading", "analytics-bars-models-loading", "analytics-bars-projects-loading"] {
        assert!(shown(&rt, key), "{key}");
    }
    for key in ["analytics-empty", "analytics-bars-models-empty", "analytics-bars-projects-empty", "analytics-error"] {
        assert!(!shown(&rt, key), "{key}");
    }
}

#[test]
fn the_overview_with_answers_has_four_cards_the_panels_and_the_ranked_bars() {
    let rt = page(Scene::Menu);
    for id in ["tokens", "cost", "sessions", "messages"] {
        assert!(shown(&rt, &format!("analytics-card-{id}")), "{id}");
    }
    assert_eq!(text_of(&rt, "analytics-card-tokens-value"), "45M");
    assert_eq!(text_of(&rt, "analytics-card-cost-secondary"), "Est. $101.20");
    assert_eq!(text_of(&rt, "analytics-estimated-total"), "Est. total $101.20");
    assert!(shown(&rt, "analytics-chart-slot"), "rows give the chart its slot");
    // The most expensive model heads its list.
    assert!(shown(&rt, "analytics-bars-models-item-0") && shown(&rt, "analytics-bars-models-item-4"));
    assert_eq!(
        info(&rt, "analytics-bars-models-track-0").and_then(|e| e.label).as_deref(),
        Some("claude-opus-5-5 cost bar at 100 percent")
    );
}

#[test]
fn a_failed_read_shows_the_error_note_with_the_reason_and_not_the_empty_words() {
    let rt = page(Scene::Failed);
    assert!(shown(&rt, "analytics-error") && shown(&rt, "analytics-bars-models-error") && shown(&rt, "analytics-bars-projects-error"));
    assert!(!shown(&rt, "analytics-empty") && !shown(&rt, "analytics-bars-models-empty"));
}

#[test]
fn nothing_found_says_so_with_the_sections_words() {
    let rt = page(Scene::Nothing);
    assert!(says(&rt, "No daily analytics matched the selected date range."));
    assert!(says(&rt, "No model analytics matched the selected date range."));
    assert!(says(&rt, "No project analytics matched the selected date range."));
    assert_eq!(text_of(&rt, "analytics-card-tokens-detail"), "No daily token breakdown available");
}

#[test]
fn other_sections_leave_the_overview_out() {
    let mut rt = page(Scene::Menu);
    rt.update(|s, _| s.open_analytics_address("/analytics/models"));
    rt.frame(FRAME);
    assert!(!shown(&rt, "analytics-overview"));
    assert_eq!(rt.state.rail, Rail::Analytics);
}
