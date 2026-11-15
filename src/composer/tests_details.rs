//! `ContextRing.test.ts`'s popover tests, on what the popover says (`details`); the clicks are in `src/tests.rs`.
use super::details::*;
use super::ring::Tone;
use crate::fleet::context::{ContextCall, ContextTurn, ContextUsage};
use crate::fleet::usage::{Status, Window};

/// `publish()` of the Vue test.
fn published(f: impl FnOnce(&mut ContextUsage)) -> ContextUsage {
    let turn = |used: u64| ContextTurn { used, limit: Some(200_000), at: format!("t{used}"), after_compaction: false };
    let mut c = ContextUsage {
        session_id: "s1".into(),
        used: Some(76_000),
        limit: Some(200_000),
        compacts_at: Some(167_000),
        model_id: Some("claude-opus-5".into()),
        provider_id: None,
        last_call: Some(ContextCall {
            input: 1_000,
            cache_read: 74_500,
            cache_write: 0,
            output: 500,
            reasoning: 0,
            used: 76_000,
            total_only: false,
        }),
        last_call_at: None,
        compacting: false,
        compacted_at: None,
        compaction_error: None,
        turns: vec![turn(40_000), turn(76_000)],
        updated_at: String::new(),
    };
    f(&mut c);
    c
}

fn facts() -> Facts<'static> {
    Facts { cost: 1.42, ..Default::default() }
}

fn all_text(d: &Details) -> String {
    let mut out = vec![d.headline.clone(), d.size_line.clone(), d.state_label.to_string()];
    out.extend(d.meter.iter().map(|m| m.legend.clone()));
    out.extend(d.chart.iter().map(|c| c.caption.clone()));
    for l in &d.lines {
        out.push(format!("{}{}{}", l.label, l.value, l.muted.clone().unwrap_or_default()));
    }
    out.join("\n")
}

// ContextRing.test.ts: "opens the details: the size, each turn, the last call and what the session cost"
#[test]
fn opens_the_details_the_size_each_turn_the_last_call_and_what_the_session_cost() {
    let d = details(&published(|_| {}), &facts());
    let text = all_text(&d);
    assert_eq!(d.headline, "38");
    assert!(d.percent_sign);
    assert!(text.contains("76,000 of 200,000 tokens \u{b7} claude-opus-5"));
    assert!(text.contains("compacts at ~84%"));
    assert!(text.contains("2 turns"));
    assert!(text.contains("Read from cache74,500"));
    assert!(text.contains("About 2 turns"));
    assert!(text.contains("$1.42"));
    assert_eq!(d.chart.unwrap().bars.len(), 2);
}

// ContextRing.test.ts: "shows the call's split by kind for a normal call"
#[test]
fn shows_the_calls_split_by_kind_for_a_normal_call() {
    let d = details(&published(|_| {}), &facts());
    let labels: Vec<_> = d.lines.iter().map(|l| l.label).collect();
    assert_eq!(&labels[..3], ["Read from cache", "New input", "Output"]);
}

// ContextRing.test.ts: "leaves out the split when the harness gave only the call's total, but keeps the size"
#[test]
fn leaves_out_the_split_when_the_harness_gave_only_the_calls_total_but_keeps_the_size() {
    let only = ContextCall { input: 76_000, cache_read: 0, cache_write: 0, output: 0, reasoning: 0, used: 76_000, total_only: true };
    let d = details(&published(|c| c.last_call = Some(only)), &facts());
    let text = all_text(&d);
    assert!(!text.contains("Read from cache") && !text.contains("New input"));
    assert!(text.contains("76,000 of 200,000 tokens"));
}

// ContextRing.test.ts: "can't compact when the session says it can't, with its reason"
#[test]
fn cant_compact_when_the_session_says_it_cant_with_its_reason() {
    let f = Facts { can_compact: Some(false), compact_disabled_reason: Some("Wait for the agent to finish its turn."), ..facts() };
    let d = details(&published(|_| {}), &f);
    assert!(!d.can_compact);
    assert_eq!(d.compact_title, "Wait for the agent to finish its turn.");
    // Compacting turns it off too, and says why.
    let busy = details(&published(|_| {}), &Facts { requesting: true, ..facts() });
    assert!(!busy.can_compact);
    assert_eq!((busy.compact_label, busy.compact_title.as_str()), ("Compacting\u{2026}", "The context is being compacted."));
}

// ContextRing.test.ts: "after a compaction, says so until the next call measures it"
#[test]
fn after_a_compaction_the_popover_says_compacted_and_the_window() {
    let d = details(&published(|c| (c.used, c.last_call, c.compacted_at) = (None, None, Some("t".into()))), &facts());
    assert_eq!((d.headline.as_str(), d.state_label, d.percent_sign), ("\u{2014}", "Compacted", false));
    assert_eq!(d.size_line, "200,000 token window \u{b7} claude-opus-5");
}

// ContextRing.test.ts: "shows the harness's usage limits under the context, when it reports them"
#[test]
fn shows_the_harness_usage_limits_under_the_context_when_it_reports_them() {
    let now = 20_000 * 86_400_000;
    let w = |name: &str, u: f64, mins: i64, status| Window {
        window: name.into(),
        utilization: Some(u),
        resets_at: Some(now + mins * 60_000),
        window_seconds: None,
        status,
    };
    let windows = crate::fleet::usage::current_windows(
        Some(&crate::fleet::usage::HarnessUsage {
            harness_type: "claude-code".into(),
            windows: vec![w("seven_day", 0.63, 4 * 1440, Status::Allowed), w("five_hour", 0.82, 125, Status::Warning)],
        }),
        now,
    );
    let d = details(&published(|_| {}), &Facts { windows, now, ..facts() });
    let labels: Vec<_> = d.limits.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(labels, ["5-hour limit", "Weekly limit"]);
    assert!(d.limits[0].value.starts_with("82% \u{b7} resets "));
    assert_eq!(d.limits[0].tone, LimitTone::Warn);
    assert!(d.limits[1].value.starts_with("63% \u{b7} resets "));
    assert_eq!(d.limits[1].tone, LimitTone::Ok);
}

// ContextRing.test.ts: "shows no limits for a harness that reports none, as on a gateway"
#[test]
fn shows_no_limits_for_a_harness_that_reports_none_as_on_a_gateway() {
    assert!(details(&published(|_| {}), &facts()).limits.is_empty());
}

// Native: a rejected window reads 100% in red; the hint and the button change with the tone.
#[test]
fn a_used_up_window_is_red_and_a_full_context_makes_compact_now_the_primary_action() {
    let now = 20_000 * 86_400_000;
    let rejected =
        Window { window: "five_hour".into(), utilization: Some(0.2), resets_at: None, window_seconds: None, status: Status::Rejected };
    let l = limits(&[rejected], now, 0);
    assert_eq!((l[0].value.as_str(), l[0].fill, l[0].tone), ("100%", 100, LimitTone::Danger));
    let d = details(&published(|c| c.used = Some(188_000)), &facts());
    assert_eq!(d.tone, Tone::Danger);
    assert_eq!(d.hint, "Compacting summarises earlier turns so the agent can keep going.");
}

// Native: the chart's caption names the hovered bar; the bar after a compaction says so.
#[test]
fn the_chart_caption_names_the_hovered_bar() {
    let c = published(|c| c.turns[1].after_compaction = true);
    assert_eq!(chart(&c, None).unwrap().caption, "2 turns");
    assert_eq!(chart(&c, Some(1)).unwrap().caption, "Turn 2 \u{b7} 76k \u{b7} after a compaction");
    assert_eq!(chart(&published(|c| c.turns.truncate(1)), None).unwrap().caption, "1 turn");
    // No window: the bars scale to the biggest turn, and a tiny one still shows 4%.
    let open = published(|c| {
        c.limit = None;
        c.turns[0].used = 100;
    });
    let bars = chart(&open, None).unwrap().bars;
    assert_eq!((bars[0].height, bars[1].height), (4.0, 100.0));
}

// Native: "Probably next turn" at 0, "25+ turns" past 25.
#[test]
fn turns_before_it_compacts_reads_as_fleet_words_it() {
    let turns = |us: &[u64]| {
        us.iter().map(|u| ContextTurn { used: *u, limit: None, at: u.to_string(), after_compaction: false }).collect::<Vec<_>>()
    };
    let line = |c: &ContextUsage| lines(c, 0.0).into_iter().find(|l| l.label == "Before it compacts").map(|l| l.value);
    assert_eq!(
        line(&published(|c| {
            c.used = Some(170_000);
            c.turns = turns(&[100_000, 170_000]);
        }))
        .as_deref(),
        Some("Probably next turn")
    );
    assert_eq!(
        line(&published(|c| {
            c.turns = turns(&[75_000, 76_000]);
        }))
        .as_deref(),
        Some("25+ turns")
    );
}
