//! Component 20, piece D: the filters in the menu (`AnalyticsFilters.vue`). The two date fields are typed (the browser's field is
//! segmented; here a whole date is what counts), the project select, Reset, and that the filters outlive a restart.
//! Fleet has no test file for this component (`AnalyticsFilters` has none), so these are the native rules of rows 3.5 to 3.8.
use crate::fleet::analytics::fields::{Typed, parse, show};
use crate::fleet::analytics::filters::Filters;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::{HOME_MACHINE_KEY, Sidebar};
use crate::mock::analytics::Scene;
use crate::tests::{FRAME, app, click, press_key};
use crate::{Rail, State, test_wait};
use rune::{Key, Mods, Runtime};
use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// 20 November 2026, noon UTC: the day the pictures are drawn for.
const NOW_MS: i64 = 20_777 * 86_400_000 + 43_200_000;

fn page(scene: Scene) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_analytics(true, scene));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

/// Replaces what a date field holds with `words`.
fn type_in(rt: &mut Runtime<State>, key: &str, words: &str) {
    click(rt, key);
    press_key(rt, Key::Command('a'), Mods::default());
    if words.is_empty() {
        press_key(rt, Key::Backspace, Mods::default());
    } else {
        press_key(rt, Key::Text(words.into()), Mods::default());
    }
}

fn filters(rt: &Runtime<State>) -> Filters {
    rt.state.analytics.filters.clone()
}

fn day(from: &str, to: &str) -> Filters {
    Filters { from: from.into(), to: to.into(), project_id: String::new() }
}

// ─── the field's form ───────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_date_is_written_month_day_year_and_read_back() {
    assert_eq!(show("2026-11-20"), "11/20/2026");
    assert_eq!(show(""), "", "no end, an empty field");
    assert_eq!(show("soon"), "", "not a date, an empty field");
    assert_eq!(parse("11/20/2026"), Typed::Whole("2026-11-20".into()));
    assert_eq!(parse(""), Typed::Empty);
    for partial in ["1", "11/2", "11/20/202", "1/2/2026", "11-20-2026", "ab/cd/efgh", "11/20/2026 "] {
        assert_eq!(parse(partial), Typed::Partial, "{partial:?}");
    }
    for not_a_day in ["02/30/2026", "13/01/2026", "00/10/2026", "11/00/2026", "11/20/0000"] {
        assert_eq!(parse(not_a_day), Typed::Partial, "{not_a_day:?} is no day");
    }
    assert_eq!(parse("02/29/2028"), Typed::Whole("2028-02-29".into()), "a leap day");
}

// ─── rows 3.5, 3.6: the range box ────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_block_shows_the_range_the_page_opened_on_as_month_day_year() {
    let rt = page(Scene::Filters);
    for key in ["analytics-filters", "analytics-filter-from", "analytics-filter-to", "analytics-project", "analytics-filter-reset"] {
        assert!(rt.find(key).is_some(), "{key} is drawn");
    }
    assert_eq!(filters(&rt), day("2026-10-21", "2026-11-20"));
    assert_eq!(rt.state.analytics.fields.from.text, "10/21/2026");
    assert_eq!(rt.state.analytics.fields.to.text, "11/20/2026");
}

#[test]
fn a_whole_typed_date_is_sent_and_a_partial_one_is_not() {
    let mut rt = page(Scene::Filters);
    type_in(&mut rt, "analytics-filter-from", "11/05/2026");
    assert_eq!(filters(&rt), day("2026-11-05", "2026-11-20"));

    let before = filters(&rt);
    type_in(&mut rt, "analytics-filter-to", "11/2");
    assert_eq!(filters(&rt), before, "half a date changes nothing");
    assert_eq!(rt.state.analytics.fields.to.text, "11/2", "and the field keeps what is being typed");
}

#[test]
fn a_field_left_half_typed_goes_back_to_the_date_it_had() {
    let mut rt = page(Scene::Filters);
    type_in(&mut rt, "analytics-filter-to", "11/2");
    click(&mut rt, "analytics-filter-from");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.analytics.fields.to.text, "11/20/2026");
    assert_eq!(filters(&rt).to, "2026-11-20");
}

#[test]
fn an_end_with_a_date_past_the_other_end_is_still_sent() {
    // The browser's `max`/`min` only limit its picker; a typed date outside them is sent as it is.
    let mut rt = page(Scene::Filters);
    type_in(&mut rt, "analytics-filter-from", "12/25/2026");
    assert_eq!(filters(&rt), day("2026-12-25", "2026-11-20"));
    type_in(&mut rt, "analytics-filter-to", "01/01/2026");
    assert_eq!(filters(&rt), day("2026-12-25", "2026-01-01"));
}

#[test]
fn an_emptied_field_clears_that_end() {
    let mut rt = page(Scene::Filters);
    type_in(&mut rt, "analytics-filter-from", "");
    assert_eq!(filters(&rt), day("", "2026-11-20"));
    assert_eq!(rt.state.analytics.fields.from.text, "");
    type_in(&mut rt, "analytics-filter-to", "");
    assert_eq!(filters(&rt), day("", ""));
}

// ─── row 3.7: the project select ─────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_project_select_lists_all_projects_then_the_ranges_projects_and_picking_one_sets_its_name() {
    let mut rt = page(Scene::Filters);
    click(&mut rt, "analytics-project");
    let names: Vec<String> = (0..6).filter_map(|i| rt.find(&format!("analytics-project-list-opt{i}")).map(|_| i.to_string())).collect();
    assert_eq!(names.len(), 6, "All projects and the five of the range");
    assert!(rt.find("analytics-project-list-opt6").is_none());
    click(&mut rt, "analytics-project-list-opt2");
    assert_eq!(filters(&rt).project_id, "kettle-web", "the project's name, as Fleet sets it");
    assert!(rt.find("analytics-project-list-menu").is_none(), "the list closes");

    click(&mut rt, "analytics-project");
    click(&mut rt, "analytics-project-list-opt0");
    assert_eq!(filters(&rt).project_id, "", "All projects clears it");
}

#[test]
fn a_picked_project_that_the_range_no_longer_lists_is_still_offered() {
    let mut rt = page(Scene::Filters);
    rt.update(|s, _| s.analytics_pick_project("old-project"));
    rt.frame(FRAME);
    click(&mut rt, "analytics-project");
    assert!(rt.find("analytics-project-list-opt6").is_some(), "after the five, the picked one");
}

// ─── row 3.8: Reset ──────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn reset_is_off_at_the_defaults_and_restores_them() {
    let mut rt = page(Scene::Filters);
    click(&mut rt, "analytics-filter-reset");
    assert_eq!(filters(&rt), day("2026-10-21", "2026-11-20"), "nothing to reset: pressing it does nothing");

    type_in(&mut rt, "analytics-filter-from", "11/05/2026");
    rt.update(|s, _| s.analytics_pick_project("kettle-web"));
    rt.frame(FRAME);
    assert_ne!(filters(&rt), Filters::defaults(NOW_MS));
    click(&mut rt, "analytics-filter-reset");
    rt.frame(FRAME);
    assert_eq!(filters(&rt), Filters::defaults(NOW_MS));
    assert_eq!(rt.state.analytics.fields.from.text, "10/21/2026", "the field shows the restored date");
    assert_eq!(rt.state.analytics.fields.to.text, "11/20/2026");
}

#[test]
fn a_day_moved_by_hand_back_to_the_default_leaves_nothing_to_reset() {
    let mut rt = page(Scene::Filters);
    type_in(&mut rt, "analytics-filter-from", "11/05/2026");
    assert!(!rt.state.analytics.filters.is_default(NOW_MS));
    type_in(&mut rt, "analytics-filter-from", "10/21/2026");
    assert!(rt.state.analytics.filters.is_default(NOW_MS));
}

// ─── row 3.9 and the settings file: the reads and the restart ───────────────────────────────────────────────────────────

#[test]
fn the_filters_survive_a_restart() {
    let mut rt = page(Scene::Filters);
    type_in(&mut rt, "analytics-filter-from", "11/05/2026");
    rt.update(|s, _| s.analytics_pick_project("kettle-web"));
    // What the settings file holds is read back by the next start.
    let written = rt.state.sidebar.to_json();
    let next = Sidebar::parse(&written);
    assert_eq!(
        next.analytics_filters(HOME_MACHINE_KEY),
        Some(&Filters { from: "2026-11-05".into(), to: "2026-11-20".into(), project_id: "kettle-web".into() })
    );
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    heard: Arc<Mutex<Vec<String>>>,
}

/// The app connected to a fake Fleet that records every analytics request path, on the Analytics rail with the clock pinned.
fn rig() -> Rig {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = heard.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let log = log.clone();
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                let body: Value = match req.route().strip_prefix("/api/analytics/") {
                    Some("summary") => json!({"totalTokens": 1, "topProjects": [{"name": "harbor-api", "tokens": 10, "cost": 2.5}]}),
                    Some(_) => json!([]),
                    None => return crate::test_http::respond_json(&mut stream, "404 X", "{}"),
                };
                log.lock().unwrap().push(req.path.clone());
                crate::test_http::respond_json(&mut stream, "200 X", &body.to_string());
            });
        }
    });
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start_machine(HOME_MACHINE_KEY.to_string(), base, Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.now_ms = Some(NOW_MS);
        s.rail = Rail::Analytics;
    });
    rt.frame(FRAME);
    Rig { rt, queue, heard }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("asked {}", s.analytics.asked));
    }

    /// The requests heard for a route.
    fn on(&self, route: &str) -> Vec<String> {
        heard_on(&self.heard, route)
    }

    /// Waits until `route` has been heard `n` times.
    fn wait_heard(&mut self, route: &'static str, n: usize) {
        let heard = self.heard.clone();
        self.wait(&format!("{n} requests to {route}"), move |_| heard_on(&heard, route).len() >= n);
    }
}

fn heard_on(heard: &Mutex<Vec<String>>, route: &str) -> Vec<String> {
    let prefix = format!("/api/analytics/{route}");
    heard.lock().unwrap().iter().filter(|p| p.starts_with(&prefix)).cloned().collect()
}

#[test]
fn a_typed_date_and_a_project_pick_each_read_the_sections_again_with_them() {
    let mut r = rig();
    r.wait("the first answers", |s| {
        let a = &s.analytics;
        !(a.summary.is_loading || a.range.is_loading || a.daily.is_loading || a.models.is_loading || a.sessions.is_loading)
    });
    assert_eq!(r.on("daily"), ["/api/analytics/daily?from=2026-10-21&to=2026-11-20"]);

    type_in(&mut r.rt, "analytics-filter-from", "11/05/2026");
    r.wait_heard("daily", 2);
    assert_eq!(r.on("daily")[1], "/api/analytics/daily?from=2026-11-05&to=2026-11-20");
    assert_eq!(r.on("summary").iter().filter(|p| p.contains("from=2026-11-05")).count(), 2, "the page's and the project list's");

    click(&mut r.rt, "analytics-project");
    // The list is the range's projects, read by the first answers.
    click(&mut r.rt, "analytics-project-list-opt1");
    assert_eq!(r.rt.state.analytics.filters.project_id, "harbor-api");
    r.wait_heard("daily", 3);
    assert_eq!(r.on("daily")[2], "/api/analytics/daily?from=2026-11-05&to=2026-11-20&projectId=harbor-api");
}
