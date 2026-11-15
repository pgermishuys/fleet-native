//! The session list (`SessionsPanel.vue`, `ProjectGroup.vue`): New session, the filter, sessions grouped by project,
//! and the Archived link. A row is `session_row.rs`.
use super::session_row::session_row;
use crate::State;
use crate::fleet::groups::{matching_sessions, session_matches_query};
use crate::icons::*;
use crate::mock::{Project, Session};
use crate::theme::{Theme, sans};
use rune::{Cursor, El, ScrollSpec, col, el, icon, row, text, text_input};

pub fn panel(s: &State, t: &Theme) -> El {
    let new_session = row()
        .key("new-session")
        .grow(1.0)
        .h(32.0)
        .px(12.0)
        .gap(5.25)
        .center()
        .radius(8.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .hover(t.wash(3.0))
        .press(t.wash(6.0))
        .cursor(Cursor::Pointer)
        .on_press(|_: &mut State| {})
        .child(icon(&PLUS, 15.0).color(t.text))
        .child(text("New session").font(sans(13.0, 500.0, 1.3)).color(t.text));
    let add_folder = el()
        .key("add-folder")
        .size(32.0, 32.0)
        .center()
        .radius(8.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .help("Add a project folder")
        .label("Add a project folder")
        .on_press(|_: &mut State| {})
        .child(icon(&FOLDER_PLUS, 15.0).color(t.muted).group_color(t.text));

    // The field is the whole 32px box, so a click anywhere in it focuses it; the icon sits on top.
    let filter = el().w_full().px(8.0).pb(10.0).child(
        el().w_full()
            .child(
                text_input(&s.filter, sans(13.0, 400.0, 1.3))
                    .key("filter")
                    .label("Filter sessions")
                    .single_line()
                    .placeholder("Filter sessions", t.muted)
                    .color(t.text)
                    .caret(t.text, t.accent.alpha(0.35))
                    .w_full()
                    .h(32.0)
                    .pl(30.0)
                    .pr(10.0)
                    .py(7.5)
                    .radius(8.0)
                    .fill(t.wash(5.0))
                    .on_edit(|s: &mut State| &mut s.filter),
            )
            .child(icon(&SEARCH, 13.0).color(t.muted).absolute(10.0, 9.5)),
    );

    let needle = s.filter.text.trim().to_lowercase();
    let clock = super::session_row::RowClock::now(s);
    let mut list = col().w_full().px(8.0).pb(12.0);
    let mut first = true;
    for (p, project) in s.projects.iter().enumerate() {
        // A project whose name matches keeps all its sessions; a group with no match and another name is dropped.
        let matches = |session: &Session| session_matches_query(session, &needle, &s.project_load.projects);
        let Some(kept) = matching_sessions(&project.name, &project.sessions, &needle, matches) else { continue };
        let rows: Vec<(usize, &Session)> = kept.into_iter().map(|i| (i, &project.sessions[i])).collect();
        let mut group = col().w_full().mt(if first { 0.0 } else { 10.0 }).child(project_header(t, p, project, rows.len()));
        first = false;
        // A filter shows its matches even in a folded project.
        if project.open || !needle.is_empty() {
            let mut body = col().w_full().pt(2.0);
            for (i, session) in rows {
                body = body.child(session_row(s, t, &clock, p, i, session));
            }
            group = group.child(body);
        }
        list = list.child(group);
    }
    if first {
        list = list.child(text("No sessions match.").font(sans(12.0, 400.0, 1.3)).color(t.muted).px(10.0).py(8.0));
    }

    let archived = row().w_full().pt(6.0).px(8.0).pb(8.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(
        row()
            .key("archived")
            .h(28.0)
            .px(8.0)
            .gap(7.0)
            .center_y()
            .radius(8.0)
            .hover(t.wash(5.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .on_press(|_: &mut State| {})
            .child(icon(&ARCHIVE, 13.0).color(t.muted).group_color(t.text))
            .child(text("Archived").font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text)),
    );

    col()
        .key("session-list")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(row().w_full().pt(8.0).px(8.0).pb(8.0).gap(6.0).child(new_session).child(add_folder))
        .child(filter)
        .child(col().key("sessions").w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .child(archived)
}

fn project_header(t: &Theme, p: usize, project: &Project, count: usize) -> El {
    let turn = if project.open { 0.0 } else { -std::f32::consts::FRAC_PI_2 };
    row()
        .key(format!("project-{p}"))
        .w_full()
        .min_h(26.0)
        .center_y()
        .gap(6.0)
        .pl(6.0)
        .pr(10.0)
        .radius(8.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.projects[p].open = !s.projects[p].open)
        .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted.alpha(0.7)).rotate(turn))
        .child(text(project.name.clone()).font(sans(12.0, 600.0, 1.3)).color(t.muted).group_color(t.text).truncate().grow(1.0))
        .child(text(count.to_string()).font(sans(12.0, 500.0, 1.3).features("'tnum' 1")).color(t.muted.alpha(0.7)))
}
