//! What the Agents tab keeps while it is mounted (`AgentsCanvas.vue`'s script): which row's detail is open, whether the Earlier
//! fold is open, the one read of a no-session item's output, and the tab's own life: introduced when something runs, its
//! badge, and its history read on show and on a session change. The words and the sections are `fleet::agents`.
use super::agents::{AgentActivity, AgentRow, AgentsLineage, DetailInput, OpenTarget, parent_row};
use super::canvases::CanvasKind;
use super::groups::{LineageKind, lineage_of};
use crate::State;
use rune::Tx;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct AgentsTab {
    /// The row picked, `""` once its detail was closed (`select`).
    pub selected: Option<String>,
    /// "Earlier agents" starts closed.
    pub earlier_open: bool,
    /// The last line of the output of work with no session, read once per item.
    outputs: HashMap<String, Option<String>>,
    /// The session the tab last asked its history for.
    pub(crate) loaded_for: Option<String>,
    /// What each row's own session showed (`useAgentActivity`), by the row's session id.
    activity: HashMap<String, Seen>,
}

/// How long a running row's session is left before it is read again. Fleet listens to the child's topic; native reads
/// `GET /api/sessions/{id}/messages` instead (no second hub subscription), so a running row is read again on this beat.
const REREAD_SECONDS: f64 = 3.0;

#[derive(Debug, Default)]
struct Seen {
    activity: Option<AgentActivity>,
    /// When the last answer arrived, on `State::clock`.
    at: f64,
    asking: bool,
}

impl AgentsTab {
    /// A scene or a test has this output read.
    pub fn set_output(&mut self, item: &str, line: Option<&str>) {
        self.outputs.insert(item.to_string(), line.map(str::to_string));
    }
}

/// The parent of a session and what it did, for the parent row (`lineageOf(self)`).
pub struct Parent {
    pub row: AgentRow,
    pub runs_on: Option<String>,
    pub note: &'static str,
}

impl State {
    fn listed(&self, id: &str) -> Option<&crate::mock::Session> {
        self.store.session_by_id(&self.projects, id)
    }

    /// The open session's Parent row, when the list says it came from another session (rows 5.12).
    pub fn agents_parent(&self, id: &str) -> Option<Parent> {
        let (parent_id, kind): (String, LineageKind) = lineage_of(&self.listed(id)?.listing)?;
        let listed = self.listed(&parent_id);
        let row = parent_row(&parent_id, kind, self.parent_title(&parent_id), listed);
        let models = self.turn_models();
        let runs_on = super::agents::parent_runs_on(row.session.as_ref(), &self.pickers.harnesses, models);
        Some(Parent { row, runs_on, note: super::agents::parent_note(kind) })
    }

    /// What a row says it runs on.
    pub fn agents_runs_on(&self, id: &str, row: &AgentRow) -> Option<String> {
        let own = self.listed(id).and_then(|s| s.row.harness_type.clone());
        let seen: HashMap<String, String> = row
            .session_id
            .as_ref()
            .and_then(|id| Some((row.key.clone(), self.agents_tab.activity.get(id)?.activity.as_ref()?.model_id.clone()?)))
            .into_iter()
            .collect();
        super::agents::runs_on(row, own.as_deref(), &seen, &self.pickers.harnesses, self.turn_models())
    }

    /// The model of the shown row's detail, from the work and the row's own session.
    pub fn agents_detail(&self, id: &str, row: &AgentRow) -> super::agents::Detail {
        let work_id = row.work.as_ref().map(|w| w.id.as_str());
        let output = work_id.and_then(|w| self.agents_tab.outputs.get(w)).and_then(|l| l.as_deref());
        super::agents::detail(&DetailInput {
            row,
            this_session: id,
            activity: row.session_id.as_ref().and_then(|id| self.agents_tab.activity.get(id)).and_then(|s| s.activity.as_ref()),
            output_line: output,
            stopping: work_id.is_some_and(|w| self.work.stopping.contains(w)),
            stop_error: work_id.and_then(|w| self.work.stop_errors.get(w)).map(String::as_str),
        })
    }

    /// Called every frame. Adds the tab the first time something runs or waits, keeps the badge's number, and while the tab is
    /// shown reads what it needs once: the history, the parent's title, a no-session item's output.
    pub fn keep_agents_tab(&mut self, tx: &mut Tx) {
        let session = self.canvas_session();
        let lineage = self.agents_lineage(&session);
        self.canvases.agents_running = lineage.active_count();
        if lineage.active_count() > 0 && !self.open_canvases().introduced.contains(&CanvasKind::Agents) {
            self.edit_canvases(|c, id| c.introduce(id, CanvasKind::Agents));
        }
        if !self.panel_body_shown(CanvasKind::Agents) {
            self.agents_tab.loaded_for = None;
            return;
        }
        if self.agents_tab.loaded_for.as_deref() != Some(session.as_str()) {
            self.agents_tab.loaded_for = Some(session.clone());
            self.load_agents_history(&session);
            if let Some(parent) = self.agents_parent(&session) {
                self.load_parent_title(parent.row.session_id.as_deref().unwrap_or_default());
            }
        }
        if let Some(row) = shown_row(&lineage, self.agents_tab.selected.as_deref()) {
            self.read_agent_output(&row);
            self.read_agent_session(&row, tx);
        }
    }

    /// 5.34: a row with a session of its own shows what that session did. Read when the detail first shows and, while the
    /// row still runs, again every few seconds.
    fn read_agent_session(&mut self, row: &AgentRow, tx: &mut Tx) {
        let Some(child) = row.session_id.clone() else { return };
        let running = row.state.is_active();
        let link = self.link_for(&child);
        let clock = self.clock;
        let seen = self.agents_tab.activity.entry(child.clone()).or_default();
        let due = seen.activity.is_none() || (running && clock - seen.at >= REREAD_SECONDS);
        if running && !seen.asking && !due {
            tx.wake_after(
                std::time::Duration::from_secs_f64(REREAD_SECONDS - (clock - seen.at)).max(std::time::Duration::from_millis(100)),
            );
        }
        if seen.asking || !due {
            return;
        }
        let Some(link) = link else { return };
        seen.asking = true;
        let asked = child.clone();
        link.off_thread(
            move |base| {
                let path = format!("/api/sessions/{}/messages?limit=200", crate::conv::export_history::encode_component(&asked));
                super::http::get_page(base, &path).ok().filter(|(status, _)| (200..300).contains(status)).and_then(|(_, body)| {
                    let body: serde_json::Value = serde_json::from_str(&body).ok()?;
                    let list = body.get("messages")?.as_array()?;
                    // A page is oldest first, as the summary reads it.
                    let messages: Vec<_> =
                        list.iter().filter_map(|m| crate::conv::export_history::convert_fleet_message(m, &asked)).collect();
                    Some(super::agents::summarize_agent_activity(&messages))
                })
            },
            move |s, _, activity| {
                let clock = s.clock;
                let seen = s.agents_tab.activity.entry(child).or_default();
                seen.asking = false;
                seen.at = clock;
                // A failed read still ends "Reading its session…": it would never end otherwise.
                seen.activity = activity.or_else(|| seen.activity.take()).or_else(|| Some(AgentActivity::default()));
            },
        );
    }

    /// 5.32: work with no session of its own, that the server can read, has its last output line read once.
    fn read_agent_output(&mut self, row: &AgentRow) {
        let Some(work) = row.work.as_ref().filter(|w| row.session_id.is_none() && w.can_read_output) else { return };
        if self.agents_tab.outputs.contains_key(&work.id) {
            return;
        }
        let Some(link) = self.link_for(&work.session_id) else { return };
        self.agents_tab.outputs.insert(work.id.clone(), None);
        let (session, id) = (work.session_id.clone(), work.id.clone());
        let asked = id.clone();
        link.off_thread(
            move |base| super::http::work_output(base, &session, &asked, 0),
            move |s, _, result| {
                let line = result
                    .ok()
                    .filter(|(status, _)| (200..300).contains(status))
                    .and_then(|(_, body)| body.get("output").and_then(|o| o.as_str()).and_then(super::agents::latest_output_line));
                s.agents_tab.outputs.insert(id, line);
            },
        );
    }

    /// A press on a row (`select`).
    pub fn select_agent(&mut self, key: &str) {
        let lineage = self.agents_lineage(&self.canvas_session());
        super::agents::select(&mut self.agents_tab.selected, &lineage, key);
    }

    /// Open session (rows 5.36). A fork or started session opens on its own; a child opens under this session.
    pub fn open_agent(&mut self, target: &OpenTarget, title: &str, tx: &mut Tx) {
        match target {
            OpenTarget::Child { child, parent, .. } => self.open_subagent_session(parent, child, title, tx),
            OpenTarget::Own { session, .. } => {
                let found =
                    self.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|s| s.id == *session).map(|i| (p, i)));
                if let Some((p, i)) = found {
                    self.open_session(p, i, tx);
                }
            }
        }
    }
}

/// The row whose detail shows.
pub fn shown_row(lineage: &AgentsLineage, selected: Option<&str>) -> Option<AgentRow> {
    let key = super::agents::shown_key(selected, lineage)?;
    lineage.rows().find(|r| r.key == key).cloned()
}
