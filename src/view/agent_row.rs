//! A sub-agent's call as one row, and the chip that says background work finished. Ports `components/session/AgentTaskRow.vue`
//! and the `.background-note` of `ActivityStream.vue`. What the row says (status word, tooltip, which glyph) is
//! `conv::background::AgentRow`; this file only draws.
use super::conversation::{Message, Part};
use super::parts::{diamond, quad, triangle};
use crate::State;
use crate::conv::background::{AgentRow, BackgroundKind, BackgroundNotice, BackgroundState, delegation_of};
use crate::conv::tools::{Category, get_tool};
use crate::fleet::delegations::DelegationStatus;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use rune::{Cursor, El, Tx, icon, row, text};

/// `.agent-task`: a sub-agent's task, sized and spaced like a tool card's header (30px, 9px gap, 8px sides). The row
/// opens the child's session; "Open" shows while the pointer is on the row.
pub fn agent_row(t: &Theme, id: &str, a: &AgentRow) -> El {
    let child = a.child_session_id.clone();
    let word = a.status_word();
    let ink = if a.asks() {
        t.status_waiting_text
    } else if a.status == DelegationStatus::Error {
        t.error_text
    } else {
        t.muted
    };
    let weight = if a.asks() { 500.0 } else { 400.0 };
    let glyph = if a.asks() {
        Some(diamond(t.idle))
    } else if a.working() {
        Some(quad(t.text))
    } else if a.status == DelegationStatus::Completed {
        Some(icon(&CHECK, 13.0).color(t.running_text))
    } else if a.status == DelegationStatus::Error {
        Some(triangle(t.error))
    } else {
        None
    };
    let mut line = row()
        .key(format!("agent-{id}"))
        .w_full()
        .min_h(30.0)
        .center_y()
        .gap(9.0)
        .px(8.0)
        .radius(RADIUS_BTN - 2.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .help(a.tip())
        .label(a.tip())
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            let parent = s.open_id().unwrap_or_default();
            s.open_child(&parent, &child, tx);
        })
        .child(icon(&BOT, 14.0).color(t.muted.alpha(0.75)));
    if !a.agent.is_empty() {
        line = line.child(text(a.agent.clone()).font(sans(13.0, 600.0, 1.3)).color(t.text));
    }
    let open = row()
        .gap(2.0)
        .center_y()
        .group_opacity(0.0, 1.0)
        .child(text("Open").font(sans(12.0, 500.0, 1.3)).color(t.accent_text))
        .child(icon(&ARROW_UP_RIGHT, 12.0).color(t.accent_text));
    let end = row()
        .key(format!("agent-{id}-end"))
        .gap(8.0)
        .center_y()
        .shrink()
        .child(open)
        .child(text(word).key(format!("agent-{id}-status")).font(sans(12.0, weight, 1.3)).color(ink).truncate().shrink())
        .children(glyph);
    line.child(text(a.task.clone()).font(sans(13.0, 400.0, 1.3)).color(t.text).truncate().grow(1.0).shrink()).child(end)
}

/// `.background-note`: the chip above a notice that background work finished. Fleet's `--surface-2` fill names a variable
/// its stylesheet never defines, so the chip is drawn with no fill there, and here.
pub fn background_note(t: &Theme, n: &BackgroundNotice) -> El {
    let glyph = if n.kind == BackgroundKind::Subagent { &BOT } else { &SQUARE_TERMINAL };
    let ink = match n.state {
        BackgroundState::Completed => t.complete_text,
        BackgroundState::Error => t.error_text,
        BackgroundState::Cancelled => t.muted,
    };
    row()
        .key("background-note")
        .gap(6.0)
        .center_y()
        .max_w(700.0)
        .px(8.0)
        .py(2.0)
        .mb(4.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .child(icon(glyph, 13.0).color(t.muted))
        .child(text(n.kind.label()).font(sans(12.0, 500.0, 1.3)).color(t.text))
        .child(text(n.label.clone()).font(mono(12.0, 400.0, 1.3)).color(t.muted).truncate().shrink())
        .child(text(n.state.word()).key("background-note-state").font(sans(12.0, 400.0, 1.3)).color(ink))
}

/// Part of a message row's memo key: how each sub-agent call's delegation stands. The row reads the delegation list at
/// draw time, which changes with no change to the message.
pub fn key(s: &State, msg: &Message) -> u64 {
    use std::hash::{Hash, Hasher};
    let Some(stream) = s.stream.as_ref().filter(|st| !st.delegations.is_empty()) else { return 0 };
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for part in &msg.parts {
        let Part::Tool { id, tool, call_id, .. } = part else { continue };
        if get_tool(tool).category != Category::Subagent {
            continue;
        }
        if let Some(d) = delegation_of(&stream.delegations, call_id, id) {
            (&d.title, d.status as u8, &d.child_session_id, &d.child_activity_status, d.background).hash(&mut hash);
        }
    }
    hash.finish()
}
