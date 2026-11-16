//! One session on the home dashboard (`DashboardSessionRow.vue`): what it is, where, how far along and what it needs. The row
//! opens the session on a click, Enter or Space. Its hover actions (Archive, Delete) are piece E's.
use super::progress_ring::progress_ring;
use super::session_row::{RowClock, glyph_el};
use crate::fleet::row_status::{self, RowItem, Tone};
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::{State, Tx};
use rune::{Align, Cursor, El, Key, KeyEvent, col, el, row, text};

/// The row's "project · branch" line: the project's name, else the folder's last part (else its group's name), then the branch.
pub fn place(session: &Session, project: &str) -> String {
    let dir = &session.listing.workspace_directory;
    let name = session
        .listing
        .project_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .or_else(|| dir.split(['/', '\\']).rfind(|p| !p.is_empty()).map(str::to_string))
        // An invented session has neither: its project group's name.
        .or_else(|| Some(project.to_string()));
    [name.unwrap_or_default(), session.branch.trim().to_string()].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// The row's key: `dash-row-<id>`, or its place in the list for an invented session (which has no id).
pub fn row_key(session: &Session, p: usize, i: usize) -> String {
    if session.id.is_empty() { format!("dash-row-{p}-{i}") } else { format!("dash-row-{}", session.id) }
}

/// A row. `p`, `i` are the session's place in `State::projects`, which opening it goes by.
pub fn dashboard_row(s: &State, t: &Theme, clock: &RowClock, p: usize, i: usize, session: &Session) -> El {
    let item = RowItem::of(session, clock.now);
    let status = row_status::row_status(&item, clock.now, clock.offset_secs, true);
    let key = row_key(session, p, i);
    let title = row_status::display_title(None, &session.title);

    // A live session has its glyph; a quiet one a 6 px dot, so titles keep one left edge.
    let glyph = match glyph_el(t, row_status::glyph(&item)) {
        Some(g) => g.help(status.description.clone()),
        None => el().size(6.0, 6.0).radius(3.0).fill(t.muted.alpha(0.45)),
    };
    let glyph = el().size(14.0, 14.0).center().child(glyph);

    let place = place(session, s.projects.get(p).map_or("", |x| x.name.as_str()));
    let mut main = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(title.clone()).key(format!("{key}-title")).font(sans(13.0, 500.0, 1.35)).color(t.text).truncate());
    if !place.is_empty() {
        main = main.child(text(place).key(format!("{key}-place")).font(sans(12.0, 400.0, 1.35)).color(t.muted).truncate());
    }

    let mut line = row()
        .key(key.clone())
        .w_full()
        .min_h(48.0)
        .px(10.0)
        .py(6.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .hover(t.wash(5.0))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, -2.0, t.accent)
        .label(format!("{title}, {}", status.description))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.open_session(p, i, tx))
        // Enter and Space open it too (`@keydown.enter.prevent`, `@keydown.space.prevent`).
        .on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            let open = match &ev.key {
                Key::Enter => true,
                Key::Text(t) => t == " ",
                _ => false,
            } && !ev.mods.ctrl
                && !ev.mods.cmd
                && !ev.mods.alt;
            if open {
                s.open_session(p, i, tx);
            }
            open
        })
        .child(glyph)
        .child(main);

    let live = row_status::is_live(item.status);
    if let Some(progress) = session.row.progress.as_ref().filter(|p| p.total > 0) {
        let owner = if session.id.is_empty() { key.clone() } else { session.id.clone() };
        let mut group = row().key(format!("{key}-progress")).gap(6.0).center_y().shrink();
        if let Some(now) = progress.current.as_deref().filter(|c| live && !c.is_empty()) {
            group = group.child(text(now.to_string()).font(sans(12.0, 400.0, 1.35)).color(t.muted).truncate());
        }
        line = line.child(group.child(progress_ring(&s.row_rings, t, &owner, progress.done, progress.total, t.panel_bg)).child(
            text(format!("{}/{}", progress.done, progress.total)).font(sans(12.0, 400.0, 1.35).features("'tnum' 1")).color(t.muted),
        ));
    }
    if session.row.archived {
        line = line.child(
            text("Archived")
                .key(format!("{key}-archived"))
                .font(sans(11.5, 400.0, 1.3))
                .color(t.muted)
                .px(7.0)
                .py(1.0)
                .radius(RADIUS_BTN)
                .fill(t.wash(7.0)),
        );
    }
    if !status.label.is_empty() {
        let (ink, weight) = match status.tone {
            Tone::Attention => (t.status_waiting_text, 500.0),
            Tone::Error => (t.error_text, 500.0),
            Tone::Retry => (t.status_waiting_text, 400.0),
            _ => (t.muted, 400.0),
        };
        let mut word = text(status.label.clone())
            .key(format!("{key}-status"))
            .font(sans(12.0, weight, 1.35).features("'tnum' 1"))
            .color(ink)
            .min_w(28.0)
            .align(Align::End);
        if status.tone == Tone::Attention {
            word = word.px(7.0).py(1.0).radius(RADIUS_BTN).fill(t.status_waiting().alpha(0.14));
        }
        if let Some(tip) = &status.title {
            word = word.help(tip.clone());
        }
        line = line.child(word);
    }
    line
}
