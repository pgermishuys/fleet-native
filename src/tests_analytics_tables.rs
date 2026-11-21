//! Component 20, piece H: Daily's table and Projects' cards. Fleet's `AnalyticsTabsErrorOrEmpty.test.ts` rows for DailyTab and
//! ProjectsTab and `AnalyticsContentLoading.test.ts`'s daily and projects rows, under their names; the row order and the
//! percent are also held in `fleet::analytics::tables`'s own tests.
use crate::State;
use crate::fleet::analytics::sections::Section;
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app};
use rune::{ElementInfo, Runtime};

fn page(scene: Scene, section: Section) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics_section(true, scene, section));
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

const BOTH: [(Section, &str); 2] = [(Section::Daily, "daily"), (Section::Projects, "projects")];

#[test]
fn daily_tab_and_projects_tab_show_the_load_error_not_the_empty_state_when_the_load_failed() {
    for (section, name) in BOTH {
        let rt = page(Scene::Failed, section);
        assert!(shown(&rt, "analytics-error"), "{name}");
        assert_eq!(info(&rt, "analytics-error").map(|e| e.role), Some("alert"), "{name}");
        assert!(!shown(&rt, "analytics-empty"), "{name}");
        let rt = page(Scene::Nothing, section);
        assert!(shown(&rt, "analytics-empty") && !shown(&rt, "analytics-error"), "{name}");
    }
}

#[test]
fn daily_tab_and_projects_tab_say_theyre_loading_not_that_nothing_matched() {
    for (section, name) in BOTH {
        let rt = page(Scene::Loading, section);
        assert!(shown(&rt, "analytics-loading"), "{name}");
        assert!(!shown(&rt, "analytics-empty"), "{name}");
    }
}

#[test]
fn the_daily_and_projects_sections_say_theyre_loading_not_that_nothing_matched() {
    let rt = page(Scene::Loading, Section::Daily);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Loading daily analytics…")));
    let rt = page(Scene::Loading, Section::Projects);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Loading project analytics…")));
}

#[test]
fn nothing_found_says_each_sections_words() {
    let says = |rt: &Runtime<State>, w: &str| rt.elements().iter().any(|e| e.text.as_deref() == Some(w));
    assert!(says(&page(Scene::Nothing, Section::Daily), "No daily analytics matched the selected date range."));
    assert!(says(&page(Scene::Nothing, Section::Projects), "No project analytics matched the selected date range."));
}

#[test]
fn daily_lists_the_newest_day_first() {
    let rt = page(Scene::Menu, Section::Daily);
    assert!(shown(&rt, "analytics-daily-table"));
    let first = text_of(&rt, "analytics-daily-0-0");
    let second = text_of(&rt, "analytics-daily-1-0");
    assert!(first > second, "{first} should come after {second}");
    assert_eq!(first, "2026-11-20");
    assert_eq!(text_of(&rt, "analytics-daily-0-2"), "$13.60");
    assert!(shown(&rt, "analytics-daily-11-0") && !shown(&rt, "analytics-daily-12-0"));
}

#[test]
fn projects_are_cards_by_cost_with_the_percent_of_the_largest() {
    let rt = page(Scene::Menu, Section::Projects);
    assert!(shown(&rt, "analytics-project-harbor-api") && shown(&rt, "analytics-project-meadow-infra"));
    assert_eq!(text_of(&rt, "analytics-project-harbor-api-percent"), "100%");
    // 27.85 / 41.30 = 67.4 %
    assert_eq!(text_of(&rt, "analytics-project-kettle-web-percent"), "67%");
    assert_eq!(
        info(&rt, "analytics-project-kettle-web-track").and_then(|e| e.label).as_deref(),
        Some("kettle-web relative cost 67 percent")
    );
    let y = |name: &str| info(&rt, &format!("analytics-project-{name}")).map(|e| e.rect[1]).unwrap();
    assert!(y("harbor-api") <= y("meadow-infra"));
}
