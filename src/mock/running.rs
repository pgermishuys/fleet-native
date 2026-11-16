//! The running-work counter's scenes (component 9, piece H): work running in three sessions, written as the server sends it
//! and put in the list's own store, with the clock fixed so a picture reads the same every time. The scripted model's
//! `background-work` job lasts about 30 s and a software renderer outruns it, so a live picture can't hold a state: these
//! stand in for it, and `fleet-running-*` are Fleet's own pictures of the same states.
use super::failure::{NOW, chrono_free_iso};
use crate::State;
use crate::fleet::work::to_item;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq)]
pub enum Case {
    /// One shell in the open session: "1 running in 1 session".
    One,
    /// Work in three sessions, the open one among them.
    Many,
    /// A session whose title is longer than the popover.
    LongTitle,
}

fn ago(secs: i64) -> String {
    chrono_free_iso(NOW - secs * 1000)
}

fn item(id: &str, session: &str, kind: &str, title: &str, label: &str, started: i64, extra: Value) -> Value {
    let mut v = json!({
        "id": id, "sessionId": session, "workId": format!("w-{id}"), "kind": kind, "title": title, "label": label,
        "status": "running", "background": true, "startedAt": ago(started),
    });
    v.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    v
}

impl State {
    /// The counter on `case`; with `open` the popover is up too.
    pub fn show_running(&mut self, case: Case, open: bool) {
        let titles = ["Speed up the search index", "Move the billing job to the queue", "Tidy the onboarding copy"];
        let long = "Rework the nightly import so a partial failure no longer re-reads every file in the archive";
        for (i, title) in titles.iter().enumerate() {
            self.set_session_id((0, i), &format!("s{}", i + 1));
            let title = if case == Case::LongTitle && i == 1 { long } else { title };
            let session = &mut self.projects[0].sessions[i];
            session.title = title.to_string();
            session.listing.workspace_display_name = Some(["demo-app", "demo-api", "demo-site"][i].into());
        }
        self.set_open((0, 0));
        let work = match case {
            Case::One => {
                vec![item("w1", "s1", "shell", "shell", "npm run dev -- --port 5174", 192, json!({"canStop": true, "canReadOutput": true}))]
            }
            Case::Many | Case::LongTitle => vec![
                item("w1", "s1", "shell", "shell", "npm run dev -- --port 5174", 192, json!({"canStop": true, "canReadOutput": true})),
                item("w2", "s2", "subagent", "code-reviewer", "Review the diff", 92, json!({"childSessionId": "child-1", "canStop": true})),
                item("w3", "s1", "shell", "shell", "npm run test:e2e", 41, json!({"canStop": true, "canReadOutput": true})),
                item("w4", "s3", "shell", "shell", "python -m http.server 8000", 20, json!({"canStop": true, "canReadOutput": true})),
            ],
        };
        self.now_ms = Some(NOW);
        self.list_work = Default::default();
        for v in &work {
            if let Some(item) = to_item(v) {
                self.list_work.apply(item);
            }
        }
        self.ui.open = open.then_some(crate::ui::Open::Popover("running"));
    }
}
