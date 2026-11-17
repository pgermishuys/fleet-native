//! The progress detail of each session that has been opened (`stores/session-progress.ts`,
//! `composables/use-session-progress.ts`, `composables/use-session-progress-updates.ts`): loaded once from the session's own
//! machine, kept current by `progress.updated` on its topic, and dropped when a row summary on the `sessions` topic shows it
//! is out of date. Progress is extra information: a failed request is not shown and is asked again on the next view.
use super::canvases::CanvasKind;
use super::events::{Event, Kind};
use super::http;
use super::progress::{ProgressDetail, parse_detail};
use super::row_chips::{self, Progress};
use super::state::Topic;
use crate::State;
use std::collections::{HashMap, HashSet};

/// What is held for one session.
#[derive(Debug, Default)]
struct Slot {
    /// `None` is a remembered "this session has no progress" (the server answered 204) or a detail forgotten at a summary.
    detail: Option<ProgressDetail>,
    /// False once a summary showed the detail out of date. The detail stays to be drawn (the server pushes the new one right
    /// after its summary, and the tab must not flash its empty state in between) but the next view asks again.
    fresh: bool,
}

#[derive(Debug, Default)]
pub struct ProgressFeed {
    by_session: HashMap<String, Slot>,
    loading: HashSet<String>,
    /// Groups the user opened or folded, by session and group key (Fleet keeps one map for every session; see the checklist).
    folds: HashMap<(String, String), bool>,
}

impl ProgressFeed {
    /// What the session's tab and strip draw: the held detail, or nothing (`progressFor`).
    pub fn of(&self, session: &str) -> Option<&ProgressDetail> {
        self.by_session.get(session)?.detail.as_ref()
    }

    /// Whether the server has answered for this session and nothing since showed it stale. A remembered "no progress" counts;
    /// "never asked" does not.
    pub fn is_loaded(&self, session: &str) -> bool {
        self.by_session.get(session).is_some_and(|s| s.fresh)
    }

    pub fn is_loading(&self, session: &str) -> bool {
        self.loading.contains(session)
    }

    /// `apply`: keeps what is held when it is newer than `detail` (ISO text compares as text).
    pub fn apply(&mut self, detail: ProgressDetail) {
        let held = self.by_session.entry(detail.session_id.clone()).or_default();
        if held.fresh && held.detail.as_ref().is_some_and(|d| d.updated_at > detail.updated_at) {
            return;
        }
        *held = Slot { detail: Some(detail), fresh: true };
    }

    /// 204: remember "no progress", unless a detail arrived (by a push, since the request went out).
    fn none(&mut self, session: &str) {
        let held = self.by_session.entry(session.to_string()).or_default();
        if !(held.fresh && held.detail.is_some()) {
            *held = Slot { detail: None, fresh: true };
        }
    }

    /// `noteSummary`: a held answer that differs from the pushed summary is out of date.
    pub fn note_summary(&mut self, session: &str, summary: &Progress) {
        let Some(held) = self.by_session.get_mut(session) else { return };
        if held.detail.as_ref().is_some_and(|d| &d.summary == summary) {
            return;
        }
        held.fresh = false;
    }

    /// Whether a load should go out now: not for a loaded session (unless `force`), and one already in flight is shared.
    fn begin(&mut self, session: &str, force: bool) -> bool {
        if session.is_empty() || (!force && self.is_loaded(session)) {
            return false;
        }
        self.loading.insert(session.to_string())
    }

    /// Whether a group is open now: what the user chose, else `default` (`openGroups`).
    pub fn group_open(&self, session: &str, key: &str, default: bool) -> bool {
        self.folds.get(&(session.to_string(), key.to_string())).copied().unwrap_or(default)
    }

    /// A click on a group head flips it from what it shows now.
    pub fn toggle_group(&mut self, session: &str, key: &str, default: bool) {
        let open = self.group_open(session, key, default);
        self.folds.insert((session.to_string(), key.to_string()), !open);
    }

    /// For a scene: the session has exactly this.
    pub fn set(&mut self, session: &str, detail: Option<ProgressDetail>) {
        self.by_session.insert(session.to_string(), Slot { detail, fresh: true });
    }
}

impl State {
    /// The Progress tab is added the first time the open session has something to count (`introduce`, `total > 0`); after
    /// that, closing it keeps it closed. Called every frame: it does nothing once the tab was introduced.
    pub fn keep_progress_tab(&mut self) {
        let Some(p) = self.session().row.progress.as_ref() else { return };
        if p.total == 0 || self.open_canvases().introduced.contains(&CanvasKind::Progress) {
            return;
        }
        self.edit_canvases(|c, session| c.introduce(session, CanvasKind::Progress));
    }

    /// `ensureLoaded`: asks the session's own machine for its progress, off the UI thread. `force` asks again.
    pub fn ensure_progress_loaded(&mut self, session: &str, force: bool) {
        let Some(link) = self.link_for(session) else { return };
        if !self.progress.begin(session, force) {
            return;
        }
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::progress(base, &asked),
            move |s, _, result| {
                s.progress.loading.remove(&id);
                match result {
                    Ok((204, _)) => s.progress.none(&id),
                    Ok((status, body)) if (200..300).contains(&status) => {
                        if let Some(detail) = parse_detail(&body) {
                            s.progress.apply(detail);
                        }
                    }
                    _ => {}
                }
            },
        );
    }

    /// `progress.updated` on a session's topic (applied only to that session's own), and the `sessions` topic's
    /// `session_progress` summary.
    pub fn on_progress_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(kind, payload) = event else { return };
        match (kind, topic) {
            (Kind::ProgressUpdated, Topic::Session(open)) => {
                if let Some(detail) = parse_detail(payload).filter(|d| &d.session_id == open) {
                    self.progress.apply(detail);
                }
            }
            (Kind::SessionProgress, _) => {
                if let Some((session, summary)) = row_chips::parse_summary(payload) {
                    self.progress.note_summary(&session, &summary);
                }
            }
            _ => {}
        }
    }
}
