//! One session on the home dashboard (`DashboardSessionRow.vue`): what it is, where, how far along and what it needs. The row
//! opens the session on a click, Enter or Space. Beside it, on this machine's rows, Archive and Delete show while the row has the
//! pointer or the keyboard; another machine's rows only open.
use super::progress_ring::progress_ring;
use super::row_actions::Buttons;
use super::session_row::{RowClock, glyph_el};
use crate::fleet::row_status::{self, RowItem, Tone};
use crate::icons::{ARCHIVE, TRASH_2};
use crate::mock::Session;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::{State, Tx};
use rune::{Align, Cursor, El, Key, KeyEvent, col, el, icon, row, text};

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

/// Where a row's session is: the live machine's list (its place in `State::projects`, which opening it goes by), or the page's
/// own list of another machine (`readOnly`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum At {
    Here(usize, usize),
    There,
}

/// The row's key: `dash-row-<id>`, or its place in the list for an invented session (which has no id).
pub fn row_key(session: &Session, at: At) -> String {
    match (session.id.is_empty(), at) {
        (false, _) => format!("dash-row-{}", session.id),
        (true, At::Here(p, i)) => format!("dash-row-{p}-{i}"),
        (true, At::There) => "dash-row-there".to_string(),
    }
}

/// What a click, Enter or Space does: opens the session on this machine, or on the other one it was read from
/// (`handleSessionSelect`).
fn opener(session: &Session, at: At) -> impl Fn(&mut State, &mut Tx) + 'static {
    let copy = session.row_copy();
    move |s: &mut State, tx: &mut Tx| match at {
        At::Here(p, i) => s.open_session(p, i, tx),
        At::There => {
            if let Some(machine) = s.dash.other.key.clone() {
                s.open_machine_row(&machine, copy.row_copy(), tx);
            }
        }
    }
}

/// A row, with its hover actions beside it.
pub fn dashboard_row(s: &State, t: &Theme, clock: &RowClock, at: At, session: &Session) -> El {
    let line = row_line(s, t, clock, at, session);
    let At::Here(p, i) = at else { return line };
    // The shell holds the pointer for the buttons beside the row: they show while it is over either.
    let key = row_key(session, at);
    let mut shell = el().key(format!("{key}-shell")).w_full().hover_group().child(line);
    let buttons = actions(t, &key, session, p, i);
    if !buttons.is_empty() {
        shell = shell.child(
            row()
                .key(format!("{key}-actions"))
                .gap(2.0)
                .pl(16.0)
                .pin(None, Some(10.0), Some(6.0), None)
                .fill(t.mix(t.text, t.panel_bg, 0.05))
                .group_opacity(0.0, 1.0)
                .children(buttons),
        );
    }
    shell
}

fn action(t: &Theme, key: String, label: &str, glyph: &'static rune::Icon, danger: bool) -> El {
    let (wash, ink) = if danger { (t.error.alpha(0.12), t.error_text) } else { (t.wash(8.0), t.text) };
    el().key(key)
        .role("button")
        .label(label.to_string())
        .help(label.split(' ').next().unwrap_or(label).to_string())
        .size(28.0, 28.0)
        .center()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -2.0, t.accent)
        .hover(wash)
        .hover_group()
        .child(icon(glyph, 14.0).color(t.muted).group_color(ink))
}

/// Archive (the same six-second Undo as the list, `archiveQueue.archive`) and Delete (asks first), for what the session allows
/// (`canArchive`, `canDelete`).
fn actions(t: &Theme, key: &str, session: &Session, p: usize, i: usize) -> Vec<El> {
    let mut out = Vec::new();
    if Buttons::of(session, false).archive {
        let id = session.id.clone();
        out.push(
            action(t, format!("{key}-archive"), "Archive session", &ARCHIVE, false)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.archive_sessions(std::slice::from_ref(&id), None, tx)),
        );
    }
    if session.row.capabilities.can_delete.unwrap_or(true) {
        out.push(
            action(t, format!("{key}-delete"), "Delete session", &TRASH_2, true)
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.ask_delete_session(p, i, tx)),
        );
    }
    out
}

/// The row itself: what the session is, where, how far along and what it needs. `p`, `i` of `At::Here` are the session's place
/// in `State::projects`, which opening it goes by.
fn row_line(s: &State, t: &Theme, clock: &RowClock, at: At, session: &Session) -> El {
    let item = RowItem::of(session, clock.now);
    let status = row_status::row_status(&item, clock.now, clock.offset_secs, true);
    let key = row_key(session, at);
    let title = row_status::display_title(None, &session.title);

    // A live session has its glyph; a quiet one a 6 px dot, so titles keep one left edge.
    let glyph = match glyph_el(t, row_status::glyph(&item)) {
        Some(g) => g.help(status.description.clone()),
        None => el().size(6.0, 6.0).radius(3.0).fill(t.muted.alpha(0.45)),
    };
    let glyph = el().size(14.0, 14.0).center().child(glyph);

    let project = match at {
        At::Here(p, _) => s.projects.get(p).map_or("", |x| x.name.as_str()),
        At::There => "",
    };
    let place = place(session, project);
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
        .on_press_tx({
            let open = opener(session, at);
            move |s: &mut State, tx: &mut Tx| open(s, tx)
        })
        // Enter and Space open it too (`@keydown.enter.prevent`, `@keydown.space.prevent`).
        .on_key({
            let open = opener(session, at);
            move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
                let go = match &ev.key {
                    Key::Enter => true,
                    Key::Text(t) => t == " ",
                    _ => false,
                } && !ev.mods.ctrl
                    && !ev.mods.cmd
                    && !ev.mods.alt;
                if go {
                    open(s, tx);
                }
                go
            }
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
