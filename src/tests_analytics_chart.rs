//! Component 20, piece G: the Overview's chart, in a headless app: what it says, what hovering and the legend do. The numbers
//! (ticks, area, points, the tooltip's box) are held against Chart.js's own in `fleet::tests::analytics_chart`.
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State};
use rune::{ElementInfo, Runtime};

fn page(scene: Scene) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics(true, scene));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt
}

fn info(rt: &Runtime<State>, key: &str) -> Option<ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    info(rt, key).and_then(|e| e.text)
}

fn tip_title(rt: &Runtime<State>) -> Option<String> {
    text_of(rt, "analytics-chart-tooltip-title")
}

/// The x of day `i`'s label's centre and a y inside the chart's area.
fn over_day(rt: &Runtime<State>, i: usize) -> (f32, f32) {
    let (x, _) = rt.find(&format!("analytics-chart-x-{i}")).unwrap_or_else(|| panic!("no label for day {i}"));
    let slot = info(rt, "analytics-chart-slot").expect("the chart").rect;
    (x, slot[1] + 120.0)
}

#[test]
fn the_chart_follows_its_panels_width_and_names_itself_for_a_screen_reader() {
    let rt = page(Scene::Menu);
    let slot = info(&rt, "analytics-chart-slot").expect("the chart");
    assert_eq!(slot.rect[3], 300.0, "300 tall");
    assert!((rt.state.analytics.chart.width - slot.rect[2]).abs() < 0.5, "laid out to the panel's width");
    assert_eq!(
        slot.label.as_deref(),
        Some("Line chart of daily tokens and cost, Oct 29 to Nov 20. The numbers follow in a table."),
        "Fleet's aria-label"
    );
    let table = info(&rt, "analytics-daily-trend-table").expect("the table for a screen reader");
    assert_eq!(table.role, "table");
}

#[test]
fn the_axes_say_zero_to_the_top_in_tokens_on_the_left_and_money_on_the_right() {
    let rt = page(Scene::Menu);
    assert_eq!(text_of(&rt, "analytics-chart-left-0").as_deref(), Some("0"));
    assert_eq!(text_of(&rt, "analytics-chart-right-0").as_deref(), Some("$0.00"));
    let last_left = (0..).take_while(|i| info(&rt, &format!("analytics-chart-left-{i}")).is_some()).last().unwrap();
    assert!(text_of(&rt, &format!("analytics-chart-left-{last_left}")).unwrap().ends_with('M'));
    assert_eq!(text_of(&rt, "analytics-chart-x-0").as_deref(), Some("Oct 29"));
    assert_eq!(text_of(&rt, "analytics-legend-word-tokens").as_deref(), Some("Tokens"));
    assert_eq!(text_of(&rt, "analytics-legend-word-cost").as_deref(), Some("Cost"));
}

#[test]
fn hovering_picks_the_nearest_day_and_the_tooltip_leaves_with_the_pointer() {
    let mut rt = page(Scene::Menu);
    assert!(tip_title(&rt).is_none(), "no tooltip until the pointer is over the chart");
    let (x, y) = over_day(&rt, 5);
    rt.pointer_move(x + 3.0, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hover, Some(5));
    let title = tip_title(&rt).expect("the tooltip's title");
    assert_eq!(Some(title), text_of(&rt, "analytics-chart-x-5"), "the day, as the axis writes it");
    let tokens = text_of(&rt, "analytics-chart-tooltip-row-0").unwrap();
    let cost = text_of(&rt, "analytics-chart-tooltip-row-1").unwrap();
    assert!(tokens.starts_with("Tokens: ") && cost.starts_with("Cost: $"), "{tokens} / {cost}");
    assert!(text_of(&rt, "analytics-chart-tooltip-after-1").unwrap().starts_with("Estimated: $"));
    // Another day.
    let (x, y) = over_day(&rt, 9);
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hover, Some(9));
    // Off the chart it goes.
    rt.pointer_move(5.0, 5.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hover, None);
    assert!(tip_title(&rt).is_none());
}

#[test]
fn a_pointer_over_an_axis_keeps_the_last_day_as_chart_js_does() {
    let mut rt = page(Scene::Menu);
    let (x, y) = over_day(&rt, 3);
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hover, Some(3));
    let slot = info(&rt, "analytics-chart-slot").unwrap().rect;
    // Below the area, over the x axis' labels, but still in the chart.
    rt.pointer_move(x + 40.0, slot[1] + 290.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hover, Some(3));
}

#[test]
fn pressing_a_legend_entry_hides_its_line_and_pressing_again_shows_it() {
    let mut rt = page(Scene::Menu);
    assert_eq!(rt.state.analytics.chart.hidden, [false, false]);
    click(&mut rt, "analytics-legend-cost");
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hidden, [false, true]);
    assert_eq!(info(&rt, "analytics-legend-cost").and_then(|e| e.label).as_deref(), Some("Cost, hidden"));
    // Cost's axis has no data: Chart.js reads it as 0 to 1.
    assert_eq!(text_of(&rt, "analytics-chart-right-0").as_deref(), Some("$0.00"));
    let last = (0..).take_while(|i| info(&rt, &format!("analytics-chart-right-{i}")).is_some()).last().unwrap();
    assert_eq!(text_of(&rt, &format!("analytics-chart-right-{last}")).as_deref(), Some("$1.00"));
    // A tooltip then has no Cost row.
    let (x, y) = over_day(&rt, 4);
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(text_of(&rt, "analytics-chart-tooltip-row-0").unwrap().starts_with("Tokens: "));
    assert!(info(&rt, "analytics-chart-tooltip-row-1").is_none());
    click(&mut rt, "analytics-legend-cost");
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hidden, [false, false]);
    assert_eq!(info(&rt, "analytics-legend-cost").and_then(|e| e.label).as_deref(), Some("Cost"));
}

#[test]
fn both_lines_hidden_leave_no_tooltip_rows_and_the_chart_still_stands() {
    let mut rt = page(Scene::Menu);
    click(&mut rt, "analytics-legend-tokens");
    click(&mut rt, "analytics-legend-cost");
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.chart.hidden, [true, true]);
    assert!(info(&rt, "analytics-chart-slot").is_some());
}

#[test]
fn the_hidden_lines_come_back_when_the_section_is_left() {
    let mut rt = page(Scene::Menu);
    click(&mut rt, "analytics-legend-cost");
    rt.update(|s, _| {
        s.open_analytics_section(crate::fleet::analytics::sections::Section::Daily);
        s.open_analytics_section(crate::fleet::analytics::sections::Section::Overview);
    });
    assert_eq!(rt.state.analytics.chart.hidden, [false, false], "Fleet's chart is destroyed with its section");
}

#[test]
fn no_rows_means_no_chart_just_the_states_around_it() {
    let rt = page(Scene::Nothing);
    assert!(info(&rt, "analytics-chart-slot").is_none());
    assert!(info(&rt, "analytics-empty").is_some());
    let _ = Rail::Analytics;
}
