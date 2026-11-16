//! The home page's summary bar (component 9, piece F). Fleet's `SummaryBar.archive.test.ts` ("drops a session from Idle as
//! soon as it's archived, and puts it back on Undo") under its name; the rest written from reading `SummaryBar.vue`,
//! `use-fleet-summary.ts` and `use-analytics-daily.ts`.
use crate::fleet::archive_queue::UNDO_SECS;
use crate::fleet::home::Scene;
use crate::fleet::summary::{Bar, Day, Point, SPARK_H, Summary, bar_height, daily_path, days, iso_day, parse_daily, parse_summary, totals};
use crate::tests::{FRAME, app, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::tests_dashboard_actions::{FALCON, on_falcon};
use rune::Runtime;
use serde_json::json;

fn shown(rt: &Runtime<crate::State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.or(e.label))
}

fn home(scene: Scene) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_home_scene(scene, true);
        let mut n = 0;
        for p in &mut s.projects {
            for x in &mut p.sessions {
                n += 1;
                x.id = format!("s{n}");
            }
        }
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn cells(rt: &Runtime<crate::State>) -> Vec<String> {
    ["active", "idle", "queued", "tokens", "cost"].iter().map(|k| shown(rt, &format!("summary-{k}-count")).unwrap_or_default()).collect()
}

// ---- SummaryBar.archive.test.ts --------------------------------------------------------------------------------------------

#[test]
fn drops_a_session_from_idle_as_soon_as_its_archived_and_puts_it_back_on_undo() {
    let mut rt = home(Scene::Three);
    assert_eq!(shown(&rt, "summary-idle-count").as_deref(), Some("2"));
    rt.update(|s, tx| s.archive_sessions(&["s3".to_string()], None, tx));
    rt.frame(FRAME);
    assert_eq!(shown(&rt, "summary-idle-count").as_deref(), Some("1"));
    rt.update(|s, _| s.undo_archive());
    rt.frame(FRAME);
    assert_eq!(shown(&rt, "summary-idle-count").as_deref(), Some("2"));
}

// ---- the bar, in Fleet's words -------------------------------------------------------------------------------------------

#[test]
fn the_bar_has_five_cells_in_fleets_words_and_the_spend_button() {
    let rt = home(Scene::Three);
    for key in ["summary-bar", "summary-spend", "summary-spend-total"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
    let labels: Vec<_> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    for word in ["Working", "Idle", "Queued", "Total tokens", "Est. spend", "Last 14 days"] {
        assert!(labels.iter().any(|l| l == word), "{word}: {labels:?}");
    }
    assert_eq!(cells(&rt), ["1", "2", "2", "1.2M", "$18.42"]);
    assert_eq!(shown(&rt, "summary-spend").as_deref(), Some("Estimated spend over the last 14 days: $47.20. Open Analytics"));
}

#[test]
fn the_bar_shows_on_the_empty_and_loading_pages_too_with_zeros() {
    for scene in [Scene::Empty, Scene::Loading, Scene::Error] {
        let rt = home(scene);
        assert_eq!(cells(&rt)[..2], ["0", "0"], "{scene:?}");
    }
}

#[test]
fn the_spend_button_opens_analytics_and_the_bar_stops_reading_behind_it() {
    let mut rt = home(Scene::Three);
    let asked = rt.state.bar.asked;
    assert!(asked >= 2);
    click(&mut rt, "summary-spend");
    assert_eq!(rt.state.rail, crate::Rail::Analytics);
    rt.frame(31.0);
    assert_eq!(rt.state.bar.asked, asked, "nothing is read while the page is covered");
}

// ---- the rules -------------------------------------------------------------------------------------------------------------

#[test]
fn a_summary_body_reads_the_five_numbers_and_a_missing_one_is_zero() {
    let body = json!({"activeSessions": 2, "idleSessions": 3, "queuedTasks": 1, "totalTokens": 17_100, "totalCost": 1.85});
    let s = parse_summary(&body).unwrap();
    assert_eq!((s.active, s.idle, s.queued, s.tokens, s.cost), (2, 3, 1, 17_100.0, 1.85));
    assert_eq!(parse_summary(&json!({})).unwrap(), Default::default());
    assert!(parse_summary(&json!([])).is_none());
}

#[test]
fn days_fill_the_quiet_ones_in_and_end_on_today() {
    let now = 1_800_000_000_000i64;
    let today = iso_day(now / 86_400_000);
    assert_eq!(today, "2027-01-15");
    let list = days(&[Point { date: today.clone(), cost: 2.0 }, Point { date: "2027-01-10".into(), cost: 1.0 }], now);
    assert_eq!(list.len(), 14);
    assert_eq!(list[13], Day { key: today, cost: 2.0 });
    assert_eq!(list[0].key, "2027-01-02");
    assert_eq!((list[8].key.as_str(), list[8].cost), ("2027-01-10", 1.0));
    assert_eq!(totals(&list), (3.0, 2.0));
    assert_eq!(daily_path(now), "/api/analytics/daily?from=2027-01-02&to=2027-01-15");
}

#[test]
fn bar_heights_are_2px_with_nothing_else_at_least_8_percent_and_the_tallest_fills_it() {
    assert_eq!(bar_height(0.0, 5.0), 2.0);
    assert_eq!(bar_height(3.0, 0.0), 2.0);
    assert_eq!(bar_height(5.0, 5.0), SPARK_H);
    assert!((bar_height(0.1, 5.0) - SPARK_H * 0.08).abs() < 1e-4, "the floor");
    assert!((bar_height(2.5, 5.0) - SPARK_H * 0.5).abs() < 1e-4);
}

#[test]
fn todays_bar_is_the_accent_and_the_rest_are_45_percent_of_it() {
    let rt = home(Scene::Three);
    let fill = |i: usize| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("summary-bar-{i}"))).map(|e| e.rect);
    assert!(fill(0).is_some() && fill(13).is_some() && fill(14).is_none());
    let (first, last) = (fill(0).unwrap(), fill(13).unwrap());
    assert_eq!((first[2], last[2]), (6.0, 6.0));
    assert_eq!(first[3], 2.0, "a day with no spend");
    assert!(last[3] > 2.0 && last[3] <= SPARK_H);
    assert!((fill(1).unwrap()[0] - first[0] - 9.0).abs() < 0.5, "3 px between 6 px bars");
}

#[test]
fn a_daily_body_is_a_list_of_dates_and_costs() {
    let body = json!([{"date": "2026-11-15", "cost": 1.5, "tokens": 3}, {"date": "2026-11-16"}]);
    let list = parse_daily(&body).unwrap();
    assert_eq!(list, [Point { date: "2026-11-15".into(), cost: 1.5 }, Point { date: "2026-11-16".into(), cost: 0.0 }]);
    assert!(parse_daily(&json!({})).is_none());
    let _ = Bar::default();
}

// ---- reading it ------------------------------------------------------------------------------------------------------------

fn fleet_answers(path: &str) -> (u16, String) {
    if path == "/api/fleet/summary" {
        (200, json!({"activeSessions": 4, "idleSessions": 6, "queuedTasks": 3, "totalTokens": 2_500_000, "totalCost": 12.5}).to_string())
    } else if path.starts_with("/api/analytics/daily") {
        let today = iso_day(crate::fleet::link::now_ms() / 86_400_000);
        (
            200,
            json!([{"date": today, "cost": 8.0}, {"date": iso_day(crate::fleet::link::now_ms() / 86_400_000 - 2), "cost": 4.0}])
                .to_string(),
        )
    } else {
        (200, "[]".into())
    }
}

#[test]
fn it_reads_the_summary_and_the_spend_from_the_server_and_shows_them() {
    let fake = Fake::start(|_, path| fleet_answers(path));
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.go(crate::fleet::store::Route::Home));
    r.wait("the summary and the spend", |s| s.bar.summary.is_some() && !s.bar.daily.is_empty());
    r.rt.frame(FRAME);
    let (q, tok, cost) = (shown(&r.rt, "summary-queued-count"), shown(&r.rt, "summary-tokens-count"), shown(&r.rt, "summary-cost-count"));
    assert_eq!((q.as_deref(), tok.as_deref(), cost.as_deref()), (Some("3"), Some("2.5M"), Some("$12.50")));
    assert_eq!(shown(&r.rt, "summary-spend-total").as_deref(), Some("$12.00"));
    assert_eq!(fake.calls("/api/fleet/summary").len(), 1);
    let daily = fake.calls("/api/analytics/daily");
    assert_eq!(daily.len(), 1);
    assert!(daily[0].1.starts_with("/api/analytics/daily?from=") && daily[0].1.contains("&to="), "{}", daily[0].1);
}

#[test]
fn the_summary_is_read_again_every_30_seconds_and_the_spend_every_5_minutes() {
    let fake = Fake::start(|_, path| fleet_answers(path));
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.go(crate::fleet::store::Route::Home));
    r.wait("the first reads", |s| s.bar.summary.is_some() && !s.bar.daily.is_empty());
    r.rt.frame(29.0);
    assert_eq!(fake.calls("/api/fleet/summary").len(), 1, "not before 30 s");
    r.rt.frame(1.5);
    r.wait("the second summary", |_| fake.calls("/api/fleet/summary").len() == 2);
    assert_eq!(fake.calls("/api/analytics/daily").len(), 1, "the spend is on its own, slower clock");
    r.rt.frame(270.0);
    r.wait("the second spend", |_| fake.calls("/api/analytics/daily").len() == 2);
}

#[test]
fn it_reads_the_summary_again_once_an_archive_has_settled() {
    let fake = Fake::start(|_, path| fleet_answers(path));
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.go(crate::fleet::store::Route::Home));
    r.wait("the first read", |s| s.bar.summary.is_some());
    let id = r.state().projects[0].sessions[0].id.clone();
    r.rt.update(move |s, tx| s.archive_sessions(&[id], None, tx));
    r.rt.frame(FRAME);
    assert_eq!(fake.calls("/api/fleet/summary").len(), 1);
    r.rt.frame(UNDO_SECS + 0.5);
    r.wait("the archive to be sent and the summary asked again", |s| {
        !s.archive.any_hidden() && fake.calls("/api/fleet/summary").len() >= 2
    });
}

#[test]
fn a_failed_read_keeps_what_the_bar_held() {
    let down = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gate = down.clone();
    let fake =
        Fake::start(move |_, path| if gate.load(std::sync::atomic::Ordering::SeqCst) { (503, "{}".into()) } else { fleet_answers(path) });
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.go(crate::fleet::store::Route::Home));
    r.wait("the first read", |s| s.bar.summary.is_some() && !s.bar.daily.is_empty());
    down.store(true, std::sync::atomic::Ordering::SeqCst);
    r.rt.frame(31.0);
    r.wait("the refused read", |_| fake.calls("/api/fleet/summary").len() == 2);
    r.rt.frame(FRAME);
    assert_eq!(cells(&r.rt)[2..], ["3", "2.5M", "$12.50"], "the last good numbers stay");
}

#[test]
fn an_answer_a_newer_read_overtook_is_dropped() {
    let mut bar = Bar::default();
    let body = |n: u32| Summary { active: n, ..Default::default() };
    bar.summary_request = 2;
    assert!(!bar.summary_answer(1, Ok(body(9))), "stale");
    assert!(bar.summary.is_none());
    assert!(bar.summary_answer(2, Ok(body(1))));
    assert!(bar.summary_answer(2, Err("down".into())));
    assert_eq!(bar.summary.map(|s| s.active), Some(1), "a failure keeps what was held");
}

#[test]
fn the_bar_on_another_machine_reads_that_machine_with_its_own_token() {
    let falcon = Fake::start(|_, path| {
        if path == "/api/fleet/summary" {
            (200, json!({"activeSessions": 1, "idleSessions": 0, "queuedTasks": 0, "totalTokens": 950, "totalCost": 0.004}).to_string())
        } else {
            fleet_answers(path)
        }
    });
    let (mut r, home) = on_falcon(&falcon);
    r.wait("falcon's summary", |s| s.bar.summary.is_some());
    r.rt.frame(FRAME);
    assert_eq!(shown(&r.rt, "summary-tokens-count").as_deref(), Some("950"));
    assert_eq!(shown(&r.rt, "summary-cost-count").as_deref(), Some("<$0.01"));
    let asked = falcon.calls("/api/fleet/summary");
    assert_eq!((asked.len(), asked[0].3.as_deref()), (1, Some("falcon-token-0123456789")));
    assert!(home.calls("/api/fleet/summary").is_empty(), "home isn't asked for falcon's numbers");
    let _ = FALCON;
}
