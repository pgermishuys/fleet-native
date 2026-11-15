//! A session's row in the list (`SessionItem.vue`, `StatusGlyph.vue`): the caret's place, the status glyph, the title and
//! the word on the right, the running chip, the pull request badge and the progress ring (`ProgressRing.vue`, `PrBadge.vue`).
//! Hover buttons, rename, the menu and lineage come after (docs/parity/06-session-list.md, rows 3.x).
use super::parts::{cut_dot, status_glyph};
use super::progress_ring::progress_ring;
use crate::fleet::actions::RENAME_FIELD;
use crate::fleet::row_chips::{self, Checks, PrBadge, PrState};
use crate::fleet::row_status::{self, Dim, Glyph, RowItem, RowStatus, Tone};
use crate::icons::{CHEVRON_RIGHT, GIT_MERGE, GIT_PULL_REQUEST, GIT_PULL_REQUEST_CLOSED, GIT_PULL_REQUEST_DRAFT, LOADER_CIRCLE};
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::{State, Tx};
use rune::{Color, Cursor, El, Key, KeyEvent, el, icon, row, text, text_input};

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

fn glyph_el(t: &Theme, g: Glyph) -> Option<El> {
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

/// What a row's right edge shows in place of its status word: a workflow step the user finishes (`stepNote`) or how the
/// session came from its parent (`kindLabel`). Nothing fills them yet: workflows and lineage are other pieces.
#[derive(Clone, Copy, Default)]
pub struct Slots<'a> {
    pub step_note: Option<&'a str>,
    pub kind_label: Option<&'a str>,
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

    let glyph = match glyph_el(t, row_status::glyph(&item)) {
        Some(g) => g.key(format!("session-glyph-{key}")).help(status.description.clone()),
        // Nothing live: the empty slot keeps titles on one left edge.
        None => el().size(8.0, 8.0).key(format!("session-slot-{key}")),
    };
    if s.row_renaming(p, i) {
        return el().w_full().py(1.0).child(editing(s, t, &key, glyph));
    }
    let title = row_status::display_title(None, &session.title);
    let mut line = row()
        .key(format!("session-{key}"))
        .w_full()
        .min_h(32.0)
        .center_y()
        .gap(9.0)
        .pl(6.0)
        .pr(10.0)
        .radius(RADIUS_BTN)
        .fill(if active { t.wash(9.0) } else { Color::TRANSPARENT })
        .hover(if active { Color::TRANSPARENT } else { t.wash(5.0) })
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .help("Double-click to rename")
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.open_session(p, i, tx))
        .on_double_press_tx(move |s: &mut State, tx: &mut Tx| s.start_row_rename(p, i, tx))
        // `handleRowKeydown`: F2 renames.
        .on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            if ev.key == Key::Function(2) {
                s.start_row_rename(p, i, tx);
                return true;
            }
            false
        })
        .on_secondary_press(move |s: &mut State, at, _: &mut Tx| {
            s.ui.point = (at.x, at.y);
            s.ui.open = Some(crate::ui::Open::Context(p, i));
        })
        // The place a row with children puts its caret in: 14x14, tucked 4px toward the glyph (`.session-caret-slot`).
        .child(el().key(format!("session-caret-{key}")).size(14.0, 14.0).mr(-4.0))
        .child(glyph)
        .child(
            text(title)
                .key(format!("session-title-{key}"))
                .font(sans(13.0, if active { 500.0 } else { 400.0 }, 1.3))
                .color(title_ink(t, active, dim))
                .group_color(t.text)
                .truncate()
                .grow(1.0),
        );
    if session.row.running_work > 0 {
        line = line.child(running_chip(t, &key, session.row.running_work));
    }
    if let Some(pr) = &session.row.pr {
        line = line.child(pr_badge(t, &key, pr));
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
        line = line.child(group);
    }
    // One of: the step note, the kind label, the status word (which the count replaces).
    if let Some(note) = slots.step_note {
        line = line.child(
            text(note.to_string())
                .key(format!("session-step-note-{key}"))
                .font(sans(12.0, 400.0, 1.3).features("'tnum' 1"))
                .color(t.text_mix(t.accent, t.row_accent)),
        );
    } else if let Some(kind) = slots.kind_label {
        // Lineage stays muted in every state (decision of 17 October 2026): the row's meta colour, set on the label itself.
        line = line.child(
            text(kind.to_uppercase())
                .key(format!("session-kind-{key}"))
                .font(sans(10.5, 600.0, 1.2).tracking(0.04 * 10.5))
                .color(t.row_meta_ink()),
        );
    } else if !progress.is_some_and(|v| v.count)
        && let Some(word) = word(t, &key, &status, dim)
    {
        line = line.child(word);
    }
    el().w_full().py(1.0).child(line)
}

/// `.session-item--editing`: the row with the title as a field, no hover, no pick, nothing on the right; the field has
/// the raw title selected (`.session-title--editing`: the title's own size, no border, no padding).
fn editing(s: &State, t: &Theme, key: &str, glyph: El) -> El {
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
        .key(format!("session-{key}"))
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
/// children show. Drawn by the lineage piece; here so the glyph exists beside the slot it replaces.
#[allow(dead_code)]
pub fn caret(t: &Theme, open: bool) -> El {
    let turn = if open { std::f32::consts::FRAC_PI_2 } else { 0.0 };
    el().size(14.0, 14.0).mr(-4.0).radius(4.0).center().child(rune::icon(&CHEVRON_RIGHT, 14.0).color(t.muted).rotate(turn))
}
