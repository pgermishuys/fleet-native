//! Component 20, piece A. `analytics-sections.test.ts` and `use-analytics-filters.test.ts` under Fleet's names (the clock
//! pinned to Fleet's 9 October 2026, noon UTC), then the shapes the four reads answer, the queries, the sessions
//! comparisons and the filters kept per machine.
use super::super::analytics::filters::{Filters, day_number};
use super::super::analytics::sections::*;
use super::super::analytics::sessions_sort::*;
use super::super::analytics::*;
use super::super::automations::schedule::days_from_civil;
use super::super::settings::{HOME_MACHINE_KEY, Sidebar};
use serde_json::json;
use std::cmp::Ordering;

// ─── analytics-sections.test.ts ─────────────────────────────────────────────

#[test]
fn gives_every_section_its_own_route_overview_being_the_bare_analytics() {
    let paths: Vec<String> = SECTIONS.iter().map(|s| section_path(*s)).collect();
    assert_eq!(paths, ["/analytics", "/analytics/daily", "/analytics/projects", "/analytics/models", "/analytics/sessions"]);
}

#[test]
fn reads_the_section_back_from_its_route_trailing_slash_included() {
    for section in SECTIONS {
        assert_eq!(section_from_path(&section_path(section)), section);
        assert_eq!(section_from_path(&format!("{}/", section_path(section))), section);
    }
}

#[test]
fn opens_overview_for_an_address_that_isnt_a_section() {
    assert_eq!(section_from_path("/analytics/harness"), Section::Overview);
    assert_eq!(section_from_path("/analytics/"), Section::Overview);
    assert_eq!(section_from_path("/analytics/daily/extra"), Section::Daily);
    assert_eq!(section_from_path("/analytics/Daily"), Section::Overview);
    assert!(!is_section_id("harness"));
    assert!(is_section_id("sessions"));
}

#[test]
fn groups_spend_overview_daily_apart_from_breakdown_projects_models_sessions() {
    let in_group = |g: Group| SECTIONS.iter().filter(|s| s.group() == g).map(|s| s.id()).collect::<Vec<_>>();
    assert_eq!(in_group(Group::Spend), ["overview", "daily"]);
    assert_eq!(in_group(Group::Breakdown), ["projects", "models", "sessions"]);
    assert_eq!(GROUPS.map(Group::label), ["Spend", "Breakdown"]);
    assert_eq!(SECTIONS.map(Section::label), ["Overview", "Daily", "Projects", "Models", "Sessions"]);
}

// ─── use-analytics-filters.test.ts ──────────────────────────────────────────

fn at(y: i64, mo: i64, d: i64) -> i64 {
    days_from_civil(y, mo - 1, d) * 86_400_000 + 12 * 3_600_000
}

/// Fleet's `vi.setSystemTime(new Date("2026-10-09T12:00:00Z"))`.
fn now() -> i64 {
    at(2026, 10, 9)
}

fn range(from: &str, to: &str, project: &str) -> Filters {
    Filters { from: from.into(), to: to.into(), project_id: project.into() }
}

/// Opens as the page does: what the settings file kept for the live machine, moved to end today.
fn open(saved: Option<Filters>, now: i64) -> Filters {
    let mut settings = Sidebar::default();
    if let Some(f) = &saved {
        settings.save_analytics_filters(HOME_MACHINE_KEY, f);
    }
    Filters::opened(settings.analytics_filters(HOME_MACHINE_KEY), now)
}

#[test]
fn opens_on_the_last_30_days_ending_today() {
    let f = open(None, now());
    assert_eq!(f, range("2026-09-09", "2026-10-09", ""));
    assert!(f.is_default(now()));
}

#[test]
fn opens_ending_today_when_fleet_has_been_open_since_an_earlier_day() {
    assert_eq!(open(None, at(2026, 10, 12)), range("2026-09-12", "2026-10-12", ""));
}

#[test]
fn moves_a_range_saved_on_an_earlier_day_to_end_today_keeping_its_length_and_project() {
    let f = open(Some(range("2026-09-03", "2026-10-06", "harbor-api")), now());
    assert_eq!(f, range("2026-09-06", "2026-10-09", "harbor-api"));
}

#[test]
fn keeps_the_default_range_the_default_on_a_later_day() {
    let f = open(Some(range("2026-09-05", "2026-10-05", "")), now());
    assert_eq!(f, range("2026-09-09", "2026-10-09", ""));
    assert!(f.is_default(now()));
}

#[test]
fn leaves_a_range_alone_while_analytics_stays_open() {
    // Only opening moves a range; a `to` picked afterwards is not moved by asking again on the same day.
    let mut f = open(None, now());
    f.to = "2026-09-30".into();
    assert_eq!(f.to, "2026-09-30");
    assert!(!f.is_default(now()));
    // The same pick, kept and opened again the same day, is moved (that is opening, not staying open).
    assert_eq!(f.ending_today(now()).to, "2026-10-09");
}

#[test]
fn leaves_a_range_with_no_end_alone() {
    assert_eq!(open(Some(range("2026-09-03", "", "")), now()), range("2026-09-03", "", ""));
}

// Beyond Fleet's cases.

#[test]
fn a_range_that_crosses_a_month_and_a_leap_day_keeps_its_length() {
    // 2028 is a leap year: 28 Feb to 1 Mar is two days.
    let f = range("2028-02-28", "2028-03-01", "p").ending_today(at(2028, 3, 5));
    assert_eq!(f, range("2028-03-03", "2028-03-05", "p"));
}

#[test]
fn a_range_with_a_bad_date_is_left_alone() {
    let bad_end = range("2026-09-03", "soon", "x");
    assert_eq!(bad_end.ending_today(now()), bad_end);
    let bad_start = range("whenever", "2026-10-06", "x");
    assert_eq!(bad_start.ending_today(now()), bad_start);
}

#[test]
fn a_range_with_no_start_still_moves_its_end() {
    assert_eq!(range("", "2026-10-06", "").ending_today(now()), range("", "2026-10-09", ""));
}

#[test]
fn today_is_the_utc_day_even_just_before_midnight() {
    let late = days_from_civil(2026, 9, 9) * 86_400_000 + 86_399_999;
    assert_eq!(Filters::defaults(late).to, "2026-10-09");
    assert_eq!(Filters::defaults(late + 1).to, "2026-10-10");
}

#[test]
fn is_default_needs_all_three() {
    let d = Filters::defaults(now());
    assert!(d.is_default(now()));
    assert!(!Filters { project_id: "harbor-api".into(), ..d.clone() }.is_default(now()));
    assert!(!Filters { from: "2026-09-10".into(), ..d.clone() }.is_default(now()));
    assert!(!Filters { to: String::new(), ..d }.is_default(now()));
}

#[test]
fn day_number_reads_bare_dates_only() {
    assert_eq!(day_number("1970-01-02"), Some(1));
    assert_eq!(day_number("2026-10-09"), Some(days_from_civil(2026, 9, 9)));
    for bad in ["", "soon", "2026-13-01", "2026-1-1", "2026-10-09T00:00:00Z"] {
        assert_eq!(day_number(bad), None, "{bad:?}");
    }
}

#[test]
fn filters_are_kept_per_machine_key_and_read_back() {
    let mut s = Sidebar::default();
    s.save_analytics_filters(HOME_MACHINE_KEY, &range("2026-09-09", "2026-10-09", ""));
    s.save_analytics_filters("m-aurora", &range("2026-08-01", "2026-08-31", "harbor-api"));
    let back = Sidebar::parse(&s.to_json());
    assert_eq!(back.analytics_filters(HOME_MACHINE_KEY), Some(&range("2026-09-09", "2026-10-09", "")));
    assert_eq!(back.analytics_filters("m-aurora"), Some(&range("2026-08-01", "2026-08-31", "harbor-api")));
    assert_eq!(back.analytics_filters("m-other"), None);
    // Saving another machine's range leaves the live one untouched.
    s.save_analytics_filters("m-aurora", &range("2026-08-02", "2026-09-01", ""));
    assert_eq!(s.analytics_filters(HOME_MACHINE_KEY), Some(&range("2026-09-09", "2026-10-09", "")));
}

#[test]
fn a_filter_entry_of_the_wrong_kind_is_dropped() {
    let s = Sidebar::parse(r#"{"analyticsFilters":{"home":"x","m1":{"from":3,"to":"2026-10-01"}}}"#);
    assert_eq!(s.analytics_filters("home"), None);
    assert_eq!(s.analytics_filters("m1"), Some(&range("", "2026-10-01", "")));
}

// ─── the shapes ─────────────────────────────────────────────────────────────

fn top(name: &str, tokens: f64, cost: f64) -> TopItem {
    TopItem { name: name.into(), tokens, cost }
}

#[test]
fn reads_the_summary() {
    let body = json!({
        "totalTokens": 1_250_000, "totalCost": 18.5, "totalEstimatedCost": 19.25, "sessionCount": 18, "messageCount": 412,
        "topModels": [{ "name": "orchid-large", "tokens": 700_000, "cost": 11.0, "extra": true }],
        "topProjects": [{ "name": "harbor-api", "tokens": 500_000, "cost": 7.5 }, { "name": "lantern-web", "tokens": 90, "cost": 0.01 }],
        "somethingNew": [1, 2],
    });
    let s = parse_summary(&body).unwrap();
    assert_eq!((s.total_tokens, s.total_cost, s.total_estimated_cost), (1_250_000.0, 18.5, 19.25));
    assert_eq!((s.session_count, s.message_count), (18.0, 412.0));
    assert_eq!(s.top_models, [top("orchid-large", 700_000.0, 11.0)]);
    assert_eq!(s.top_projects, [top("harbor-api", 500_000.0, 7.5), top("lantern-web", 90.0, 0.01)]);
}

#[test]
fn a_summary_with_nulls_and_gaps_reads_as_zeros() {
    let s = parse_summary(&json!({ "totalTokens": null, "topModels": null, "topProjects": [null, { "name": null }] })).unwrap();
    assert_eq!(s, Summary { top_projects: vec![top("", 0.0, 0.0)], ..Default::default() });
    assert_eq!(parse_summary(&json!({})).unwrap(), Summary::default());
    assert_eq!(parse_summary(&json!([])), None);
    assert_eq!(parse_summary(&json!(null)), None);
}

#[test]
fn reads_the_daily_rows() {
    let body = json!([
        { "date": "2026-11-19", "tokens": 4200, "cost": 1.5, "estimatedCost": 1.75, "sessions": 3, "messages": 40, "x": 1 },
        { "date": "2026-11-20", "tokens": null, "cost": 0.004 },
        { "tokens": 5 },
        "stray",
    ]);
    let days = parse_days(&body).unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(
        days[0],
        Daily { date: "2026-11-19".into(), tokens: 4200.0, cost: 1.5, estimated_cost: 1.75, sessions: 3.0, messages: 40.0 }
    );
    assert_eq!(days[1], Daily { date: "2026-11-20".into(), cost: 0.004, ..Default::default() });
    assert_eq!(parse_days(&json!({ "date": "x" })), None);
    assert_eq!(parse_days(&json!([])), Some(vec![]));
}

#[test]
fn the_home_sparkline_reads_the_same_rows() {
    let points = crate::fleet::summary::parse_daily(&json!([{ "date": "2026-11-19", "cost": 2.5, "tokens": 9 }, { "cost": 1 }])).unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!((points[0].date.as_str(), points[0].cost), ("2026-11-19", 2.5));
}

#[test]
fn reads_the_model_rows() {
    let body = json!([{
        "modelId": "orchid-large", "providerId": "lumen", "tokens": 700_000, "cost": 11.0, "estimatedCost": 11.5,
        "messageCount": 220, "avgCostPerMessage": 0.05, "added": "later",
    }, { "modelId": "reed-small" }]);
    let models = parse_models(&body).unwrap();
    assert_eq!(
        models[0],
        ModelRow {
            model_id: "orchid-large".into(),
            provider_id: "lumen".into(),
            tokens: 700_000.0,
            cost: 11.0,
            estimated_cost: 11.5,
            message_count: 220.0,
            avg_cost_per_message: 0.05
        }
    );
    assert_eq!(models[1], ModelRow { model_id: "reed-small".into(), ..Default::default() });
    assert_eq!(parse_models(&json!("no")), None);
}

#[test]
fn reads_the_session_rows() {
    let body = json!([{
        "sessionId": "s-1", "title": "Add paging", "projectId": "p-1", "projectName": "harbor-api", "tokens": 52_000, "cost": 0.8,
        "estimatedCost": 0.9, "models": ["orchid-large", 7, "reed-small"], "durationSeconds": 840, "createdAt": "2026-11-18T09:30:00Z",
    }, { "sessionId": "s-2", "title": null, "projectId": null, "projectName": null, "models": null, "durationSeconds": null }]);
    let rows = parse_sessions(&body).unwrap();
    assert_eq!(rows[0].models, ["orchid-large", "reed-small"]);
    assert_eq!(
        (rows[0].title.as_deref(), rows[0].project_name.as_deref(), rows[0].duration_seconds),
        (Some("Add paging"), Some("harbor-api"), Some(840.0))
    );
    assert_eq!(rows[1], SessionRow { session_id: "s-2".into(), ..Default::default() });
}

// ─── the queries ────────────────────────────────────────────────────────────

#[test]
fn each_read_sends_what_fleet_sends() {
    let f = range("2026-10-01", "2026-10-31", " harbor api ");
    assert_eq!(summary_path(&f, false), "/api/analytics/summary?from=2026-10-01&to=2026-10-31&projectId=harbor+api");
    assert_eq!(summary_path(&f, true), "/api/analytics/summary?from=2026-10-01&to=2026-10-31");
    assert_eq!(daily_path(&f), "/api/analytics/daily?from=2026-10-01&to=2026-10-31&projectId=harbor+api");
    assert_eq!(models_path(&f), "/api/analytics/models?from=2026-10-01&projectId=harbor+api&to=2026-10-31");
    assert_eq!(
        sessions_path(&f, SortBy::EstimatedCost, SortDir::Asc),
        "/api/analytics/sessions?from=2026-10-01&to=2026-10-31&projectId=harbor+api&sortBy=estimatedCost&sortDir=asc"
    );
    assert_eq!(export_path(&f), "/api/analytics/export?format=csv&from=2026-10-01&to=2026-10-31&projectId=harbor+api");
}

#[test]
fn an_empty_end_or_a_blank_project_is_left_out() {
    let f = range("2026-10-01", "", "   ");
    assert_eq!(summary_path(&f, false), "/api/analytics/summary?from=2026-10-01");
    assert_eq!(models_path(&Filters::default()), "/api/analytics/models");
    assert_eq!(export_path(&Filters::default()), "/api/analytics/export?format=csv");
    assert_eq!(daily_path(&range("", "2026-10-31", "a&b=c/é")), "/api/analytics/daily?to=2026-10-31&projectId=a%26b%3Dc%2F%C3%A9");
}

#[test]
fn the_sort_keys_are_the_names_the_read_sends() {
    assert_eq!(COLUMNS.map(SortBy::key), ["title", "project", "tokens", "cost", "estimatedCost", "durationSeconds", "models", "createdAt"]);
    assert_eq!(
        sessions_path(&Filters::default(), SortBy::CreatedAt, SortDir::Desc),
        "/api/analytics/sessions?sortBy=createdAt&sortDir=desc"
    );
}

// ─── the words ──────────────────────────────────────────────────────────────

#[test]
fn empty_sections_say_what_fleet_says() {
    assert_eq!(daily_empty(false), "No daily analytics matched the selected date range.");
    assert_eq!(daily_empty(true), "No daily analytics matched the selected date range and project.");
    assert_eq!(models_empty(false), "No model analytics matched the selected date range.");
    assert_eq!(models_empty(true), "No model analytics matched the selected date range and project.");
    assert_eq!(projects_empty(false), "No project analytics matched the selected date range.");
    assert_eq!(projects_empty(true), "No projects matched the selected date range and project.");
    assert_eq!(SESSIONS_EMPTY, "No sessions matched the current analytics filters");
}

#[test]
fn the_picked_project_stays_in_the_menu_when_the_range_lacks_it() {
    let tops = [top("harbor-api", 500.0, 7.5), top("lantern-web", 90.0, 0.5)];
    assert_eq!(with_selected_project(&tops, ""), tops);
    assert_eq!(with_selected_project(&tops, "lantern-web"), tops);
    let with = with_selected_project(&tops, "quiet-docs");
    assert_eq!(with.len(), 3);
    assert_eq!(with[2], top("quiet-docs", 0.0, 0.0));
    assert!(with_selected_project(&[], "quiet-docs").iter().any(|p| p.name == "quiet-docs"));
}

// ─── the sessions comparisons ───────────────────────────────────────────────

fn session(id: &str) -> SessionRow {
    SessionRow { session_id: id.into(), created_at: "2026-11-18T09:30:00Z".into(), ..Default::default() }
}

fn order(rows: &[SessionRow], by: SortBy, dir: SortDir) -> Vec<String> {
    sorted(rows, by, dir).into_iter().map(|r| r.session_id).collect()
}

#[test]
fn a_blank_title_sorts_as_untitled_session() {
    let mut a = session("a");
    a.title = Some("   ".into());
    let mut b = session("b");
    b.title = Some("Add paging".into());
    let c = session("c");
    assert_eq!(title_label(&a), "Untitled session");
    assert_eq!(title_label(&c), "Untitled session");
    assert_eq!(title_label(&SessionRow { title: Some("  Add paging ".into()), ..session("d") }), "Add paging");
    // "Add paging" < "Untitled session"; the two untitled keep the order they came in.
    assert_eq!(order(&[a.clone(), b.clone(), c.clone()], SortBy::Title, SortDir::Asc), ["b", "a", "c"]);
    assert_eq!(order(&[a, b, c], SortBy::Title, SortDir::Desc), ["a", "c", "b"]);
}

#[test]
fn text_compares_without_regard_to_case_or_accent() {
    assert_eq!(locale_compare("Édith", "edith"), Ordering::Equal);
    assert_eq!(locale_compare("apple", "Banana"), Ordering::Less);
    assert_eq!(locale_compare("Banana", "apple"), Ordering::Greater);
    assert_eq!(locale_compare("app", "apple"), Ordering::Less);
    assert_eq!(locale_compare("a-1", "a1"), Ordering::Less);
    assert_eq!(locale_compare("10", "a"), Ordering::Less);
}

#[test]
fn a_project_is_its_name_else_its_id_else_unassigned() {
    let mut s = session("a");
    assert_eq!(project_label(&s), "Unassigned");
    s.project_id = Some("p-9".into());
    assert_eq!(project_label(&s), "p-9");
    s.project_name = Some(" ".into());
    assert_eq!(project_label(&s), "p-9");
    s.project_name = Some("harbor-api".into());
    assert_eq!(project_label(&s), "harbor-api");
    let mut b = session("b");
    b.project_name = Some("Atlas".into());
    assert_eq!(order(&[s, b], SortBy::Project, SortDir::Asc), ["b", "a"]);
}

#[test]
fn models_compare_by_their_joined_list_and_none_is_unknown_model() {
    let mut a = session("a");
    a.models = vec!["reed-small".into(), "orchid-large".into()];
    let mut b = session("b");
    b.models = vec!["orchid-large".into()];
    let c = session("c");
    assert_eq!(models_label(&a.models), "reed-small, orchid-large");
    assert_eq!(models_label(&c.models), "Unknown model");
    assert_eq!(order(&[c, a, b], SortBy::Models, SortDir::Asc), ["b", "a", "c"]);
}

#[test]
fn numbers_sort_both_ways() {
    let rows: Vec<SessionRow> = [("a", 5.0), ("b", 30.0), ("c", 1.0)]
        .iter()
        .map(|(id, n)| SessionRow { tokens: *n, cost: *n, estimated_cost: *n, ..session(id) })
        .collect();
    for by in [SortBy::Tokens, SortBy::Cost, SortBy::EstimatedCost] {
        assert_eq!(order(&rows, by, SortDir::Asc), ["c", "a", "b"]);
        assert_eq!(order(&rows, by, SortDir::Desc), ["b", "a", "c"]);
    }
}

#[test]
fn a_missing_duration_goes_last_whichever_way() {
    let with = |id: &str, d: Option<f64>| SessionRow { duration_seconds: d, ..session(id) };
    let rows = [with("none", None), with("long", Some(900.0)), with("short", Some(30.0)), with("also-none", None)];
    assert_eq!(order(&rows, SortBy::DurationSeconds, SortDir::Asc), ["short", "long", "none", "also-none"]);
    assert_eq!(order(&rows, SortBy::DurationSeconds, SortDir::Desc), ["long", "short", "none", "also-none"]);
}

#[test]
fn created_sorts_by_time_not_text() {
    let at = |id: &str, t: &str| SessionRow { created_at: t.into(), ..session(id) };
    // Same instant written two ways, and an offset that makes the later-looking text earlier.
    let rows = [at("late", "2026-11-18T12:00:00+05:00"), at("early", "2026-11-18T09:00:00Z"), at("mid", "2026-11-18T10:00:00Z")];
    assert_eq!(order(&rows, SortBy::CreatedAt, SortDir::Asc), ["late", "early", "mid"]);
    assert_eq!(order(&rows, SortBy::CreatedAt, SortDir::Desc), ["mid", "early", "late"]);
    // A date that doesn't parse ties with everything, as `NaN` does in a JavaScript sort.
    assert_eq!(compare_sessions(&at("x", "never"), &at("y", "2026-11-18T10:00:00Z"), SortBy::CreatedAt, SortDir::Asc), Ordering::Equal);
}

#[test]
fn pressing_a_column_flips_the_sorted_one_and_defaults_another() {
    assert_eq!(DEFAULT_SORT, (SortBy::CreatedAt, SortDir::Desc));
    assert_eq!(pressed(DEFAULT_SORT, SortBy::CreatedAt), (SortBy::CreatedAt, SortDir::Asc));
    assert_eq!(pressed((SortBy::CreatedAt, SortDir::Asc), SortBy::CreatedAt), (SortBy::CreatedAt, SortDir::Desc));
    assert_eq!(pressed(DEFAULT_SORT, SortBy::Title), (SortBy::Title, SortDir::Asc));
    assert_eq!(pressed(DEFAULT_SORT, SortBy::Cost), (SortBy::Cost, SortDir::Desc));
    let dirs: Vec<SortDir> = COLUMNS.iter().map(|c| c.default_dir()).collect();
    use SortDir::{Asc, Desc};
    assert_eq!(dirs, [Asc, Asc, Desc, Desc, Desc, Desc, Asc, Desc]);
    assert_eq!(COLUMNS.iter().filter(|c| c.align_right()).count(), 4);
}
