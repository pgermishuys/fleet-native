//! A session's row in the list (`SessionItem.vue`, `StatusGlyph.vue`): the caret's place, the status glyph, the title and
//! the word on the right. Chips, progress and the ring are piece D; hover buttons, rename, the menu and lineage come
//! after (docs/parity/06-session-list.md, rows 3.x).
use super::parts::{cut_dot, status_glyph};
use crate::fleet::row_status::{self, Dim, Glyph, RowItem, RowStatus, Tone};
use crate::icons::CHEVRON_RIGHT;
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::{State, Tx};
use rune::{Color, Cursor, El, el, row, text};

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

pub fn session_row(s: &State, t: &Theme, clock: &RowClock, p: usize, i: usize, session: &Session) -> El {
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
        .focus_ring(2.0, -2.0, t.accent)
        .help("Double-click to rename")
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.open_session(p, i, tx))
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
    if let Some(word) = word(t, &key, &status, dim) {
        line = line.child(word);
    }
    el().w_full().py(1.0).child(line)
}

/// The chevron a parent's row carries in place of the empty slot (`.session-caret`): muted, rotated 90 degrees while the
/// children show. Drawn by the lineage piece; here so the glyph exists beside the slot it replaces.
#[allow(dead_code)]
pub fn caret(t: &Theme, open: bool) -> El {
    let turn = if open { std::f32::consts::FRAC_PI_2 } else { 0.0 };
    el().size(14.0, 14.0).mr(-4.0).radius(4.0).center().child(rune::icon(&CHEVRON_RIGHT, 14.0).color(t.muted).rotate(turn))
}
