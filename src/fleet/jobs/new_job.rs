//! New job's form and the seats on a machine's card (component 23, piece H; `NewJobDialog.vue`, `MachineJobSeats.vue`). The
//! requests are the store's (`store.rs`); this file holds what is typed and picked, `can_post`, and what an answer does to the
//! form. The views are `view::jobs::new_job` and `view::machine_job_seats`.
use super::{JobMachine, JobRepository, MAX_JOB_SEATS, NewJob, parse_labels};
use crate::fleet::labs::Feature;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::settings_nav::Page;
use crate::{Rail, State, Tx};
use rune::TextEdit;

pub const TITLE_FIELD: &str = "new-job-title";
pub const ASK_FIELD: &str = "new-job-ask";
pub const LABELS_FIELD: &str = "new-job-labels";

/// Which of the two selects is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Repository,
    Machine,
}

/// The dialog while it is open. Closing drops it, which clears every field and the error (`watch(open)`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewJobForm {
    pub title: TextEdit,
    pub ask: TextEdit,
    pub labels: TextEdit,
    /// What a job names; empty until the first repository is picked.
    pub repository: String,
    /// The machine the job is pinned to; empty is "Any machine that fits".
    pub pinned: String,
    pub picking: Option<Pick>,
    pub error: Option<String>,
}

impl NewJobForm {
    /// `canPost`: a title, an ask (neither blank) and a repository, and nothing being posted.
    pub fn can_post(&self, posting: bool) -> bool {
        !posting && !self.title.text.trim().is_empty() && !self.ask.text.trim().is_empty() && !self.repository.is_empty()
    }

    /// What Post job sends: the trimmed title and ask, the labels parsed, the pinned id or none.
    pub fn request(&self) -> NewJob {
        NewJob {
            title: self.title.text.trim().to_string(),
            ask: self.ask.text.trim().to_string(),
            repository: self.repository.clone(),
            labels: parse_labels(&self.labels.text),
            pinned_machine_id: (!self.pinned.is_empty()).then(|| self.pinned.clone()),
        }
    }
}

/// The option of the repository select: "ledger-api · atlas, harbor".
pub fn repository_label(r: &JobRepository) -> String {
    format!("{} · {}", r.name, r.machines.join(", "))
}

impl State {
    /// New job: an empty form with the first repository once they are read, and the title focused.
    pub fn open_new_job(&mut self, tx: &mut Tx) {
        self.jobs.new = Some(NewJobForm::default());
        self.jobs.repositories.list = None;
        self.jobs_load_repositories();
        tx.focus(TITLE_FIELD, false);
    }

    /// Cancel, the X, Escape and the scrim. An open select closes alone.
    pub fn close_new_job(&mut self) {
        match self.jobs.new.as_mut() {
            Some(f) if f.picking.is_some() => f.picking = None,
            _ => self.jobs.new = None,
        }
    }

    /// Post job, and Enter in a field: nothing unless `can_post`.
    pub fn new_job_submit(&mut self) {
        let posting = self.jobs.post.busy;
        let Some(form) = self.jobs.new.as_mut().filter(|f| f.can_post(posting)) else { return };
        form.error = None;
        let job = form.request();
        self.jobs_post(&job);
    }

    /// The post answered (called by the store): a good one closes the dialog, a refusal keeps it with the words.
    pub(crate) fn new_job_posted(&mut self) {
        match self.jobs.post.error.clone() {
            None => self.jobs.new = None,
            Some(words) => {
                if let Some(f) = self.jobs.new.as_mut() {
                    f.error = Some(words);
                }
            }
        }
    }

    /// The repositories were read (called by the store): the first is picked when none is, and a refusal shows its words.
    pub(crate) fn new_job_repositories_arrived(&mut self) {
        let first = self.jobs.repositories.list.as_ref().and_then(|l| l.first()).map(|r| r.repository.clone());
        let error = self.jobs.repositories.error.clone();
        let Some(f) = self.jobs.new.as_mut() else { return };
        if f.repository.is_empty() {
            f.repository = first.unwrap_or_default();
        }
        if error.is_some() {
            f.error = error;
        }
    }
}

// ─── Settings → Machines: seats and labels ──────────────────────────────────────────────────────────────────────────

/// Which field of a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatField {
    Seats,
    Labels,
}

/// One card's two fields, and what the server held when they were last filled.
#[derive(Clone, Debug, Default)]
pub struct SeatFields {
    pub seats: TextEdit,
    pub labels: TextEdit,
    synced: Option<(u32, Vec<String>)>,
}

pub fn seats_key(id: &str) -> String {
    format!("machine-job-seats-input-{id}")
}

pub fn labels_key(id: &str) -> String {
    format!("machine-job-labels-input-{id}")
}

/// A card's machine in the home Fleet's list: home's card is the local one, another card is named by its id.
pub fn machine_of<'a>(machines: &'a [JobMachine], entry_key: &str) -> Option<&'a JobMachine> {
    machines.iter().find(|m| if entry_key == HOME_MACHINE_KEY { m.is_local } else { m.id == entry_key })
}

/// `saveSeats`' clamp: truncated, a non-number is 0, and 0..=16.
pub fn clamp_seats(text: &str) -> u32 {
    let n = text.trim().parse::<f64>().map_or(0.0, f64::trunc);
    if n.is_nan() { 0 } else { n.clamp(0.0, MAX_JOB_SEATS as f64) as u32 }
}

impl State {
    /// Settings → Machines is shown with Jobs on.
    fn job_seats_shown(&self) -> bool {
        self.rail == Rail::Settings && self.settings.page == Page::Machines && self.labs_feature(Feature::Jobs) == Some(true)
    }

    /// Fills each card's fields from the store when the store's values changed (`watch(machine)`), but never over a field being
    /// typed in; reads the queue once per visit when nothing has (`onMounted`); and saves a field whose focus has moved on
    /// (`@change`). True when something drawn changed.
    pub(crate) fn keep_job_seats(&mut self, tx: &mut Tx) -> bool {
        if !self.job_seats_shown() {
            self.jobs.seats_visit = false;
            self.jobs.seat_focus = None;
            return false;
        }
        let mut changed = false;
        if let (false, false, Some(base)) = (std::mem::replace(&mut self.jobs.seats_visit, true), self.jobs.is_loaded, self.jobs_base()) {
            self.jobs_load(&base);
        }
        let focused = tx.focused().map(str::to_string);
        if let Some((id, which)) = self.jobs.seat_focus.clone() {
            let key = match which {
                SeatField::Seats => seats_key(&id),
                SeatField::Labels => labels_key(&id),
            };
            if focused.as_deref() != Some(key.as_str()) {
                self.jobs.seat_focus = None;
                self.job_seats_commit(&id, which);
                changed = true;
            }
        }
        if let Some(key) = focused {
            for m in &self.jobs.machines {
                for which in [SeatField::Seats, SeatField::Labels] {
                    let mine = match which {
                        SeatField::Seats => seats_key(&m.id),
                        SeatField::Labels => labels_key(&m.id),
                    };
                    if key == mine {
                        self.jobs.seat_focus = Some((m.id.clone(), which));
                    }
                }
            }
        }
        changed | self.sync_seat_fields()
    }

    /// Fills each card's fields from the store where the store's values changed (`watch(machine)`), never over the field being
    /// typed in. True when a field changed.
    pub(crate) fn sync_seat_fields(&mut self) -> bool {
        let mut changed = false;
        let focus = self.jobs.seat_focus.clone();
        for m in &self.jobs.machines {
            let typing = focus.as_ref().is_some_and(|(id, _)| *id == m.id);
            let fields = self.jobs.seat_fields.entry(m.id.clone()).or_default();
            let now = (m.seats, m.labels.clone());
            if fields.synced.as_ref() != Some(&now) && !typing {
                fields.seats = TextEdit::new(m.seats.to_string());
                fields.labels = TextEdit::new(m.labels.join(", "));
                fields.synced = Some(now);
                changed = true;
            }
        }
        changed
    }

    /// `@change` of a field: saves what was typed when it differs from what the server holds. Seats show their clamped value.
    pub fn job_seats_commit(&mut self, id: &str, which: SeatField) {
        let Some(fields) = self.jobs.seat_fields.get_mut(id) else { return };
        let Some((seats, labels)) = fields.synced.clone() else { return };
        match which {
            SeatField::Seats => {
                if fields.seats.text == seats.to_string() {
                    return;
                }
                let clamped = clamp_seats(&fields.seats.text);
                fields.seats = TextEdit::new(clamped.to_string());
                // Held as saved until the server's answer says otherwise, so leaving the field after Enter doesn't save twice.
                fields.synced = Some((clamped, labels));
                self.jobs_save_machine(id, Some(clamped), None);
            }
            SeatField::Labels => {
                if fields.labels.text == labels.join(", ") {
                    return;
                }
                let parsed = parse_labels(&fields.labels.text);
                fields.labels = TextEdit::new(parsed.join(", "));
                fields.synced = Some((seats, parsed.clone()));
                self.jobs_save_machine(id, None, Some(parsed));
            }
        }
    }
}
