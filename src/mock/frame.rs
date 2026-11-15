//! The "message frame" scenes (piece 3E): a prompt with attached files, a slash command, a steered prompt and one a mod
//! sent, written as the server sends them and boiled down by the code a live conversation goes through. Times are
//! relative to `NOW`, which a scene fixes, so the pills read the same in every picture.
use super::tools::{conversation, message};
use crate::State;
use serde_json::{Value, json};

/// The scene's "now", in unix milliseconds (2025-05-19 23:18 UTC).
pub const NOW: i64 = 1_747_696_680_000;

const MINUTE: i64 = 60_000;

fn text(id: &str, body: &str) -> Value {
    json!({"type": "text", "id": id, "text": body})
}

fn with_info(mut m: Value, extra: Value) -> Value {
    m["info"].as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    m
}

const LOG: &str = "Attached: .weave/attachments/3f9a2c1d/build-output.log (2,031 lines, 184 KB)";
const PDF: &str = "Attached: .weave/attachments/3f9a2c1d/incident-report.pdf (PDF, 412 KB)";
const TEMPLATE: &str = "Tidy up the code in src/auth without changing what it does.\n\n1. Read every file in that area.\n2. Remove dead code and rename what is unclear.\n3. Run the tests and report what changed.";

pub fn of_frame() -> Vec<crate::view::conversation::Message> {
    let at = |minutes: i64| NOW - minutes * MINUTE;
    conversation(vec![
        message(
            "u1",
            "user",
            at(14),
            vec![text("u1-t", &format!("Why does the build fail on the second run?\n\n{LOG}\n{PDF}\nRead them as needed."))],
        ),
        with_info(
            message(
                "a1",
                "assistant",
                at(13),
                vec![text(
                    "a1-t",
                    "The second run reuses a stale lock file, so the cache step stops before it writes anything. Removing the lock at the start of the build fixes it.",
                )],
            ),
            json!({"modelID": "model-one"}),
        ),
        with_info(
            message("u2", "user", at(9), vec![text("u2-t", TEMPLATE)]),
            json!({"command": {"name": "tidy", "arguments": "src/auth"}}),
        ),
        with_info(message("u3", "user", at(4), vec![text("u3-t", "Also check the retry path.")]), json!({"steered": true})),
        with_info(
            message("u4", "user", at(2), vec![text("u4-t", "Run the tests again and tell me what failed.")]),
            json!({"mod": {"kind": "sent", "name": "Fleet Nudge"}}),
        ),
        with_info(
            message("a2", "assistant", at(1), vec![text("a2-t", "Two tests failed: both wait for a lock the build no longer creates.")]),
            json!({"modelID": "model-one"}),
        ),
    ])
}

impl State {
    /// The frame scenes: the first session shows `of_frame` at a fixed time.
    pub fn show_frame(&mut self) {
        self.now_ms = Some(NOW);
        self.show_conversation(of_frame());
    }
}
