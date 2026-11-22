//! The Jobs page (`JobsScreen.vue`): the header with its Queue and Agents tabs, a chip of seats per machine and New job; the
//! store's error under it; the Queue's body (the open job, or what to do); a failed Cancel's words under the page. The Agents
//! tab and New job come with piece H.
use super::detail;
use crate::State;
use crate::fleet::jobs::JobMachine;
use crate::fleet::labs::JobsTab;
use crate::icons::{LIST_TODO, PLUS};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::{empty_state, error_note, page_header, rem};
use crate::view::workflows::sheet;
use rune::{Color, Cursor, El, ScrollSpec, col, row, text};

/// `[data-testid=jobs-seat-chip]`: "atlas 1/2" in a pill, half faded while the machine isn't online.
fn chip(t: &Theme, m: &JobMachine) -> El {
    let e = row().key(format!("jobs-seat-{}", m.id)).px(rem(0.75)).py(rem(0.25)).radius(999.0).border(1.0, t.border).border_layout().child(
        text(format!("{} {}/{}", m.name, m.used, m.seats))
            .key(format!("jobs-seat-{}-text", m.id))
            .font(mono(rem(0.75), 400.0, 1.33))
            .color(t.text),
    );
    if m.status == "online" { e } else { e.opacity(0.5) }
}

/// One tab of the switch: the name capitalised, the picked one on `--border` at 60%.
fn tab(t: &Theme, which: JobsTab, picked: JobsTab) -> El {
    let on = which == picked;
    let e = row()
        .key(format!("jobs-tab-{}", which.name()))
        .role("tab")
        .label(which.title())
        .px(rem(0.75))
        .py(rem(0.25))
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .fill(if on { t.border.alpha(t.border.a * 0.6) } else { Color::TRANSPARENT })
        .child(text(which.title()).font(sans(rem(0.875), 400.0, 1.43)).color(if on { t.text } else { t.muted }).group_color(t.text))
        .hover_group();
    e.on_press(move |s: &mut State| s.labs.jobs_tab = which)
}

fn header(s: &State, t: &Theme) -> El {
    let tabs = row()
        .key("jobs-tabs")
        .role("tablist")
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .padding(2.0)
        .children([tab(t, JobsTab::Queue, s.labs.jobs_tab), tab(t, JobsTab::Agents, s.labs.jobs_tab)]);
    // Seats are given to machines, so one with none is left out.
    let seats = row()
        .key("jobs-seats")
        .wrap()
        .center_y()
        .gap(rem(0.5))
        .children(s.jobs.machines.iter().filter(|m| m.seats > 0).map(|m| chip(t, m)));
    // Piece H opens the New job dialog; until then the button does nothing.
    let new = button(t, Variant::Default, Size::Sm, Some(&PLUS), "New job").key("jobs-new").on_press(|_: &mut State| {});
    page_header::bar_with(t, "jobs-header", "Jobs", vec![tabs], vec![seats, new])
}

/// The Queue: the open job, else what to do. Nothing before the first read has answered.
fn queue(s: &State, t: &Theme) -> El {
    let open = s.labs.job.as_ref().and_then(|id| s.jobs.jobs.iter().find(|j| &j.id == id));
    let body = match open {
        Some(job) => detail::view(s, t, job),
        None if s.jobs.is_loaded && s.jobs.jobs.is_empty() => col().w_full().items_center().child(empty_state::full(
            t,
            "jobs-empty",
            &LIST_TODO,
            "No jobs yet",
            "Post one with New job, and Fleet starts it on a machine with a free seat.",
            None,
        )),
        None if s.jobs.is_loaded => {
            col().w_full().items_center().child(empty_state::full(t, "jobs-pick", &LIST_TODO, "Pick a job from the list", "", None))
        }
        None => col(),
    };
    col().key("jobs-queue").grow(1.0).min_w(0.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)
}

/// A note under the header or the page: margin 20/8.
fn note(t: &Theme, key: &str, words: &str) -> El {
    row().w_full().px(rem(1.25)).py(rem(0.5)).child(error_note::message(t, key, words))
}

pub fn screen(s: &State, t: &Theme) -> El {
    let body = match s.labs.jobs_tab {
        JobsTab::Queue => queue(s, t),
        // The seats per machine are piece H's.
        JobsTab::Agents => col().key("jobs-agents").grow(1.0).min_h(0.0),
    };
    // `center__body`: the page sits in 24 px of padding all round inside the sheet.
    let page = col()
        .key("jobs-screen")
        .w_full()
        .h_full()
        .min_h(0.0)
        .padding(24.0)
        .child(header(s, t))
        .children(s.jobs.error.as_ref().map(|e| note(t, "jobs-error", e)))
        .child(row().w_full().grow(1.0).min_h(0.0).child(body))
        .children(s.jobs.cancel.error.as_ref().map(|e| note(t, "jobs-action-error", e)));
    sheet(t, "jobs-detail").child(page)
}
