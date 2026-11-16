//! The usage chip's and popover's scenes (component 9, piece J): providers written as the server sends them
//! (`GET /api/usage/limits`), read by the same code a live Fleet's answer goes through, with the clock fixed so a picture reads
//! the same every time. The scratch Fleet has no provider to read, so Fleet's own client gets the same bodies through
//! Playwright route interception (the pictures say so).
use super::failure::{NOW, chrono_free_iso};
use crate::State;
use crate::fleet::usage::read_usage_limits;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// 42% of Claude's 5-hour window: the quiet chip.
    Quiet,
    /// 85%: amber.
    Warn,
    /// Used up: red.
    Used,
    /// A provider waiting for Allow and nothing readable: "Usage" with no bar.
    NoBar,
    /// Claude with two windows, OpenRouter's credits and Copilot's organization limit.
    Providers,
    /// Every state that has no bars.
    States,
    /// Ahead of pace, and under pace.
    Pace,
    /// This machine and a laptop, one account on both.
    Machines,
}

pub fn at(hours: f64) -> String {
    chrono_free_iso(NOW + (hours * 3_600_000.0) as i64)
}

pub fn provider(id: &str, name: &str, over: Value) -> Value {
    let mut v = json!({
        "provider": id, "name": name, "account": null, "state": "ok", "source": "Claude Code", "harnesses": [],
        "readAt": at(-0.05), "windows": [], "credits": null,
    });
    v.as_object_mut().unwrap().extend(over.as_object().cloned().unwrap_or_default());
    v
}

pub fn window(name: &str, used: f64, hours: f64, seconds: Option<u32>, status: &str) -> Value {
    json!({ "window": name, "utilization": used, "resetsAt": at(hours), "windowSeconds": seconds, "status": status })
}

/// The body for a case: this machine's answer.
pub fn body(case: Case) -> Value {
    let claude = |used: f64, status: &str| {
        provider("claude", "Claude", json!({ "account": "Max plan", "windows": [window("five_hour", used, 2.0, None, status)] }))
    };
    let providers = match case {
        Case::Quiet => vec![claude(0.42, "allowed")],
        Case::Warn => vec![claude(0.85, "warning")],
        Case::Used => vec![claude(0.5, "rejected")],
        Case::NoBar => vec![provider("codex", "Codex", json!({ "state": "not_allowed", "source": "Codex CLI" }))],
        Case::Providers => vec![
            provider(
                "claude",
                "Claude",
                json!({ "account": "Max plan", "windows": [
                    window("five_hour", 0.42, 2.0, None, "allowed"),
                    window("seven_day", 0.1, 100.0, None, "allowed"),
                ] }),
            ),
            provider(
                "openrouter",
                "OpenRouter",
                json!({ "source": "OpenCode", "credits": { "remaining": 7.6, "limit": 10.0, "used": 2.4, "unit": null } }),
            ),
            provider(
                "copilot",
                "GitHub Copilot",
                json!({ "source": "GitHub CLI", "windows": [window("organization", 1.0, 100.0, None, "rejected")] }),
            ),
        ],
        Case::States => vec![
            provider("codex", "Codex", json!({ "state": "not_allowed", "source": "Codex CLI" })),
            provider(
                "copilot",
                "GitHub Copilot",
                json!({ "state": "no_window", "source": "GitHub CLI", "credits": { "remaining": null, "limit": null, "used": 307846, "unit": "ai_credits" } }),
            ),
            provider("a", "Alpha", json!({ "state": "unreadable", "source": null })),
            provider("b", "Beta", json!({ "state": "signed_out" })),
            provider("c", "Gamma", json!({ "state": "unsupported", "source": null })),
        ],
        // 3 of 5 hours gone and 90% used: ahead of pace. 20% used with the same length: a tick, no note. The week has no length.
        Case::Pace => vec![provider(
            "claude",
            "Claude",
            json!({ "account": "Max plan", "windows": [
                window("five_hour", 0.9, 2.0, Some(18_000), "warning"),
                window("seven_day", 0.2, 120.0, Some(604_800), "allowed"),
                window("seven_day_opus", 0.1, 80.0, None, "allowed"),
            ] }),
        )],
        Case::Machines => vec![
            provider(
                "codex",
                "Codex",
                json!({ "accountKey": "k1", "source": "Codex CLI", "windows": [window("primary", 0.3, 1.0, None, "allowed")] }),
            ),
            provider("claude", "Claude", json!({ "windows": [window("five_hour", 0.42, 2.0, None, "allowed")] })),
        ],
    };
    json!({ "enabled": true, "providers": providers })
}

/// What the laptop answers in `Machines`: the same Codex account, and a provider of its own.
fn laptop() -> Value {
    json!({ "enabled": true, "providers": [
        provider("codex", "Codex", json!({ "accountKey": "k1", "source": "Codex CLI", "windows": [window("primary", 0.3, 1.0, None, "allowed")] })),
        provider("openrouter", "OpenRouter", json!({ "source": "OpenCode", "credits": { "remaining": 7.6, "limit": 10.0, "used": 2.4, "unit": null } })),
    ] })
}

impl State {
    /// The chip on `case`; with `open` the popover is up too.
    pub fn show_usage(&mut self, case: Case, open: bool) {
        self.now_ms = Some(NOW);
        self.pickers.usage.limits = read_usage_limits(&body(case)).expect("a body that reads");
        if case == Case::Machines {
            self.show_machines_only(super::machines::Case::Several);
            if let Some(provided) = read_usage_limits(&laptop()) {
                self.pickers.usage.others.insert("m-falcon".into(), provided.providers);
            }
        }
        self.ui.open = open.then_some(crate::ui::Open::Popover("usage"));
    }
}
