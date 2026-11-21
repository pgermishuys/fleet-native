//! Jobs' data (component 23, piece F; `lib/jobs.ts`): a job, a machine's seats, a repository New job can pick, the
//! jobs-list's groups and the labels field's parsing. `store.rs` holds them and asks the home Fleet. No view here: the
//! jobs-list, the page and Settings → Machines read these in G and H.
//!
//! Not ported: `jobColumns` (Waiting, Running, Needs you, Done): exported, and nothing on Fleet's `main` uses it.
use serde_json::Value;

pub mod store;

/// How many jobs one machine may run at once, at most (`MAX_JOB_SEATS`).
pub const MAX_JOB_SEATS: u32 = 16;

/// `JobState`. A state this app doesn't know (a newer server) is `Other`, which no group lists, as Fleet's filters leave it out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JobState {
    #[default]
    Waiting,
    Running,
    NeedsYou,
    Done,
    Failed,
    Other,
}

impl JobState {
    fn parse(text: &str) -> JobState {
        match text {
            "waiting" => JobState::Waiting,
            "running" => JobState::Running,
            "needs_you" => JobState::NeedsYou,
            "done" => JobState::Done,
            "failed" => JobState::Failed,
            _ => JobState::Other,
        }
    }
}

/// `Job`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Job {
    pub id: String,
    pub title: String,
    pub ask: String,
    pub repository: String,
    pub labels: Vec<String>,
    pub pinned_machine_id: Option<String>,
    pub state: JobState,
    pub machine_id: Option<String>,
    pub machine_name: Option<String>,
    pub session_id: Option<String>,
    /// In the server's plain words, why a waiting job hasn't started ("No machine has a free seat.").
    pub why_waiting: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
}

/// `JobMachine`: how many jobs a machine runs at once, how many it runs now, and its labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JobMachine {
    pub id: String,
    pub name: String,
    pub is_local: bool,
    pub seats: u32,
    pub used: u32,
    pub labels: Vec<String>,
    /// "online", or whatever else the server says.
    pub status: String,
}

/// `JobRepository`: what New job can pick, a repository once with the machines that have it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JobRepository {
    pub name: String,
    /// What a job names: the origin remote, or the folder name when there is none.
    pub repository: String,
    pub machines: Vec<String>,
}

/// `NewJob`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewJob {
    pub title: String,
    pub ask: String,
    pub repository: String,
    pub labels: Vec<String>,
    pub pinned_machine_id: Option<String>,
}

impl NewJob {
    /// The body of `POST /api/jobs`.
    pub fn body(&self) -> Value {
        serde_json::json!({
            "title": self.title, "ask": self.ask, "repository": self.repository, "labels": self.labels,
            "pinnedMachineId": self.pinned_machine_id,
        })
    }
}

/// `JobGroup`'s id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupId {
    Running,
    Queued,
    Done,
}

pub struct JobGroup<'a> {
    pub id: GroupId,
    pub label: &'static str,
    pub jobs: Vec<&'a Job>,
}

/// `jobGroups`: a job that needs you is still Running; a failed one stays under Done, marked. Each group keeps the
/// server's order.
pub fn job_groups(jobs: &[Job]) -> [JobGroup<'_>; 3] {
    let of = |states: &[JobState]| jobs.iter().filter(|j| states.contains(&j.state)).collect();
    [
        JobGroup { id: GroupId::Running, label: "Running", jobs: of(&[JobState::Running, JobState::NeedsYou]) },
        JobGroup { id: GroupId::Queued, label: "Queued", jobs: of(&[JobState::Waiting]) },
        JobGroup { id: GroupId::Done, label: "Done", jobs: of(&[JobState::Done, JobState::Failed]) },
    ]
}

/// `parseLabels`: "mac, gpu" split on commas, trimmed, without blanks or repeats (the first spelling of a repeat stays).
pub fn parse_labels(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.split(',').map(str::trim).filter(|label| !label.is_empty() && seen.insert(label.to_lowercase())).map(str::to_string).collect()
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn some_text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn words(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn count(v: &Value, key: &str) -> u32 {
    v.get(key).and_then(Value::as_f64).map_or(0, |n| n.max(0.0) as u32)
}

pub fn parse_job(v: &Value) -> Option<Job> {
    let id = some_text(v, "id")?;
    Some(Job {
        id,
        title: text(v, "title"),
        ask: text(v, "ask"),
        repository: text(v, "repository"),
        labels: words(v, "labels"),
        pinned_machine_id: some_text(v, "pinnedMachineId"),
        state: JobState::parse(&text(v, "state")),
        machine_id: some_text(v, "machineId"),
        machine_name: some_text(v, "machineName"),
        session_id: some_text(v, "sessionId"),
        why_waiting: some_text(v, "whyWaiting"),
        error: some_text(v, "error"),
        created_at: text(v, "createdAt"),
    })
}

pub fn parse_machine(v: &Value) -> Option<JobMachine> {
    Some(JobMachine {
        id: some_text(v, "id")?,
        name: text(v, "name"),
        is_local: v.get("isLocal").and_then(Value::as_bool).unwrap_or(false),
        seats: count(v, "seats"),
        used: count(v, "used"),
        labels: words(v, "labels"),
        status: text(v, "status"),
    })
}

/// `JobsResponse`: `{jobs, machines}`; a body without both lists is no answer.
pub fn parse_jobs(body: &Value) -> Option<(Vec<Job>, Vec<JobMachine>)> {
    let list = |key: &str| body.get(key).and_then(Value::as_array);
    let jobs = list("jobs")?.iter().filter_map(parse_job).collect();
    let machines = list("machines")?.iter().filter_map(parse_machine).collect();
    Some((jobs, machines))
}

pub fn parse_repositories(body: &Value) -> Option<Vec<JobRepository>> {
    Some(
        body.as_array()?
            .iter()
            .map(|r| JobRepository { name: text(r, "name"), repository: text(r, "repository"), machines: words(r, "machines") })
            .collect(),
    )
}
