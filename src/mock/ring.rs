//! The context ring's scenes (piece 5G): one context for each of the six cases of row 13.6, and the two tones the
//! amber rule tells apart. Invented numbers and an invented model name.
use crate::State;
use crate::fleet::context::{ContextCall, ContextTurn, ContextUsage};
use crate::fleet::usage::{HarnessUsage, Status, Window};

/// Which context a scene shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// 38% of 200k: a quiet arc.
    Quiet,
    /// 78%: "getting full", not yet amber (amber is above 80).
    Warm,
    /// 84%: getting full, amber.
    Amber,
    /// 94%: nearly full, red.
    Danger,
    /// A count and no window.
    NoWindow,
    /// Right after a compaction: no size yet.
    Compacted,
    /// A compaction under way.
    Compacting,
}

pub fn context(case: Case) -> ContextUsage {
    let turn =
        |used: u64, after_compaction: bool| ContextTurn { used, limit: Some(200_000), at: "2026-10-06T10:00:00Z".into(), after_compaction };
    let used = match case {
        Case::Quiet | Case::Compacting => Some(76_000),
        Case::Warm => Some(156_000),
        Case::Amber => Some(168_000),
        Case::Danger => Some(188_000),
        Case::NoWindow => Some(17_148),
        Case::Compacted => None,
    };
    ContextUsage {
        session_id: "mock".into(),
        used,
        limit: (case != Case::NoWindow).then_some(200_000),
        compacts_at: Some(167_000),
        model_id: Some("orchard-model-5".into()),
        provider_id: Some("orchard".into()),
        last_call: used.map(|used| ContextCall {
            input: 1_000,
            cache_read: used.saturating_sub(1_500),
            cache_write: 0,
            output: 500,
            reasoning: 0,
            used,
            total_only: false,
        }),
        last_call_at: None,
        compacting: case == Case::Compacting,
        compacted_at: (case == Case::Compacted).then(|| "2026-10-06T10:05:00Z".into()),
        compaction_error: None,
        turns: turns(case, used, &turn),
        updated_at: "2026-10-06T10:05:00Z".into(),
    }
}

/// The chart's turns: a climb with one compaction in it, so the bars show the dot and the dashed line something to cross.
/// A context with no size yet keeps the turns it had before the compaction.
fn turns(case: Case, used: Option<u64>, turn: &dyn Fn(u64, bool) -> ContextTurn) -> Vec<ContextTurn> {
    let Some(used) = used else {
        return [60_000, 90_000, 120_000, 150_000].iter().map(|u| turn(*u, false)).collect();
    };
    let steps: [(f64, bool); 8] =
        [(0.24, false), (0.40, false), (0.58, false), (0.41, true), (0.63, false), (0.78, false), (0.88, false), (1.0, false)];
    let mut out: Vec<ContextTurn> = steps.iter().map(|(f, after)| turn((used as f64 * f) as u64, *after)).collect();
    if case == Case::NoWindow {
        for t in &mut out {
            t.limit = None;
        }
    }
    out
}

impl State {
    /// A scene: the ring's popover is open on `case`; `extra` adds what the popover shows beyond the context: "limits"
    /// (the harness reports its windows), "error" (Fleet refused Compact now) or "disabled" (the session can't compact).
    pub fn show_context_popover(&mut self, case: Case, extra: &str) {
        self.show_context(case);
        let now = 20_000 * 86_400_000 + (12 * 3600 - crate::conv::time::local_offset_secs()) * 1000;
        self.now_ms = Some(now);
        let row = &mut self.projects[0].sessions[2].row;
        row.cost = 1.42;
        match extra {
            "limits" => {
                row.harness_type = Some("claude-code".into());
                let window = |name: &str, utilization: f64, minutes: i64, status| Window {
                    window: name.into(),
                    utilization: Some(utilization),
                    resets_at: Some(now + minutes * 60_000),
                    window_seconds: None,
                    status,
                };
                let windows = vec![
                    window("seven_day", 0.63, 4 * 1440, Status::Allowed),
                    window("five_hour", 0.82, 125, Status::Warning),
                    window("seven_day_opus", 1.0, 2 * 1440, Status::Rejected),
                ];
                self.pickers.usage.harnesses.insert("claude-code".into(), HarnessUsage { harness_type: "claude-code".into(), windows });
            }
            "error" => {
                self.compact.errors.insert(String::new(), "Wait for the agent to finish its turn, then compact.".into());
            }
            "disabled" => {
                row.can_compact = Some(false);
                row.compact_disabled_reason = Some("Wait for the agent to finish its turn.".into());
            }
            _ => {}
        }
        self.ui.open = Some(crate::ui::Open::Popover("context"));
    }

    /// A scene: the open session (the invented one, idle) has reported `case`.
    pub fn show_context(&mut self, case: Case) {
        self.open = (0, 2);
        self.ring.mock = Some(context(case));
    }
}
