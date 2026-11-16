//! A group's rows with each session's family under it (`ProjectGroup.vue`'s `.session-children`): the top-level row, then
//! its sub-agents and the sessions it forked or started, each one indent in with the lines beside it. The lines, the kind
//! label and the caret are muted in every state and for the open session's family: nothing here is lit (decided 17 October 2026).
use super::session_row::{RowClock, Slots, session_row_with};
use super::subagent_row::subagent_row;
use crate::State;
use crate::fleet::family::Entry;
use crate::fleet::lineage::{Connector, indent, kind_label, line_x};
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

/// The rows of one group, top to bottom.
pub fn family_rows(s: &State, t: &Theme, clock: &RowClock, rows: &[(usize, usize)]) -> Vec<El> {
    let mut out = Vec::new();
    let mut kids: Option<(String, El)> = None;
    let flush = |kids: &mut Option<(String, El)>, out: &mut Vec<El>| {
        if let Some((_, group)) = kids.take() {
            out.push(group.mb(2.0));
        }
    };
    let mut root_title = String::new();
    for entry in s.family_plan(rows) {
        match entry {
            Entry::Root { at: (p, i), caret } => {
                flush(&mut kids, &mut out);
                let session = &s.projects[p].sessions[i];
                root_title = session.title.trim().to_string();
                out.push(session_row_with(s, t, clock, p, i, session, Slots { caret, ..Slots::default() }));
                kids = None;
            }
            Entry::Child { at: (p, i), kind, line, .. } => {
                let session = &s.projects[p].sessions[i];
                let slots = Slots { kind_label: Some(kind_label(kind)), child: Some(indent(line.depth)), ..Slots::default() };
                let row = session_row_with(s, t, clock, p, i, session, slots);
                let key = format!("{p}-{i}");
                push(&mut kids, &root_title, lines(t, &key, &line, row));
            }
            Entry::Subagent { work, line } => {
                let row = subagent_row(s, t, &work, indent(line.depth));
                let key = work.id.clone();
                push(&mut kids, &root_title, lines(t, &key, &line, row));
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
