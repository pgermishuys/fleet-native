//! The home dashboard's rules (`lib/needs-you.ts`, `FleetDashboard.vue`, `routes/index.tsx`): which sessions need the user,
//! which are working, the order and the words of the header, and the rule that lands a start on Mission control. No Rune
//! and no I/O: the page (piece D) only reads what this decides.
use super::groups::Listed;
use super::store::Retention;
use crate::mock::Session;

/// The dashboard's three sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    NeedsYou,
    Working,
    Recent,
}

/// Waiting on an answer, or stopped with an error: the user has to act. A workflow run waiting on the user counts too,
/// through the step session its card is in (`workflowWaiting`): that is the run's state, not the session's.
pub fn session_bucket(session: &Session, workflow_waiting: &dyn Fn(&str) -> bool) -> Bucket {
    let status = session.status_word();
    if status == "waiting_input" || status == "error" || workflow_waiting(&session.id) {
        return Bucket::NeedsYou;
    }
    if status == "active" { Bucket::Working } else { Bucket::Recent }
}

/// When the session last changed, in milliseconds (`sessionUpdatedAt`): updated, else created, else 0.
pub fn updated_at(session: &Session) -> i64 {
    if session.row.updated != 0 { session.row.updated } else { session.row.created }
}

/// Top-level sessions: no sub-agent. (A hidden session never gets this far: `parse_sessions` drops it.)
pub fn is_top_level(session: &Session) -> bool {
    session.listing.parent.is_none()
}

/// What the dashboard lists, newest first: top-level, not waiting to archive (`hides`), and of the shown retention.
pub fn shown<'a>(sessions: impl IntoIterator<Item = &'a Session>, retention: Retention, hides: &dyn Fn(&str) -> bool) -> Vec<&'a Session> {
    let mut kept: Vec<&Session> = sessions
        .into_iter()
        .filter(|s| is_top_level(s) && !hides(&s.id))
        .filter(|s| match retention {
            Retention::All => true,
            Retention::Active => !s.row.archived,
            Retention::Archived => s.row.archived,
        })
        .collect();
    // `sort_by_key` is stable, as the web's sort is: equal times keep the list's order.
    kept.sort_by_key(|s| std::cmp::Reverse(updated_at(s)));
    kept
}

/// The three sections of `shown`, each newest first.
pub fn sections<'a>(shown: &[&'a Session], workflow_waiting: &dyn Fn(&str) -> bool) -> [Vec<&'a Session>; 3] {
    let mut out: [Vec<&Session>; 3] = Default::default();
    for s in shown {
        out[session_bucket(s, workflow_waiting) as usize].push(s);
    }
    out
}

/// The server's fleet-wide counts, less what was archived a moment ago (`countsWithoutArchiving`): each such session leaves
/// Working when it is in a turn (working, or stopped on a question), else Idle.
pub fn counts_without_archiving<'a>(
    (mut active, mut idle): (u32, u32),
    sessions: impl IntoIterator<Item = &'a Session>,
    pending: &dyn Fn(&str) -> bool,
) -> (u32, u32) {
    for s in sessions {
        if !is_top_level(s) || s.row.archived || !pending(&s.id) {
            continue;
        }
        if matches!(s.status_word(), "active" | "waiting_input") {
            active = active.saturating_sub(1);
        } else {
            idle = idle.saturating_sub(1);
        }
    }
    (active, idle)
}

/// The local hour (0-23) of `now_ms` (unix milliseconds) in a zone `offset_secs` east of UTC.
pub fn hour_of(now_ms: i64, offset_secs: i64) -> u32 {
    (now_ms.div_euclid(1000) + offset_secs).rem_euclid(86_400) as u32 / 3600
}

/// The greeting for the local `hour` (0-23).
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        0..=4 => "Working late",
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The line under the greeting: how many need the user and how many are working.
pub fn status_line(needs_you: usize, working: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    if needs_you > 0 {
        parts.push(format!("{} you", plural(needs_you, "session needs", "sessions need")));
    }
    if working > 0 {
        parts.push(if parts.is_empty() {
            format!("{} working", plural(working, "session", "sessions"))
        } else {
            format!("{working} working")
        });
    }
    if parts.is_empty() { "All quiet. Nothing needs you right now.".into() } else { format!("{}.", parts.join(", ")) }
}

/// The preference behind Settings → Labs → Mission control (`MISSION_CONTROL_PREFERENCE_KEY`).
pub const MISSION_CONTROL_KEY: &str = "features.mission-control.enabled";

/// The landing rule of `routes/index.tsx`. Only the first `/` of a start may land: a start on a deep link, or a later visit to
/// `/` from Sessions, never does. It lands once the preferences have loaded, and only with Mission control on. The phone
/// app's own landing is not ported (desktop only).
#[derive(Clone, Debug)]
pub struct Landing {
    /// The module's `landing`: the start was at `/` and no showing of it has used the chance up.
    start: bool,
    /// This showing of Home may still land (`mayLand`), waiting for the preferences.
    pending: bool,
    /// Whether the rule sent the window to Mission control (`/fleet`, component 23, which routes to it).
    pub landed: bool,
}

impl Landing {
    /// A start: `at_home` is whether the window opened at `/` rather than on a deep link.
    pub fn at_start(at_home: bool) -> Self {
        Landing { start: at_home, pending: false, landed: false }
    }

    /// Home is shown. The first showing of a start at `/` may land; any later one never does.
    pub fn home_shown(&mut self) {
        self.pending = std::mem::take(&mut self.start);
    }

    /// Home is left before the preferences came: the chance goes with the page.
    pub fn home_left(&mut self) {
        self.pending = false;
    }

    /// Whether to go to Mission control now (a replace, so Back doesn't return to `/`). Ask whenever the preferences or the
    /// switch change; it is true at most once per start.
    pub fn lands(&mut self, preferences_loaded: bool, mission_control_on: bool) -> bool {
        if !self.pending || !preferences_loaded {
            return false;
        }
        self.pending = false;
        self.landed = mission_control_on;
        mission_control_on
    }
}
