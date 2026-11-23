//! Invented jobs for the Jobs scenes and tests (component 23, piece G): one of each state, a title too long for the list, a job
//! pinned to a machine, and two machines with seats beside one with none. Names, repositories and words are made up.
use crate::State;
use crate::fleet::jobs::{Job, JobMachine, JobState};
use crate::fleet::labs::JobsTab;
use serde_json::json;

/// What a scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Every group filled; a failed job open.
    Mixed,
    /// Loaded, nothing queued.
    Empty,
    /// Jobs exist and none is open.
    Pick,
    /// A waiting job pinned to a machine, open.
    Waiting,
    /// A running job with its session, open.
    Running,
    /// The read failed, over the list it had.
    Error,
}

fn job(id: &str, title: &str, state: JobState) -> Job {
    Job {
        id: id.into(),
        title: title.into(),
        ask: "Add a per-key rate limit to the public endpoints and cover it with a test.".into(),
        repository: "https://example.com/acme/ledger-api".into(),
        state,
        created_at: "2026-11-21T08:00:00Z".into(),
        ..Default::default()
    }
}

fn on(mut j: Job, machine: &str, session: Option<&str>) -> Job {
    j.machine_id = Some(format!("m-{machine}"));
    j.machine_name = Some(machine.into());
    j.session_id = session.map(str::to_string);
    j
}

/// One of each state, in the server's order.
pub fn jobs() -> Vec<Job> {
    let mut running = on(job("j1", "Add rate limits to the public endpoints", JobState::Running), "atlas", Some("s-rate"));
    running.ask = "Add a per-key rate limit to the public endpoints.\nKeep the limit in config, and cover it with a test.".into();
    let needs = on(job("j2", "Fix the rounding in the invoice totals", JobState::NeedsYou), "harbor", Some("s-round"));
    let mut waiting = job("j3", "Update the README with the new setup steps and the long list of environment variables", JobState::Waiting);
    waiting.why_waiting = Some("No machine has a free seat.".into());
    waiting.labels = vec!["mac".into(), "gpu".into()];
    waiting.pinned_machine_id = Some("m-harbor".into());
    let done = on(job("j4", "Build the release notes", JobState::Done), "atlas", Some("s-notes"));
    let mut failed = on(job("j5", "Bump the lockfile", JobState::Failed), "harbor", Some("s-bump"));
    failed.error = Some("The install failed: no network on the machine.".into());
    vec![running, needs, waiting, done, failed]
}

fn machine(id: &str, name: &str, local: bool, seats: u32, used: u32, status: &str) -> JobMachine {
    JobMachine { id: id.into(), name: name.into(), is_local: local, seats, used, labels: Vec::new(), status: status.into() }
}

/// Atlas (this machine) and harbor take jobs; the laptop takes none and is left out of the bar.
pub fn machines() -> Vec<JobMachine> {
    vec![
        machine("m-atlas", "atlas", true, 2, 1, "online"),
        machine("m-harbor", "harbor", false, 1, 1, "online"),
        machine("m-laptop", "laptop", false, 0, 0, "online"),
    ]
}

impl State {
    /// The Jobs view with `case`'s queue in place of the home Fleet's.
    pub fn show_jobs(&mut self, case: Case, dark: bool) {
        self.dark = Some(dark);
        self.pickers.prefs.adopt(&json!({ "Jobs": "true" }));
        let j = &mut self.jobs;
        j.is_loaded = true;
        j.machines = machines();
        j.jobs = if case == Case::Empty { Vec::new() } else { jobs() };
        if case == Case::Error {
            j.error = Some("Couldn't reach Fleet.".into());
        }
        self.labs.jobs_tab = JobsTab::Queue;
        let open = match case {
            Case::Mixed => "/jobs/j5",
            Case::Waiting => "/jobs/j3",
            Case::Running => "/jobs/j1",
            _ => "/jobs",
        };
        self.open_jobs_address(open);
    }
}

/// What the New job scene shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NewCase {
    /// Filled in, a repository picked.
    Filled,
    /// No machine that takes jobs has a repository.
    NoRepositories,
}

fn repositories() -> Vec<crate::fleet::jobs::JobRepository> {
    let repo = |name: &str, machines: &[&str]| crate::fleet::jobs::JobRepository {
        name: name.into(),
        repository: format!("https://example.com/acme/{name}"),
        machines: machines.iter().map(|m| m.to_string()).collect(),
    };
    vec![repo("ledger-api", &["atlas", "harbor"]), repo("docs-site", &["atlas"])]
}

impl State {
    /// The Agents tab: atlas holds one job and has a free seat, harbor's one seat is taken by a job that needs you, the laptop
    /// takes none.
    pub fn show_jobs_agents(&mut self, dark: bool) {
        self.show_jobs(Case::Pick, dark);
        self.jobs.machines[0].labels = vec!["linux".into(), "gpu".into()];
        self.jobs.machines[1].labels = vec!["mac".into()];
        self.labs.jobs_tab = JobsTab::Agents;
    }

    /// New job over the Queue.
    pub fn show_new_job(&mut self, case: NewCase, dark: bool) {
        use crate::fleet::jobs::new_job::NewJobForm;
        use rune::TextEdit;
        self.show_jobs(Case::Pick, dark);
        let mut form = NewJobForm::default();
        if case == NewCase::Filled {
            let list = repositories();
            form.title = TextEdit::new("Add rate limits");
            form.ask = TextEdit::new("Add a per-key rate limit to the public endpoints.\nKeep the limit in config.");
            form.labels = TextEdit::new("mac, gpu");
            form.repository = list[0].repository.clone();
            form.pinned = "m-harbor".into();
            self.jobs.repositories.list = Some(list);
        } else {
            self.jobs.repositories.list = Some(Vec::new());
        }
        self.jobs.new = Some(form);
    }

    /// Settings → Machines with Jobs on: this machine has 2 seats and two labels, falcon one seat, harbor none.
    pub fn show_machine_job_seats(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.pickers.prefs.adopt(&json!({ "Jobs": "true" }));
        self.jobs.is_loaded = true;
        self.jobs.machines = vec![
            machine("m-home", "atlas", true, 2, 1, "online"),
            machine("m-falcon", "falcon", false, 1, 0, "online"),
            machine("m-harbor", "harbor", false, 0, 0, "online"),
        ];
        self.jobs.machines[0].labels = vec!["linux".into(), "gpu".into()];
        self.jobs.machines[1].labels = vec!["mac".into()];
        self.sync_seat_fields();
        self.manage_machine("home");
    }
}
