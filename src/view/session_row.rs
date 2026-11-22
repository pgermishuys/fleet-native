//! A session's row in the list (`SessionItem.vue`, `StatusGlyph.vue`): the caret's place, the status glyph, the title and
//! the word on the right, the running chip, the pull request badge, the progress ring (`ProgressRing.vue`, `PrBadge.vue`) and
//! the In terminal chip and From terminal marker of a Claude Code session started in a terminal (#1099).
//! Hover buttons, rename, the menu and lineage come after (docs/parity/06-session-list.md, rows 3.x).
use super::parts::{cut_dot, status_glyph};
use super::progress_ring::progress_ring;
use super::row_actions::{Buttons, actions};
use crate::fleet::actions::RENAME_FIELD;
use crate::fleet::row_chips::{self, Checks, PrBadge, PrState};
use crate::fleet::row_status::{self, Dim, Glyph, RowItem, RowStatus, Tone};
use crate::icons::{
    CHECK, CHEVRON_RIGHT, GIT_MERGE, GIT_PULL_REQUEST, GIT_PULL_REQUEST_CLOSED, GIT_PULL_REQUEST_DRAFT, LOADER_CIRCLE, SQUARE_TERMINAL,
};
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Color, Cursor, Drag, El, Key, KeyEvent, Phase, el, icon, row, text};

/// The clock a list is drawn by: read once for all rows, so they agree and the time zone is asked once.
pub struct RowClock {
    pub now: i64,
    pub offset_secs: i64,
}

impl RowClock {
    pub fn now(s: &State) -> RowClock {
        RowClock { now: super::message::now_ms(s), offset_secs: crate::conv::time::local_offset_secs() }
    }
}

/// The colour of a row's word (`.session-meta--<tone>`); `None` for a working row, which draws no word.
pub fn meta_ink(t: &Theme, tone: Tone, dim: Dim) -> Option<Color> {
    Some(match tone {
        Tone::Working => return None,
        Tone::Attention | Tone::Retry => t.text_mix(t.idle, t.row_waiting),
        Tone::Error => t.text_mix(t.error, t.row_error),
        // The quietest rows' age fades with the title, to the same floor.
        Tone::Quiet if dim == Dim::Two => t.text.alpha(t.row_dim[1]),
        Tone::Quiet => t.row_meta_ink(),
    })
}

/// The colour of a row's title at rest: `--text` for the open row, else 86% of it, or `--row-dim-1/2` once quiet for
/// a day or three. Hovering restores `--text`.
pub fn title_ink(t: &Theme, active: bool, dim: Dim) -> Color {
    match (active, dim) {
        (true, _) => t.text,
        (_, Dim::One) => t.text.alpha(t.row_dim[0]),
        (_, Dim::Two) => t.text.alpha(t.row_dim[1]),
        _ => t.text.alpha(0.86),
    }
}

pub fn glyph_el(t: &Theme, g: Glyph) -> Option<El> {
    Some(match g {
        Glyph::Quiet => return None,
        Glyph::Cut => cut_dot(t.idle),
        Glyph::Working => status_glyph(t, "active", false),
        Glyph::Retry => status_glyph(t, "active", true),
        Glyph::Waiting => status_glyph(t, "waiting_input", false),
        Glyph::Error => status_glyph(t, "error", false),
    })
}

/// The word on the right: plain text, or the "Needs input" pill.
fn word(t: &Theme, id: &str, status: &RowStatus, dim: Dim) -> Option<El> {
    let ink = meta_ink(t, status.tone, dim)?;
    if status.label.is_empty() {
        return None;
    }
    let weight = if status.tone == Tone::Attention { 500.0 } else { 400.0 };
    let mut label =
        text(status.label.clone()).font(sans(12.0, weight, 1.3).features("'tnum' 1")).color(ink).key(format!("session-status-{id}"));
    if status.tone == Tone::Quiet {
        // Dim-1 leaves the age alone; dim-2 fades it with the title but restores it under the pointer.
        label = label.group_color(t.row_meta_ink());
    }
    if status.tone == Tone::Attention {
        label = label.px(7.0).py(1.0).radius(RADIUS_BTN).fill(t.idle.alpha(0.14));
    }
    if let Some(title) = &status.title {
        label = label.help(title.clone());
    }
    Some(label)
}

/// What a row's right edge shows in place of its status word: a workflow step the user finishes (`stepNote`, workflows are
/// another component) or how the session came from its parent (`kindLabel`); and what lineage adds at its left.
#[derive(Clone, Copy, Default)]
pub struct Slots<'a> {
    /// A workflow step's name, which the row shows in place of the session's title (`label`).
    pub label: Option<&'a str>,
    /// A step row of a workflow run's group: no caret place, and the glyph 10 px in (`.wf-group__step`'s `SessionItem`).
    pub step: bool,
    pub step_note: Option<&'a str>,
    pub kind_label: Option<&'a str>,
    /// `Some(open)` on a top-level row with something under it: its caret, and Right and Left fold it.
    pub caret: Option<bool>,
    /// The content's left edge on a child row (`--lineage-indent`); a child has no caret place and is 28 px tall.
    pub child: Option<f32>,
}

/// The chip counting what the agent has running (`.session-running-chip`): green on a 14% green pill.
fn running_chip(t: &Theme, key: &str, count: u32) -> El {
    let ink = t.text_mix(t.running, t.row_running);
    row()
        .key(format!("session-running-chip-{key}"))
        .h(18.0)
        .px(6.0)
        .gap(3.0)
        .center_y()
        .radius(999.0)
        .fill(t.running.alpha(0.14))
        .help(row_chips::running_label(count))
        .child(icon(&LOADER_CIRCLE, 11.0).color(ink))
        .child(text(count.to_string()).font(sans(11.0, 600.0, 1.3).features("'tnum' 1")).color(ink))
}

/// The pull request badge (`PrBadge.vue`): the state's icon, `#<number>` in mono and a dot for the checks. The icon keeps
/// the state's colour whole; the number takes as much of it as still reads on a row (`--row-*`).
fn pr_badge(t: &Theme, key: &str, pr: &PrBadge) -> El {
    // `--pr-open` is `--running`, `--pr-blocked` `--status-waiting`, `--pr-merged` `--queued`, `--pr-closed` `--error`,
    // `--pr-draft` `--muted`.
    let (glyph, color, ink) = match pr.state {
        PrState::Open => (&GIT_PULL_REQUEST, t.running, t.text_mix(t.running, t.row_running)),
        PrState::Draft => (&GIT_PULL_REQUEST_DRAFT, t.muted, t.row_meta_ink()),
        PrState::Blocked => (&GIT_PULL_REQUEST, t.idle, t.text_mix(t.idle, t.row_waiting)),
        PrState::Merged => (&GIT_MERGE, t.queued, t.text_mix(t.queued, t.row_queued)),
        PrState::Closed => (&GIT_PULL_REQUEST_CLOSED, t.error, t.text_mix(t.error, t.row_error)),
    };
    let mut badge = row()
        .key(format!("session-pr-{key}"))
        .gap(3.0)
        .pb(1.0)
        .center_y()
        .help(pr.description.clone())
        .child(icon(glyph, 12.0).color(color))
        .child(text(format!("#{}", pr.number)).font(mono(11.0, 400.0, 1.3).features("'tnum' 1")).color(ink));
    if pr.shows_checks() {
        // `--check-pass` is `--running`, `--check-fail` `--error`, `--check-pending` `--status-waiting`.
        let dot = match pr.checks {
            Checks::Passing => t.running,
            Checks::Failing => t.error,
            _ => t.idle,
        };
        badge = badge.child(el().size(6.0, 6.0).ml(2.0).radius(3.0).fill(dot));
    }
    badge
}

pub fn session_row(s: &State, t: &Theme, clock: &RowClock, p: usize, i: usize, session: &Session) -> El {
    session_row_with(s, t, clock, p, i, session, Slots::default())
}

pub fn session_row_with(s: &State, t: &Theme, clock: &RowClock, p: usize, i: usize, session: &Session, slots: Slots) -> El {
    let active = s.open_at() == (p, i);
    let item = RowItem::of(session, clock.now);
    let status = row_status::row_status(&item, clock.now, clock.offset_secs, true);
    // The open row never dims (`rowDim = active ? 0 : sessionRowDim`).
    let dim = if active { Dim::None } else { row_status::row_dim(&item, clock.now) };
    let key = format!("{p}-{i}");

    // Picking rows swaps the glyph for a checkbox in the same slot (`.session-check`).
    let selecting = s.selection.is_selecting_on(None);
    let picked = selecting && s.selection.is_selected(&session.id);
    let glyph = match glyph_el(t, row_status::glyph(&item)) {
        Some(g) => g.key(format!("session-glyph-{key}")).help(status.description.clone()),
        // Nothing live: the empty slot keeps titles on one left edge.
        None => el().size(8.0, 8.0).key(format!("session-slot-{key}")),
    };
    let glyph = if selecting { check_box(t, &key, picked) } else { glyph };
    if s.row_renaming(p, i) {
        return el().w_full().py(1.0).child(editing(s, t, format!("session-{key}"), glyph));
    }
    let title = row_status::display_title(slots.label, &session.title);
    let caret_open = slots.caret;
    // The place a row with children puts its caret in: 14x14, tucked 4px toward the glyph (`.session-caret-slot`). A child has none.
    let slot = (slots.child.is_none() && !slots.step).then(|| match slots.caret {
        Some(open) => caret(t, &key, &session.id, open),
        None => el().key(format!("session-caret-{key}")).size(14.0, 14.0).mr(-4.0),
    });
    let mut line = row()
        .key(format!("session-{key}"))
        .w_full()
        .min_h(if slots.child.is_some() { 28.0 } else { 32.0 })
        .center_y()
        .gap(9.0)
        .pl(slots.child.unwrap_or(if slots.step { 10.0 } else { 6.0 }))
        .pr(10.0)
        .radius(RADIUS_BTN)
        .fill(if picked {
            t.accent_dim
        } else if active {
            t.wash(9.0)
        } else {
            Color::TRANSPARENT
        })
        .hover(if picked {
            t.accent_dim
        } else if active {
            Color::TRANSPARENT
        } else {
            t.wash(5.0)
        })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .help("Double-click to rename")
        .on_press_tx({
            let id = session.id.clone();
            move |s: &mut State, tx: &mut Tx| s.press_row(&id, None, move |s, tx| s.open_session(p, i, tx), tx)
        })
        .on_double_press_tx(move |s: &mut State, tx: &mut Tx| s.start_row_rename(p, i, tx))
        .opacity(if s.row_drag.as_ref().is_some_and(|d| d.id == session.id) { 0.4 } else { 1.0 })
        .on_drag({
            let id = session.id.clone();
            move |s: &mut State, d: &mut Drag, tx: &mut Tx| match d.phase {
                Phase::Down => {}
                Phase::Move => s.row_drag_move(&id, d.window_point(), tx),
                Phase::Up => s.row_drag_end(d.cancelled(), tx),
            }
        })
        // `handleRowKeydown`: F2 renames.
        .on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            let id = s.projects.get(p).and_then(|project| project.sessions.get(i)).map(|x| x.id.clone()).unwrap_or_default();
            if ev.key == Key::Function(2) {
                s.start_row_rename(p, i, tx);
                return true;
            }
            // A row with children opens and closes like a tree item; a key that changes nothing is left alone.
            if let (Some(open), Key::Right | Key::Left) = (caret_open, &ev.key) {
                let want = ev.key == Key::Right;
                if want != open {
                    s.set_family_open(&id, want);
                    return true;
                }
                return false;
            }
            move_pin_key(s, ev, &id)
        })
        .on_secondary_press(move |s: &mut State, at, tx: &mut Tx| {
            s.ui.show_context(crate::ui::Open::Context(p, i), (at.x, at.y));
            let dir = s.projects.get(p).and_then(|x| x.sessions.get(i)).map(|x| x.listing.workspace_directory.clone());
            s.open_in_prepare(&dir.unwrap_or_default(), tx);
        })
        .children(slot)
        .child(glyph)
        .child(
            text(title.clone())
                .key(format!("session-title-{key}"))
                .font(sans(13.0, if active { 500.0 } else { 400.0 }, 1.3))
                // A picked row has full contrast, like the open one (`.session-item--dim-n:not(.session-item--selected)`).
                .color(if picked { t.text } else { title_ink(t, active, dim) })
                .group_color(t.text)
                .truncate()
                .grow(1.0),
        );
    // The buttons give way to the checkbox while rows are picked (`canPin && !isSelecting`).
    let buttons = if selecting { Buttons::default() } else { Buttons::of(session, false) };
    line = tail(s, t, &key, session, &status, dim, slots, buttons, line);
    // The shell holds the pointer for the buttons beside the row: they show while it is over either.
    let mut shell = el().w_full().py(1.0).hover_group().child(line);
    if s.pin_line_before(&session.id) {
        shell = shell.child(super::drop_zones::pin_line(t, s.sidebar_w - 36.0).absolute(10.0, -1.0));
    }
    for button in actions(s, t, &key, session, &title, buttons, None) {
        shell = shell.child(button);
    }
    shell
}

/// `.session-check`: 14 square in the glyph's 8px slot (3px of margin each side), a 1.5px muted ring, the accent filled with a
/// check once picked.
pub fn check_box(t: &Theme, key: &str, picked: bool) -> El {
    let b = el()
        .key(format!("session-check-{key}"))
        .size(14.0, 14.0)
        .mx(-3.0)
        .center()
        .radius(4.0)
        .border(1.5, if picked { t.accent } else { t.muted.alpha(0.7) });
    if picked {
        return b.fill(t.accent).child(icon(&CHECK, 10.0).color(t.primary_fg));
    }
    b
}

/// `handleRowKeydown`: Alt+Up and Alt+Down move a pinned row's pin one place.
pub fn move_pin_key(s: &mut State, ev: &KeyEvent, id: &str) -> bool {
    if !ev.mods.alt || !matches!(ev.key, Key::Up | Key::Down) || !s.is_pinned(id) {
        return false;
    }
    s.move_pin(id, if ev.key == Key::Up { -1 } else { 1 });
    true
}

/// What follows the title on a row: the running chip, the pull request, the progress ring and one of the step note, the kind
/// label and the status word. Both the list's own rows and another machine's rows end like this.
#[allow(clippy::too_many_arguments)]
pub fn tail(
    s: &State,
    t: &Theme,
    key: &str,
    session: &Session,
    status: &RowStatus,
    dim: Dim,
    slots: Slots,
    buttons: Buttons,
    mut line: El,
) -> El {
    let key = key.to_string();
    if session.row.running_work > 0 {
        // The buttons take the right edge: the chip and the badge give way to two of them, the word and the ring to one.
        let chip = running_chip(t, &key, session.row.running_work);
        line = line.child(if buttons.two() { chip.group_opacity(1.0, 0.0) } else { chip });
    }
    // A row's own badge (mock data) first, else the one its session's pull request link gives.
    if let Some(pr) = session.row.pr.clone().or_else(|| s.smart_links.pr_badge(&session.id)) {
        let badge = pr_badge(t, &key, &pr);
        line = line.child(if buttons.two() { badge.group_opacity(1.0, 0.0) } else { badge });
    }
    let progress = row_chips::progress_view(session.row.progress.as_ref(), status.tone);
    if let Some(view) = progress {
        let owner = if session.id.is_empty() { key.clone() } else { session.id.clone() };
        let mut group = row()
            .key(format!("session-progress-{key}"))
            .gap(6.0)
            .center_y()
            .help(row_chips::progress_description(view.progress))
            .child(progress_ring(&s.row_rings, t, &owner, view.progress.done, view.progress.total, t.card_bg));
        if view.count {
            group = group.child(
                text(format!("{}/{}", view.progress.done, view.progress.total))
                    .key(format!("session-progress-count-{key}"))
                    .font(sans(12.0, 400.0, 1.3).features("'tnum' 1"))
                    .color(t.row_meta_ink()),
            );
        }
        line = line.child(if buttons.any() { group.group_opacity(1.0, 0.0) } else { group });
    }
    // One of: the step note, the kind label, the status word (which the count replaces).
    if let Some(note) = slots.step_note {
        line = line.child(
            text(note.to_string())
                .key(format!("session-step-note-{key}"))
                .font(sans(12.0, 400.0, 1.3).features("'tnum' 1"))
                .color(t.text_mix(t.accent, t.row_accent)),
        );
    } else if session.row.in_terminal {
        let chip = terminal_chip(t, &key);
        line = line.child(if buttons.any() { chip.group_opacity(1.0, 0.0) } else { chip });
    } else if let Some(kind) = slots.kind_label {
        // Lineage stays muted in every state (decision of 17 October 2026): the row's meta colour, set on the label itself.
        // Hidden while the row's buttons show (Fleet #1048), so pin and archive never cover it.
        let label = kind_text(t, kind).key(format!("session-kind-{key}"));
        line = line.child(if buttons.any() { label.group_opacity(1.0, 0.0) } else { label });
    } else if session.listing.spawn_kind.as_deref() == Some("terminal") && (status.label.is_empty() || status.tone == Tone::Quiet) {
        // Started in a terminal that has let go of it: the marker takes the age's place, and the age moves to its tooltip.
        let help = match &status.title {
            Some(at) => format!("Started in a terminal. Updated {at}"),
            None => "Started in a terminal".to_string(),
        };
        let label = kind_text(t, "From terminal").key(format!("session-from-terminal-{key}")).help(help);
        line = line.child(if buttons.any() { label.group_opacity(1.0, 0.0) } else { label });
    } else if !progress.is_some_and(|v| v.count)
        && let Some(word) = word(t, &key, status, dim)
    {
        line = line.child(if buttons.any() { word.group_opacity(1.0, 0.0) } else { word });
    }
    line
}

/// `.session-kind`: lineage's label and the From terminal marker, uppercase in the row's meta colour.
fn kind_text(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(10.5, 600.0, 1.2).tracking(0.04 * 10.5)).color(t.row_meta_ink())
}

/// `.session-terminal-chip`: a terminal has the session open. Quiet, in the row's meta colour on an 8% wash.
fn terminal_chip(t: &Theme, key: &str) -> El {
    let ink = t.row_meta_ink();
    row()
        .key(format!("session-terminal-chip-{key}"))
        .h(18.0)
        .px(6.0)
        .gap(4.0)
        .center_y()
        .radius(RADIUS_BTN)
        .fill(t.wash(8.0))
        .help("A terminal has this session open")
        .child(icon(&SQUARE_TERMINAL, 11.0).color(ink))
        .child(text("In terminal").font(sans(11.0, 600.0, 1.3)).color(ink))
}

/// `.session-item--editing`: the row with the title as a field, no hover, no pick, nothing on the right; the field has
/// the raw title selected (`.session-title--editing`: the title's own size, no border, no padding).
pub fn editing(s: &State, t: &Theme, row_key: String, glyph: El) -> El {
    let field = text_input(&s.act.edit, sans(13.0, 400.0, 1.2))
        .key(RENAME_FIELD)
        .label("Session name")
        .placeholder("Session name", t.row_meta_ink())
        .single_line()
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .min_w(0.0)
        .grow(1.0)
        .on_edit(|s: &mut State| &mut s.act.edit)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Enter => {
                s.finish_row_rename(true, tx);
                true
            }
            Key::Escape => {
                s.finish_row_rename(false, tx);
                true
            }
            _ => false,
        });
    row()
        .key(row_key)
        .w_full()
        .min_h(32.0)
        .center_y()
        .gap(9.0)
        .pl(6.0)
        .pr(10.0)
        .radius(RADIUS_BTN)
        .child(el().size(14.0, 14.0).mr(-4.0))
        .child(glyph)
        .child(field)
}

/// The chevron a parent's row carries in place of the empty slot (`.session-caret`): muted, rotated 90 degrees while the
/// children show. Always muted, hovered or not (decided 17 October 2026): nothing here changes its colour.
pub fn caret(t: &Theme, key: &str, id: &str, open: bool) -> El {
    let turn = if open { std::f32::consts::FRAC_PI_2 } else { 0.0 };
    let id = id.to_string();
    el().key(format!("session-caret-{key}"))
        .size(14.0, 14.0)
        .mr(-4.0)
        .radius(4.0)
        .center()
        .help(if open { "Hide what it started" } else { "Show what it started" })
        .on_press(move |s: &mut State| s.set_family_open(&id, !open))
        .child(rune::icon(&CHEVRON_RIGHT, 14.0).color(t.muted).rotate(turn))
}
