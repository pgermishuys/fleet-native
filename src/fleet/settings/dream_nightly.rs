//! "Dream each night" as plain data (`DreamNightly.vue`, `lib/dreaming.ts`): what the server says about the nightly dream, the
//! changes Settings sends, the choices the row offers (harness, budget, time) and the state the row holds. No Rune but the
//! text box, and no I/O: `dream_nightly_actions.rs` sends the calls.
use super::dreaming::{Dream, parse_dream};
use super::memory::Request;
use crate::fleet::new_session::feeds::Catalog;
use rune::TextEdit;
use serde_json::{Value, json};

/// `DREAM_BUDGETS`: what a night may spend, in tokens.
pub const BUDGETS: [i64; 4] = [100_000, 300_000, 500_000, 1_000_000];
/// `NIGHTLY_HARNESSES`: the only harnesses a night dreams on.
pub const HARNESSES: [&str; 2] = ["opencode2", "claude-code"];

/// `NightlyDreamingView`.
#[derive(Clone, Debug, PartialEq)]
pub struct Nightly {
    pub enabled: bool,
    pub automation_id: Option<String>,
    pub paused: bool,
    /// "HH:MM", 24-hour.
    pub time: String,
    pub time_zone: Option<String>,
    pub harness_type: Option<String>,
    /// `provider/model`.
    pub model: Option<String>,
    pub budget_tokens: i64,
    pub next_run_at: Option<String>,
    /// Dreams from a night that finished in the last 24 hours, newest first.
    pub recent: Vec<Dream>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn parse_nightly(v: &Value) -> Option<Nightly> {
    v.as_object()?;
    Some(Nightly {
        enabled: v.get("enabled").and_then(Value::as_bool) == Some(true),
        automation_id: text(v, "automationId"),
        paused: v.get("paused").and_then(Value::as_bool) == Some(true),
        time: text(v, "time").unwrap_or_default(),
        time_zone: text(v, "timeZone"),
        harness_type: text(v, "harnessType"),
        model: text(v, "model"),
        budget_tokens: v.get("budgetTokens").and_then(Value::as_f64).map_or(0, |n| n as i64),
        next_run_at: text(v, "nextRunAt"),
        recent: v.get("recent").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_dream).collect()).unwrap_or_default(),
    })
}

// ─── What Settings changes ──────────────────────────────────────────────────

/// `NightlyDreamingChange`: only what is set goes out. `model: Some(None)` clears the model back to the harness's own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Change {
    pub enabled: Option<bool>,
    pub time: Option<String>,
    pub time_zone: Option<String>,
    pub harness_type: Option<String>,
    pub model: Option<Option<String>>,
    pub budget_tokens: Option<i64>,
}

impl Change {
    /// `setNightlyDreaming`'s body: a cleared model goes out as `""`, because the server reads that as "back to default" and a
    /// missing one as "unchanged".
    pub fn body(&self) -> Value {
        let mut body = serde_json::Map::new();
        if let Some(on) = self.enabled {
            body.insert("enabled".into(), json!(on));
        }
        if let Some(time) = &self.time {
            body.insert("time".into(), json!(time));
        }
        if let Some(zone) = &self.time_zone {
            body.insert("timeZone".into(), json!(zone));
        }
        if let Some(harness) = &self.harness_type {
            body.insert("harnessType".into(), json!(harness));
        }
        if let Some(model) = &self.model {
            body.insert("model".into(), json!(model.as_deref().unwrap_or("")));
        }
        if let Some(budget) = self.budget_tokens {
            body.insert("budgetTokens".into(), json!(budget));
        }
        Value::Object(body)
    }
}

const PATH: &str = "/api/memory/dreaming/nightly";

pub fn get_nightly() -> Request {
    Request { method: "GET", path: PATH.into(), body: None, fallback: "Couldn't load nightly dreaming.".into() }
}

pub fn set_nightly(change: &Change) -> Request {
    Request { method: "PUT", path: PATH.into(), body: Some(change.body()), fallback: "Couldn't save nightly dreaming.".into() }
}

/// The machine's IANA zone ("Europe/Paris"), as the server wants it with a time (`localTimeZone`). Unix names it in `TZ` or in
/// the `/etc/localtime` link; Windows keeps its own names, so there none is sent and the server keeps the zone it has.
pub fn local_time_zone() -> Option<String> {
    let zone = |name: &str| Some(name.trim_start_matches(':').to_string()).filter(|n| n.contains('/') || n == "UTC");
    if let Some(name) = std::env::var("TZ").ok().and_then(|n| zone(&n)) {
        return Some(name);
    }
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let target = target.to_string_lossy();
    zone(target.split("zoneinfo/").nth(1)?)
}

// ─── Choices ────────────────────────────────────────────────────────────────

/// "Up to 300k tokens a night".
pub fn budget_label(tokens: i64) -> String {
    format!("Up to {} tokens a night", crate::composer::format::format_tokens(Some(tokens as f64)))
}

/// The harnesses a night can dream on: those enabled, limited to the two it knows. `enabled` are `(type, display name)`.
pub fn harness_choices(enabled: &[(String, String)]) -> Vec<(String, String)> {
    enabled.iter().filter(|(kind, _)| HARNESSES.contains(&kind.as_str())).cloned().collect()
}

/// The harness the select shows: the saved one if offered, else OpenCode 2 if offered, else the first, else none.
pub fn shown_harness(saved: Option<&str>, choices: &[(String, String)]) -> String {
    if let Some(saved) = saved.filter(|s| choices.iter().any(|(k, _)| k == s)) {
        return saved.to_string();
    }
    choices.iter().find(|(k, _)| k == "opencode2").or(choices.first()).map(|(k, _)| k.clone()).unwrap_or_default()
}

/// "04:30" from what was typed: 24-hour digits, with or without the leading zero or the colon. None when it is not a time.
pub fn parse_time(typed: &str) -> Option<String> {
    let typed = typed.trim();
    let (h, m) = match typed.split_once(':') {
        Some((h, m)) => (h, m),
        None if typed.len() == 4 => typed.split_at(2),
        None => return None,
    };
    let digits = |s: &str, n: usize| s.len() <= n && !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if !digits(h, 2) || m.len() != 2 || !digits(m, 2) {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then(|| format!("{h:02}:{m:02}"))
}

/// Said under the time box when what was typed is not a time.
pub const BAD_TIME: &str = "Use the 24-hour form, like 03:30.";

// ─── The row's state ────────────────────────────────────────────────────────

/// What the row holds while Settings → Memory is open (`DreamNightly.vue`'s refs). It lives in the page, so it goes with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NightlyPage {
    pub nightly: Option<Nightly>,
    pub error: Option<String>,
    /// A change is on its way (`isSaving`): one at a time.
    pub saving: bool,
    /// A read is on its way.
    pub reading: bool,
    /// The read of the page the row last asked for.
    pub asked: Option<u64>,
    /// The time box and the time it was last set from.
    pub time: TextEdit,
    pub time_shown: String,
    pub time_was_focused: bool,
    pub time_error: bool,
    /// The harness whose models the picker lists, and what it said (`useHarnessCatalog`).
    pub catalog_for: Option<String>,
    pub catalog: Option<Catalog>,
}

impl Eq for NightlyPage {}

impl NightlyPage {
    /// The row shows once the server has answered; the answer is dropped when Memory goes off.
    pub fn shows(&self) -> bool {
        self.nightly.is_some()
    }

    /// With no answer, the failed read says why and offers Try again (weave-fleet #1051).
    pub fn shows_error(&self) -> bool {
        self.nightly.is_none() && self.error.is_some()
    }

    /// The harness the row dreams on, which Dream now follows: only while the row is switched on.
    pub fn harness(&self, choices: &[(String, String)]) -> Option<String> {
        let n = self.nightly.as_ref().filter(|n| n.enabled)?;
        Some(shown_harness(n.harness_type.as_deref(), choices)).filter(|h| !h.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_server_s_answer_with_the_dreams_of_last_night() {
        let n = parse_nightly(&json!({ "enabled": true, "automationId": "a1", "paused": true, "time": "03:30", "timeZone": "Europe/Paris",
            "harnessType": "claude-code", "model": "anthropic/sonnet", "budgetTokens": 300000, "nextRunAt": null,
            "recent": [{ "id": "d1", "status": "done", "runId": "r1" }, { "nope": 1 }] }))
        .expect("an answer");
        assert!(n.enabled && n.paused);
        assert_eq!((n.time.as_str(), n.budget_tokens), ("03:30", 300_000));
        assert_eq!(n.automation_id.as_deref(), Some("a1"));
        assert_eq!(n.recent.len(), 1);
        assert!(parse_nightly(&json!("no")).is_none());
        assert!(!parse_nightly(&json!({})).unwrap().enabled);
    }

    #[test]
    fn sends_only_what_changed_and_a_cleared_model_as_an_empty_one() {
        let on = Change { enabled: Some(true), time_zone: Some("Europe/Paris".into()), ..Default::default() };
        assert_eq!(on.body(), json!({ "enabled": true, "timeZone": "Europe/Paris" }));
        let harness = Change { harness_type: Some("claude-code".into()), model: Some(None), ..Default::default() };
        assert_eq!(harness.body(), json!({ "harnessType": "claude-code", "model": "" }));
        let model = Change { model: Some(Some("anthropic/sonnet".into())), ..Default::default() };
        assert_eq!(model.body(), json!({ "model": "anthropic/sonnet" }));
        assert_eq!(Change { budget_tokens: Some(1_000_000), ..Default::default() }.body(), json!({ "budgetTokens": 1000000 }));
        assert_eq!(set_nightly(&on).fallback, "Couldn't save nightly dreaming.");
        assert_eq!(get_nightly().fallback, "Couldn't load nightly dreaming.");
    }

    #[test]
    fn labels_the_four_budgets() {
        let labels: Vec<String> = BUDGETS.iter().map(|b| budget_label(*b)).collect();
        assert_eq!(
            labels,
            ["Up to 100k tokens a night", "Up to 300k tokens a night", "Up to 500k tokens a night", "Up to 1M tokens a night"]
        );
    }

    #[test]
    fn offers_only_the_two_harnesses_and_shows_the_saved_one_when_offered() {
        let all = |kinds: &[&str]| kinds.iter().map(|k| (k.to_string(), k.to_uppercase())).collect::<Vec<_>>();
        let choices = harness_choices(&all(&["pi", "claude-code", "opencode2", "codex"]));
        assert_eq!(choices.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["claude-code", "opencode2"]);
        assert_eq!(shown_harness(Some("claude-code"), &choices), "claude-code");
        assert_eq!(shown_harness(Some("pi"), &choices), "opencode2");
        assert_eq!(shown_harness(None, &choices), "opencode2");
        let only = harness_choices(&all(&["claude-code"]));
        assert_eq!(shown_harness(Some("opencode2"), &only), "claude-code");
        assert_eq!(shown_harness(None, &[]), "");
    }

    #[test]
    fn takes_a_time_in_24_hour_digits_and_says_no_to_the_rest() {
        assert_eq!(parse_time("04:30").as_deref(), Some("04:30"));
        assert_eq!(parse_time(" 4:05 ").as_deref(), Some("04:05"));
        assert_eq!(parse_time("0430").as_deref(), Some("04:30"));
        assert_eq!(parse_time("23:59").as_deref(), Some("23:59"));
        for bad in ["", "24:00", "12:60", "7", "12:5", "ab:cd", "1:2:3", "4:30pm", "-1:30"] {
            assert_eq!(parse_time(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn follows_the_row_s_harness_only_while_it_is_switched_on() {
        let choices = vec![("opencode2".to_string(), "OpenCode 2".to_string())];
        let mut page = NightlyPage::default();
        assert_eq!(page.harness(&choices), None);
        page.nightly = parse_nightly(&json!({ "enabled": false }));
        assert_eq!(page.harness(&choices), None);
        page.nightly = parse_nightly(&json!({ "enabled": true }));
        assert_eq!(page.harness(&choices).as_deref(), Some("opencode2"));
        assert_eq!(page.harness(&[]), None);
    }

    #[test]
    fn a_failed_read_with_no_answer_shows_its_error_and_an_answer_shows_the_row() {
        let mut page = NightlyPage { error: Some("Nightly dreaming is unavailable.".into()), ..Default::default() };
        assert!(page.shows_error() && !page.shows());
        page.nightly = parse_nightly(&json!({ "enabled": false }));
        assert!(!page.shows_error() && page.shows());
    }
}
