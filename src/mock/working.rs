//! The "working line" and "interrupted card" scenes (piece 3I): a turn on a long tool call, one held for a steered
//! message, a question holding the turn, a retry counting down, a machine that stopped answering, and the card a restart
//! leaves. Written as a server sends them and drawn through the reducer, with the clock fixed so a picture reads the same
//! every time.
use super::failure::{NOW, chrono_free_iso};
use super::tools::{call, message};
use crate::State;
use crate::fleet::reducer;
use crate::fleet::rows::{Interruption, Retrying};
use crate::mock::Status;
use serde_json::{Value, json};

/// Which state a scene shows.
#[derive(Clone, Copy)]
pub enum Case {
    /// A turn 12 minutes old, running a shell command for 10 of them.
    Tool,
    /// The same, with two more calls running and a steered message waiting below.
    Held,
    /// A turn 14 seconds old that has no call running yet.
    Fresh,
    /// A question holds the turn.
    Waiting,
    /// A failed model call, the third attempt of ten, 12 seconds off.
    Retry,
    /// The session's machine stopped answering.
    Quiet,
}

/// Which card a scene shows.
#[derive(Clone, Copy)]
pub enum Cut {
    Working,
    Waiting,
    Auto,
    Failed,
}

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn minutes(n: i64) -> i64 {
    NOW - n * 60_000
}

impl State {
    fn show_stream(&mut self, messages: Vec<Value>, status: Status, activity: &str) {
        self.set_open((0, 0));
        let state = reducer::create_state(&json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": activity}));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = status;
            session.row.activity = activity.into();
        }
        self.now_ms = Some(NOW);
        self.stream = Some(state);
        self.stream_of = None;
        self.sync_open();
    }

    pub fn show_working(&mut self, case: Case) {
        let ask = |at: i64| message("u1", "user", at, vec![text("u1-t", "Run the order tests and fix whatever breaks.")]);
        let said = message("a0", "assistant", minutes(12) + 2_000, vec![text("a0-t", "I'll run the suite first and read what fails.")]);
        let running = |id: &str, tool: &str, input: Value, ago: i64| {
            let mut part = call(id, tool, "running", input, json!({}));
            part["state"]["time"] = json!({"start": minutes(ago)});
            part
        };
        let (messages, status, activity) = match case {
            Case::Tool | Case::Quiet | Case::Retry | Case::Waiting => (
                vec![
                    ask(minutes(12)),
                    said,
                    message(
                        "a1",
                        "assistant",
                        minutes(10),
                        vec![running("c1", "bash", json!({"command": "bun test src/orders", "description": "Run the order tests"}), 10)],
                    ),
                ],
                if matches!(case, Case::Waiting) { Status::NeedsInput } else { Status::Working },
                match case {
                    Case::Retry => "retry",
                    Case::Waiting => "waiting_input",
                    _ => "busy",
                },
            ),
            Case::Held => {
                let mut steered = message("u2", "user", minutes(5), vec![text("u2-t", "Skip the slow ones.")]);
                steered["info"]["steered"] = json!(true);
                (
                    vec![
                        ask(minutes(12)),
                        said,
                        message(
                            "a1",
                            "assistant",
                            minutes(10),
                            vec![
                                running("c1", "bash", json!({"command": "bun test src/orders"}), 10),
                                running("c2", "webfetch", json!({"url": "https://example.test/docs"}), 4),
                                running("c3", "grep", json!({"pattern": "applyDiscount"}), 1),
                            ],
                        ),
                        steered,
                    ],
                    Status::Working,
                    "busy",
                )
            }
            Case::Fresh => (vec![ask(NOW - 14_000)], Status::Working, "busy"),
        };
        self.show_stream(messages, status, activity);
        if matches!(case, Case::Held) {
            self.pickers.harnesses = vec![crate::composer::status::HarnessInfo {
                kind: "opencode2".into(),
                supports_steering: true,
                steers_by_default: true,
                steer_delivery: Some(crate::composer::status::SteerDelivery::NextStep),
                ..Default::default()
            }];
            if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
                session.row.harness_type = Some("opencode2".into());
            }
        }
        if matches!(case, Case::Retry)
            && let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut())
        {
            session.row.retrying = Retrying {
                attempt: Some(3),
                max_attempts: Some(10),
                message: Some("API overloaded (529)".into()),
                next: Some(chrono_free_iso(NOW + 12_000)),
            };
        }
        if matches!(case, Case::Quiet) {
            self.quiet_machine = Some("studio".into());
        }
    }

    pub fn show_interrupted(&mut self, cut: Cut) {
        let messages = vec![
            message("u1", "user", minutes(30), vec![text("u1-t", "Run the order tests and fix whatever breaks.")]),
            message("a1", "assistant", minutes(29), vec![text("a1-t", "I'll run the suite first and read what fails.")]),
        ];
        let waiting = matches!(cut, Cut::Waiting);
        self.show_stream(messages, if waiting { Status::NeedsInput } else { Status::Idle }, if waiting { "waiting_input" } else { "idle" });
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Idle;
            session.row.activity = "idle".into();
            session.row.interrupted = Some(Interruption {
                at: chrono_free_iso(minutes(8)),
                waiting,
                last_call: (!waiting).then(|| "bash: bun test src/orders".into()),
                changed_files: (!waiting).then_some(3),
                auto: matches!(cut, Cut::Auto),
            });
        }
        if matches!(cut, Cut::Failed) {
            self.interrupt_acts.insert(
                "mock.0.0".into(),
                crate::fleet::retry::Act { busy: false, error: Some("Fleet couldn't continue the session (HTTP 500).".into()) },
            );
        }
    }
}

impl State {
    /// Messages other sessions sent, as the chip above each: a session on falcon (a machine this app lists), an update that
    /// finished, one that failed, and a sender on a machine that isn't listed. `show_machines_only` first, so falcon is known.
    pub fn show_peer_messages(&mut self) {
        self.show_machines_only(super::machines::Case::Several);
        let tag = |kind: &str, from: &str, title: &str, extra: &str, text: &str| {
            format!("<fleet-session-{kind} {from} title=\"{title}\"{extra}>\n{text}\n</fleet-session-{kind}>")
        };
        let messages = vec![
            message(
                "u1",
                "user",
                minutes(9),
                vec![text(
                    "u1-t",
                    &tag(
                        "message",
                        "from=\"ses_peer1\"",
                        "Fix the login flake",
                        " machine=\"m-falcon\" machine-name=\"falcon\"",
                        "The installer test fails about one run in ten. Could you check the timeout?",
                    ),
                )],
            ),
            message("a1", "assistant", minutes(8), vec![text("a1-t", "I'll look at the timeout in the installer test.")]),
            message(
                "u2",
                "user",
                minutes(5),
                vec![text(
                    "u2-t",
                    &tag("update", "session=\"ses_peer2\"", "Update the docs", " outcome=\"finished\"", "Docs updated; two pages changed."),
                )],
            ),
            message(
                "u3",
                "user",
                minutes(3),
                vec![text(
                    "u3-t",
                    &tag(
                        "update",
                        "session=\"ses_peer3\"",
                        "Run the tests",
                        " outcome=\"failed\" machine=\"m-unlisted\" machine-name=\"harbor-two\"",
                        "Rate limited.",
                    ),
                )],
            ),
        ];
        self.show_stream(messages, Status::Idle, "idle");
    }
}
