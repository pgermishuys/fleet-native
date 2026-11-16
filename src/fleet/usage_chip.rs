//! What the status bar's usage chip and its popover show, as plain data (`UsageLimitsPopover.vue`, piece J): the chip's
//! words and tone, the "read from" line, and the machines' groups. The views in `view::usage_chip` and `view::usage_popover`
//! only draw it.
use super::usage::{ProviderUsage, Tone, tightest_window, usage_tone, window_percent};
use super::usage_rules::{
    MachineProviders, MachineUsageGroup, group_usage_by_machine, provider_window_label, provider_window_short_label, reset_label,
};
use crate::State;
use crate::conv::time::{civil, clock, local_offset_secs};

/// The chip: "Claude 5 h" and a bar, or just "Usage" while no window reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Chip {
    pub name: String,
    /// `None`: no bar and no number ("Usage").
    pub percent: Option<i64>,
    pub tone: Tone,
    /// The title and the accessible name.
    pub title: String,
}

/// `chip`: nothing without a provider; "Usage" when none has a readable window; else the tightest one.
pub fn chip(providers: &[ProviderUsage], now: i64, offset_secs: i64) -> Option<Chip> {
    let refs: Vec<&ProviderUsage> = providers.iter().collect();
    let Some(found) = tightest_window(&refs, now) else {
        return (!providers.is_empty()).then(|| Chip {
            name: "Usage".into(),
            percent: None,
            tone: Tone::Quiet,
            title: "Usage limits".into(),
        });
    };
    let percent = window_percent(&found.window);
    let reset = reset_label(found.window.resets_at, now, offset_secs).map(|r| format!("; it {r}")).unwrap_or_default();
    Some(Chip {
        name: format!("{} {}", found.provider.name, provider_window_short_label(&found.window.window)),
        percent: Some(percent),
        tone: usage_tone(&found.window),
        title: format!(
            "{}'s {} is {percent}% used{reset}.",
            found.provider.name,
            provider_window_label(&found.window.window).to_lowercase()
        ),
    })
}

/// `readLine`: "read from Claude Code, last asked 2:05 PM", or either half, or nothing.
pub fn read_line(p: &ProviderUsage, offset_secs: i64) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(source) = p.source.as_ref().filter(|s| !s.is_empty()) {
        parts.push(format!("read from {source}"));
    }
    if let Some(at) = p.read_at {
        let (.., h, m) = civil(at, offset_secs);
        parts.push(format!("last asked {}", clock(h, m)));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

impl State {
    /// Whether the chip is on: the `UsageLimitsChip` preference isn't "false" (`usageLimitsChipShown`).
    pub fn usage_chip_shown(&self) -> bool {
        super::usage::chip_enabled(&self.pickers.prefs)
    }

    pub fn usage_chip(&self) -> Option<Chip> {
        chip(&self.pickers.usage.limits.providers, crate::view::message::now_ms(self), local_offset_secs())
    }

    /// The live machine's key, as the groups carry it (`liveKey`).
    pub fn usage_live_key(&self) -> String {
        self.machines.entries().into_iter().find(|e| e.is_home).map_or_else(|| "home".into(), |e| e.key)
    }

    /// `groups`: this machine first, then each connected other as it last answered, one account shown once.
    pub fn usage_groups(&self) -> Vec<MachineUsageGroup> {
        let entries = self.machines.entries();
        let name = entries.iter().find(|e| e.is_home).map_or_else(|| "This machine".to_string(), |e| e.name.clone());
        let mut list = vec![MachineProviders { key: self.usage_live_key(), name, providers: self.pickers.usage.limits.providers.clone() }];
        let others = self.usage_other_machines();
        for e in entries.iter().filter(|e| !e.is_home && others.iter().any(|m| m.id == e.key)) {
            list.push(MachineProviders {
                key: e.key.clone(),
                name: e.name.clone(),
                providers: self.pickers.usage.others.get(&e.key).cloned().unwrap_or_default(),
            });
        }
        group_usage_by_machine(&list)
    }
}
