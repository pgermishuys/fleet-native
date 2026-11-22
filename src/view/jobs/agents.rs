//! The Agents tab (`JobsAgents.vue`): a card per job machine with the jobs its seats hold and the seats still free.
//! Two columns (`lg:grid-cols-2`); Rune has no grid, so the cards are laid out in rows of two.
use crate::State;
use crate::fleet::jobs::{Job, JobMachine, JobState};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::rem;
use rune::{El, ScrollSpec, col, el, row, text};

/// The jobs on `machine` that run or need the user: what its seats hold.
pub fn holding<'a>(jobs: &'a [Job], machine: &JobMachine) -> Vec<&'a Job> {
    jobs.iter()
        .filter(|j| j.machine_id.as_deref() == Some(machine.id.as_str()) && matches!(j.state, JobState::Running | JobState::NeedsYou))
        .collect()
}

fn small(t: &Theme, words: String) -> El {
    text(words).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)
}

fn card(t: &Theme, jobs: &[Job], m: &JobMachine) -> El {
    let held = holding(jobs, m);
    let free = (m.seats as usize).saturating_sub(held.len());
    let id = &m.id;
    let head = row().items_end().gap(rem(0.5)).child(text(m.name.clone()).font(mono(rem(0.875), 600.0, 1.43)).color(t.text)).child(
        small(t, format!("{}{} of {} seats", if m.is_local { "this machine · " } else { "" }, held.len(), m.seats))
            .key(format!("jobs-agent-{id}-seats")),
    );
    let mut e = col()
        .key(format!("jobs-agent-{id}"))
        .role("group")
        .label(m.name.clone())
        .w_full()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .padding(rem(1.0))
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(head);
    if !m.labels.is_empty() {
        e = e.child(row().wrap().gap(rem(0.375)).mt(rem(0.5)).children(m.labels.iter().map(|l| {
            row()
                .px(rem(0.375))
                .py(1.0)
                .radius(4.0)
                .border(1.0, t.border)
                .border_layout()
                .child(text(l.clone()).font(mono(rem(0.75), 400.0, 1.33)).color(t.muted))
        })));
    }
    if m.seats == 0 {
        return e.opacity(0.6).child(
            small(t, "Takes no jobs. Give it seats in Settings → Machines.".into())
                .key(format!("jobs-agent-{id}-none"))
                .wrap()
                .w_full()
                .mt(rem(0.75)),
        );
    }
    let mut list = col().w_full().gap(rem(0.375)).mt(rem(0.75));
    for j in held {
        list = list.child(
            row()
                .key(format!("jobs-agent-job-{}", j.id))
                .w_full()
                .items_center()
                .justify_between()
                .gap(rem(0.5))
                .px(rem(0.75))
                .py(rem(0.5))
                .radius(RADIUS_BTN)
                .fill(t.border.alpha(t.border.a * 0.3))
                .child(text(j.title.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text).wrap().shrink().min_w(0.0))
                .child(small(t, if j.state == JobState::NeedsYou { "needs you" } else { "running" }.into())),
        );
    }
    for n in 0..free {
        list = list.child(
            row()
                .key(format!("jobs-agent-{id}-free-{n}"))
                .w_full()
                .px(rem(0.75))
                .py(rem(0.5))
                .radius(RADIUS_BTN)
                .border(1.0, t.border)
                .border_layout()
                .border_dash(4.0, 3.0)
                .child(small(t, "Free · takes the next job that fits".into())),
        );
    }
    e.child(list)
}

pub fn view(s: &State, t: &Theme) -> El {
    let cards: Vec<El> = s.jobs.machines.iter().map(|m| card(t, &s.jobs.jobs, m)).collect();
    let mut grid = col().w_full().gap(rem(1.0));
    for pair in cards.chunks(2) {
        let mut r = row().w_full().gap(rem(1.0)).items_stretch();
        for c in pair {
            r = r.child(c.clone());
        }
        // An odd last card keeps its half of the row.
        if pair.len() == 1 {
            r = r.child(el().grow(1.0).w(0.0));
        }
        grid = grid.child(r);
    }
    col().key("jobs-agents").grow(1.0).min_w(0.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).padding(rem(1.25)).child(grid)
}
