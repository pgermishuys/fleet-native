//! The context ring's scenes (piece 5G): one context for each of the six cases of row 13.6, and the two tones the
//! amber rule tells apart. Invented numbers and an invented model name.
use crate::State;
use crate::fleet::context::{ContextCall, ContextTurn, ContextUsage};

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
        turns: used.map(|u| vec![turn(u / 2, false), turn(u, false)]).unwrap_or_default(),
        updated_at: "2026-10-06T10:05:00Z".into(),
    }
}

impl State {
    /// A scene: the open session (the invented one, idle) has reported `case`.
    pub fn show_context(&mut self, case: Case) {
        self.open = (0, 2);
        self.ring.mock = Some(context(case));
    }
}
