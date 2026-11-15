//! The session list (`SessionsPanel.vue`, `ProjectGroup.vue`): New session, the filter, sessions grouped by project,
//! and the Archived link. A row is `session_row.rs`.
use super::project_header::{Header, project_header, project_shell};
use super::session_row::session_row;
use crate::State;
use crate::fleet::groups::{matching_sessions, session_matches_query};
use crate::fleet::panel::{Feedback, banner_copy, empty_words, feedback};
use crate::icons::*;
use crate::mock::Session;
use crate::theme::{Theme, sans};
use rune::{Color, Cursor, El, ScrollSpec, col, el, icon, row, text, text_input};

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
        .help("New project")
        .label("New project")
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
                    // `:focus`: the sheet's own fill and the accent at 55 % for the 1px border that is transparent at rest.
                    // Fleet eases it over 150 ms; Rune can't (docs/RUNE.md).
                    .fill(if s.filter.focused { t.panel_bg } else { t.wash(5.0) })
                    .border(1.0, if s.filter.focused { t.accent.alpha(0.55) } else { Color::TRANSPARENT })
                    .on_edit(|s: &mut State| &mut s.filter),
            )
            .child(icon(&SEARCH, 13.0).color(t.muted).absolute(10.0, 9.5)),
    );

    let needle = s.filter.text.trim().to_lowercase();
    let clock = super::session_row::RowClock::now(s);
    let mut list = col().w_full().px(8.0).pb(12.0);
    if s.sample_headers {
        // The Pinned group is piece J's: until then a scene draws one header of each kind for the picture.
        let samples = [
            ("sample-pinned", Header::Pinned, "Pinned", 2, true),
            ("sample-ungrouped", Header::Ungrouped, "Ungrouped", 3, false),
            ("sample-project", Header::Project, "A project with a very long name that has to be cut short", 12, true),
        ];
        for (key, kind, name, count, open) in samples {
            list = list.child(col().w_full().mb(10.0).child(project_header(t, key.into(), kind, name, count, open, false)));
        }
    }
    let error = s.list_error();
    // Rows from an earlier read stay on screen under a banner that says the refresh failed.
    if let (Some(e), true) = (&error, s.has_rows()) {
        list = list.child(banner(t, e));
    }
    let mut first = true;
    for (p, project) in s.projects.iter().enumerate() {
        // A project whose name matches keeps all its sessions; a group with no match and another name is dropped.
        let matches = |session: &Session| session_matches_query(session, &needle, &s.project_load.projects);
        let Some(kept) = matching_sessions(&project.name, &project.sessions, &needle, matches) else { continue };
        let rows: Vec<(usize, &Session)> = kept.into_iter().map(|i| (i, &project.sessions[i])).collect();
        let kind = if project.id == crate::fleet::groups::UNGROUPED_ID { Header::Ungrouped } else { Header::Project };
        let header = if kind == Header::Project {
            project_shell(s, t, p, rows.len())
        } else {
            project_header(t, format!("project-{p}"), kind, &project.name, rows.len(), project.open, false)
                .on_press(move |s: &mut State| s.toggle_project(p))
        };
        let mut group = col().w_full().mt(if first { 0.0 } else { 10.0 }).child(header);
        first = false;
        // A folded project stays folded while a filter is typed: Fleet's `expanded` doesn't know the query.
        if project.open {
            let mut body = col().w_full().pt(2.0);
            for (i, session) in rows {
                body = body.child(session_row(s, t, &clock, p, i, session));
            }
            group = group.child(body);
        }
        list = list.child(group);
    }
    // Fleet counts a group even with no session in it, so "no group left" is `first` here.
    // No Fleet was found: nothing is listed, and there is nothing to try again.
    if s.no_fleet && !s.has_rows() {
        list = list.child(empty_state(t, "No Fleet found", "Start Fleet, or run fleet-native with --server <url>."));
    }
    let kind = feedback(s.list_is_loading(), error.as_deref(), s.has_rows(), usize::from(!first));
    match kind {
        Feedback::Loading => list = list.child(loading_block(t)),
        Feedback::Error(e) => list = list.child(error_block(t, &e)),
        Feedback::Empty if s.no_fleet => {}
        Feedback::Empty => {
            let (title, line) = empty_words(!needle.is_empty(), false);
            list = list.child(empty_state(t, title, line));
        }
        Feedback::None => {}
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
        .label("Sessions context panel")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .child(row().w_full().pt(8.0).px(8.0).pb(8.0).gap(6.0).child(new_session).child(add_folder))
        .child(filter)
        .child(col().key("sessions").w_full().grow(1.0).shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .child(archived)
}

/// `.sessions-feedback-state`: the first load.
fn loading_block(t: &Theme) -> El {
    col()
        .key("sessions-loading")
        .w_full()
        .padding(16.0)
        .items_start()
        .gap(8.0)
        .child(icon(&LOADER_CIRCLE, 16.0).color(t.muted).cycle(super::ring_popover::spinner()))
        .child(feedback_title(t, "Loading sessions"))
        .child(feedback_copy(t, "Fetching the latest sessions and projects."))
}

/// `.sessions-feedback-state--error`: nothing to show and the reason.
fn error_block(t: &Theme, error: &str) -> El {
    col()
        .key("sessions-error")
        .w_full()
        .padding(16.0)
        .items_start()
        .gap(8.0)
        .child(feedback_title(t, "Unable to load sessions"))
        .child(feedback_copy(t, error))
        .child(try_again(t, 30.0))
}

/// `.sessions-feedback-banner`: the rows are from the last good read.
fn banner(t: &Theme, error: &str) -> El {
    row()
        .key("sessions-banner")
        .w_full()
        .center_y()
        .gap(12.0)
        .px(12.0)
        .py(8.0)
        .mb(8.0)
        .border(1.0, t.feedback_red(0.2))
        .fill(t.feedback_red(0.08))
        .child(text(banner_copy(error)).font(sans(11.0, 400.0, 1.3)).color(t.text).wrap().grow(1.0))
        .child(try_again(t, 28.0))
}

/// Both "Try again" buttons: square-cornered, bare until hovered.
fn try_again(t: &Theme, min_h: f32) -> El {
    row()
        .key("sessions-retry")
        .min_h(min_h)
        .px(10.0)
        .center()
        .border(1.0, Color::TRANSPARENT)
        .hover(t.main_bg)
        .hover_group()
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.retry_list())
        .child(text("Try again").font(sans(11.0, 500.0, 1.3)).color(t.muted).group_color(t.text))
}

fn feedback_title(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(12.0, 600.0, 1.3)).color(t.text)
}

fn feedback_copy(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(11.0, 400.0, 1.3)).color(t.muted).wrap().w_full()
}

/// `EmptyState compact`: centred, no icon.
fn empty_state(t: &Theme, title: &str, line: &str) -> El {
    col()
        .key("sessions-empty")
        .w_full()
        .items_center()
        .gap(4.0)
        .px(10.5)
        .py(14.0)
        .child(text(title.to_string()).font(sans(12.25, 500.0, 1.3)).color(t.text).align(rune::Align::Center))
        .child(text(line.to_string()).font(sans(10.5, 400.0, 1.3)).color(t.muted).wrap().w_full().align(rune::Align::Center))
}
