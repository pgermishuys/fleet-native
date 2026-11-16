//! The rules of the usage popover and chip that need no Rune: the short and reset labels (`usage-limits.ts`), a
//! provider's live windows, the pace tick and note, one account shown once across machines, and the credits lines
//! (`usage-providers.ts`, `UsageLimitsPopover.vue`). The popover and chip (piece J) draw what these return.
use super::usage::{ProviderUsage, Status, Window, window_label};
use crate::composer::format::{format_count_digits, format_money};
use crate::conv::retry::{format_when, minutes_text};

/// A window's bar fills past this share of its own length before the pace note speaks.
const MIN_ELAPSED_SHARE: f64 = 0.05;
/// The pace note stays quiet when it would run out less than this long before the reset, in seconds.
const MIN_AHEAD_SECONDS: f64 = 15.0 * 60.0;

/// `windowShortLabel`: the status bar's short name, "5 h", "week", "Opus week".
pub fn window_short_label(window: &str) -> String {
    match window {
        "five_hour" => "5 h".into(),
        "seven_day" => "week".into(),
        "seven_day_opus" => "Opus week".into(),
        "seven_day_sonnet" => "Sonnet week".into(),
        "organization" => "org".into(),
        other => other.replace('_', " "),
    }
}

/// `providerWindowLabel`: Codex's two windows on top of `window_label`.
pub fn provider_window_label(window: &str) -> String {
    match window {
        "primary" => "Primary limit".into(),
        "secondary" => "Secondary limit".into(),
        other => window_label(other),
    }
}

/// `providerWindowShortLabel`.
pub fn provider_window_short_label(window: &str) -> String {
    match window {
        "primary" => "primary".into(),
        "secondary" => "secondary".into(),
        other => window_short_label(other),
    }
}

/// `resetLabel`: "resets 2:05 PM", `None` when the harness didn't say when.
pub fn reset_label(resets_at: Option<i64>, now: i64, offset_secs: i64) -> Option<String> {
    resets_at.map(|at| format!("resets {}", format_when(at, now, offset_secs)))
}

/// `providerWindows`: the provider's windows that haven't reset, the shortest first.
pub fn provider_windows(provider: &ProviderUsage, now: i64) -> Vec<Window> {
    super::usage::live_provider_windows(provider, now)
}

/// `expectedShare`: the share of the window gone by now, 0 to 1; `None` without its length or reset time.
pub fn expected_share(w: &Window, now: i64) -> Option<f64> {
    let length = w.window_seconds.filter(|l| *l > 0.0)?;
    let span = length * 1000.0;
    let start = w.resets_at? as f64 - span;
    Some(((now as f64 - start) / span).clamp(0.0, 1.0))
}

/// The tick's place on the bar, a whole percent (`tickPercent`).
pub fn tick_percent(w: &Window, now: i64) -> Option<i64> {
    expected_share(w, now).map(|s| (s * 100.0).round() as i64)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaceNote {
    pub ahead_by_seconds: f64,
    pub text: String,
}

/// `paceNote`: set only when the user is over an even pace and, at this rate, would run out at least 15 minutes before
/// the window resets. Quiet for a window under 5% gone, a used-up one and one with no length.
pub fn pace_note(w: &Window, now: i64) -> Option<PaceNote> {
    let share = expected_share(w, now)?;
    let (u, length, resets_at) = (w.utilization?, w.window_seconds?, w.resets_at?);
    if u <= 0.0 || u >= 1.0 || w.status == Status::Rejected || u <= share || share < MIN_ELAPSED_SHARE {
        return None;
    }
    let elapsed = share * length;
    let remaining = (resets_at - now) as f64 / 1000.0;
    let ahead = remaining - elapsed * (1.0 - u) / u;
    if ahead < MIN_AHEAD_SECONDS {
        return None;
    }
    Some(PaceNote {
        ahead_by_seconds: ahead,
        text: format!("Ahead of pace: at this rate it runs out about {} before it resets", minutes_text((ahead * 1000.0) as i64)),
    })
}

/// The popover's "Credits: …" amount: "$7.60 left of $10.00", "$2.40 used"; `None` for none, and for GitHub's AI credits
/// (`credits`).
pub fn credits_text(p: &ProviderUsage) -> Option<String> {
    let c = p.credits.as_ref()?;
    if c.unit.as_deref() == Some("ai_credits") {
        return None;
    }
    if let Some(left) = c.remaining {
        let of = c.limit.map(|l| format!(" of {}", format_money(Some(l)))).unwrap_or_default();
        return Some(format!("{} left{of}", format_money(Some(left))));
    }
    c.used.map(|u| format!("{} used", format_money(Some(u))))
}

/// `aiCreditsLine`: GitHub's count of AI credits used this month; `None` for any other credits.
pub fn ai_credits_line(p: &ProviderUsage) -> Option<String> {
    let c = p.credits.as_ref()?;
    let used = c.used.filter(|_| c.unit.as_deref() == Some("ai_credits"))?;
    Some(format!("AI credits used this month: {}", format_count_digits(Some(used), 2)))
}

/// One machine's providers, for `group_usage_by_machine`.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineProviders {
    pub key: String,
    pub name: String,
    pub providers: Vec<ProviderUsage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub provider: ProviderUsage,
    /// Names of the later machines signed in to the same account.
    pub also_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MachineUsageGroup {
    pub key: String,
    pub name: String,
    pub rows: Vec<UsageRow>,
}

/// `groupUsageByMachine`: one group per machine, in the order given (this machine first). A provider whose account key an
/// earlier machine already has is dropped from the later group and noted on the first as "also on".
pub fn group_usage_by_machine(machines: &[MachineProviders]) -> Vec<MachineUsageGroup> {
    // Where each account was first seen: its group and row.
    let mut seen: std::collections::HashMap<String, (usize, usize)> = Default::default();
    let mut groups: Vec<MachineUsageGroup> = Vec::new();
    for (g, machine) in machines.iter().enumerate() {
        let mut rows: Vec<UsageRow> = Vec::new();
        for provider in &machine.providers {
            let id = provider.account_key.as_ref().map(|k| format!("{}:{k}", provider.provider));
            if let Some((fg, fr)) = id.as_ref().and_then(|id| seen.get(id)).copied() {
                groups[fg].rows[fr].also_on.push(machine.name.clone());
                continue;
            }
            if let Some(id) = id {
                seen.insert(id, (g, rows.len()));
            }
            rows.push(UsageRow { provider: provider.clone(), also_on: Vec::new() });
        }
        groups.push(MachineUsageGroup { key: machine.key.clone(), name: machine.name.clone(), rows });
    }
    groups
}
