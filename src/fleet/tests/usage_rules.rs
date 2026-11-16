//! Usage limits, piece I of the app frame: the cases of weave-fleet's `usage-providers.test.ts`, `usage-limits.test.ts`
//! and `use-usage-limits.test.ts` that piece F (`usage.rs`) hadn't taken, under Fleet's names. The composable's cases
//! run against a tiny fake server on a loopback port, with the poll's clock handed in so a minute costs nothing.
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::usage::*;
use crate::fleet::usage_rules::*;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const H: i64 = 3_600_000;
const NOW: i64 = 1_000_000_000_000;

fn read(body: Value) -> Option<UsageLimits> {
    read_usage_limits(&body)
}

fn only(body: Value) -> ProviderUsage {
    read(body).unwrap().providers.remove(0)
}

fn provider(name: &str, state: &str, windows: Vec<Value>) -> ProviderUsage {
    only(json!({"enabled": true, "providers": [{"provider": name, "name": name, "state": state, "windows": windows}]}))
}

fn iso(ms: i64) -> String {
    let (days, rest) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    // Civil date from days since the epoch (Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / H, rest / 60_000 % 60, rest / 1000 % 60)
}

fn at(hours: i64) -> String {
    iso(NOW + hours * H)
}

// ─── usage-providers.test.ts: toUsageLimits ─────────────────────────────────

#[test]
fn reads_the_contracts_body() {
    let limits = read(json!({"enabled": true, "providers": [{
        "provider": "openrouter", "account": null, "name": "OpenRouter", "state": "ok", "source": "OpenCode",
        "harnesses": ["opencode"], "readAt": at(0), "windows": [], "credits": {"remaining": 7.6, "limit": 10, "used": 2.4},
    }]}))
    .unwrap();
    let p = &limits.providers[0];
    assert_eq!((p.provider.as_str(), p.source.as_deref()), ("openrouter", Some("OpenCode")));
    assert_eq!(p.credits, Some(Credits { remaining: Some(7.6), limit: Some(10.0), used: Some(2.4), unit: None }));
    assert_eq!(p.harnesses, ["opencode"]);
    assert_eq!(p.read_at, Some(NOW));
}

#[test]
fn reads_the_credits_unit() {
    let p = only(json!({"enabled": true, "providers": [{"provider": "copilot", "state": "no_window",
        "credits": {"remaining": null, "limit": null, "used": 5, "unit": "ai_credits"}}]}));
    assert_eq!(p.credits, Some(Credits { remaining: None, limit: None, used: Some(5.0), unit: Some("ai_credits".into()) }));
}

#[test]
fn refuses_what_isnt_a_body_and_skips_entries_and_windows_that_dont_read() {
    assert!(read(Value::Null).is_none());
    assert!(read(json!({"providers": []})).is_none());
    let limits = read(json!({"enabled": true, "providers": [
        null, {"name": "no id"},
        {"provider": "codex", "state": "from the future", "windows": [{"utilization": 1}, {"window": "primary", "utilization": 4}]},
    ]}))
    .unwrap();
    assert_eq!(limits.providers.len(), 1);
    assert_eq!(limits.providers[0].state, ProviderState::Unreadable);
    assert_eq!(limits.providers[0].windows, [Window { window: "primary".into(), utilization: Some(1.0), ..Window::default() }]);
}

// ─── usage-providers.test.ts: tightestWindow ────────────────────────────────

#[test]
fn picks_the_fullest_live_window_of_a_readable_provider() {
    let w = |name: &str, u: f64, h: i64| json!({"window": name, "utilization": u, "resetsAt": at(h)});
    let all = [
        provider("claude", "ok", vec![w("five_hour", 0.4, 1), w("seven_day", 0.9, 1)]),
        provider("codex", "not_allowed", vec![w("primary", 0.99, 1)]),
        provider("old", "ok", vec![w("primary", 1.0, -1)]),
    ];
    let refs: Vec<&ProviderUsage> = all.iter().collect();
    let found = tightest_window(&refs, NOW).unwrap();
    assert_eq!((found.provider.provider.as_str(), found.window.window.as_str()), ("claude", "seven_day"));
    assert!(tightest_window(&[], NOW).is_none());
}

#[test]
fn is_quiet_below_80_amber_from_there_and_red_when_used_up() {
    let w = |u: f64, status| Window { window: "x".into(), utilization: Some(u), status, ..Window::default() };
    assert_eq!(usage_tone(&w(0.79, Status::Allowed)), Tone::Quiet);
    assert_eq!(usage_tone(&w(0.8, Status::Allowed)), Tone::Warn);
    assert_eq!(usage_tone(&w(0.3, Status::Rejected)), Tone::Used);
    assert_eq!(window_percent(&w(0.3, Status::Rejected)), 100);
}

#[test]
fn names_codexs_windows() {
    assert_eq!(provider_window_label("primary"), "Primary limit");
    assert_eq!(provider_window_label("five_hour"), "5-hour limit");
    assert_eq!(provider_window_label("secondary"), "Secondary limit");
}

// ─── usage-providers.test.ts: pace ──────────────────────────────────────────

/// A 10 h window with `left` hours to go.
fn win(utilization: Option<f64>, left: f64) -> Window {
    Window {
        window: "five_hour".into(),
        utilization,
        resets_at: Some(NOW + (left * H as f64) as i64),
        window_seconds: Some(36_000.0),
        status: Status::Allowed,
    }
}

#[test]
fn reads_window_seconds_null_when_missing_or_not_positive() {
    let read_seconds = |v: Value| {
        let mut w = json!({"window": "w"});
        if !v.is_null() {
            w["windowSeconds"] = v;
        }
        only(json!({"enabled": true, "providers": [{"provider": "x", "windows": [w]}]})).windows[0].window_seconds
    };
    assert_eq!(read_seconds(json!(18000)), Some(18000.0));
    assert_eq!(read_seconds(Value::Null), None);
    assert_eq!(read_seconds(json!(0)), None);
    assert_eq!(read_seconds(json!(-5)), None);
}

#[test]
fn expected_share_is_the_share_of_the_window_gone_clamped_null_without_a_length() {
    assert!((expected_share(&win(Some(0.5), 5.0), NOW).unwrap() - 0.5).abs() < 1e-9);
    assert_eq!(expected_share(&win(Some(0.5), 20.0), NOW), Some(0.0));
    assert_eq!(expected_share(&win(Some(0.5), -1.0), NOW), Some(1.0));
    assert_eq!(expected_share(&Window { window_seconds: None, ..win(Some(0.5), 5.0) }, NOW), None);
    assert_eq!(tick_percent(&win(Some(0.5), 6.0), NOW), Some(40));
}

#[test]
fn notes_a_window_that_will_run_out_well_before_its_reset() {
    // 4 h gone (0.4), 80% used: 1 h to burn the last 20%, 6 h left => 5 h ahead.
    let note = pace_note(&win(Some(0.8), 6.0), NOW).unwrap();
    assert!((note.ahead_by_seconds - 5.0 * 3600.0).abs() < 1e-6);
    assert_eq!(note.text, "Ahead of pace: at this rate it runs out about 5 h before it resets");
}

#[test]
fn stays_quiet_when_not_over_pace_at_pace_empty_full_or_without_a_length() {
    assert!(pace_note(&win(Some(0.3), 6.0), NOW).is_none());
    assert!(pace_note(&win(Some(0.4), 6.0), NOW).is_none());
    assert!(pace_note(&win(Some(0.0), 6.0), NOW).is_none());
    assert!(pace_note(&win(Some(1.0), 6.0), NOW).is_none());
    assert!(pace_note(&Window { window_seconds: None, ..win(Some(0.8), 6.0) }, NOW).is_none());
    assert!(pace_note(&win(None, 6.0), NOW).is_none());
}

#[test]
fn stays_quiet_under_5_percent_elapsed_after_the_reset_and_when_it_runs_out_less_than_15_min_early() {
    assert!(pace_note(&win(Some(0.5), 9.7), NOW).is_none());
    assert!(pace_note(&win(Some(0.5), -1.0), NOW).is_none());
    // 5 h gone, 51% used: projected 4.8 h, 5 h left => 0.2 h early.
    assert!(pace_note(&win(Some(0.51), 5.0), NOW).is_none());
}

// ─── usage-providers.test.ts: the rest ──────────────────────────────────────

fn usage(over: impl FnOnce(&mut ProviderUsage)) -> ProviderUsage {
    let mut p = ProviderUsage { provider: "claude".into(), name: "Claude".into(), ..ProviderUsage::default() };
    over(&mut p);
    p
}

fn five_hour(u: f64, status: Status) -> Window {
    Window { window: "five_hour".into(), utilization: Some(u), resets_at: Some(NOW + H), window_seconds: Some(18_000.0), status }
}

#[test]
fn account_key_reads_when_sent_and_is_null_when_missing_or_not_text() {
    let key = |v: Value| {
        let mut p = json!({"provider": "codex"});
        if !v.is_null() {
            p["accountKey"] = v;
        }
        only(json!({"enabled": true, "providers": [p]})).account_key
    };
    assert_eq!(key(json!("ab12")).as_deref(), Some("ab12"));
    assert_eq!(key(Value::Null), None);
    assert_eq!(key(json!(7)), None);
    assert_eq!(key(json!("")), None);
}

#[test]
fn maps_the_model_providers_that_have_limits() {
    for (model, wanted) in [
        ("anthropic", Some("claude")),
        ("claude-code", Some("claude")),
        ("openai", Some("codex")),
        ("openrouter", Some("openrouter")),
        ("github-copilot", Some("copilot")),
        ("copilot", Some("copilot")),
        ("google", None),
    ] {
        assert_eq!(usage_provider_for(model), wanted, "{model}");
    }
}

#[test]
fn gives_the_tightest_window_and_does_not_dim_below_95() {
    let weekly = Window { window: "seven_day".into(), ..five_hour(0.9, Status::Allowed) };
    let p = usage(|p| p.windows = vec![five_hour(0.4, Status::Allowed), weekly]);
    let r = model_provider_usage(&[p], "anthropic", NOW, 0).unwrap();
    assert_eq!((r.percent, r.tone, r.dimmed, r.reset_text), (90, Tone::Warn, false, None));
}

#[test]
fn dims_from_95_and_when_rejected_with_the_reset_time() {
    let at95 = model_provider_usage(&[usage(|p| p.windows = vec![five_hour(0.95, Status::Allowed)])], "anthropic", NOW, 0).unwrap();
    assert!(at95.dimmed);
    assert!(at95.reset_text.unwrap().starts_with("resets "));
    let used = model_provider_usage(&[usage(|p| p.windows = vec![five_hour(0.2, Status::Rejected)])], "claude-code", NOW, 0).unwrap();
    assert_eq!((used.percent, used.tone, used.dimmed), (100, Tone::Used, true));
}

#[test]
fn is_null_for_an_unmapped_provider_another_providers_data_or_an_unreadable_one() {
    let good = usage(|p| p.windows = vec![five_hour(0.5, Status::Allowed)]);
    assert!(model_provider_usage(std::slice::from_ref(&good), "google", NOW, 0).is_none());
    assert!(model_provider_usage(std::slice::from_ref(&good), "openai", NOW, 0).is_none());
    let blocked = usage(|p| p.state = ProviderState::NotAllowed);
    assert!(model_provider_usage(&[blocked], "anthropic", NOW, 0).is_none());
}

fn codex(key: Option<&str>) -> ProviderUsage {
    usage(|p| {
        p.provider = "codex".into();
        p.name = "Codex".into();
        p.account_key = key.map(str::to_string);
    })
}

fn machine(key: &str, name: &str, providers: Vec<ProviderUsage>) -> MachineProviders {
    MachineProviders { key: key.into(), name: name.into(), providers }
}

#[test]
fn shows_an_account_once_on_the_first_machine_with_where_else_it_is() {
    let groups = group_usage_by_machine(&[
        machine("home", "Desk", vec![codex(Some("k1")), usage(|_| {})]),
        machine("m2", "Laptop", vec![codex(Some("k1")), usage(|p| p.provider = "openrouter".into())]),
        machine("m3", "Server", vec![codex(Some("k1"))]),
    ]);
    let ids: Vec<Vec<&str>> = groups.iter().map(|g| g.rows.iter().map(|r| r.provider.provider.as_str()).collect()).collect();
    assert_eq!(ids, [vec!["codex", "claude"], vec!["openrouter"], vec![]]);
    assert_eq!(groups[0].rows[0].also_on, ["Laptop", "Server"]);
}

#[test]
fn keeps_providers_without_an_account_key_or_with_different_ones_on_every_machine() {
    let groups = group_usage_by_machine(&[
        machine("a", "A", vec![codex(None), codex(Some("x"))]),
        machine("b", "B", vec![codex(None), codex(Some("y"))]),
    ]);
    assert_eq!(groups.iter().map(|g| g.rows.len()).collect::<Vec<_>>(), [2, 2]);
}

// ─── usage-limits.test.ts ───────────────────────────────────────────────────

fn harness(windows: Vec<Value>) -> HarnessUsage {
    read_harness_usage(&json!({"harnessType": "claude-code", "windows": windows, "updatedAt": at(0)})).unwrap()
}

#[test]
fn reads_the_servers_shape_clamping_use_to_0_1() {
    let h =
        harness(vec![json!({"window": "five_hour", "utilization": 1.4, "resetsAt": at(2), "status": "rejected"}), json!({"nope": true})]);
    assert_eq!(h.harness_type, "claude-code");
    assert_eq!(
        h.windows,
        [Window {
            window: "five_hour".into(),
            utilization: Some(1.0),
            resets_at: Some(NOW + 2 * H),
            window_seconds: None,
            status: Status::Rejected
        }]
    );
    assert!(read_harness_usage(&json!({"windows": []})).is_none());
}

#[test]
fn orders_the_windows_shortest_first_and_leaves_out_the_ones_that_reset() {
    let h = harness(vec![
        json!({"window": "seven_day_opus", "utilization": 0.2, "resetsAt": at(50)}),
        json!({"window": "seven_day", "utilization": 0.6, "resetsAt": at(50)}),
        json!({"window": "five_hour", "utilization": 0.4, "resetsAt": at(-1)}),
    ]);
    let names: Vec<String> = current_windows(Some(&h), NOW).into_iter().map(|w| w.window).collect();
    assert_eq!(names, ["seven_day", "seven_day_opus"]);
}

#[test]
fn names_the_windows_and_when_they_reset() {
    assert_eq!(window_label("five_hour"), "5-hour limit");
    assert_eq!(window_label("seven_day"), "Weekly limit");
    assert_eq!(window_label("seven_day_opus"), "Weekly Opus limit");
    for (long, short) in [("seven_day_sonnet", "Sonnet week"), ("organization", "org"), ("five_hour", "5 h"), ("seven_day", "week")] {
        assert_eq!(window_short_label(long), short);
    }
    assert_eq!(window_short_label("some_new_window"), "some new window");
    assert_eq!(provider_window_short_label("primary"), "primary");
    assert_eq!(provider_window_short_label("secondary"), "secondary");
    // In the viewer's zone: the clock today, the weekday this week, else the date.
    let off = local_offset_secs();
    let now = 20_000 * 86_400_000 + (10 * 3600 - off) * 1000;
    assert_eq!(reset_label(Some(now + 2 * H), now, off).as_deref(), Some("resets 12:00 PM"));
    let week = reset_label(Some(now + 48 * H), now, off).unwrap();
    assert!(week.starts_with("resets ") && week.ends_with("10:00 AM") && week.split(' ').count() == 4, "{week}");
    let far = reset_label(Some(now + 24 * 9 * H), now, off).unwrap();
    assert!(far.starts_with("resets ") && far.contains(','), "{far}");
    assert_eq!(reset_label(None, now, off), None);
}

#[test]
fn says_a_used_up_window_is_at_100_whatever_its_last_reading() {
    let w = Window { utilization: Some(0.97), status: Status::Rejected, ..Window::default() };
    assert_eq!(percent_label(&w).as_deref(), Some("100%"));
}

// ─── the credits lines (UsageLimitsPopover.vue) ─────────────────────────────

#[test]
fn words_credits_as_money_left_or_used_and_github_s_ai_credits_apart() {
    let with = |c: Option<Credits>| usage(|p| p.credits = c);
    let money = Credits { remaining: Some(7.6), limit: Some(10.0), used: Some(2.4), unit: None };
    assert_eq!(credits_text(&with(Some(money.clone()))).as_deref(), Some("$7.60 left of $10.00"));
    assert_eq!(credits_text(&with(Some(Credits { limit: None, ..money.clone() }))).as_deref(), Some("$7.60 left"));
    assert_eq!(credits_text(&with(Some(Credits { remaining: None, ..money }))).as_deref(), Some("$2.40 used"));
    assert_eq!(credits_text(&with(None)), None);
    let ai = Credits { remaining: None, limit: None, used: Some(307_846.0), unit: Some("ai_credits".into()) };
    assert_eq!(credits_text(&with(Some(ai.clone()))), None);
    assert_eq!(ai_credits_line(&with(Some(ai))).as_deref(), Some("AI credits used this month: 307,846"));
    assert_eq!(ai_credits_line(&with(Some(Credits { used: Some(2.4), ..Credits::default() }))), None);
}

// ─── use-usage-limits.test.ts ───────────────────────────────────────────────

fn body(name: &str) -> String {
    json!({"enabled": true, "providers": [{"provider": name, "name": name, "state": "ok", "windows": []}]}).to_string()
}

/// A server that remembers what it was asked ("GET /path?query") and answers the paths in `routes` (a path without its
/// query, or a prefix ending in `/`) with a status and a body, 404 for the rest.
struct Fake {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

type Routes = Vec<(&'static str, u16, String)>;

fn fake(routes: Routes) -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let (log, routes) = (asked.clone(), Arc::new(routes));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (log, routes) = (log.clone(), routes.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                let _ = reader.read_line(&mut first);
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = n.trim().parse().unwrap_or(0);
                    }
                }
                let _ = reader.read_exact(&mut vec![0; length]);
                let mut parts = first.split(' ');
                let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                log.lock().unwrap().push(format!("{method} {target}"));
                let path = target.split('?').next().unwrap_or("");
                // The most specific route wins.
                let found =
                    routes.iter().filter(|(r, ..)| path == *r || (r.ends_with('/') && path.starts_with(r))).max_by_key(|(r, ..)| r.len());
                let (status, text) = found.map_or((404, "{}".to_string()), |(_, s, b)| (*s, b.clone()));
                let _ = stream.try_clone().unwrap().write_all(
                    format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                        text.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    Fake { base, asked }
}

/// A Fleet answering the usage limits with `limits`, and an Allow or revoke with `allow_status`.
fn usage_fleet(limits: &str, allow_status: u16, mut more: Routes) -> Fake {
    more.push(("/api/usage/limits", 200, limits.to_string()));
    more.push(("/api/usage/limits/", allow_status, String::new()));
    fake(more)
}

impl Fake {
    /// What it was asked about usage: Fleet's link asks for other things too (the machines, the hub, plugins).
    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().iter().filter(|a| a.contains("/api/usage/")).cloned().collect()
    }

    fn count(&self, what: &str) -> usize {
        self.asked().iter().filter(|a| a.as_str() == what).count()
    }
}

struct Rig {
    rt: rune::Runtime<State>,
    jobs: Receiver<Job>,
}

impl Rig {
    fn new(home: &Fake) -> Rig {
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let mut rt = crate::tests::app();
        let link = Link::start(home.base.clone(), Post::new(move |job| drop(tx.lock().unwrap().send(job))));
        rt.update(move |s, _| s.link = Some(link));
        Rig { rt, jobs }
    }

    fn until(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        crate::test_wait::drive(&mut self.rt, &self.jobs, what, done, |s| format!("providers {:?}", names(s)));
    }

    /// The poll's turn at `now` seconds.
    fn poll(&mut self, now: f64) {
        self.rt.update(move |s, tx| {
            s.keep_usage_watch(now, tx);
        });
    }
}

fn names(s: &State) -> Vec<String> {
    s.pickers.usage.limits.providers.iter().map(|p| p.provider.clone()).collect()
}

/// Waits for `n` reads of exactly `what` to have reached the fake.
fn reads(rig: &mut Rig, fake: &Fake, what: &str, n: usize) {
    crate::test_wait::until_true(&format!("{n} × {what}"), || {
        while let Ok(job) = rig.jobs.try_recv() {
            rig.rt.update(job);
        }
        fake.count(what) >= n
    });
}

#[test]
fn loads_once_and_follows_the_push() {
    let home = usage_fleet(&body("claude"), 204, vec![]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| s.load_usage());
    rig.until("the first load", |s| names(s) == ["claude"]);
    assert_eq!(home.count("GET /api/usage/limits"), 1);
    rig.rt.update(|s, _| {
        s.on_usage_event(&crate::fleet::events::Event::Other(
            crate::fleet::events::Kind::UsageLimits,
            serde_json::from_str(&body("codex")).unwrap(),
        ));
    });
    assert_eq!(names(&rig.rt.state), ["codex"]);
}

#[test]
fn asks_with_watch_true_now_and_every_minute_while_watched_and_stops_after() {
    let home = usage_fleet(&body("claude"), 204, vec![]);
    let mut rig = Rig::new(&home);
    rig.poll(0.0);
    assert_eq!(home.asked(), Vec::<String>::new(), "nothing is asked while nothing watches");

    rig.rt.update(|s, _| s.pickers.usage.watch());
    rig.poll(10.0);
    reads(&mut rig, &home, "GET /api/usage/limits?watch=true", 1);
    // Not yet a minute on: nothing more.
    rig.poll(69.0);
    rig.poll(70.0);
    reads(&mut rig, &home, "GET /api/usage/limits?watch=true", 2);
    assert_eq!(home.count("GET /api/usage/limits?watch=true"), 2);

    rig.rt.update(|s, _| s.pickers.usage.unwatch());
    rig.poll(200.0);
    rig.poll(400.0);
    assert!(!rig.rt.state.pickers.usage.watching());
    // Give a stray request time to arrive: the wait below only ends once the app has applied what it heard.
    rig.until("the answers applied", |s| names(s) == ["claude"]);
    assert_eq!(home.count("GET /api/usage/limits?watch=true"), 2);
}

#[test]
fn several_watchers_share_one_timer_and_stopping_twice_does_no_harm() {
    let mut usage = Usage::default();
    assert!(!usage.watch_read_due(0.0));
    usage.watch();
    usage.watch();
    assert!(usage.watch_read_due(5.0), "the first look is due at once");
    assert!(!usage.watch_read_due(64.9));
    assert!(usage.watch_read_due(65.0));
    usage.unwatch();
    assert!(usage.watching(), "one popover is still open");
    usage.unwatch();
    usage.unwatch();
    assert!(!usage.watching());
    usage.watch();
    assert!(usage.watch_read_due(66.0), "a new open reads at once");
}

fn machine_json(base: &str) -> Value {
    json!({"id": "laptop", "name": "Laptop", "baseUrl": base, "os": "macos", "addedAt": "2026-09-26T00:00:00Z", "token": "laptop-token-0123456789",
        "addresses": [{"url": base, "kind": "lan", "learned": false}]})
}

#[test]
fn asks_the_other_machines_only_while_watched_keeps_the_live_ones_answer_apart_and_keeps_a_last_answer_on_failure() {
    let identity = json!({"id": "laptop", "name": "Laptop", "hostName": "laptop", "os": "macos", "version": "0.72.0", "apiVersion": 1,
        "authMode": "token", "remoteReachable": true, "requiresToken": true})
    .to_string();
    let laptop = usage_fleet(
        &body("codex"),
        204,
        vec![("/api/sessions", 200, "[]".into()), ("/api/projects", 200, "[]".into()), ("/api/machine", 200, identity)],
    );
    let listed = json!({"machines": [machine_json(&laptop.base)]}).to_string();
    let home = usage_fleet(&body("claude"), 204, vec![("/api/machines", 200, listed)]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| s.load_usage());
    // Home's machine list and the laptop's first read come through the link's own poll.
    rig.until("home's load and the laptop answering", |s| names(s) == ["claude"] && !s.usage_other_machines().is_empty());
    rig.poll(0.0);
    assert_eq!(laptop.asked(), Vec::<String>::new(), "not asked while closed");

    rig.rt.update(|s, _| s.pickers.usage.watch());
    rig.poll(1.0);
    rig.until("the laptop's answer", |s| s.pickers.usage.others.contains_key("laptop"));
    let kept: Vec<&str> = rig.rt.state.pickers.usage.others["laptop"].iter().map(|p| p.provider.as_str()).collect();
    assert_eq!(kept, ["codex"]);
    assert_eq!(laptop.count("GET /api/usage/limits?watch=true"), 1);
    assert_eq!(names(&rig.rt.state), ["claude"], "the live machine's list is its own");

    // The laptop stops answering: its last answer stays.
    rig.rt.update(|s, _| s.pickers.usage.other_loaded("laptop", &json!("not a body")));
    assert_eq!(rig.rt.state.pickers.usage.others["laptop"].len(), 1);
    // A machine whose last check failed isn't asked.
    rig.rt.update(|s, _| s.machines.others.get_mut("laptop").unwrap().error = Some("away".into()));
    assert!(rig.rt.state.usage_other_machines().is_empty());
    rig.rt.update(|s, _| s.pickers.usage.unwatch());
}

#[test]
fn allows_and_revokes_a_provider_then_reloads() {
    let home = usage_fleet(&body("openrouter"), 204, vec![]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| s.set_usage_allowed("openrouter", true));
    assert_eq!(rig.rt.state.pickers.usage.allowing.as_deref(), Some("openrouter"), "the button is disabled while it runs");
    rig.until("the reload after Allow", |s| names(s) == ["openrouter"] && s.pickers.usage.allowing.is_none());
    let asked = home.asked();
    assert_eq!(asked[0], "POST /api/usage/limits/openrouter/allow");
    assert_eq!(asked[1], "GET /api/usage/limits");
    rig.rt.update(|s, _| s.set_usage_allowed("openrouter", false));
    rig.until("the revoke", |s| s.pickers.usage.allowing.is_none());
    reads(&mut rig, &home, "DELETE /api/usage/limits/openrouter/allow", 1);
}

#[test]
fn throws_when_the_server_refuses() {
    let home = usage_fleet(&body("claude"), 403, vec![]);
    let mut rig = Rig::new(&home);
    rig.rt.update(|s, _| s.set_usage_allowed("codex", true));
    rig.until("the refusal", |s| s.pickers.usage.allowing.is_none());
    // A refusal reads nothing again and changes nothing.
    assert_eq!(home.asked(), ["POST /api/usage/limits/codex/allow"]);
    assert!(names(&rig.rt.state).is_empty());
}

#[test]
fn the_chip_shows_unless_the_preference_says_false() {
    let mut prefs = crate::fleet::prefs::Prefs::default();
    assert!(chip_enabled(&prefs));
    prefs.set(CHIP_PREFERENCE_KEY, "false");
    assert!(!chip_enabled(&prefs));
    prefs.set(CHIP_PREFERENCE_KEY, "true");
    assert!(chip_enabled(&prefs));
}

/// The body the picture of Fleet's own popover was taken with (`shots/parity/09i-usage-rules/`): this is what the rules
/// give for it, line for line what Fleet's popover shows.
#[test]
fn gives_what_fleets_popover_shows_for_the_same_body() {
    let hours = |h: i64| iso(NOW + h * H);
    let limits = read(json!({"enabled": true, "providers": [
        {"provider": "claude", "account": "Max", "accountKey": "k1", "name": "Claude", "state": "ok", "source": "Claude Code",
         "readAt": iso(NOW - 120_000), "windows": [
            {"window": "five_hour", "utilization": 0.8, "resetsAt": hours(1), "windowSeconds": 18000, "status": "warning"},
            {"window": "seven_day", "utilization": 0.3, "resetsAt": hours(72), "windowSeconds": 604800, "status": "allowed"}]},
        {"provider": "codex", "name": "Codex", "state": "ok", "windows": [
            {"window": "primary", "utilization": 0.5, "resetsAt": hours(4), "windowSeconds": 18000, "status": "allowed"}]},
        {"provider": "openrouter", "name": "OpenRouter", "state": "ok", "credits": {"remaining": 7.6, "limit": 10, "used": 2.4}},
    ]}))
    .unwrap();
    let [claude, codex, router] = &limits.providers[..] else { panic!("three providers") };
    let rows = |p: &ProviderUsage| {
        provider_windows(p, NOW)
            .iter()
            .map(|w| (provider_window_label(&w.window), window_percent(w), tick_percent(w, NOW), pace_note(w, NOW).map(|n| n.text)))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(claude), [("5-hour limit".to_string(), 80, Some(80), None), ("Weekly limit".to_string(), 30, Some(57), None)]);
    let note = "Ahead of pace: at this rate it runs out about 3 h before it resets";
    assert_eq!(rows(codex), [("Primary limit".to_string(), 50, Some(20), Some(note.to_string()))]);
    assert_eq!(credits_text(router).as_deref(), Some("$7.60 left of $10.00"));
    // The chip: the tightest window, "Claude 5 h".
    let all: Vec<&ProviderUsage> = limits.providers.iter().collect();
    let t = tightest_window(&all, NOW).unwrap();
    assert_eq!(format!("{} {}", t.provider.name, provider_window_short_label(&t.window.window)), "Claude 5 h");
}
