//! Ports `components/canvas/AgentsCanvasRow.vue` and `AgentDetail.vue`: one row of the Agents tab (its mark, name, task, state
//! word and sub line) and the box under the one that is open (what it was asked, tools, tokens, what it said last, and Open
//! session and Stop). What each says is worked out in `fleet::agents`; this lays it out.
use crate::State;
use crate::fleet::agents::{AgentRow, AgentRowState, Detail, kind_label, label, note, state_text};
use crate::icons::{ARROW_UP_RIGHT, BAN, CHECK, CIRCLE_STOP, X};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::view::message::now_ms;
use rune::{Cursor, Cycle, El, Icon, col, el, icon, row, text};

/// A working row's dot: opacity 1 → 0.45 → 1 over 2 s.
static PULSE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.45), (1.0, 1.0)];

/// The mark in its 8×8 box.
fn mark(t: &Theme, state: AgentRowState) -> El {
    // The 12 px glyph sits in the 8 px column, spilling 2 px each side.
    let glyph = |g: &'static Icon, ink| row().size(12.0, 12.0).ml(-2.0).mr(-2.0).center().child(icon(g, 12.0).color(ink));
    match state {
        AgentRowState::Idle => el().size(8.0, 8.0).radius(4.0).border(1.5, t.muted),
        AgentRowState::Running => {
            el().size(8.0, 8.0).radius(4.0).fill(t.running).cycle(Cycle { period: 2.0, phase: 0.0, stops: &PULSE, still: 1.0, fps: 30.0 })
        }
        AgentRowState::Waiting => el().size(8.0, 8.0).radius(4.0).fill(t.idle),
        AgentRowState::Done => glyph(&CHECK, t.running),
        AgentRowState::Failed => glyph(&X, t.error_text),
        AgentRowState::Stopped => glyph(&BAN, t.muted),
    }
}

fn state_ink(t: &Theme, state: AgentRowState) -> rune::Color {
    match state {
        AgentRowState::Running => t.running_text,
        AgentRowState::Waiting => t.status_waiting_text,
        AgentRowState::Failed => t.error_text,
        _ => t.muted,
    }
}

/// The row. `runs_on` is where it runs; a press picks it (the parent's opens that session instead).
pub fn agent_row(s: &State, t: &Theme, r: &AgentRow, runs_on: Option<&str>, shown: bool, parent_note: Option<&'static str>) -> El {
    let now = now_ms(s);
    let is_parent = parent_note.is_some();
    let mut first = row().w_full().gap(6.0).center_y().min_w(0.0);
    first = first.child(text(r.name.clone()).font(sans(13.0, 500.0, 1.4)).color(t.text).truncate());
    if let Some(task) = &r.task {
        first = first.child(text(task.clone()).font(sans(12.5, 400.0, 1.4)).color(t.muted).truncate().shrink());
    }
    let mut word = text(state_text(r, now)).font(sans(12.0, 400.0, 1.4).features("'tnum' 1")).color(state_ink(t, r.state));
    word = word.key(format!("agents-state-{}", r.key));
    if let Some(w) = &r.work {
        word = word.help(crate::conv::work_clock::time_title(w, now, crate::conv::time::local_offset_secs()));
    }
    let head = row().w_full().gap(10.0).center_y().child(mark(t, r.state)).child(first.grow(1.0).shrink().min_w(0.0)).child(word);

    let mut sub = row().gap(0.0).center_y().min_w(0.0).w_full();
    if !is_parent {
        sub = sub.child(text(kind_label(r.kind).to_uppercase()).font(sans(10.5, 600.0, 1.7).tracking(0.42)).color(t.muted).mr(6.0));
    }
    if let Some(on) = runs_on {
        sub = sub.child(text(on.to_string()).font(sans(12.0, 400.0, 1.5)).color(t.muted).truncate().shrink());
    }
    let note_words = parent_note.or_else(|| note(r));
    if let Some(n) = note_words {
        let words = if runs_on.is_some() { format!("\u{00b7} {n}") } else { n.to_string() };
        sub = sub.child(text(words).font(sans(12.0, 400.0, 1.5)).color(t.muted).ml(4.0));
    }
    let has_sub = !is_parent || runs_on.is_some() || note_words.is_some();
    let mut body = col().w_full().gap(1.0).items_stretch().child(head);
    if has_sub {
        body = body.child(row().w_full().pl(18.0).child(sub.grow(1.0).min_w(0.0)));
    }
    let key = r.key.clone();
    let target = r.session_id.clone();
    body.key(format!("agents-row-{}", r.key))
        .role("button")
        .label(label(r, is_parent, runs_on, now))
        .px(8.0)
        .py(7.0)
        .radius(8.0)
        .fill(if shown { t.card_bg } else { rune::Color::TRANSPARENT })
        .hover(if shown { t.card_bg } else { t.wash(5.0) })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| {
            if is_parent {
                if let Some(id) = &target {
                    s.open_parent(id, tx);
                }
            } else {
                s.select_agent(&key);
            }
        })
}

fn kv(t: &Theme, key: &str, name: &str, value: &str, tip: Option<&str>) -> El {
    let mut dd = text(value.to_string()).key(key.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.text).truncate().shrink().min_w(0.0);
    if let Some(tip) = tip {
        dd = dd.help(tip.to_string());
    }
    row()
        .w_full()
        .gap(14.0)
        .center_y()
        .child(text(name.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.muted).w(56.0))
        .child(dd.grow(1.0))
}

fn button(t: &Theme, key: String, glyph: &'static Icon, words: &str, disabled: bool) -> El {
    let b = row()
        .key(key)
        .role("button")
        .label(words.to_string())
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .border(1.0, t.border)
        .radius(7.0)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(icon(glyph, 13.0).color(t.text))
        .child(text(words.to_string()).font(sans(12.5, 400.0, 1.4)).color(t.text));
    if disabled { b.opacity(0.6).disabled(true) } else { b.hover(t.wash(6.0)) }
}

/// The box under a shown row.
pub fn agent_detail(t: &Theme, r: &AgentRow, d: &Detail, title: &str) -> El {
    let mut b = col()
        .key("agents-detail")
        .grow(1.0)
        .min_w(0.0)
        .px(12.0)
        .py(10.0)
        .gap(4.0)
        .items_stretch()
        .border(1.0, t.border)
        .radius(RADIUS_CARD)
        .fill(t.card_bg);
    if let Some(a) = &d.asked {
        b = b.child(kv(t, "agents-asked", "Asked to", a, Some(a)));
    }
    if let Some(v) = &d.tools {
        b = b.child(kv(t, "agents-tools", "Tools", v, None));
    }
    if let Some(v) = &d.tokens {
        b = b.child(kv(t, "agents-tokens", "Tokens", v, None));
    }
    if let Some(v) = &d.ended {
        b = b.child(kv(t, "agents-ended", "Ended", v, None));
    }
    if let Some(line) = &d.latest {
        b =
            b.child(
                row().w_full().mt(2.0).px(8.0).py(6.0).radius(6.0).fill(t.wash(5.0)).child(
                    text(line.clone()).key("agents-latest").font(mono(11.5, 400.0, 1.5)).color(t.muted).truncate().help(line.clone()),
                ),
            );
    } else if d.reading {
        b = b.child(text("Reading its session\u{2026}").key("agents-reading").font(sans(12.0, 400.0, 1.5)).color(t.muted).mt(2.0));
    }
    if d.open.is_some() || d.can_stop {
        let mut actions = row().key("agents-actions").w_full().gap(6.0).mt(4.0);
        if let Some(target) = d.open.clone() {
            let title = title.to_string();
            actions = actions.child(
                button(t, "agents-open".into(), &ARROW_UP_RIGHT, "Open session", false)
                    .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_agent(&target, &title, tx)),
            );
        }
        if d.can_stop
            && let Some(w) = &r.work
        {
            let id = w.id.clone();
            let words = if d.stopping { "Stopping\u{2026}" } else { "Stop" };
            actions = actions
                .child(button(t, "agents-stop".into(), &CIRCLE_STOP, words, d.stopping).on_press(move |s: &mut State| s.stop_work(&id)));
        }
        b = b.child(actions);
    } else if d.inside_turn {
        b = b.child(
            text("Runs inside the tool call. Interrupt the turn to stop it.")
                .key("agents-inside")
                .font(sans(12.0, 400.0, 1.5))
                .color(t.muted),
        );
    }
    if let Some(why) = &d.stop_error {
        b = b.child(text(why.clone()).key("agents-stop-error").role("alert").font(sans(12.0, 400.0, 1.5)).color(t.error_text).wrap());
    }
    // Margins 2 top, 6 bottom, 18 left (the column of the name).
    row().w_full().pl(18.0).pt(2.0).pb(6.0).child(b)
}
