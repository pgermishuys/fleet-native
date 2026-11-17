//! Ports `components/canvas/AgentsCanvas.vue`: the Agents tab. "Lineage" with what is running or waiting, the parent, Running
//! now, Started by this session, the Earlier fold, and the empty state. The rows and their detail are `view::agents_row`.
use super::agents_row::{agent_detail, agent_row};
use crate::State;
use crate::fleet::agents::AgentRow;
use crate::fleet::agents_tab::shown_row;
use crate::icons::{CHEVRON_DOWN, NETWORK};
use crate::theme::{Theme, sans};
use rune::{Align, Cursor, El, ScrollSpec, col, icon, row, text};
use std::f32::consts::FRAC_PI_2;

/// The tab's body for the open session.
pub fn agents_tab(s: &State, t: &Theme) -> El {
    let session = s.canvas_session();
    let lineage = s.agents_lineage(&session);
    let parent = s.agents_parent(&session);
    let shown = shown_row(&lineage, s.agents_tab.selected.as_deref()).map(|r| r.key);
    let mut head = row().gap(8.0).center_y().child(text("Lineage").font(sans(13.0, 600.0, 1.4)).color(t.text));
    if let Some(sum) = lineage.summary() {
        head = head.child(text(sum).key("agents-summary").font(sans(12.0, 400.0, 1.4)).color(t.muted));
    }
    let mut body = col().w_full().gap(16.0).px(14.0).pt(14.0).pb(20.0).items_stretch().child(head.key("agents-head"));
    let one = |r: &AgentRow, parent_note: Option<&'static str>, runs_on: Option<String>| {
        let on = runs_on.or_else(|| s.agents_runs_on(&session, r));
        let is_shown = shown.as_deref() == Some(r.key.as_str());
        let mut c = col().w_full().items_stretch().child(agent_row(s, t, r, on.as_deref(), is_shown, parent_note));
        if is_shown {
            c = c.child(agent_detail(t, r, &s.agents_detail(&session, r), &r.name));
        }
        c
    };
    if lineage.is_empty(parent.is_some()) {
        return frame(t, body.child(empty(t)));
    }
    if let Some(p) = &parent {
        let sec = section(t, "agents-parent", "Parent", None).child(one(&p.row, Some(p.note), p.runs_on.clone()));
        body = body.child(sec);
    }
    if !lineage.running.is_empty() {
        let mut sec = section(t, "agents-running", "Running now", Some(lineage.running.len()));
        for r in &lineage.running {
            sec = sec.child(one(r, None, None));
        }
        body = body.child(sec);
    }
    if !lineage.started.is_empty() {
        let mut sec = section(t, "agents-started", "Started by this session", Some(lineage.started.len()));
        for r in &lineage.started {
            sec = sec.child(one(r, None, None));
        }
        body = body.child(sec);
    }
    if !lineage.earlier.is_empty() {
        let open = s.agents_tab.earlier_open;
        let mut sec = col().key("agents-earlier").w_full().gap(2.0).items_stretch().child(
            row()
                .key("agents-earlier-toggle")
                .role("button")
                .label(lineage.earlier_label())
                .w_full()
                .gap(6.0)
                .padding(6.0)
                .center_y()
                .radius(6.0)
                .cursor(Cursor::Pointer)
                .hover_group()
                .focusable(true)
                .focus_ring(2.0, -2.0, t.accent)
                .on_press(|s: &mut State| s.agents_tab.earlier_open = !s.agents_tab.earlier_open)
                .child(icon(&CHEVRON_DOWN, 14.0).color(t.muted).group_color(t.text).rotate(if open { 0.0 } else { -FRAC_PI_2 }))
                .child(text(lineage.earlier_label()).font(sans(12.5, 400.0, 1.4)).color(t.muted).group_color(t.text)),
        );
        if open {
            for r in &lineage.earlier {
                sec = sec.child(one(r, None, None));
            }
        }
        body = body.child(sec);
    }
    frame(t, body)
}

fn frame(_t: &Theme, body: El) -> El {
    col()
        .key("agents-tab")
        .role("region")
        .label("Agents")
        .w_full()
        .h_full()
        .child(body.scroll(ScrollSpec { stick_bottom: false }).key("agents-scroll").w_full().grow(1.0).min_h(0.0))
}

/// A section: its head (11 px, uppercase, tracked, the count at the right) and its rows.
fn section(t: &Theme, key: &str, title: &str, count: Option<usize>) -> El {
    let mut head = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .px(6.0)
        .pb(4.0)
        .child(text(title.to_uppercase()).font(sans(11.0, 600.0, 1.4).tracking(0.44)).color(t.muted));
    if let Some(n) = count {
        head = head.child(rune::el().grow(1.0)).child(text(n.to_string()).font(sans(11.0, 500.0, 1.4).features("'tnum' 1")).color(t.muted));
    }
    col().key(key.to_string()).w_full().gap(2.0).items_stretch().child(head)
}

/// `EmptyState compact` with the network icon.
fn empty(t: &Theme) -> El {
    col().key("agents-empty").w_full().items_center().gap(3.5).px(10.5).py(14.0).child(icon(&NETWORK, 20.0).color(t.muted).mb(7.0)).child(
        text("Subagents this session's agent starts, and sessions it forks or starts, show here")
            .font(sans(12.25, 500.0, 1.5))
            .color(t.text)
            .wrap()
            .w_full()
            .align(Align::Center),
    )
}
