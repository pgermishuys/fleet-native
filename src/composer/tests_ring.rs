//! Fleet's tests for the ring's maths, under their names (the Vue sentence in the comment). The view's tests
//! (`ContextRing.test.ts`) are in `src/tests.rs`.
use super::format::format_tokens;
use super::ring::*;
use crate::fleet::context::{ContextTurn, ContextUsage};
use crate::mock::ring::{Case as Scene, context as scene};

fn context(f: impl FnOnce(&mut ContextUsage)) -> ContextUsage {
    let mut c = ContextUsage {
        session_id: "s1".into(),
        used: Some(50_000),
        limit: Some(200_000),
        compacts_at: Some(167_000),
        model_id: Some("claude-opus-5".into()),
        provider_id: Some("anthropic".into()),
        last_call: None,
        last_call_at: None,
        compacting: false,
        compacted_at: None,
        compaction_error: None,
        turns: Vec::new(),
        updated_at: "2026-10-06T10:00:00Z".into(),
    };
    f(&mut c);
    c
}

fn turns(used: &[u64]) -> Vec<ContextTurn> {
    used.iter()
        .enumerate()
        .map(|(i, u)| ContextTurn { used: *u, limit: Some(200_000), at: format!("2026-10-06T10:0{i}:00Z"), after_compaction: false })
        .collect()
}

// contextPercent and contextTone: "is the size against the window, quiet until three-quarters full"
#[test]
fn is_the_size_against_the_window_quiet_until_three_quarters_full() {
    assert_eq!(percent(&context(|c| c.used = Some(76_000))), Some(38));
    assert_eq!(tone(Some(38)), Tone::Ok);
    assert_eq!(tone(Some(75)), Tone::Warn);
    assert_eq!(tone(Some(90)), Tone::Danger);
}

// contextPercent and contextTone: "is unknown without a size or a window, and never over 100"
#[test]
fn is_unknown_without_a_size_or_a_window_and_never_over_100() {
    assert_eq!(percent(&context(|c| c.used = None)), None);
    assert_eq!(percent(&context(|c| c.limit = None)), None);
    assert_eq!(percent(&context(|c| c.used = Some(250_000))), Some(100));
    assert_eq!(tone(None), Tone::Ok);
}

// turnsBeforeCompaction: "is how many more turns fit at the recent pace"
#[test]
fn is_how_many_more_turns_fit_at_the_recent_pace() {
    let c = context(|c| c.turns = turns(&[20_000, 30_000, 40_000, 50_000]));
    assert_eq!(turns_before_compaction(&c, 5), Some(11));
}

// turnsBeforeCompaction: "counts only the turns since the last compaction"
#[test]
fn counts_only_the_turns_since_the_last_compaction() {
    let mut history = turns(&[150_000, 160_000]);
    history.push(ContextTurn { used: 20_000, limit: Some(200_000), at: "2026-10-06T11:00:00Z".into(), after_compaction: true });
    history.extend(turns(&[25_000]));
    let c = context(|c| {
        c.used = Some(25_000);
        c.turns = history;
    });
    assert_eq!(turns_before_compaction(&c, 5), Some(28));
}

// turnsBeforeCompaction: "can't tell without a compaction point, enough turns, or growth"
#[test]
fn cant_tell_without_a_compaction_point_enough_turns_or_growth() {
    assert_eq!(
        turns_before_compaction(
            &context(|c| {
                c.compacts_at = None;
                c.turns = turns(&[1, 2, 3]);
            }),
            5
        ),
        None
    );
    assert_eq!(turns_before_compaction(&context(|c| c.turns = turns(&[50_000])), 5), None);
    assert_eq!(turns_before_compaction(&context(|c| c.turns = turns(&[50_000, 40_000])), 5), None);
}

// formatTokens: "writes counts the way the meter shows them"
#[test]
fn writes_counts_the_way_the_meter_shows_them() {
    let f = |n: f64| format_tokens(Some(n));
    assert_eq!(f(950.0), "950");
    assert_eq!(f(17_148.0), "17.1k");
    assert_eq!(f(200_000.0), "200k");
    assert_eq!(f(1_000_000.0), "1M");
    assert_eq!(f(1_500_000.0), "1.5M");
}

// Native: only the last five turns set the pace.
#[test]
fn the_pace_comes_from_the_last_five_turns() {
    let c = context(|c| c.turns = turns(&[1_000, 40_000, 41_000, 42_000, 43_000, 44_000, 50_000]));
    // The last five: 41k..50k over four steps, 2.25k a turn; 117k left.
    assert_eq!(turns_before_compaction(&c, 5), Some(52));
}

// Native: amber is strictly above 80, whatever the tone says, and danger wins.
#[test]
fn amber_is_above_80_and_independent_of_the_tone() {
    assert!(!amber(Some(80)));
    assert!(amber(Some(81)));
    assert!(!amber(None));
    assert_eq!(tone(Some(80)), Tone::Warn);
}

// Native: the fill, 0 to 1.
#[test]
fn the_fill_is_empty_without_a_size_and_full_without_a_window() {
    assert_eq!(fill(&context(|c| c.used = None)), 0.0);
    assert_eq!(fill(&context(|c| c.limit = None)), 1.0);
    assert_eq!(fill(&context(|c| c.used = Some(50_000))), 0.25);
    assert_eq!(fill(&context(|c| c.used = Some(500_000))), 1.0);
}

#[test]
fn compacts_at_percent_stops_at_the_window() {
    assert_eq!(compacts_at_percent(&context(|_| {})), Some(83.5));
    assert_eq!(compacts_at_percent(&context(|c| c.compacts_at = Some(400_000))), Some(100.0));
    assert_eq!(compacts_at_percent(&context(|c| c.limit = None)), None);
}

// Row 13.6: the six cases, and the label order (compacting, size unknown, no percent, danger, warn, ok).
#[test]
fn the_six_cases_say_what_they_are() {
    let check = |s: Scene, state: &str, summary: &str| {
        let c = scene(s);
        assert_eq!(state_label(&c, false), state, "{s:?}");
        assert_eq!(super::ring::summary(&c, false), summary, "{s:?}");
    };
    check(Scene::Quiet, "Plenty of room", "38% of the context window used");
    check(Scene::Amber, "Getting full", "84% of the context window used");
    check(Scene::Danger, "Nearly full", "94% of the context window used");
    check(Scene::NoWindow, "Window unknown", "17.1k tokens in the context. The model's window isn't known.");
    check(Scene::Compacted, "Compacted", "Context compacted. Its size shows after the next reply.");
    check(Scene::Compacting, "Compacting\u{2026}", "Compacting the context\u{2026}");
    let mut unmeasured = scene(Scene::Compacted);
    unmeasured.compacted_at = None;
    assert_eq!(state_label(&unmeasured, false), "Not measured yet");
    // A compact request of ours in flight counts as compacting, whatever the context says.
    assert_eq!(state_label(&scene(Scene::Quiet), true), "Compacting\u{2026}");
}

#[test]
fn the_tooltip_names_the_window_or_falls_back_to_the_summary() {
    assert_eq!(title(&scene(Scene::Quiet), false), "Context: 38% of 200k tokens");
    assert_eq!(title(&scene(Scene::NoWindow), false), "17.1k tokens in the context. The model's window isn't known. \u{b7} details");
    assert_eq!(label(&scene(Scene::Quiet), false), "38% of the context window used. Details");
}
