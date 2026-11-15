//! The words of the working line and of the card a restart leaves (`WorkingIndicator.vue`, `InterruptedCard.vue`,
//! `lib/running-work.ts`, `lib/working-on.ts`, `lib/retry-status.ts`, `lib/session-interruption.ts`). No Rune in here.
use super::retry::{format_countdown, minutes_text};
use super::tools::get_tool;
use crate::fleet::model::Msg;
use crate::fleet::rows::{Interruption, Retrying};
use crate::fleet::work::parse_iso_ms;
use crate::view::conversation::Role;
use serde_json::Value;

/// `formatDuration(ms, { precision: "minutes" })`: "14 s", "1 min", "1 h 32 min". Never negative.
pub fn format_duration(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    if seconds < 60 { format!("{seconds} s") } else { minutes_text(ms) }
}

/// How often an elapsed time of `ms` needs to tick to stay right: every second in its first minute, then every
/// minute (`elapsedTickMs`).
pub fn elapsed_tick_ms(ms: i64) -> i64 {
    if ms < 60_000 { 1_000 } else { 60_000 }
}

/// When the running turn began: your last prompt that started it (`turnStartedAt`). A prompt sent into the running
/// turn (steered) is part of that turn, so it doesn't restart the clock.
pub fn turn_started_at(messages: &[Msg]) -> Option<i64> {
    messages.iter().rev().find(|m| m.role == Role::User && !m.steered)?.created_at
}

/// What "Working" is waiting on (`WorkingOn`).
#[derive(Clone, Debug, PartialEq)]
pub struct WorkingOn {
    /// The longest-running call's name, as its row shows it (Bash, Web Fetch).
    pub name: String,
    /// When it started, unix ms; `None` when the harness gave no start.
    pub since: Option<i64>,
    /// How many other calls run besides it.
    pub more: usize,
    /// Your steered message sits below the call and the harness reads it only when the call finishes.
    pub held: bool,
}

/// The tool call the turn is running (`workingOn`): the earliest started of the running ones. `next_step` is the
/// harness's `steerDelivery == "next-step"`.
pub fn working_on(messages: &[Msg], next_step: bool) -> Option<WorkingOn> {
    let mut running: Vec<(String, Option<i64>, usize)> = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if message.role != Role::Assistant {
            continue;
        }
        for part in &message.parts {
            let crate::fleet::model::PartData::Tool { tool, state, .. } = part else { continue };
            if state.get("status").and_then(Value::as_str).map(str::to_lowercase).as_deref() != Some("running") {
                continue;
            }
            // A harness that keeps no start on the call: the message that holds it began about then.
            let since = state.pointer("/time/start").and_then(Value::as_f64).map(|t| t as i64).or(message.created_at);
            running.push((get_tool(tool).heading, since, index));
        }
    }
    let mut first = running.first()?.clone();
    for item in &running[1..] {
        if item.1.unwrap_or(i64::MAX) < first.1.unwrap_or(i64::MAX) {
            first = item.clone();
        }
    }
    let held = next_step && messages.iter().enumerate().any(|(i, m)| i > first.2 && m.role == Role::User && m.steered);
    Some(WorkingOn { name: first.0, since: first.1, more: running.len() - 1, held })
}

/// "Bash running 10 min", "Bash running 10 min +2 more", and the held sentence after it (`workingOnText`).
pub fn working_on_text(work: &WorkingOn, now: i64) -> String {
    let time = work.since.map_or(String::new(), |s| format!(" {}", format_duration(now - s)));
    let more = if work.more > 0 { format!(" +{} more", work.more) } else { String::new() };
    let line = format!("{} running{time}{more}", work.name);
    if work.held { format!("{line}. Your message goes to the agent when it finishes.") } else { line }
}

/// What the working line says (`WorkingIndicator.vue`): the first of these that holds, in this order.
#[derive(Clone, Debug, PartialEq)]
pub enum Working {
    /// A question holds the turn: "Needs input" with the waiting diamond.
    Waiting,
    /// The session's machine went quiet mid-turn: its name, and no ticking time.
    NotAnswering(String),
    /// Waiting out a failed model call: the sentence.
    Retry(String),
    /// "Working", the turn's time ("14 s") and the running call's text, each only when known.
    Busy { elapsed: Option<String>, tool: Option<String> },
}

/// What the line is told (`WorkingIndicator`'s props).
#[derive(Default)]
pub struct Inputs<'a> {
    /// When the turn began (your last prompt), unix ms; the time is left out without it.
    pub since: Option<i64>,
    pub waiting: bool,
    pub retry: Option<&'a Retrying>,
    pub not_answering: Option<&'a str>,
    pub work: Option<&'a WorkingOn>,
}

/// Picks the line's variant and writes its words at `now`: waiting, then not answering, then retry, then working.
pub fn working_line(i: &Inputs, now: i64) -> Working {
    if i.waiting {
        Working::Waiting
    } else if let Some(name) = i.not_answering {
        Working::NotAnswering(name.to_string())
    } else if let Some(retry) = i.retry {
        Working::Retry(describe_retry(retry, now))
    } else {
        Working::Busy {
            elapsed: i.since.filter(|s| *s != 0).map(|s| format_duration(now - s)),
            tool: i.work.map(|w| working_on_text(w, now)),
        }
    }
}

/// When the line next changes, in ms: every second while a retry counts down or the clock is in its first minute, then
/// every minute (`tick`). The call's own clock replaces the turn's.
pub fn working_tick_ms(i: &Inputs, now: i64) -> i64 {
    let clock = i.work.and_then(|w| w.since).or(i.since).filter(|c| *c != 0);
    match clock {
        Some(c) if i.retry.is_none() => elapsed_tick_ms(now - c),
        _ => 1_000,
    }
}

/// "attempt 3 of 10", "attempt 3", or `None` (`retryAttemptLabel`).
pub fn retry_attempt_label(retry: &Retrying) -> Option<String> {
    let attempt = retry.attempt.filter(|a| *a != 0)?;
    Some(match retry.max_attempts.filter(|m| *m != 0) {
        Some(max) => format!("attempt {attempt} of {max}"),
        None => format!("attempt {attempt}"),
    })
}

/// "in 12 s", "in 2 min", "now", or `None` when the harness didn't say when (`retryCountdown`).
pub fn retry_countdown(retry: &Retrying, now: i64) -> Option<String> {
    let next = parse_iso_ms(retry.next.as_deref()?)?;
    Some(format_countdown(next - now))
}

/// "Retrying · attempt 3 of 10 · in 12 s · API overloaded (529)", leaving out what the harness didn't say
/// (`describeRetry`).
pub fn describe_retry(retry: &Retrying, now: i64) -> String {
    let message = retry.message.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(str::to_string);
    ["Retrying".to_string()]
        .into_iter()
        .chain(retry_attempt_label(retry))
        .chain(retry_countdown(retry, now))
        .chain(message)
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// "3 files changed in this worktree" (`changedFilesText`).
pub fn changed_files_text(count: i64) -> String {
    format!("{count} {} changed in this worktree", if count == 1 { "file" } else { "files" })
}

/// One paragraph of the card: its words, and which byte ranges of them are bold (`<b>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Para {
    pub text: String,
    pub bold: Vec<std::ops::Range<usize>>,
}

/// Builds a paragraph from pieces; a piece marked `true` is bold.
fn para(pieces: &[(&str, bool)]) -> Para {
    let mut out = Para { text: String::new(), bold: Vec::new() };
    for (piece, bold) in pieces {
        let start = out.text.len();
        out.text.push_str(piece);
        if *bold {
            out.bold.push(start..out.text.len());
        }
    }
    out
}

/// What the card says besides its title (`InterruptedCard.vue`), one entry per paragraph. `clock` is `formatWhen(at)`.
pub fn interruption_lines(i: &Interruption, clock: &str) -> Vec<Para> {
    if i.waiting {
        return vec![para(&[
            ("Fleet restarted while this was waiting for you (since ", false),
            (clock, true),
            ("). ", false),
            ("The question was lost.", true),
            (" Fleet doesn't answer for you.", false),
        ])];
    }
    let mut lines = vec![para(&[("Fleet restarted while this was working (since ", false), (clock, true), (").", false)])];
    if let Some(call) = &i.last_call {
        lines.push(para(&[("Last call that didn't finish: ", false), (call, true)]));
    }
    if let Some(n) = i.changed_files.filter(|n| *n > 0) {
        lines.push(para(&[(&format!("{}, still on disk.", changed_files_text(n)), false)]));
    }
    lines
}
