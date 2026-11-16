//! Another machine's sessions in the list (`MachineSessionsGroup.vue`, `SessionsPanel.vue`'s machine part): its heading, the
//! note and error line under it, its Pinned group and projects in its own order, and a row for each session as it last
//! listed them. Pressing a row opens it; while the machine answers a row also renames (double-click, F2), has the
//! right-click menu and its project renames from its header; while it doesn't, rows only open.
use super::machine_header::{Heading, machine_header};
use super::project_header::{Header, editing as project_editing, project_header};
use super::session_row::{RowClock, Slots, editing, glyph_el, tail, title_ink};
use crate::State;
use crate::fleet::groups::{LineageKind, Listed};
use crate::fleet::machine_tree::{self, TreeGroup, TreeRow};
use crate::fleet::machines::{Entry, MachineSessions};
use crate::fleet::row_status::{self, Dim, Glyph, RowItem};
use crate::icons::{CHEVRON_RIGHT, PLUS};
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Align, Color, Cursor, El, Key, KeyEvent, Tx, col, el, icon, row, text};

/// The word on the right of a machine's heading when it has no count (`note`).
pub fn note_of(state: Option<&MachineSessions>) -> Option<&'static str> {
    let state = state?;
    if state.error.is_some() {
        return Some(if state.loaded_at.is_some() { "unreachable \u{b7} cached" } else { "unreachable" });
    }
    if state.loaded_at.is_none() && state.loading { Some("\u{2026}") } else { None }
}

pub fn machine_group(s: &State, t: &Theme, clock: &RowClock, machine: &Entry, query: &str) -> El {
    let key = machine.key.as_str();
    let state = s.machines.others.get(key);
    let empty = MachineSessions::default();
    let held = state.unwrap_or(&empty);
    let tree = machine_tree::build(&held.sessions, &held.projects, query);
    let down = held.error.is_some();
    let open = s.machine_is_open(key);
    let note = note_of(state);
    let id = key.to_string();
    let header = machine_header(
        t,
        Heading {
            key: format!("machine-{key}"),
            name: &machine.name,
            unreachable: down,
            note,
            expanded: Some(open),
            count: Some(tree.count),
            menu: true,
        },
    )
    .on_press(move |s: &mut State| s.toggle_machine(&id));
    let mut group = col().key(format!("machine-group-{key}")).label(format!("Sessions on {}", machine.name)).w_full().child(header);
    if !open {
        return group;
    }
    if let Some(error) = &held.error {
        // `.machine-group__error`: the reason, in the error ink mixed with the muted one.
        group = group.child(
            text(error.clone())
                .key(format!("machine-error-{key}"))
                .font(sans(11.5, 400.0, 1.4))
                .color(t.mix(t.error_text, t.muted, 0.8))
                .wrap()
                .mt(2.0)
                .mr(10.0)
                .mb(4.0)
                .ml(26.0),
        );
    }
    let answering = s.machines.is_answering(key);
    let mut body = col().w_full();
    for (n, g) in tree.groups.iter().enumerate() {
        body = body.child(group_of(s, t, clock, machine, g, &held.sessions, answering, n == 0));
    }
    if !down && held.loaded_at.is_some() && tree.groups.is_empty() {
        body = body.child(empty_state(t, key, !query.is_empty()));
    }
    group.child(body)
}

#[allow(clippy::too_many_arguments)]
fn group_of(
    s: &State,
    t: &Theme,
    clock: &RowClock,
    machine: &Entry,
    g: &TreeGroup,
    sessions: &[Session],
    answering: bool,
    first: bool,
) -> El {
    let key = machine.key.as_str();
    let open = s.machine_project_is_open(key, &g.id);
    let kind = if g.pinned { Header::Pinned } else { Header::Project };
    let (mkey, gid) = (key.to_string(), g.id.clone());
    let header_key = format!("machine-project-{key}-{}", g.id);
    // A real project of an answering machine renames from its header and menu; the Pinned and Ungrouped groups don't.
    let named = answering && !g.pinned && g.id != crate::fleet::groups::UNGROUPED_ID;
    let header = if named && s.machine_project_renaming(key, &g.id) {
        project_editing(s, t, header_key, open)
    } else {
        let (m, id) = (key.to_string(), g.id.clone());
        let header = project_header(t, header_key, kind, &g.name, g.count, open, false)
            .on_press(move |s: &mut State| s.toggle_machine_project(&mkey, &gid));
        if named {
            let (m2, id2) = (m.clone(), id.clone());
            el().w_full()
                .on_secondary_press(move |s: &mut State, at, _: &mut Tx| {
                    s.ui.point = (at.x, at.y);
                    s.machine_menu = Some((m.clone(), id.clone()));
                    s.ui.open = Some(crate::ui::Open::MachineProjectMenu);
                })
                .child(header.on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
                    if ev.key == Key::Function(2) {
                        s.start_machine_rename(&m2, &id2, tx);
                        return true;
                    }
                    false
                }))
        } else {
            header
        }
    };
    let out = col().w_full().mt(if first { 0.0 } else { 10.0 }).child(header);
    if !open {
        return out;
    }
    let mut rows = col().w_full().pt(2.0);
    let mut hide_below: Option<usize> = None;
    for (n, r) in g.rows.iter().enumerate() {
        if let Some(depth) = hide_below {
            if r.depth > depth {
                continue;
            }
            hide_below = None;
        }
        let kids = children_of(g, n);
        let shown = kids.is_empty() || children_open(s, sessions, &sessions[r.index], &kids);
        if !kids.is_empty() && !shown {
            hide_below = Some(r.depth);
        }
        let caret = (!kids.is_empty()).then_some(shown);
        rows = rows.child(read_only_row(s, t, clock, machine, &sessions[r.index], r, caret, answering));
    }
    // An unreachable machine's rows are the last ones it returned (`.machine-group--stale .session-item`): the rows dim, the
    // headings don't.
    out.child(if answering { rows } else { rows.opacity(0.6) })
}

/// The rows below `n` that came from it.
fn children_of(g: &TreeGroup, n: usize) -> Vec<TreeRow> {
    let depth = g.rows[n].depth;
    g.rows[n + 1..].iter().take_while(|r| r.depth > depth).copied().collect()
}

/// A parent shows its children while the user has opened them, or, unasked, while it or one of them is open or one works or
/// waits on the user (`opensByItself`).
fn children_open(s: &State, sessions: &[Session], parent: &Session, kids: &[TreeRow]) -> bool {
    let by_itself = kids.iter().any(|k| matches!(sessions[k.index].status_word(), "active" | "waiting_input"));
    s.machine_children.get(&parent.id).copied().unwrap_or(by_itself)
}

/// One session of another machine, as the list draws a row (`SessionItem.vue` in `MachineSessionsGroup`): pressing it opens the
/// session in place. While the machine answers it also renames and has the menu; while it doesn't it only opens.
#[allow(clippy::too_many_arguments)]
fn read_only_row(
    s: &State,
    t: &Theme,
    clock: &RowClock,
    machine: &Entry,
    session: &Session,
    at: &TreeRow,
    caret: Option<bool>,
    answering: bool,
) -> El {
    let key = format!("{}-{}", machine.key, session.id);
    // The open mark sits on the row of its own machine only (`openSessionMachineKey`).
    let active = s.is_open_on(&machine.key, &session.id);
    let item = RowItem::of(session, clock.now);
    let status = row_status::row_status(&item, clock.now, clock.offset_secs, answering);
    let dim = if active { Dim::None } else { row_status::row_dim(&item, clock.now) };
    // A working session on a machine that isn't answering shows no working glyph (its word says "Not answering").
    let glyph = match row_status::glyph(&item) {
        Glyph::Working | Glyph::Retry if !answering => Glyph::Quiet,
        g => g,
    };
    let glyph = match glyph_el(t, glyph) {
        Some(g) => g.key(format!("machine-glyph-{key}")).help(status.description.clone()),
        None => el().size(8.0, 8.0).key(format!("machine-slot-{key}")),
    };
    if answering && s.machine_row_renaming(&session.id) {
        return el().w_full().py(1.0).child(editing(s, t, format!("machine-session-{key}"), glyph));
    }
    let slot = match caret {
        Some(open) => {
            let (id, was) = (session.id.clone(), open);
            let turn = if open { std::f32::consts::FRAC_PI_2 } else { 0.0 };
            el().key(format!("machine-caret-{key}"))
                .size(14.0, 14.0)
                .mr(-4.0)
                .radius(4.0)
                .center()
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| s.toggle_children(&id, was))
                .child(icon(&CHEVRON_RIGHT, 14.0).color(t.muted).rotate(turn))
        }
        None => el().size(14.0, 14.0).mr(-4.0),
    };
    // A child sits one level in from its parent (`--lineage-indent`).
    let indent = if at.depth == 0 { 6.0 } else { 25.0 + (at.depth as f32 - 1.0) * 16.0 };
    let (machine_key, session_id) = (machine.key.clone(), session.id.clone());
    let mut line = row()
        .key(format!("machine-session-{key}"))
        .w_full()
        .min_h(32.0)
        .center_y()
        .gap(9.0)
        .pl(indent)
        .pr(10.0)
        .radius(RADIUS_BTN)
        .fill(if active { t.wash(9.0) } else { Color::TRANSPARENT })
        .hover(if active { Color::TRANSPARENT } else { t.wash(5.0) })
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.open_machine_session(&machine_key, &session_id, tx))
        .child(slot)
        .child(glyph)
        .child(
            text(row_status::display_title(None, &session.title))
                .key(format!("machine-title-{key}"))
                .font(sans(13.0, if active { 500.0 } else { 400.0 }, 1.3))
                .color(title_ink(t, active, dim))
                .group_color(t.text)
                .truncate()
                .grow(1.0),
        );
    if answering {
        let (m, id, m2, id2, m3, id3) =
            (machine.key.clone(), session.id.clone(), machine.key.clone(), session.id.clone(), machine.key.clone(), session.id.clone());
        line = line
            .help("Double-click to rename")
            .on_double_press_tx(move |s: &mut State, tx: &mut Tx| s.start_machine_row_rename(&m, &id, tx))
            // `handleRowKeydown`: F2 renames.
            .on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
                if ev.key == Key::Function(2) {
                    s.start_machine_row_rename(&m2, &id2, tx);
                    return true;
                }
                false
            })
            .on_secondary_press(move |s: &mut State, at, _: &mut Tx| {
                s.ui.point = (at.x, at.y);
                s.machine_menu = Some((m3.clone(), id3.clone()));
                s.ui.open = Some(crate::ui::Open::MachineContext);
            });
    } else {
        line = line.help(format!("Open on {}", machine.name));
    }
    // Lineage stays muted in every state: the label takes the row's meta colour.
    let label = match at.kind {
        Some(LineageKind::Fork) => Some("fork"),
        Some(_) => Some("started"),
        None => None,
    };
    let slots = Slots { step_note: None, kind_label: label };
    let line = tail(s, t, &key, session, &status, if answering { dim } else { Dim::None }, slots, line);
    el().w_full().py(1.0).child(line)
}

/// `EmptyState compact`: "No sessions yet" with a New session button, or "No sessions found" while the filter has text.
fn empty_state(t: &Theme, key: &str, filtered: bool) -> El {
    let mut empty = col().key(format!("machine-empty-{key}")).w_full().items_center().gap(4.0).px(10.5).py(14.0).child(
        text(if filtered { "No sessions found" } else { "No sessions yet" })
            .font(sans(12.25, 500.0, 1.3))
            .color(t.text)
            .align(Align::Center),
    );
    if !filtered {
        // The button asks for a session on this machine; the new-session page takes a machine in component 7.
        empty = empty.child(
            row()
                .key(format!("machine-new-session-{key}"))
                .mt(4.0)
                .h(28.0)
                .px(10.0)
                .center()
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .child(text("New session").font(sans(12.0, 500.0, 1.3)).color(t.text)),
        );
    }
    empty
}

/// The row at the foot of the machines (`.sessions-add-machine`).
pub fn add_machine_row(t: &Theme) -> El {
    row()
        .key("sessions-add-machine")
        .w_full()
        .center_y()
        .gap(6.0)
        .px(12.0)
        .py(6.0)
        .cursor(Cursor::Pointer)
        .hover_group()
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .on_press(|s: &mut State| s.add_machine())
        .child(icon(&PLUS, 13.0).color(t.muted.alpha(0.8)).group_color(t.text))
        .child(text("Add a machine").font(sans(12.0, 400.0, 1.3)).color(t.muted.alpha(0.8)).group_color(t.text))
}
