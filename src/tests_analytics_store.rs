//! Component 20, piece B: the Analytics store and its reads. Fleet's `use-analytics-machine.test.ts` under its name, then the
//! rules of the four reads (`use-analytics-{summary,daily,models,sessions}.ts`) and of `AnalyticsContent.vue`'s refreshes,
//! against a fake Fleet that numbers its answers (the Nth answer of a route says N, so a test can tell which one it holds).
use crate::fleet::analytics::filters::Filters;
use crate::fleet::analytics::sessions_sort::{SortBy, SortDir};
use crate::fleet::analytics::store::Kind;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::machines::Case;
use crate::tests::{FRAME, app};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const ROUTES: [&str; 4] = ["summary", "daily", "models", "sessions"];

/// 20 November 2026, noon UTC: the day the filters open on.
const NOW_MS: i64 = 20_777 * 86_400_000 + 43_200_000;

#[derive(Default)]
struct Heard {
    /// Every request path, query included, in the order heard.
    paths: Vec<String>,
    /// How many times each route was answered, for the numbered bodies.
    answers: HashMap<String, u32>,
}

/// The route whose next request waits, and the flag that lets it go.
type Held = Option<(String, Arc<AtomicBool>)>;

struct Fake {
    base: String,
    heard: Arc<Mutex<Heard>>,
    /// The routes that answer 503 "Analytics is disabled." while set.
    refuse: Arc<Mutex<Vec<String>>>,
    /// The next request to this route waits for the flag before it answers.
    hold: Arc<Mutex<Held>>,
}

fn body(route: &str, n: u32) -> Value {
    match route {
        "summary" => {
            json!({"totalTokens": n, "totalCost": 1.5, "sessionCount": 2, "topProjects": [{"name": "harbor-api", "tokens": 10, "cost": 2.5}]})
        }
        "daily" => json!([{"date": "2026-11-19", "tokens": 5, "cost": n}]),
        "models" => json!([{"modelId": "claude-sonnet-5-5", "providerId": "anthropic", "tokens": 5, "cost": n, "messageCount": 3}]),
        _ => json!([{"sessionId": "s1", "title": format!("answer {n}"), "createdAt": "2026-11-19T09:00:00Z"}]),
    }
}

fn fake() -> Fake {
    let listener = crate::test_http::bind().expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let heard = Arc::new(Mutex::new(Heard::default()));
    let refuse = Arc::new(Mutex::new(Vec::<String>::new()));
    let hold: Arc<Mutex<Held>> = Arc::default();
    let (log, refused, held) = (heard.clone(), refuse.clone(), hold.clone());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (log, refused, held) = (log.clone(), refused.clone(), held.clone());
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                let Some(route) = req.route().strip_prefix("/api/analytics/").map(str::to_string) else {
                    return crate::test_http::respond_json(&mut stream, "404 X", "{}");
                };
                let n = {
                    let mut heard = log.lock().unwrap();
                    heard.paths.push(req.path.clone());
                    let n = heard.answers.entry(route.clone()).or_default();
                    *n += 1;
                    *n
                };
                let wait = {
                    let mut slot = held.lock().unwrap();
                    if slot.as_ref().is_some_and(|(r, _)| *r == route) { slot.take().map(|(_, flag)| flag) } else { None }
                };
                if let Some(release) = wait {
                    while !release.load(Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
                if refused.lock().unwrap().contains(&route) {
                    return crate::test_http::respond_json(&mut stream, "503 X", &json!({"error": "Analytics is disabled."}).to_string());
                }
                crate::test_http::respond_json(&mut stream, "200 X", &body(&route, n).to_string());
            });
        }
    });
    Fake { base, heard, refuse, hold }
}

impl Fake {
    /// The requests heard for a route, as paths.
    fn on(&self, route: &str) -> Vec<String> {
        let prefix = format!("/api/analytics/{route}");
        self.heard.lock().unwrap().paths.iter().filter(|p| p.starts_with(&prefix)).cloned().collect()
    }

    /// How many requests to `route` have been heard, for a wait.
    fn counter(&self, route: &str) -> impl Fn() -> usize + use<> {
        let (heard, prefix) = (self.heard.clone(), format!("/api/analytics/{route}"));
        move || heard.lock().unwrap().paths.iter().filter(|p| p.starts_with(&prefix)).count()
    }

    fn total(&self) -> usize {
        self.heard.lock().unwrap().paths.len()
    }

    /// The next request to `route` waits; the flag lets it go.
    fn hold_next(&self, route: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        *self.hold.lock().unwrap() = Some((route.to_string(), flag.clone()));
        flag
    }
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
    harbor: Fake,
}

/// The app connected to a fake Fleet, with `m-harbor` answering at a second fake, on the Sessions rail, the clock pinned.
fn rig() -> Rig {
    let (home, harbor) = (fake(), fake());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    let base = harbor.base.clone();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_machines_only(Case::Several);
        s.machines.list.iter_mut().find(|m| m.id == "m-harbor").expect("harbor is listed").base_url = base;
        s.now_ms = Some(NOW_MS);
    });
    rt.frame(FRAME);
    Rig { rt, queue, home, harbor }
}

impl Rig {
    /// Applies what the reads hand back until `done` holds. Unlike `test_wait::drive` its frames don't move the clock: the
    /// polls are due 30 s after a read, and a slow runner looking a few thousand times would otherwise make one due in the
    /// middle of a test. The clock moves only where a test says so (`later`).
    fn wait(&mut self, what: &str, mut done: impl FnMut(&State) -> bool) {
        let queue = &self.queue;
        test_wait::until(
            &mut self.rt,
            what,
            |rt| {
                while let Ok(job) = queue.try_recv() {
                    rt.update(job);
                }
                rt.frame(0.0);
                done(&rt.state)
            },
            |rt| {
                let a = &rt.state.analytics;
                format!(
                    "machine {:?}, asked {}, loading {:?}, errors {:?}",
                    a.machine(),
                    a.asked,
                    [a.summary.is_loading, a.daily.is_loading, a.models.is_loading, a.sessions.is_loading],
                    [&a.summary.error, &a.daily.error, &a.models.error, &a.sessions.error]
                )
            },
        );
    }

    /// Shows Analytics and waits for the first answers of all five reads.
    fn open(&mut self) {
        self.rt.update(|s, _| s.rail = Rail::Analytics);
        self.rt.frame(FRAME);
        self.wait("the first answers", |s| {
            let a = &s.analytics;
            !(a.summary.is_loading || a.range.is_loading || a.daily.is_loading || a.models.is_loading || a.sessions.is_loading)
        });
    }

    fn asked(&self) -> u32 {
        self.rt.state.analytics.asked
    }

    /// Moves the clock on, so a poll falls due.
    fn later(&mut self, secs: f64) {
        self.rt.frame(secs);
    }
}

// ─── composables/__tests__/use-analytics-machine.test.ts ────────────────────────────────────────────────────────────────

#[test]
fn asks_only_that_machine_and_keeps_its_own_filters() {
    let mut r = rig();
    r.rt.update(|s, _| s.analytics.choice.pick("m-harbor"));
    r.open();

    assert!(!r.harbor.on("summary").is_empty(), "the picked machine is asked");
    assert_eq!(r.home.total(), 0, "the machine you're on hears nothing");
    let mut moved = r.rt.state.analytics.filters.clone();
    moved.project_id = "harbor-only".into();
    r.rt.update(move |s, _| s.analytics_set_filters(moved));
    assert_eq!(r.rt.state.sidebar.analytics_filters("m-harbor").map(|f| f.project_id.as_str()), Some("harbor-only"));
    assert_eq!(r.rt.state.sidebar.analytics_filters(HOME_MACHINE_KEY), None, "the live machine's own filters are untouched");
}

// ─── the reads ──────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn opens_on_the_last_30_days_and_asks_the_five_reads_with_them() {
    let mut r = rig();
    r.open();
    assert_eq!(r.rt.state.analytics.filters, Filters { from: "2026-10-21".into(), to: "2026-11-20".into(), project_id: String::new() });
    assert_eq!(r.home.on("summary").len(), 2, "the page's and the menu's");
    assert_eq!(r.home.on("daily"), ["/api/analytics/daily?from=2026-10-21&to=2026-11-20"]);
    assert_eq!(r.home.on("models"), ["/api/analytics/models?from=2026-10-21&to=2026-11-20"]);
    assert_eq!(r.home.on("sessions"), ["/api/analytics/sessions?from=2026-10-21&to=2026-11-20&sortBy=createdAt&sortDir=desc"]);
    assert_eq!(r.asked(), 5);
    let a = &r.rt.state.analytics;
    assert_eq!(a.daily.data[0].date, "2026-11-19");
    assert_eq!(a.models.data[0].model_id, "claude-sonnet-5-5");
    assert_eq!(a.sessions.data.len(), 1);
    assert!(a.summary.data.as_ref().is_some() && a.range.data.is_some());
}

#[test]
fn is_loading_until_the_first_answer_and_not_again_for_a_poll() {
    let mut r = rig();
    let release = r.home.hold_next("daily");
    let heard = r.home.counter("daily");
    r.rt.update(|s, _| s.rail = Rail::Analytics);
    r.rt.frame(FRAME);
    r.wait("the daily request to be heard", move |_| heard() == 1);
    r.wait("the other reads", |s| !s.analytics.models.is_loading && !s.analytics.sessions.is_loading);
    assert!(r.rt.state.analytics.daily.is_loading, "the held read has not answered");
    assert!(r.rt.state.analytics.refreshing_all(), "and the first refresh is still running");
    release.store(true, Ordering::SeqCst);
    r.wait("the daily answer", |s| !s.analytics.daily.is_loading);
    r.wait("the refresh to end", |s| !s.analytics.refreshing_all());

    let hold = r.home.hold_next("daily");
    let heard = r.home.counter("daily");
    r.later(31.0);
    r.wait("the poll", move |_| heard() == 2);
    assert!(!r.rt.state.analytics.daily.is_loading, "a poll keeps the data and doesn't bring the spinner back");
    hold.store(true, Ordering::SeqCst);
}

#[test]
fn polls_every_30_seconds_while_shown_and_not_before() {
    let mut r = rig();
    r.open();
    assert_eq!(r.asked(), 5);
    r.later(25.0);
    assert_eq!(r.asked(), 5, "not yet");
    r.later(6.0);
    assert_eq!(r.asked(), 10, "all five again");
    let heard = r.home.counter("models");
    r.wait("the second models request", move |_| heard() == 2);
    r.wait("its answer", |s| s.analytics.models.data[0].cost == 2.0);
}

#[test]
fn stops_polling_when_the_rail_is_left_and_starts_over_when_it_is_back() {
    let mut r = rig();
    r.open();
    r.rt.update(|s, _| s.rail = Rail::Sessions);
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.analytics.machine(), None, "unmounted");
    r.later(61.0);
    assert_eq!(r.asked(), 5, "nothing is read behind another rail");

    r.rt.update(|s, _| s.rail = Rail::Analytics);
    r.rt.frame(FRAME);
    assert!(r.rt.state.analytics.summary.is_loading, "the page is new again");
    assert_eq!(r.asked(), 10);
}

#[test]
fn asks_nothing_while_the_window_is_hidden_and_at_once_when_it_shows_again() {
    let mut r = rig();
    r.open();
    r.rt.update(|s, _| s.quiet.visible = false);
    r.later(61.0);
    assert_eq!(r.asked(), 5);
    r.rt.update(|s, _| s.quiet.visible = true);
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), 10, "all five on the first look after");
}

#[test]
fn drops_an_answer_a_newer_request_overtook() {
    let mut r = rig();
    r.open();
    // A filter change's daily read is held; a poll's read of the same thing overtakes it.
    let release = r.home.hold_next("daily");
    let heard = r.home.counter("daily");
    let mut next = r.rt.state.analytics.filters.clone();
    next.project_id = "harbor-api".into();
    r.rt.update(move |s, _| s.analytics_set_filters(next));
    r.rt.frame(FRAME);
    r.wait("the held read to be heard", {
        let heard = r.home.counter("daily");
        move |_| heard() == 2
    });
    drop(heard);
    r.later(31.0);
    r.wait("the newer answer", |s| s.analytics.daily.data[0].cost == 3.0);
    assert!(r.rt.state.analytics.refreshing_all(), "the held read still keeps the refresh open");

    // Its answer is the second the server numbered; it settles the refresh but must not replace the newer numbers.
    release.store(true, Ordering::SeqCst);
    r.wait("the old answer to land", |s| !s.analytics.refreshing_all());
    assert_eq!(r.rt.state.analytics.daily.data[0].cost, 3.0);
}

#[test]
fn a_failure_keeps_the_data_and_says_the_clients_words_and_a_success_clears_them() {
    let mut r = rig();
    r.open();
    let kept = r.rt.state.analytics.daily.data.clone();
    for route in ROUTES {
        r.home.refuse.lock().unwrap().push(route.to_string());
    }
    r.later(31.0);
    r.wait("every read to fail", |s| {
        let a = &s.analytics;
        a.summary.error.is_some()
            && a.daily.error.is_some()
            && a.models.error.is_some()
            && a.sessions.error.is_some()
            && a.range.error.is_some()
    });
    let a = &r.rt.state.analytics;
    assert_eq!(a.summary.error.as_deref(), Some("Failed to fetch analytics summary"));
    assert_eq!(a.daily.error.as_deref(), Some("Failed to fetch daily analytics"));
    assert_eq!(a.models.error.as_deref(), Some("Failed to fetch model analytics"));
    assert_eq!(a.sessions.error.as_deref(), Some("Failed to fetch session analytics"));
    assert_eq!(a.daily.data, kept, "the numbers stay");
    assert!(!a.daily.is_loading);

    r.home.refuse.lock().unwrap().clear();
    r.later(31.0);
    r.wait("every read to recover", |s| s.analytics.summary.error.is_none() && s.analytics.daily.error.is_none());
    assert_eq!(Kind::Range.failure(), Kind::Summary.failure());
}

#[test]
fn a_failure_before_any_answer_ends_the_loading_with_the_error() {
    let mut r = rig();
    r.home.refuse.lock().unwrap().push("daily".into());
    r.open();
    let a = &r.rt.state.analytics;
    assert!(!a.daily.is_loading && a.daily.error.is_some() && a.daily.data.is_empty());
    assert!(!a.daily_loading(), "and Daily shows the error, not the spinner");
}

#[test]
fn a_filter_change_asks_all_four_page_reads_at_once_and_refreshes_until_the_last_answers() {
    let mut r = rig();
    r.open();
    let release = r.home.hold_next("models");
    let heard = r.home.counter("models");
    let mut next = r.rt.state.analytics.filters.clone();
    next.project_id = " harbor-api ".into();
    r.rt.update(move |s, _| s.analytics_set_filters(next));
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), 9, "four page reads; the range didn't move, so not the project list");
    r.wait("the models request", move |_| heard() == 2);
    r.wait("the other three", |s| {
        !s.analytics.daily.is_loading
            && s.analytics.daily.data[0].cost == 2.0
            && s.analytics.sessions.data[0].title.as_deref() == Some("answer 2")
    });
    assert!(r.rt.state.analytics.refreshing_all(), "one is still out");
    assert!(r.rt.state.analytics.models_loading() && r.rt.state.analytics.sessions_loading());
    release.store(true, Ordering::SeqCst);
    r.wait("the refresh to end", |s| !s.analytics.refreshing_all());
    assert_eq!(r.home.on("daily")[1], "/api/analytics/daily?from=2026-10-21&to=2026-11-20&projectId=harbor-api");
    assert_eq!(r.home.on("models")[1], "/api/analytics/models?from=2026-10-21&projectId=harbor-api&to=2026-11-20");
}

#[test]
fn a_range_change_also_reads_the_project_list_and_reset_goes_back_to_the_defaults() {
    let mut r = rig();
    r.open();
    let mut next = r.rt.state.analytics.filters.clone();
    next.from = "2026-11-01".into();
    r.rt.update(move |s, _| s.analytics_set_filters(next));
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), 10);
    let heard = r.home.counter("summary");
    r.wait("the four summary requests", move |_| heard() == 4);
    let range_only = r.home.on("summary").into_iter().filter(|p| !p.contains("projectId")).count();
    assert_eq!(range_only, 4, "the page's and the menu's, before and after (neither carries a project here)");
    r.wait("the refresh to end", |s| !s.analytics.refreshing_all());

    r.rt.update(|s, _| s.analytics_reset_filters());
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.analytics.filters, Filters::defaults(NOW_MS));
    assert_eq!(r.asked(), 15);
    assert!(r.rt.state.analytics.filters.is_default(NOW_MS));
    r.rt.update(|s, _| s.analytics_reset_filters());
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), 15, "nothing changed, nothing asked");
}

#[test]
fn a_sort_change_asks_only_the_sessions_and_says_so_until_they_answer() {
    let mut r = rig();
    r.open();
    let release = r.home.hold_next("sessions");
    let heard = r.home.counter("sessions");
    r.rt.update(|s, _| s.analytics_set_sort(SortBy::Cost, SortDir::Desc));
    r.rt.frame(FRAME);
    assert_eq!(r.asked(), 6, "the sessions alone");
    r.wait("the sessions request", move |_| heard() == 2);
    assert!(r.rt.state.analytics.refreshing_sessions());
    assert!(!r.rt.state.analytics.refreshing_all());
    assert!(r.rt.state.analytics.sessions_loading());
    assert!(!r.rt.state.analytics.daily_loading(), "the other sections don't wait");
    release.store(true, Ordering::SeqCst);
    r.wait("the sessions", |s| !s.analytics.refreshing_sessions());
    assert_eq!(r.home.on("sessions")[1], "/api/analytics/sessions?from=2026-10-21&to=2026-11-20&sortBy=cost&sortDir=desc");
    assert_eq!(r.home.on("daily").len(), 1);
}

#[test]
fn each_sections_loading_follows_its_reads() {
    let mut r = rig();
    r.rt.update(|s, _| s.rail = Rail::Analytics);
    r.rt.frame(FRAME);
    let a = &r.rt.state.analytics;
    assert!(a.daily_loading() && a.projects_loading() && a.models_loading() && a.models_bars_loading() && a.sessions_loading());
    r.wait("the first answers", |s| !s.analytics.refreshing_all());
    let a = &r.rt.state.analytics;
    assert!(!(a.daily_loading() || a.projects_loading() || a.models_loading() || a.models_bars_loading() || a.sessions_loading()));
}

#[test]
fn another_machine_starts_over_with_its_own_filters_and_loading_states() {
    let mut r = rig();
    r.rt.update(|s, _| {
        let mut saved = Filters::defaults(NOW_MS);
        saved.project_id = "harbor-api".into();
        s.sidebar.save_analytics_filters("m-harbor", &saved);
    });
    r.open();
    assert_eq!(r.rt.state.analytics.filters.project_id, "");
    r.rt.update(|s, _| s.analytics.choice.pick("m-harbor"));
    r.rt.frame(FRAME);
    assert_eq!(r.rt.state.analytics.machine(), Some("m-harbor"));
    assert_eq!(r.rt.state.analytics.filters.project_id, "harbor-api");
    assert!(r.rt.state.analytics.summary.is_loading, "fresh reads");
    // Each read is its own request on its own thread, so the summary and the sessions can answer before the daily one is even
    // heard: wait for every read, the daily one the assertion is about among them.
    r.wait("harbor's answers", |s| {
        let a = &s.analytics;
        !(a.summary.is_loading || a.range.is_loading || a.daily.is_loading || a.models.is_loading || a.sessions.is_loading)
    });
    assert!(r.harbor.on("daily")[0].contains("projectId=harbor-api"));
}

#[test]
fn with_no_server_the_invented_answers_stand_in() {
    let mut rt = app();
    rt.update(|s, _| {
        s.now_ms = Some(NOW_MS);
        s.rail = Rail::Analytics;
    });
    rt.frame(FRAME);
    let a = &rt.state.analytics;
    assert!(!a.summary.is_loading && !a.refreshing_all());
    assert_eq!((a.models.data.len(), a.sessions.data.len(), a.summary.data.as_ref().map(|s| s.top_projects.len())), (5, 18, Some(5)));
    assert!(a.daily.data.len() >= 10 && a.daily.data.last().is_some_and(|d| d.date == "2026-11-20"));
}
