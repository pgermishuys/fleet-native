//! A group's rows with each session's family under it (`ProjectGroup.vue`'s `.session-children`): the top-level row, then
//! its sub-agents and the sessions it forked or started, each one indent in with the lines beside it. The lines, the kind
//! label and the caret are muted in every state and for the open session's family: nothing here is lit (decided 17 October 2026).
use super::session_row::{RowClock, Slots, session_row_with};
use super::subagent_row::subagent_row;
use super::workflows::run_group::run_group;
use crate::State;
use crate::fleet::family::Entry;
use crate::fleet::lineage::{Connector, indent, kind_label, line_x};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::helpers::{ListEntry, RunMember, group_run_sessions};
use crate::mock::Session;
use crate::theme::Theme;
use rune::{El, col, el};

/// A child row's height: 28 for the row and 1 above and below.
const CHILD_H: f32 = 30.0;

/// The lines beside one child (`.lineage-line`, `.lineage-stub`): `muted` at 16%, 1px wide. The own line runs the full height,
/// or stops at the middle on the last child; the stub is 9px long at the middle.
fn lines(t: &Theme, key: &str, line: &Connector, row: El) -> El {
    let ink = t.muted.alpha(0.16);
    let mut wrap = el().key(format!("lineage-{key}")).w_full();
    for level in &line.through {
        wrap = wrap.child(el().absolute(line_x(*level), 0.0).w(1.0).h_full().fill(ink));
    }
    let own = el().key(format!("lineage-line-{key}")).absolute(line_x(line.depth), 0.0).w(1.0).fill(ink);
    wrap = wrap.child(if line.last { own.h(CHILD_H / 2.0) } else { own.h_full() });
    wrap.child(el().key(format!("lineage-stub-{key}")).absolute(line_x(line.depth), CHILD_H / 2.0).w(9.0).h(1.0).fill(ink)).child(row)
}

/// A top-level row as the run grouping reads it (`groupRunSessions`' items).
struct Root<'a> {
    at: (usize, usize),
    session: &'a Session,
}

impl RunMember for Root<'_> {
    fn id(&self) -> &str {
        &self.session.id
    }
    fn title(&self) -> Option<&str> {
        Some(&self.session.title)
    }
    fn workflow_run_id(&self) -> Option<&str> {
        self.session.listing.workflow_run_id.as_deref()
    }
}

/// The rows of one group, top to bottom: a workflow run's step sessions sit under one row where its newest session was, and
/// every other top-level session is a row of its own with its family (`entries` of `ProjectGroup.vue`). Only the live
/// machine's rows come here: another machine's steps don't group.
pub fn family_rows(s: &State, t: &Theme, clock: &RowClock, rows: &[(usize, usize)]) -> Vec<El> {
    // The plan, one chunk per top-level row: the row, then what hangs under it.
    let mut chunks: Vec<Vec<Entry>> = Vec::new();
    for entry in s.family_plan(rows) {
        match (&entry, chunks.last_mut()) {
            (Entry::Root { .. }, _) | (_, None) => chunks.push(vec![entry]),
            (_, Some(chunk)) => chunk.push(entry),
        }
    }
    let roots: Vec<Root> = chunks
        .iter()
        .filter_map(|c| match c.first() {
            Some(Entry::Root { at, .. }) => Some(Root { at: *at, session: &s.projects[at.0].sessions[at.1] }),
            _ => None,
        })
        .collect();
    let store = s.workflow_runs(HOME_MACHINE_KEY);
    let mut out = Vec::new();
    for entry in group_run_sessions(&roots, |id| store.run_for_session(id)) {
        match entry {
            ListEntry::Session(root) => {
                if let Some(chunk) = chunks.iter().find(|c| matches!(c.first(), Some(Entry::Root { at, .. }) if *at == root.at)) {
                    out.extend(chunk_rows(s, t, clock, chunk));
                }
            }
            ListEntry::Run { run_id, run, steps } => {
                let steps: Vec<((usize, usize), String)> = steps.into_iter().map(|(root, label)| (root.at, label)).collect();
                out.push(run_group(s, t, clock, &run_id, run, &steps));
            }
        }
    }
    out
}

/// One top-level row and its family.
fn chunk_rows(s: &State, t: &Theme, clock: &RowClock, plan: &[Entry]) -> Vec<El> {
    let mut out = Vec::new();
    let mut kids: Option<(String, El)> = None;
    let flush = |kids: &mut Option<(String, El)>, out: &mut Vec<El>| {
        if let Some((_, group)) = kids.take() {
            out.push(group.mb(2.0));
        }
    };
    let mut root_title = String::new();
    for entry in plan {
        match entry {
            Entry::Root { at: (p, i), caret } => {
                flush(&mut kids, &mut out);
                let session = &s.projects[*p].sessions[*i];
                root_title = session.title.trim().to_string();
                out.push(session_row_with(s, t, clock, *p, *i, session, Slots { caret: *caret, ..Slots::default() }));
                kids = None;
            }
            Entry::Child { at: (p, i), kind, line, .. } => {
                let session = &s.projects[*p].sessions[*i];
                let slots = Slots { kind_label: Some(kind_label(*kind)), child: Some(indent(line.depth)), ..Slots::default() };
                let row = session_row_with(s, t, clock, *p, *i, session, slots);
                let key = format!("{p}-{i}");
                push(&mut kids, &root_title, lines(t, &key, line, row));
            }
            Entry::Subagent { work, line } => {
                let row = subagent_row(s, t, work, indent(line.depth));
                let key = work.id.clone();
                push(&mut kids, &root_title, lines(t, &key, line, row));
            }
        }
    }
    flush(&mut kids, &mut out);
    out
}

/// Adds a child to the family's container (`div.session-children`, `role=group`).
fn push(kids: &mut Option<(String, El)>, root_title: &str, child: El) {
    let (label, group) = kids.take().unwrap_or_else(|| {
        let name = if root_title.is_empty() { "this session" } else { root_title };
        let label = format!("Started from {name}");
        (label.clone(), col().key("session-children").role("group").label(label).w_full())
    });
    *kids = Some((label, group.child(child)));
}
