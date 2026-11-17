//! The Agents tab's scenes (piece 10L): what the scripted model's `spawn-agent` and `background-work` leave, a session with a
//! fork, one that was forked, and every ended state. Written as the server sends it, with the clock fixed.
use super::failure::{NOW, chrono_free_iso};
use crate::State;
use crate::fleet::canvases::CanvasKind;
use crate::fleet::work::to_items;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Something running with its detail open, one waiting, one started and one ended.
    Main,
    /// The Earlier fold open: done, failed, lost and stopped, with a done one's detail.
    Earlier,
    /// The open session is a fork: a Parent row, nothing else.
    Parent,
    /// A Stop the server refused, under the running row's detail.
    Refused,
    /// Work that runs inside its call: no session, no Stop.
    Inside,
    /// Names and tasks far longer than the column.
    Long,
    Empty,
}

fn ago(secs: i64) -> String {
    chrono_free_iso(NOW - secs * 1000)
}

fn item(id: &str, title: &str, label: &str, started: i64, extra: Value) -> Value {
    let mut v = json!({
        "id": id, "sessionId": "scene", "workId": format!("w-{id}"), "kind": "subagent", "title": title, "label": label,
        "status": "running", "background": true, "startedAt": ago(started),
    });
    v.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    v
}

fn ended(mut v: Value, since: i64, status: &str, reason: &str, detail: Option<&str>) -> Value {
    v["status"] = json!(status);
    v["endedReason"] = json!(reason);
    v["endedAt"] = json!(ago(since));
    if let Some(d) = detail {
        v["detail"] = json!(d);
    }
    v
}

impl State {
    pub fn show_agents_tab(&mut self, case: Case, dark: bool) {
        self.show_chips(super::context::Case::Chips, dark);
        self.sidebar_open = true;
        // The invented list has two sessions here; the scene adds the ones it needs.
        let titles = ["Fork: try a smaller page size", "Draft the release notes", "Tidy the old fixtures"];
        for (i, title) in titles.iter().enumerate() {
            self.projects[0].sessions.push(crate::mock::Session {
                id: format!("scene-extra-{i}"),
                title: (*title).into(),
                status: crate::mock::Status::Idle,
                ago_secs: Some(600),
                branch: String::new(),
                messages: Vec::new(),
                tokens: 0,
                row: Default::default(),
                listing: Default::default(),
            });
        }
        let ids: Vec<String> = self.projects[0].sessions.iter().map(|s| s.id.clone()).collect();
        let this = if case == Case::Parent { ids[2].clone() } else { ids[0].clone() };
        let child = |s: &mut crate::mock::Session, kind: &str, parent: &str, lifecycle: &str, activity: &str| {
            s.listing.spawn_kind = Some(kind.into());
            if kind == "fork" {
                s.listing.forked_from = Some(parent.into());
            } else {
                s.listing.spawned_by = Some(parent.into());
            }
            s.row.lifecycle = lifecycle.into();
            s.row.activity = activity.into();
        };
        let mut work = vec![];
        match case {
            Case::Main => {
                child(&mut self.projects[0].sessions[2], "fork", &ids[0], "active", "busy");
                child(&mut self.projects[0].sessions[3], "agent", &ids[0], "waiting_input", "waiting_input");
                child(&mut self.projects[0].sessions[4], "agent", &ids[0], "idle", "idle");
                work.push(item("a91d", "general", "Migrate the cache", 38, json!({"childSessionId": "child-1", "canStop": true})));
                work.push(ended(
                    item("a1", "code-reviewer", "Review the diff", 150, json!({"childSessionId": "child-2"})),
                    90,
                    "completed",
                    "completed",
                    None,
                ));
            }
            Case::Earlier => {
                work.push(ended(
                    item("e1", "general", "Check the rounding rules", 400, json!({"childSessionId": "c1"})),
                    340,
                    "completed",
                    "completed",
                    Some("2 files read"),
                ));
                work.push(ended(
                    item("e2", "code-reviewer", "Review the diff", 300, json!({})),
                    280,
                    "error",
                    "error",
                    Some("the model refused"),
                ));
                work.push(ended(item("e3", "general", "Warm the cache", 250, json!({})), 200, "error", "lost", None));
                work.push(ended(item("e4", "general", "Index the docs", 160, json!({})), 120, "cancelled", "cancelled", None));
                work.push(ended(item("e5", "general", "Say hello", 100, json!({})), 100, "completed", "completed", None));
                self.agents_tab.selected = Some("work:e1".into());
                self.agents_tab.earlier_open = true;
            }
            Case::Refused => {
                work.push(item("a91d", "general", "Migrate the cache", 95, json!({"childSessionId": "child-1", "canStop": true})));
                self.work.stop_errors.insert("a91d".into(), "The session isn't running.".into());
            }
            Case::Inside => {
                work.push(item("p1", "reviewer", "Review the diff", 5, json!({"background": false, "detail": "step 1 of 2"})));
                self.agents_tab.set_output("p1", None);
            }
            Case::Long => {
                work.push(item(
                    "l1",
                    "an-agent-with-a-very-long-name-that-never-fits-in-the-column",
                    "Go through every call site of the pagination helper and report each place that still passes a page size by hand",
                    70,
                    json!({"childSessionId": "child-1", "canStop": true}),
                ));
                child(&mut self.projects[0].sessions[3], "agent", &ids[0], "idle", "idle");
                self.projects[0].sessions[3].title = "A session whose title also runs on well past the edge of the tab".into();
            }
            Case::Parent => {
                child(&mut self.projects[0].sessions[2], "fork", &ids[0], "idle", "idle");
            }
            Case::Empty => {}
        }
        for w in &mut work {
            w["sessionId"] = json!(this);
        }
        self.agents_history.insert(this.clone(), to_items(Some(&Value::Array(work))));
        self.now_ms = Some(NOW);
        self.set_open(if case == Case::Parent { (0, 2) } else { (0, 0) });
        self.edit_canvases(|c, id| {
            c.introduce(id, CanvasKind::Agents);
            c.open(id, CanvasKind::Agents);
        });
        self.set_content_open(true);
        let mut tx = rune::Tx::default();
        self.keep_agents_tab(&mut tx);
    }
}
