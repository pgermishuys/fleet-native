//! The "turn failure" scenes (piece 3H): a turn that stopped on a failure, a limit with Fleet's retry waiting, and the
//! bar's states. The scripted model has no failing keyword, so these are written as a server would send them (invented
//! messages) and drawn through the reducer, as the app draws a session from a server.
use super::tools::message;
use crate::State;
use crate::fleet::reducer;
use crate::fleet::rows::ScheduledRetry;
use crate::mock::Status;
use serde_json::{Value, json};

/// 7 October 2026, 01:51:43 UTC: the clock the scenes fix.
pub const NOW: i64 = 1_791_337_903_000;

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn failed(id: &str, created: i64, body: Option<&str>, error: Value) -> Value {
    let mut m = message(id, "assistant", created, body.map(|b| vec![text(&format!("{id}-t"), b)]).unwrap_or_default());
    m["info"]["turnError"] = error;
    m["info"]["agent"] = json!("build");
    m
}

fn user(id: &str, created: i64, body: &str) -> Value {
    message(id, "user", created, vec![text(&format!("{id}-t"), body)])
}

const STACK: &str = "ProviderModelNotFoundError: Model not found: acme/fast-1. Did you mean: acme/fast-2? at <anonymous> (/$bunfs/root/chunk-a1.js:439:94278) at SessionPrompt.getModel (/$bunfs/root/chunk-b2.js:1142:11505)";

/// Which failure a scene shows.
#[derive(Clone, Copy)]
pub enum Case {
    /// A harness error with its stack behind Details.
    Stopped,
    /// A plain failure, as a provider's 500 leaves it.
    Plain,
    /// A usage limit, Fleet's retry waiting for the reset.
    Usage,
    /// A rate limit on the third attempt, 40 seconds off.
    RateLimited,
    /// A limit whose Try now failed.
    ActionFailed,
    /// A failure followed by a later turn.
    Earlier,
}

impl State {
    pub fn show_failure(&mut self, case: Case, details_open: bool) {
        self.open = (0, 0);
        let error = |name: &str, message: &str, kind: Option<&str>| json!({"name": name, "message": message, "isRetryable": kind.is_some(), "kind": kind});
        let ask = user("u1", 1_000, "Add cursor paging to the order list.");
        let messages = match case {
            Case::Stopped => vec![
                ask,
                failed(
                    "a1",
                    2_000,
                    Some("I read the list endpoint and started on the cursor."),
                    error("ProviderModelNotFoundError", STACK, None),
                ),
            ],
            Case::Plain => vec![
                user("u1", 1_000, "fail-error Add cursor paging to the order list."),
                failed("a1", 2_000, None, error("APIError", "The provider could not finish this request.", None)),
            ],
            Case::Usage => vec![
                user("u1", 1_000, "fail-limit please"),
                failed(
                    "a1",
                    2_000,
                    None,
                    error(
                        "APIError",
                        "You've hit your usage limit. It resets in 2 hours. Manage your plan at https://example.test/plan.",
                        Some("usage_limit"),
                    ),
                ),
            ],
            Case::RateLimited | Case::ActionFailed => vec![
                ask,
                failed("a1", 2_000, None, error("RateLimitError", "Too many requests. Try again in a minute.", Some("rate_limit"))),
            ],
            Case::Earlier => vec![
                ask,
                failed(
                    "a1",
                    2_000,
                    Some("I read the list endpoint and started on the cursor."),
                    error("ProviderModelNotFoundError", STACK, None),
                ),
                user("u2", 3_000, "Try again with the other model."),
                message("a2", "assistant", 4_000, vec![text("a2-t", "Done: the list takes a `cursor` and returns `next_cursor`.")]),
            ],
        };
        let state = reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"}));
        let due = |ms: i64| chrono_free_iso(NOW + ms);
        let retry = match case {
            Case::Usage => Some(ScheduledRetry {
                due_at: due(119 * 60_000),
                attempt: 1,
                kind: "usage_limit".into(),
                reason: String::new(),
                provider_said: true,
            }),
            Case::RateLimited | Case::ActionFailed => Some(ScheduledRetry {
                due_at: due(40_000),
                attempt: 3,
                kind: "rate_limit".into(),
                reason: String::new(),
                provider_said: false,
            }),
            _ => None,
        };
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Idle;
            session.row.scheduled_retry = retry;
        }
        if matches!(case, Case::ActionFailed) {
            self.retry_acts.insert(
                "mock.0.0".into(),
                crate::fleet::retry::Act { busy: false, error: Some("Fleet couldn't try again now (HTTP 500).".into()) },
            );
        }
        self.now_ms = Some(NOW);
        if details_open {
            self.failure_open.insert("a1".into());
        }
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
    }
}

/// Unix milliseconds as the ISO text the server writes (`2026-10-07T03:43:43.000Z`).
pub fn chrono_free_iso(ms: i64) -> String {
    let (secs, millis) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let days = secs.div_euclid(86_400);
    let (h, mi, s) = (secs.rem_euclid(86_400) / 3600, secs.rem_euclid(3600) / 60, secs.rem_euclid(60));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z")
}
