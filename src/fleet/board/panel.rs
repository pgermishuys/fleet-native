//! What the Board's right panel keeps (component 23, piece R): `BoardRightPanel.vue`'s tab, `BoardSummaryPanel.vue`'s numbers and
//! `BoardActivityPanel.vue`'s feed. The numbers are plain functions of the visible sessions, so tests read them without a window.
//! The panel's fold is the shared one: `Sidebar::right_collapsed(None)` (FB8: folding leaves the gutter).
use super::cards::{CardSession, Status};
use crate::fleet::summary::{SUMMARY_SECS, Summary, Target, parse_summary};
use crate::fleet::{labs::Feature, right_panel as rp};
use crate::{Rail, State, Tx};
use std::time::Duration;

/// `boardTabs`: Summary first, on every mount.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Summary,
    Activity,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Summary, Tab::Activity];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Summary => "Summary",
            Tab::Activity => "Activity",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Tab::Summary => "board-tab-summary",
            Tab::Activity => "board-tab-activity",
        }
    }
}

/// One status row of "Status breakdown".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusStat {
    pub status: Status,
    pub count: usize,
    pub share: f64,
    /// The fill's width in percent: the share, at least 6 when there is any.
    pub width: f64,
}

/// `statusStats`: a row per status, over the visible sessions.
pub fn status_stats(visible: &[CardSession]) -> Vec<StatusStat> {
    Status::ALL
        .iter()
        .map(|&status| {
            let count = visible.iter().filter(|s| s.status == status).count();
            let share = if visible.is_empty() { 0.0 } else { count as f64 / visible.len() as f64 };
            StatusStat { status, count, share, width: (share * 100.0).max(if count > 0 { 6.0 } else { 0.0 }) }
        })
        .collect()
}

/// `AgentSummary`.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentStat {
    pub agent: String,
    pub sessions: usize,
    pub cost: f64,
    pub tokens: f64,
}

/// `agentStats`: most sessions first, then most cost, then by name.
pub fn agent_stats(visible: &[CardSession]) -> Vec<AgentStat> {
    let mut out: Vec<AgentStat> = Vec::new();
    for s in visible {
        match out.iter_mut().find(|a| a.agent == s.agent) {
            Some(a) => {
                a.sessions += 1;
                a.cost += s.cost;
                a.tokens += s.total_tokens as f64;
            }
            None => out.push(AgentStat { agent: s.agent.clone(), sessions: 1, cost: s.cost, tokens: s.total_tokens as f64 }),
        }
    }
    out.sort_by(|l, r| {
        r.sessions.cmp(&l.sessions).then_with(|| r.cost.total_cmp(&l.cost)).then_with(|| super::filters::locale_cmp(&l.agent, &r.agent))
    });
    out
}

/// What the cost rows add up (`visibleCostTotal`, `visibleTokensTotal`, `averageVisibleCost`).
pub fn totals(visible: &[CardSession]) -> (f64, f64, f64) {
    let cost: f64 = visible.iter().map(|s| s.cost).sum();
    let tokens: f64 = visible.iter().map(|s| s.total_tokens as f64).sum();
    (cost, tokens, if visible.is_empty() { 0.0 } else { cost / visible.len() as f64 })
}

/// `spendSegments`: each agent's share of the bar in percent: by cost, or by session count when nothing costs anything (FB5:
/// always, as cards carry no cost).
pub fn spend_segments(visible: &[CardSession]) -> Vec<(AgentStat, f64)> {
    let (cost, ..) = totals(visible);
    agent_stats(visible)
        .into_iter()
        .map(|a| {
            let width = if cost <= 0.0 {
                if visible.is_empty() { 0.0 } else { a.sessions as f64 / visible.len() as f64 * 100.0 }
            } else {
                a.cost / cost * 100.0
            };
            (a, width)
        })
        .collect()
}

/// `spendLabel`, the bar's words for a screen reader: also its tooltip here.
pub fn spend_label(visible: &[CardSession]) -> String {
    let segments = spend_segments(visible);
    if segments.is_empty() {
        return "No spend by agent yet".into();
    }
    let parts: Vec<String> = segments.iter().map(|(a, w)| format!("{} {}%", a.agent, w.round())).collect();
    let of = if totals(visible).0 > 0.0 { "spend" } else { "sessions" };
    format!("Share of {of} by agent: {}", parts.join(", "))
}

/// `liveFleetSummaryLabel`.
pub fn live_label(summary: &Summary) -> String {
    format!("{} active • {} idle from fleet summary", summary.active, summary.idle)
}

// ---- Activity (FB4: Fleet's invented feed, as decision D1 says) ----

pub const MAX_EVENTS: usize = 40;
pub const INITIAL_COUNT: usize = 4;
pub const EVENT_SECS: f64 = 2.5;

/// `BoardActivityTone`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Running,
    Complete,
    Error,
    Queued,
    Muted,
}

/// `fallbackSessions`: the four made-up sessions' title and project, by id.
const SESSIONS: [(&str, &str, &str); 4] = [
    ("sess-auth", "JWT Auth Module", "API Platform"),
    ("sess-ui", "Board Activity Feed", "Fleet UI"),
    ("sess-ci", "CI Workflow Cleanup", "DX"),
    ("sess-billing", "Retry Failed Webhooks", "Billing"),
];

/// `mockActivitySeeds`: id, session, action, tone, minutes ago.
const SEEDS: [(&str, &str, &str, Tone, i64); 8] = [
    ("activity-001", "sess-auth", "session started", Tone::Running, 14),
    ("activity-002", "sess-ui", "delegate accepted review task", Tone::Muted, 12),
    ("activity-003", "sess-ci", "session queued for follow-up", Tone::Queued, 10),
    ("activity-004", "sess-auth", "tests completed successfully", Tone::Complete, 8),
    ("activity-005", "sess-billing", "session completed", Tone::Complete, 6),
    ("activity-006", "sess-ui", "new board activity event received", Tone::Running, 4),
    ("activity-007", "sess-ci", "build failed on lint step", Tone::Error, 2),
    ("activity-008", "sess-ui", "session updated with handoff notes", Tone::Muted, 1),
];

/// `BoardActivityEntry`.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub project: Option<String>,
    pub action: String,
    pub occurred_at: i64,
    pub tone: Tone,
}

/// `createMockTimeline`: the eight seeds, oldest first, as of `now_ms`.
fn timeline(now_ms: i64) -> Vec<Entry> {
    SEEDS
        .iter()
        .map(|&(id, session, action, tone, minutes)| {
            let known = SESSIONS.iter().find(|(s, ..)| *s == session);
            Entry {
                id: id.into(),
                title: known.map_or_else(|| format!("Session {session}"), |(_, title, _)| (*title).into()),
                project: known.map(|(_, _, project)| (*project).into()),
                action: action.into(),
                occurred_at: now_ms - minutes * 60_000,
                tone,
            }
        })
        .collect()
}

/// The feed while the tab is shown: the four newest seeds at once, the older four one every 2.5 s.
#[derive(Clone, Debug, PartialEq)]
pub struct Feed {
    entries: Vec<Entry>,
    pending: Vec<Entry>,
    /// The second on the app clock the next one arrives.
    next_at: Option<f64>,
}

impl Feed {
    /// `onMounted`.
    pub fn mount(now_ms: i64, now: f64) -> Feed {
        let mut all = timeline(now_ms);
        let split = all.len() - INITIAL_COUNT;
        let mut entries = all.split_off(split);
        entries.reverse();
        let next_at = (!all.is_empty()).then_some(now + EVENT_SECS);
        Feed { entries, pending: all, next_at }
    }

    /// `orderedEntries`: newest first.
    pub fn ordered(&self) -> Vec<&Entry> {
        let mut out: Vec<&Entry> = self.entries.iter().collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.occurred_at));
        out
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// `flushNextMockEntry`: true when one came in.
    fn tick(&mut self, now: f64) -> bool {
        let mut changed = false;
        while self.next_at.is_some_and(|at| now >= at) {
            if self.pending.is_empty() {
                self.next_at = None;
                break;
            }
            let next = self.pending.remove(0);
            self.entries.retain(|e| e.id != next.id);
            self.entries.insert(0, next);
            self.entries.truncate(MAX_EVENTS);
            changed = true;
            self.next_at = if self.pending.is_empty() { None } else { self.next_at.map(|at| at + EVENT_SECS) };
        }
        changed
    }
}

/// What the panel keeps while it is shown; all of it goes when it is not (the unmount).
#[derive(Debug, Default)]
pub struct RightPanel {
    pub tab: Tab,
    /// `useFleetSummary`'s refs.
    pub summary: Option<Summary>,
    pub loading: bool,
    pub error: Option<String>,
    request: u64,
    due: Option<f64>,
    mounted: bool,
    pub feed: Option<Feed>,
    /// Counts the reads that were asked for, for a test.
    pub asked: u32,
}

impl RightPanel {
    fn unmount(&mut self) {
        *self = RightPanel { request: self.request + 1, asked: self.asked, ..Default::default() };
    }

    pub(crate) fn answer(&mut self, request: u64, result: Result<Summary, String>) {
        if request != self.request {
            return;
        }
        self.loading = false;
        match result {
            Ok(summary) => {
                self.summary = Some(summary);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

impl State {
    /// The panel is a column on screen: the Board is shown with its switch on, the panel not folded and the sheet wide enough.
    /// A narrow window's sheet over the page is component 10's (25.2) and not drawn here.
    pub fn board_right_shown(&self) -> bool {
        self.rail == Rail::Board
            && self.labs_feature(Feature::Board) == Some(true)
            && !self.sidebar.right_collapsed(None)
            && rp::fits(self.sheet_w)
    }

    /// `Cmd/Ctrl+Shift+B` and the palette's "Show/Hide right panel", on the Board: the shared fold, with no session's own.
    pub fn toggle_board_right(&mut self) {
        let collapsed = self.sidebar.right_collapsed(None);
        self.sidebar.set_right_collapsed(None, !collapsed);
        self.save_settings();
    }

    /// The palette's "Show/Hide right panel" label follows whichever panel the rail owns.
    pub fn right_panel_open(&self) -> bool {
        if self.rail == Rail::Board { !self.sidebar.right_collapsed(None) } else { self.content_open }
    }

    pub fn toggle_right_panel(&mut self) {
        if self.rail == Rail::Board {
            self.toggle_board_right();
        } else {
            self.content_open = !self.content_open;
        }
    }

    /// A tab was pressed; the open one does nothing (`handleSelect`).
    pub fn board_pick_tab(&mut self, tab: Tab) {
        let right = &mut self.board.right;
        if right.tab != tab {
            // Each tab mounts its own reader: the summary asks again, the feed starts over.
            *right = RightPanel { tab, mounted: true, request: right.request + 1, asked: right.asked, ..Default::default() };
        }
    }

    /// From `poll`: mounts and unmounts the panel's two readers with the panel, reads the fleet summary every 30 s and brings
    /// the feed's next entry in on time. True when something on screen changed.
    pub(crate) fn keep_board_right(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.board_right_shown() {
            let held = self.board.right.mounted;
            self.board.right.unmount();
            return held;
        }
        let mut changed = false;
        if !self.board.right.mounted {
            self.board.right.mounted = true;
            changed = true;
        }
        match self.board.right.tab {
            Tab::Summary => {
                if self.board.right.due.is_none_or(|due| now >= due) {
                    changed |= self.read_board_summary(now);
                }
                self.board.right.feed = None;
                tx.wake_after(Duration::from_secs(1));
            }
            Tab::Activity => {
                let now_ms = self.now_ms.unwrap_or_else(crate::fleet::link::now_ms);
                let feed = self.board.right.feed.get_or_insert_with(|| Feed::mount(now_ms, now));
                changed |= feed.tick(now);
                if feed.pending() > 0 {
                    tx.wake_after(Duration::from_millis(250));
                }
            }
        }
        changed
    }

    fn read_board_summary(&mut self, now: f64) -> bool {
        self.board.right.due = Some(now + SUMMARY_SECS);
        self.board.right.asked += 1;
        let Some((base, post)) = self.board_server() else {
            // No server: the invented fleet of the mock data.
            self.board.right.summary = Some(Summary { active: 3, idle: 5, queued: 2, tokens: 1_240_000.0, cost: 18.42 });
            self.board.right.loading = false;
            return true;
        };
        let first = self.board.right.summary.is_none() && self.board.right.error.is_none();
        self.board.right.loading = first;
        self.board.right.request += 1;
        let request = self.board.right.request;
        std::thread::spawn(move || {
            let result =
                Target::Here(base).get("/api/fleet/summary").and_then(|b| parse_summary(&b).ok_or_else(|| "No data returned".to_string()));
            post.update(move |s, _| s.board.right.answer(request, result));
        });
        first
    }
}
