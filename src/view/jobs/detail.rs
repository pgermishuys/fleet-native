//! A job's detail (`JobDetail.vue`): its state and title, the ask as written, what it needs and where it runs, and the buttons.
use crate::State;
use crate::fleet::jobs::{Job, JobState};
use crate::theme::{RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use rune::{El, col, row, text};

/// The state line: "Running on atlas", or "Waiting" while no machine has it.
pub fn state_line(job: &Job) -> String {
    let Some(machine) = &job.machine_name else { return "Waiting".into() };
    let state = match job.state {
        JobState::NeedsYou => "Needs you",
        JobState::Done => "Done",
        JobState::Failed => "Failed",
        _ => "Running",
    };
    format!("{state} on {machine}")
}

/// One row of the two-column list: the name (muted) and its value.
fn entry(t: &Theme, key: &str, name: &str, value: El) -> El {
    row()
        .key(key.to_string())
        .w_full()
        .gap(rem(1.0))
        .items_start()
        // `grid-cols-[auto_1fr]`: the names are as wide as "Repository", which is always there.
        .child(text(name.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).w(62.0))
        .child(value.grow(1.0).shrink().min_w(0.0))
}

fn plain(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text).wrap()
}

/// `JobDetail`: `machine_name` finds a machine by its id.
pub fn view(s: &State, t: &Theme, job: &Job) -> El {
    let machine_name = |id: &str| s.jobs.machines.iter().find(|m| m.id == id).map(|m| m.name.clone());
    let head = col()
        .child(text(state_line(job)).key("job-detail-state").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted))
        .child(text(job.title.clone()).key("job-detail-title").font(sans(rem(1.125), 600.0, 1.556)).color(t.text).wrap().mt(rem(0.25)));
    let ask = col()
        .key("job-detail-ask")
        .w_full()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .padding(rem(0.75))
        .child(text(job.ask.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text).wrap().w_full());
    let mut facts = col().w_full().gap(rem(0.5)).child(entry(
        t,
        "job-detail-repository",
        "Repository",
        text(job.repository.clone()).font(mono(rem(0.75), 400.0, 1.5)).color(t.text).wrap_anywhere().w_full(),
    ));
    if !job.labels.is_empty() {
        facts = facts.child(entry(t, "job-detail-needs", "Needs", plain(t, "job-detail-needs-value", &job.labels.join(", "))));
    }
    if let Some(id) = &job.pinned_machine_id {
        let name = machine_name(id).unwrap_or_else(|| "A machine that is no longer listed".into());
        facts = facts.child(entry(t, "job-detail-run-on", "Run on", plain(t, "job-detail-run-on-value", &name)));
    }
    if let Some(why) = job.why_waiting.as_ref().or(job.error.as_ref()) {
        facts = facts.child(entry(t, "job-detail-why-row", "Why", plain(t, "job-detail-why", why)));
    }
    let id = job.id.clone();
    let mut buttons = row().gap(rem(0.5));
    if let Some(session) = job.session_id.clone() {
        let key = match s.jobs.machines.iter().find(|m| Some(&m.id) == job.machine_id.as_ref()) {
            Some(m) if !m.is_local => m.id.clone(),
            _ => crate::fleet::settings::HOME_MACHINE_KEY.to_string(),
        };
        buttons = buttons.child(button(t, Variant::Default, Size::Sm, None, "Open session").key("job-open-session").on_press_tx(
            move |s: &mut State, tx: &mut rune::Tx| {
                s.machines.remember_sessions(&key, std::slice::from_ref(&session));
                s.open_dreaming_session(&session, tx);
            },
        ));
    }
    if job.state == JobState::Waiting {
        buttons = buttons.child(
            button(t, Variant::Outline, Size::Sm, None, "Cancel").key("job-cancel").on_press(move |s: &mut State| s.jobs_cancel(&id)),
        );
    }
    col()
        .key("job-detail")
        .label("Job")
        .w_full()
        .max_w(rem(42.0))
        .padding(rem(1.25))
        .gap(rem(1.0))
        .child(head)
        .child(ask)
        .child(facts)
        .child(buttons)
}
