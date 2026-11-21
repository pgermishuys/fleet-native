//! Component 20, piece J: the Sessions table. Fleet's `AnalyticsTabsErrorOrEmpty.test.ts` row "SessionsTab keeps its own words
//! for both" under its name, and the native checks: a column head sorts by its default way and flips on a second press, a sort
//! change shows the loading words until the answer, and the words for a missing title, project, model and duration.
use crate::State;
use crate::fleet::analytics::SessionRow;
use crate::fleet::analytics::sections::Section;
use crate::fleet::analytics::sessions_sort::{SortBy, SortDir};
use crate::fleet::analytics::store::MockAnswers;
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app, click};
use rune::{ElementInfo, Runtime};

fn page(scene: Scene) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics_section(true, scene, Section::Sessions));
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

fn has_text(rt: &Runtime<State>, words: &str) -> bool {
    rt.elements().iter().any(|e| e.text.as_deref() == Some(words))
}

#[test]
fn sessions_tab_keeps_its_own_words_for_both() {
    let rt = page(Scene::Failed);
    assert_eq!(info(&rt, "analytics-error").map(|e| e.role), Some("alert"));
    assert!(has_text(&rt, "Unable to load session analytics."));
    assert!(!shown(&rt, "analytics-empty"));
    let rt = page(Scene::Nothing);
    assert!(has_text(&rt, "No sessions matched the current analytics filters"));
    assert!(shown(&rt, "analytics-empty") && !shown(&rt, "analytics-error"));
}

#[test]
fn sessions_tab_says_its_loading_not_that_nothing_matched() {
    let rt = page(Scene::Loading);
    assert!(has_text(&rt, "Loading session analytics…"));
    assert!(!shown(&rt, "analytics-empty"));
}

#[test]
fn sessions_show_a_table_opening_on_created_newest_first() {
    let rt = page(Scene::Menu);
    assert!(shown(&rt, "analytics-sessions-table"));
    assert_eq!(rt.state.analytics.sort, (SortBy::CreatedAt, SortDir::Desc));
    assert!(has_text(&rt, "Estimated cost") && has_text(&rt, "Created"));
    // The mock's newest session is the first one.
    assert!(has_text(&rt, "s1"));
}

#[test]
fn pressing_a_head_sorts_by_its_default_way_and_again_flips_it() {
    let mut rt = page(Scene::Menu);
    click(&mut rt, "analytics-sessions-sort-title");
    assert_eq!(rt.state.analytics.sort, (SortBy::Title, SortDir::Asc));
    click(&mut rt, "analytics-sessions-sort-title");
    assert_eq!(rt.state.analytics.sort, (SortBy::Title, SortDir::Desc));
    click(&mut rt, "analytics-sessions-sort-cost");
    assert_eq!(rt.state.analytics.sort, (SortBy::Cost, SortDir::Desc));
    assert_eq!(info(&rt, "analytics-sessions-sort-cost").and_then(|e| e.label).as_deref(), Some("Cost, descending"));
}

#[test]
fn a_sort_change_says_its_loading_until_the_sessions_answer() {
    let mut rt = page(Scene::Menu);
    assert!(shown(&rt, "analytics-sessions-table"));
    // The next read gets no answer, as a slow server.
    rt.update(|s, _| s.analytics.mock_answers = MockAnswers::Silent);
    click(&mut rt, "analytics-sessions-sort-cost");
    rt.frame(FRAME);
    assert!(rt.state.analytics.refreshing_sessions());
    assert!(has_text(&rt, "Loading session analytics…") && !shown(&rt, "analytics-sessions-table"));
}

#[test]
fn missing_things_get_fleets_words() {
    let mut rt = page(Scene::Menu);
    rt.update(|s, _| {
        s.analytics.sessions.data = vec![SessionRow {
            session_id: "s-x".into(),
            title: Some("   ".into()),
            project_id: None,
            project_name: None,
            tokens: 1500.0,
            cost: 0.004,
            estimated_cost: 0.004,
            models: vec![],
            duration_seconds: None,
            created_at: "2026-11-20T09:10:00Z".into(),
        }];
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    for words in ["Untitled session", "Unassigned", "Unknown model", "—", "1,500", "<$0.01"] {
        assert!(has_text(&rt, words), "{words}");
    }
}
