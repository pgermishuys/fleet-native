//! Usage limits, piece F of the composer: `ModelSelectorUsage.test.ts` (4) driven through the open menu, and the pure
//! rules of `usage-limits.ts` and `usage-providers.ts`. Fleet has no test files for `currentWindows`, `percentLabel`,
//! `isNearLimit`, `windowLabel` or the readers' clamping, so those cases are new here (named for what they say).
use super::reconnect::app;
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::fleet::events::{Event, Kind};
use crate::fleet::usage::*;
use rune::Runtime;
use serde_json::json;

const FRAME: f64 = 1.0 / 60.0;
/// 10:00 in the machine's zone on a fixed day, so "in two hours" never crosses midnight.
fn now() -> i64 {
    20_000 * 86_400_000 + (10 * 3600 - local_offset_secs()) * 1000
}
const H: i64 = 3_600_000;

fn win(name: &str, utilization: Option<f64>, resets_at: Option<i64>, status: Status) -> Window {
    Window { window: name.into(), utilization, resets_at, window_seconds: None, status }
}

fn claude(utilization: f64, status: Status) -> ProviderUsage {
    ProviderUsage {
        provider: "claude".into(),
        name: "Claude".into(),
        windows: vec![Window { window_seconds: Some(18000.0), ..win("five_hour", Some(utilization), Some(now() + 2 * H), status) }],
        ..ProviderUsage::default()
    }
}

/// The picker with more than eight models from several providers (so the headings are the providers'), and the
/// providers' usage as given.
fn open_menu(providers: Vec<ProviderUsage>) -> Runtime<State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.now_ms = Some(now());
        s.pickers.add_mock_models(15);
        s.pickers.usage.limits = UsageLimits { enabled: true, providers };
        s.open_selector("model");
    });
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    rt
}

fn keyed(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with(prefix)).collect()
}

fn opacity(rt: &Runtime<State>, key: &str) -> f32 {
    let e = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    (e.opacity * 100.0).round() / 100.0
}

/// The anthropic heading's section and its rows' indexes.
fn anthropic(rt: &Runtime<State>) -> (usize, Vec<usize>) {
    let shown = rt.state.selector_shown("model");
    let at = shown.iter().position(|s| s.provider && s.title.as_deref() == Some("Anthropic")).expect("an Anthropic heading");
    (at, shown[at].rows.iter().map(|r| r.index).collect())
}

// ModelSelector usage bars: "puts the tightest window's bar on a mapped provider's heading and none on others"
#[test]
fn puts_the_tightest_windows_bar_on_a_mapped_providers_heading_and_none_on_others() {
    let rt = open_menu(vec![claude(0.6, Status::Allowed)]);
    let (at, rows) = anthropic(&rt);
    assert_eq!(keyed(&rt, "model-provider-usage"), [format!("model-provider-usage{at}")], "one bar, on Anthropic's heading");
    let usage = rt.state.selector_shown("model")[at].usage.clone().unwrap();
    assert_eq!(usage_tooltip(&usage), "60% of the limit used");
    assert!(keyed(&rt, "model-provider-resets").is_empty());
    for r in rows {
        assert_eq!(opacity(&rt, &format!("model-opt{r}")), 1.0, "nothing dims at 60%");
    }
}

// ModelSelector usage bars: "dims a nearly used-up provider's models with its reset time, and they stay pickable"
#[test]
fn dims_a_nearly_used_up_providers_models_with_its_reset_time_and_they_stay_pickable() {
    let mut rt = open_menu(vec![claude(0.97, Status::Allowed)]);
    let (at, rows) = anthropic(&rt);
    let resets = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("model-provider-resets{at}"))).unwrap();
    assert_eq!(resets.text.as_deref(), Some("resets 12:00 PM"));
    for r in &rows {
        assert_eq!(opacity(&rt, &format!("model-opt{r}")), 0.55);
    }
    let (x, y) = rt.find(&format!("model-pop-item{}", rows[0])).unwrap();
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.choice().model, rt.state.lists().models[0].selection_key, "a dimmed row is picked like any other");
}

// ModelSelector usage bars: "dims a rejected provider"
#[test]
fn dims_a_rejected_provider() {
    let rt = open_menu(vec![claude(0.2, Status::Rejected)]);
    let (_, rows) = anthropic(&rt);
    assert!(!rows.is_empty());
    for r in rows {
        assert_eq!(opacity(&rt, &format!("model-opt{r}")), 0.55);
    }
}

// ModelSelector usage bars: "shows no bar without readable usage"
#[test]
fn shows_no_bar_without_readable_usage() {
    let rt = open_menu(vec![]);
    assert!(keyed(&rt, "model-provider-usage").is_empty());
}

#[test]
fn a_provider_that_cannot_be_read_gives_no_bar() {
    let signed_out = ProviderUsage { state: ProviderState::SignedOut, ..claude(0.97, Status::Allowed) };
    assert_eq!(model_provider_usage(&[signed_out], "anthropic", now(), 0), None);
}

#[test]
fn the_bar_is_the_tightest_window_and_a_used_up_one_reads_a_hundred() {
    let mut p = claude(0.3, Status::Allowed);
    p.windows.push(win("seven_day", Some(0.82), Some(now() + 90 * H), Status::Warning));
    let u = model_provider_usage(&[p.clone()], "claude-code", now(), 0).unwrap();
    assert_eq!((u.percent, u.tone, u.dimmed), (82, Tone::Warn, false));
    p.windows.push(win("seven_day_opus", Some(0.1), Some(now() + 90 * H), Status::Rejected));
    let u = model_provider_usage(&[p], "anthropic", now(), 0).unwrap();
    assert_eq!((u.percent, u.tone, u.dimmed), (100, Tone::Used, true));
    assert!(u.reset_text.unwrap().starts_with("resets "));
}

#[test]
fn a_window_that_reset_is_not_counted_for_the_bar() {
    let mut p = claude(0.99, Status::Allowed);
    p.windows[0].resets_at = Some(now() - H);
    assert_eq!(model_provider_usage(&[p], "anthropic", now(), 0), None);
}

#[test]
fn model_providers_map_to_the_usage_providers_that_have_windows() {
    let mapped: Vec<_> = ["Anthropic", "claude-code", "openai", "openrouter", "github-copilot", "copilot", "google"]
        .iter()
        .map(|p| usage_provider_for(p))
        .collect();
    assert_eq!(mapped, [Some("claude"), Some("claude"), Some("codex"), Some("openrouter"), Some("copilot"), Some("copilot"), None]);
}

// ─── the harness's windows (no Fleet test files: new cases) ─────────────────

fn harness(windows: Vec<Window>) -> HarnessUsage {
    HarnessUsage { harness_type: "claude-code".into(), windows }
}

#[test]
fn current_windows_drops_the_windows_that_already_reset() {
    let usage = harness(vec![
        win("five_hour", Some(0.5), Some(now() - 1), Status::Allowed),
        win("seven_day", Some(0.5), Some(now() + H), Status::Allowed),
        win("seven_day_opus", Some(0.5), None, Status::Allowed),
    ]);
    let names: Vec<_> = current_windows(Some(&usage), now()).into_iter().map(|w| w.window).collect();
    assert_eq!(names, ["seven_day", "seven_day_opus"], "a window with no reset time stays");
    assert!(current_windows(None, now()).is_empty());
}

#[test]
fn current_windows_orders_the_five_hour_window_first_then_the_weekly_ones_then_the_rest_by_name() {
    let at = Some(now() + H);
    let usage = harness(vec![
        win("zeta", None, at, Status::Allowed),
        win("seven_day_sonnet", None, at, Status::Allowed),
        win("alpha", None, at, Status::Allowed),
        win("seven_day_opus", None, at, Status::Allowed),
        win("seven_day", None, at, Status::Allowed),
        win("five_hour", None, at, Status::Allowed),
    ]);
    let names: Vec<_> = current_windows(Some(&usage), now()).into_iter().map(|w| w.window).collect();
    assert_eq!(names, ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet", "alpha", "zeta"]);
}

#[test]
fn percent_label_rounds_says_a_hundred_when_rejected_and_nothing_when_unknown() {
    assert_eq!(percent_label(&win("w", Some(0.824), None, Status::Allowed)).as_deref(), Some("82%"));
    assert_eq!(percent_label(&win("w", Some(0.825), None, Status::Allowed)).as_deref(), Some("83%"));
    assert_eq!(percent_label(&win("w", Some(0.2), None, Status::Rejected)).as_deref(), Some("100%"));
    assert_eq!(percent_label(&win("w", None, None, Status::Allowed)), None);
}

#[test]
fn is_near_limit_from_eighty_percent_or_when_rejected() {
    assert!(!is_near_limit(&win("w", Some(0.79), None, Status::Allowed)));
    assert!(is_near_limit(&win("w", Some(0.8), None, Status::Allowed)));
    assert!(is_near_limit(&win("w", None, None, Status::Rejected)));
    assert!(!is_near_limit(&win("w", None, None, Status::Warning)));
}

#[test]
fn window_labels_read_as_fleet_words_them() {
    let labels: Vec<_> = ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet", "organization", "some_other"]
        .iter()
        .map(|w| window_label(w))
        .collect();
    assert_eq!(
        labels,
        ["5-hour limit", "Weekly limit", "Weekly Opus limit", "Weekly Sonnet limit", "Organization limit", "some other limit"]
    );
}

// ─── reading, and keeping ───────────────────────────────────────────────────

#[test]
fn the_limits_body_reads_clamps_and_leaves_out_what_does_not_read() {
    let body = json!({"enabled": true, "providers": [
        {"provider": "claude", "name": "", "state": "from-the-future", "windows": [
            {"window": "five_hour", "utilization": 1.7, "resetsAt": "2026-10-04T10:00:00Z", "windowSeconds": 18000, "status": "warning"},
            {"utilization": 0.5}
        ]},
        {"name": "no provider id"}
    ]});
    let limits = read_usage_limits(&body).unwrap();
    assert_eq!(limits.providers.len(), 1);
    let p = &limits.providers[0];
    assert_eq!((p.name.as_str(), p.state), ("claude", ProviderState::Unreadable));
    assert_eq!(p.windows.len(), 1);
    assert_eq!((p.windows[0].utilization, p.windows[0].status, p.windows[0].window_seconds), (Some(1.0), Status::Warning, Some(18000.0)));
    assert_eq!(p.windows[0].resets_at, Some(1_791_108_000_000));
    assert_eq!(read_usage_limits(&json!({"enabled": "yes", "providers": []})), None);
    assert_eq!(read_usage_limits(&json!({"enabled": true})), None);
}

#[test]
fn a_scratch_fleets_empty_answers_read_as_no_limits() {
    let mut usage = Usage::default();
    usage.limits_loaded(usage.limits_asked(), &json!({"enabled": true, "providers": []}));
    usage.harnesses_loaded(usage.harness_asked(), &json!([]));
    assert!(usage.limits.enabled && usage.limits.providers.is_empty() && usage.harnesses.is_empty());
}

#[test]
fn a_push_is_newer_than_a_load_in_flight() {
    let mut usage = Usage::default();
    let asked = usage.limits_asked();
    usage.limits_pushed(&json!({"enabled": true, "providers": [{"provider": "codex", "state": "ok"}]}));
    usage.limits_loaded(asked, &json!({"enabled": true, "providers": []}));
    assert_eq!(usage.limits.providers.len(), 1, "the stale answer is dropped");
    let asked = usage.harness_asked();
    usage.harness_pushed(&json!({"harnessType": "claude-code", "windows": [{"window": "five_hour", "utilization": 0.4}]}));
    usage.harnesses_loaded(asked, &json!([]));
    assert_eq!(usage.harnesses.len(), 1);
}

#[test]
fn the_events_set_what_the_state_keeps() {
    let mut rt = app();
    rt.update(|s, _| {
        s.on_usage_event(&Event::Other(Kind::UsageLimits, json!({"enabled": true, "providers": [{"provider": "claude", "state": "ok"}]})));
        s.on_usage_event(&Event::Other(
            Kind::HarnessUsage,
            json!({"harnessType": "claude-code", "windows": [{"window": "seven_day", "utilization": 0.5, "status": "allowed"}]}),
        ));
        // Not a usage event: nothing changes.
        s.on_usage_event(&Event::Other(Kind::HarnessCatalogChanged, json!({})));
    });
    assert_eq!(rt.state.pickers.usage.limits.providers.len(), 1);
    assert_eq!(rt.state.pickers.usage.harnesses["claude-code"].windows[0].window, "seven_day");
}
