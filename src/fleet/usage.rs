//! How much of their accounts' usage limits the user has used: `GET /api/usage/limits` (every provider's windows,
//! `usage-providers.ts`), `GET /api/harnesses/usage` (each harness's own windows, `usage-limits.ts`) and the
//! `usage.limits` and `harness.usage` events that follow them. Pure: reading, the rules that choose and word what is
//! shown, and what the state keeps. The requests run on threads (`State::load_usage`).
use super::events::{Event, Kind};
use super::work::parse_iso_ms;
use crate::conv::retry::format_when;
use crate::conv::time::local_offset_secs;
use crate::{State, Tx};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// `WATCH_INTERVAL_MS`: how often the popover, while open, asks again.
pub const WATCH_SECS: f64 = 60.0;
/// `USAGE_LIMITS_CHIP_PREFERENCE_KEY`: "false" hides the status bar chip (the feature stays on).
pub const CHIP_PREFERENCE_KEY: &str = "UsageLimitsChip";

/// `GET /api/usage/limits`, asking the server to refresh stale providers when `watch`.
pub fn limits_path(watch: bool) -> String {
    format!("/api/usage/limits{}", if watch { "?watch=true" } else { "" })
}

/// `/api/usage/limits/{provider}/allow`.
pub fn allow_path(provider: &str) -> String {
    format!("/api/usage/limits/{}/allow", urlencode(provider))
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Whether the status bar shows the chip: unless the preference says "false".
pub fn chip_enabled(prefs: &super::prefs::Prefs) -> bool {
    prefs.get(CHIP_PREFERENCE_KEY) != Some("false")
}

/// `USAGE_WARNING_THRESHOLD`: from this much of a window used, it is called near its limit.
pub const WARNING: f64 = 0.8;
/// `DIM_FROM_PERCENT`: from this percent a provider's models dim in the model menu.
const DIM_FROM_PERCENT: i64 = 95;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Allowed,
    Warning,
    Rejected,
}

/// One window (`UsageLimitWindow`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Window {
    /// "five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet", or the harness's own name.
    pub window: String,
    /// 0 to 1, when the harness says.
    pub utilization: Option<f64>,
    /// Epoch milliseconds, when the harness says.
    pub resets_at: Option<i64>,
    pub window_seconds: Option<f64>,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HarnessUsage {
    pub harness_type: String,
    pub windows: Vec<Window>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderState {
    #[default]
    Ok,
    NotAllowed,
    NoWindow,
    Unreadable,
    SignedOut,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProviderUsage {
    /// "claude", "codex", "openrouter", "copilot".
    pub provider: String,
    pub account: Option<String>,
    /// Opaque stable key of the account: the same on two machines signed in to the same account (`accountKey`).
    pub account_key: Option<String>,
    pub name: String,
    pub state: ProviderState,
    /// Where the sign-in was read from: "Claude Code", "OpenCode".
    pub source: Option<String>,
    pub harnesses: Vec<String>,
    /// Epoch milliseconds Fleet last asked the provider.
    pub read_at: Option<i64>,
    pub windows: Vec<Window>,
    pub credits: Option<Credits>,
}

/// `ProviderCredits`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Credits {
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub used: Option<f64>,
    /// `None` for dollars; "ai_credits" for GitHub's AI credits ($0.01 each).
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageLimits {
    pub enabled: bool,
    pub providers: Vec<ProviderUsage>,
}

// ─── reading ────────────────────────────────────────────────────────────────

fn finite(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|n| n.is_finite())
}

fn read_window(raw: &Value) -> Option<Window> {
    let window = raw.get("window")?.as_str()?.to_string();
    Some(Window {
        window,
        utilization: finite(raw.get("utilization")).map(|u| u.clamp(0.0, 1.0)),
        resets_at: raw.get("resetsAt").and_then(Value::as_str).and_then(parse_iso_ms),
        window_seconds: finite(raw.get("windowSeconds")).filter(|s| *s > 0.0),
        status: match raw.get("status").and_then(Value::as_str) {
            Some("rejected") => Status::Rejected,
            Some("warning") => Status::Warning,
            _ => Status::Allowed,
        },
    })
}

fn read_windows(raw: &Value) -> Vec<Window> {
    raw.as_array().into_iter().flatten().filter_map(read_window).collect()
}

/// `toHarnessUsage`: `None` when it doesn't read.
pub fn read_harness_usage(raw: &Value) -> Option<HarnessUsage> {
    let harness_type = raw.get("harnessType")?.as_str()?.to_string();
    let windows = raw.get("windows").filter(|w| w.is_array())?;
    Some(HarnessUsage { harness_type, windows: read_windows(windows) })
}

fn read_provider(raw: &Value) -> Option<ProviderUsage> {
    let provider = raw.get("provider")?.as_str()?.to_string();
    let text = |k: &str| raw.get(k).and_then(Value::as_str).map(str::to_string);
    Some(ProviderUsage {
        name: text("name").filter(|n| !n.is_empty()).unwrap_or_else(|| provider.clone()),
        account: text("account"),
        account_key: text("accountKey").filter(|k| !k.is_empty()),
        source: text("source"),
        harnesses: raw
            .get("harnesses")
            .and_then(Value::as_array)
            .map(|h| h.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        read_at: raw.get("readAt").and_then(Value::as_str).and_then(parse_iso_ms),
        credits: raw.get("credits").filter(|c| c.is_object()).map(|c| Credits {
            remaining: finite(c.get("remaining")),
            limit: finite(c.get("limit")),
            used: finite(c.get("used")),
            unit: c.get("unit").and_then(Value::as_str).map(str::to_string),
        }),
        // A state this client doesn't know reads as unreadable rather than as a lie.
        state: match raw.get("state").and_then(Value::as_str) {
            Some("ok") => ProviderState::Ok,
            Some("not_allowed") => ProviderState::NotAllowed,
            Some("no_window") => ProviderState::NoWindow,
            Some("signed_out") => ProviderState::SignedOut,
            Some("unsupported") => ProviderState::Unsupported,
            _ => ProviderState::Unreadable,
        },
        windows: read_windows(raw.get("windows").unwrap_or(&Value::Null)),
        provider,
    })
}

/// `toUsageLimits`: `None` when the body doesn't read; entries that don't read are left out.
pub fn read_usage_limits(raw: &Value) -> Option<UsageLimits> {
    let enabled = raw.get("enabled")?.as_bool()?;
    let providers = raw.get("providers")?.as_array()?;
    Some(UsageLimits { enabled, providers: providers.iter().filter_map(read_provider).collect() })
}

// ─── the rules ──────────────────────────────────────────────────────────────

const HARNESS_ORDER: [&str; 4] = ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"];
const PROVIDER_ORDER: [&str; 6] = ["five_hour", "primary", "seven_day", "secondary", "seven_day_opus", "seven_day_sonnet"];

/// Windows that haven't reset yet, in `order` (others after, by name).
fn live(windows: &[Window], order: &[&str], now: i64) -> Vec<Window> {
    let rank = |w: &str| order.iter().position(|o| *o == w).unwrap_or(order.len());
    let mut kept: Vec<Window> = windows.iter().filter(|w| w.resets_at.is_none_or(|at| at > now)).cloned().collect();
    kept.sort_by(|a, b| rank(&a.window).cmp(&rank(&b.window)).then_with(|| a.window.cmp(&b.window)));
    kept
}

/// `currentWindows`: shortest window first, then the model-scoped ones; windows that reset already are left out.
pub fn current_windows(usage: Option<&HarnessUsage>, now: i64) -> Vec<Window> {
    usage.map(|u| live(&u.windows, &HARNESS_ORDER, now)).unwrap_or_default()
}

/// `windowLabel`: "5-hour limit", "Weekly limit", "Weekly Opus limit".
pub fn window_label(window: &str) -> String {
    match window {
        "five_hour" => "5-hour limit".into(),
        "seven_day" => "Weekly limit".into(),
        "seven_day_opus" => "Weekly Opus limit".into(),
        "seven_day_sonnet" => "Weekly Sonnet limit".into(),
        "organization" => "Organization limit".into(),
        other => format!("{} limit", other.replace('_', " ")),
    }
}

/// `percentLabel`: "82%", "100%" when rejected, `None` when the harness didn't say.
pub fn percent_label(w: &Window) -> Option<String> {
    if w.status == Status::Rejected {
        return Some("100%".into());
    }
    w.utilization.map(|u| format!("{}%", (u * 100.0).round()))
}

/// `isNearLimit`: close to its limit, or at it.
pub fn is_near_limit(w: &Window) -> bool {
    w.status == Status::Rejected || w.utilization.unwrap_or(0.0) >= WARNING
}

/// `windowPercent`: 0 to 100, a used-up window counting as full.
pub fn window_percent(w: &Window) -> i64 {
    if w.status == Status::Rejected { 100 } else { (w.utilization.unwrap_or(0.0) * 100.0).round() as i64 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Quiet,
    Warn,
    Used,
}

/// `usageTone`: amber from 80%, red when used up, quiet below.
pub fn usage_tone(w: &Window) -> Tone {
    if w.status == Status::Rejected {
        Tone::Used
    } else if window_percent(w) as f64 >= WARNING * 100.0 {
        Tone::Warn
    } else {
        Tone::Quiet
    }
}

/// `usageProviderForModelProvider`: which usage provider a model provider draws on; `None` when none has windows.
pub fn usage_provider_for(model_provider: &str) -> Option<&'static str> {
    match model_provider.to_lowercase().as_str() {
        "anthropic" | "claude-code" => Some("claude"),
        "openai" => Some("codex"),
        "openrouter" => Some("openrouter"),
        "github-copilot" | "copilot" => Some("copilot"),
        _ => None,
    }
}

/// A provider's windows that haven't reset, the shortest first (`providerWindows`).
pub fn live_provider_windows(p: &ProviderUsage, now: i64) -> Vec<Window> {
    live(&p.windows, &PROVIDER_ORDER, now)
}

/// A window and the provider it belongs to (`TightestWindow`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tightest<'a> {
    pub provider: &'a ProviderUsage,
    pub window: Window,
}

/// `tightestWindow`: the fullest live window of any readable provider (the first one on a tie): what the status bar
/// chip shows. `None` when none is readable.
pub fn tightest_window<'a>(providers: &[&'a ProviderUsage], now: i64) -> Option<Tightest<'a>> {
    let mut best: Option<Tightest<'a>> = None;
    for p in providers.iter().copied().filter(|p| p.state == ProviderState::Ok) {
        for w in live_provider_windows(p, now) {
            if best.as_ref().is_none_or(|b| window_percent(&w) > window_percent(&b.window)) {
                best = Some(Tightest { provider: p, window: w });
            }
        }
    }
    best
}

/// What a provider heading in the model menu shows (`ModelProviderUsage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProviderUsage {
    pub percent: i64,
    pub tone: Tone,
    /// Used up or nearly: the models dim, but can still be picked.
    pub dimmed: bool,
    /// "resets 2:05 PM" while dimmed and the reset time is known.
    pub reset_text: Option<String>,
}

/// `modelProviderUsage`: the tightest live window of the usage provider behind a model provider.
pub fn model_provider_usage(providers: &[ProviderUsage], model_provider: &str, now: i64, offset_secs: i64) -> Option<ModelProviderUsage> {
    let wanted = usage_provider_for(model_provider)?;
    let of_it: Vec<&ProviderUsage> = providers.iter().filter(|p| p.provider == wanted).collect();
    let found = tightest_window(&of_it, now)?.window;
    let percent = window_percent(&found);
    let dimmed = found.status == Status::Rejected || percent >= DIM_FROM_PERCENT;
    let reset_text = found.resets_at.filter(|_| dimmed).map(|at| format!("resets {}", format_when(at, now, offset_secs)));
    Some(ModelProviderUsage { percent, tone: usage_tone(&found), dimmed, reset_text })
}

/// The bar's tooltip (`title`).
pub fn usage_tooltip(u: &ModelProviderUsage) -> String {
    format!("{}% of the limit used", u.percent)
}

// ─── what the state keeps ───────────────────────────────────────────────────

/// `use-usage-limits.ts` and `use-harness-usage.ts`: one copy each. A load that was asked for before a newer push (or
/// a newer load) arrived is dropped, as Fleet's `loadId` does.
#[derive(Debug, Default)]
pub struct Usage {
    pub limits: UsageLimits,
    pub harnesses: BTreeMap<String, HarnessUsage>,
    /// Other machines' providers by machine key: asked only while the popover is open, never pushed. A failed answer
    /// leaves the last one (`otherProviders`).
    pub others: BTreeMap<String, Vec<ProviderUsage>>,
    /// The provider whose Allow is running: the button is disabled meanwhile (`allowing`).
    pub allowing: Option<String>,
    /// How many popovers are open: several share one timer.
    watchers: usize,
    /// When the next watched read is due, on the app clock; `None` means now.
    watch_due: Option<f64>,
    limits_id: AtomicU64,
    harness_id: AtomicU64,
}

impl Usage {
    /// A request for the limits starts: its number, to hand back with the answer.
    pub fn limits_asked(&self) -> u64 {
        self.limits_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn harness_asked(&self) -> u64 {
        self.harness_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn limits_loaded(&mut self, id: u64, body: &Value) {
        if let Some(next) = read_usage_limits(body)
            && id == self.limits_id.load(Ordering::SeqCst)
        {
            self.limits = next;
        }
    }

    /// The whole list replaces what was kept.
    pub fn harnesses_loaded(&mut self, id: u64, body: &Value) {
        let Some(list) = body.as_array() else { return };
        if id != self.harness_id.load(Ordering::SeqCst) {
            return;
        }
        self.harnesses = list.iter().filter_map(read_harness_usage).map(|u| (u.harness_type.clone(), u)).collect();
    }

    /// A popover opens (`watch`): reading starts at the next poll and repeats every minute while any is open.
    pub fn watch(&mut self) {
        self.watchers += 1;
        if self.watchers == 1 {
            self.watch_due = None;
        }
    }

    /// A popover closes. The timer stops with the last one; stopping twice does no harm.
    pub fn unwatch(&mut self) {
        self.watchers = self.watchers.saturating_sub(1);
        if self.watchers == 0 {
            self.watch_due = None;
        }
    }

    pub fn watching(&self) -> bool {
        self.watchers > 0
    }

    /// Whether a watched read is due at `now`; if so it is counted as made and the next is a minute on.
    pub fn watch_read_due(&mut self, now: f64) -> bool {
        if self.watchers == 0 || self.watch_due.is_some_and(|due| now < due) {
            return false;
        }
        self.watch_due = Some(now + WATCH_SECS);
        true
    }

    /// Another machine's answer; one that doesn't read keeps what was kept.
    pub fn other_loaded(&mut self, key: &str, body: &Value) {
        if let Some(next) = read_usage_limits(body) {
            self.others.insert(key.to_string(), next.providers);
        }
    }

    /// `usage.limits`: newer than any load in flight.
    pub fn limits_pushed(&mut self, body: &Value) {
        if let Some(next) = read_usage_limits(body) {
            self.limits_id.fetch_add(1, Ordering::SeqCst);
            self.limits = next;
        }
    }

    /// `harness.usage`: one harness's windows.
    pub fn harness_pushed(&mut self, body: &Value) {
        if let Some(usage) = read_harness_usage(body) {
            self.harness_id.fetch_add(1, Ordering::SeqCst);
            self.harnesses.insert(usage.harness_type.clone(), usage);
        }
    }
}

impl State {
    /// Asks for both bodies (on start and after a reconnect), each on its own thread.
    pub fn load_usage(&self) {
        self.load_usage_limits(false);
        let Some(link) = &self.link else { return };
        let id = self.pickers.usage.harness_asked();
        link.off_thread(super::http::harness_usage, move |s, _, result| {
            if let Ok(body) = result {
                s.pickers.usage.harnesses_loaded(id, &body);
            }
        });
    }

    /// `load`: this machine's providers on a thread; `watch` asks the server to refresh stale ones first.
    pub fn load_usage_limits(&self, watch: bool) {
        let Some(link) = &self.link else { return };
        let id = self.pickers.usage.limits_asked();
        link.off_thread(
            move |base| super::http::usage_limits(base, watch),
            move |s, _, result| {
                if let Ok(body) = result {
                    s.pickers.usage.limits_loaded(id, &body);
                }
            },
        );
    }

    /// The machines other than this one that answered their last check: the only ones asked (`connectedOthers`).
    pub fn usage_other_machines(&self) -> Vec<super::machine::Machine> {
        self.machines
            .entries()
            .into_iter()
            .filter(|e| !e.is_home && self.machines.others.get(&e.key).is_some_and(|s| s.loaded_at.is_some() && s.error.is_none()))
            .filter_map(|e| e.machine)
            .collect()
    }

    /// `loadOthers`: each such machine on its own thread, with its own token and addresses.
    fn load_usage_others(&self) {
        let Some(link) = &self.link else { return };
        for machine in self.usage_other_machines() {
            let post = link.post();
            std::thread::spawn(move || {
                let path = limits_path(true);
                let Ok((status, body)) = super::machine_routes::get_on(&machine, &path) else { return };
                let Some(body) = (200..300).contains(&status).then(|| serde_json::from_str::<Value>(&body).ok()).flatten() else {
                    return;
                };
                post.update(move |s, _| s.pickers.usage.other_loaded(&machine.id, &body));
            });
        }
    }

    /// The poll's turn: while a popover is open, this machine and the others are read at once and then every minute.
    pub(crate) fn keep_usage_watch(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.pickers.usage.watch_read_due(now) {
            self.load_usage_limits(true);
            self.load_usage_others();
        }
        if self.pickers.usage.watching() {
            tx.wake_after(std::time::Duration::from_secs(1));
        }
        false
    }

    /// `setAllowed`: Allow (`POST`) or revoke (`DELETE`) reading a provider's sign-in, then the limits are read again.
    /// Nothing while that provider's call is running.
    pub fn set_usage_allowed(&mut self, provider: &str, allowed: bool) {
        let Some(link) = &self.link else { return };
        if self.pickers.usage.allowing.is_some() {
            return;
        }
        self.pickers.usage.allowing = Some(provider.to_string());
        let name = provider.to_string();
        link.off_thread(
            move |base| super::http::usage_allow(base, &name, allowed),
            move |s, _, result| {
                s.pickers.usage.allowing = None;
                // A refusal changes nothing; the popover keeps showing what it showed.
                if result.is_ok() {
                    s.load_usage_limits(false);
                }
            },
        );
    }

    /// `usage.limits` and `harness.usage` on the `sessions` topic.
    pub fn on_usage_event(&mut self, event: &Event) {
        match event {
            Event::Other(Kind::UsageLimits, props) => self.pickers.usage.limits_pushed(props),
            Event::Other(Kind::HarnessUsage, props) => self.pickers.usage.harness_pushed(props),
            _ => {}
        }
    }

    /// The heading bar for a model provider now.
    pub fn provider_usage(&self, model_provider: &str) -> Option<ModelProviderUsage> {
        let now = crate::view::message::now_ms(self);
        model_provider_usage(&self.pickers.usage.limits.providers, model_provider, now, local_offset_secs())
    }
}
