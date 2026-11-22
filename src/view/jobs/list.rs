//! The jobs-list (`JobsNavPanel.vue`, `use-jobs-menu.ts`): Jobs' panel. Running, Queued and Done, each with its count and its
//! jobs; a job that needs you stays under Running and a failed one under Done, flagged. The one the address names is marked;
//! pressing another opens its route, pressing the open one does nothing.
use crate::State;
use crate::fleet::jobs::{Job, JobState, job_groups};
use crate::fleet::labs::job_path;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::{empty_state, rem};
use rune::{Cursor, El, ScrollSpec, col, row, text};

/// `.jobs-nav__group-label`: the name and, at the end, how many (tabular), 10 px 600 upper case, margin 8/0/2, padding 0 12.
fn label(t: &Theme, key: &str, words: &str, count: usize) -> El {
    let style = sans(rem(0.71429), 600.0, 1.5).tracking(0.5);
    row()
        .key(key.to_string())
        .w_full()
        .px(12.0)
        .mt(8.0)
        .mb(2.0)
        .child(text(words.to_uppercase()).font(style).color(t.muted).grow(1.0))
        .child(text(count.to_string()).key(format!("{key}-count")).font(style.tracking(0.0)).color(t.muted))
}

/// `.jobs-nav__flag`: "Needs you" in the accent's text colour, "Failed" muted, 10 px.
fn flag(t: &Theme, job: &Job) -> Option<El> {
    let (words, ink) = match job.state {
        JobState::NeedsYou => ("Needs you", t.accent_text),
        JobState::Failed => ("Failed", t.muted),
        _ => return None,
    };
    Some(text(words).key(format!("job-{}-flag", job.id)).font(sans(rem(0.71429), 400.0, 1.4)).color(ink))
}

/// `.jobs-nav__item`: as the fleet-menu's, the title cut with an ellipsis and the flag at the end.
fn item(s: &State, t: &Theme, job: &Job) -> El {
    let open = s.labs.job.as_deref() == Some(job.id.as_str());
    let b = row()
        .key(format!("job-{}", job.id))
        .role("button")
        .label(job.title.clone())
        .w_full()
        .min_h(36.0)
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(RADIUS_BTN)
        .border(1.0, if open { t.accent.alpha(0.4) } else { rune::Color::TRANSPARENT })
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent)
        .child(
            text(job.title.clone())
                .font(sans(rem(0.85714), 500.0, 1.4))
                .color(if open { t.text } else { t.muted })
                .truncate()
                .grow(1.0)
                .shrink()
                .min_w(0.0),
        )
        .children(flag(t, job));
    let b = if open { b.fill(t.mix(t.accent, t.panel_bg, 0.12)) } else { b.hover(t.main_bg) };
    let id = job.id.clone();
    b.on_press(move |s: &mut State| {
        if s.labs.job.as_deref() != Some(id.as_str()) {
            s.open_jobs_address(&job_path(&id));
        }
    })
}

/// `section` "Jobs navigation".
pub fn panel(s: &State, t: &Theme) -> El {
    let header = row().px(4.0).pt(18.0).pb(10.0).child(text("JOBS").font(sans(rem(0.71429), 600.0, 1.5).tracking(0.5)).color(t.muted));
    let groups = job_groups(&s.jobs.jobs);
    let all_empty = groups.iter().all(|g| g.jobs.is_empty());
    let mut body = col().key("jobs-nav").w_full().child(header);
    for group in &groups {
        let key = format!("jobs-group-{}", group.label.to_lowercase());
        body = body.child(
            col()
                .key(key.clone())
                .role("group")
                .label(group.label)
                .w_full()
                .gap(4.0)
                .mb(8.0)
                .child(label(t, &format!("{key}-label"), group.label, group.jobs.len()))
                .children(group.jobs.iter().map(|job| item(s, t, job))),
        );
    }
    if s.jobs.is_loaded && all_empty {
        body = body.child(empty_state::compact_bare(t, "jobs-nav-empty", "No jobs yet"));
    }
    col()
        .key("jobs-list")
        .label("Jobs navigation")
        .w(s.sidebar_w)
        .h_full()
        .min_h(0.0)
        .px(12.0)
        .pb(12.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(body)
}
