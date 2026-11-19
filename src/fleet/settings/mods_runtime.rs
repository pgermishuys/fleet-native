//! The Bun that Mods run on (`lib/mods-runtime.ts`, `stores/mods-runtime.ts`, `plugins/builtin/mods/mods-switch.ts`): what
//! `GET /api/features/mods/runtime` says, the four states of the panel under the Mods switch, and what the switch and the
//! panel's buttons ask Fleet to do. The install itself runs on the server and goes on whether or not Settings is open, so this
//! only mirrors it: a read when the page opens, and the pushed `mods.runtime` events. Nothing here blocks: a thread asks.
use super::plugins::MODS_KEY;
use crate::fleet::events::{Event, Kind};
use crate::fleet::link::now_ms;
use crate::fleet::settings_nav::Page;
use crate::fleet::state::Topic;
use crate::{State, Tx};
use serde_json::{Value, json};

pub const RUNTIME: &str = "/api/features/mods/runtime";

/// The install running now, or the last one since Fleet started (`ModsRuntimeJob`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Job {
    /// `downloading`, `verifying`, `extracting`, `succeeded` or `failed`.
    pub phase: String,
    pub version: String,
    pub message: Option<String>,
    /// Why it failed: `offline`, `blocked`, `stopped`, `checksum`, `no-build`, `cancelled` or `other`.
    pub reason: Option<String>,
    pub bytes_received: Option<i64>,
    pub bytes_total: Option<i64>,
    pub started_at: Option<String>,
}

/// The Bun Mods run on (`ModsRuntimeBun`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bun {
    pub display_path: String,
    /// `configured` when it is the user's own; else Fleet's (`installed`).
    pub configured: bool,
    pub version: String,
}

/// What Fleet would download (`ModsRuntimeRelease`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub size: Option<i64>,
    pub has_build: bool,
    pub install_folder: String,
    pub source: String,
}

/// A Bun Fleet found on this computer (`ModsRuntimeFound`): `path` is what `POST …/use` takes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub path: String,
    pub display_path: String,
    pub version: String,
}

/// `ModsRuntimeView`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub bun: Option<Bun>,
    pub configured_path: Option<String>,
    pub release: Release,
    pub job: Option<Job>,
    pub found: Vec<Found>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn parse_job(v: &Value) -> Option<Job> {
    let phase = text(v, "phase")?;
    Some(Job {
        phase,
        version: text(v, "version").unwrap_or_default(),
        message: text(v, "message"),
        reason: text(v, "reason"),
        bytes_received: v.get("bytesReceived").and_then(Value::as_f64).map(|n| n as i64),
        bytes_total: v.get("bytesTotal").and_then(Value::as_f64).map(|n| n as i64),
        started_at: text(v, "startedAt"),
    })
}

/// The view as the server wrote it. A body with no release is not a view (the page keeps what it shows).
pub fn parse_view(body: &Value) -> Option<View> {
    let release = body.get("release")?;
    Some(View {
        bun: body.get("bun").filter(|b| b.is_object()).map(|b| Bun {
            display_path: text(b, "displayPath").or_else(|| text(b, "path")).unwrap_or_default(),
            configured: text(b, "source").as_deref() == Some("configured"),
            version: text(b, "version").unwrap_or_default(),
        }),
        configured_path: text(body, "configuredPath").filter(|p| !p.is_empty()),
        release: Release {
            version: text(release, "version").unwrap_or_default(),
            size: release.get("size").and_then(Value::as_f64).map(|n| n as i64),
            has_build: release.get("hasBuild").and_then(Value::as_bool).unwrap_or(false),
            install_folder: text(release, "installFolder").unwrap_or_default(),
            source: text(release, "source").unwrap_or_default(),
        },
        job: body.get("job").and_then(parse_job),
        found: body
            .get("found")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|f| Found {
                path: text(f, "path").unwrap_or_default(),
                display_path: text(f, "displayPath").unwrap_or_default(),
                version: text(f, "version").unwrap_or_default(),
            })
            .collect(),
    })
}

pub fn job_running(job: Option<&Job>) -> bool {
    job.is_some_and(|j| matches!(j.phase.as_str(), "downloading" | "verifying" | "extracting"))
}

/// Megabytes the way the panel says them: bytes / 1,000,000, rounded.
pub fn megabytes(bytes: Option<i64>) -> i64 {
    ((bytes.unwrap_or(0) as f64) / 1_000_000.0).round() as i64
}

/// What a running install's heading says.
pub fn progress_title(job: &Job) -> String {
    match job.phase.as_str() {
        "verifying" => "Checking the download".into(),
        "extracting" => "Unpacking".into(),
        _ => format!("Downloading Bun {}", job.version),
    }
}

/// The three steps of an install, with where each stands: `done`, `current` or `todo`.
pub fn steps(job: &Job) -> [(&'static str, &'static str, &'static str); 3] {
    let at = ["downloading", "verifying", "extracting"].iter().position(|p| *p == job.phase).unwrap_or(0);
    let status = |i: usize| {
        if i < at {
            "done"
        } else if i == at {
            "current"
        } else {
            "todo"
        }
    };
    [("download", "Download", status(0)), ("check", "Check", status(1)), ("unpack", "Unpack", status(2))]
}

/// "N of M MB", and the bar's share of it.
pub fn size_words(job: &Job, release: &Release) -> (String, i64, i64, f32) {
    let total = megabytes(job.bytes_total.or(release.size));
    let received = if job.phase == "downloading" { megabytes(job.bytes_received) } else { total };
    let share = if total > 0 { (received as f32 / total as f32).clamp(0.0, 1.0) } else { 0.0 };
    (format!("{received} of {total} MB"), received, total, share)
}

/// The hint under a failure's message.
pub fn failed_hint(job: &Job, release: &Release) -> String {
    match job.reason.as_deref() {
        Some("offline") => "If GitHub is blocked here, install Bun yourself and point Fleet at it.".into(),
        Some("blocked") => {
            format!("Ask whoever runs your network to allow {}, or install Bun yourself and point Fleet at it.", release.source)
        }
        _ => String::new(),
    }
}

/// What the panel shows (`ModsRuntimePanel`'s `state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Confirm,
    Progress,
    Failed,
    Ready,
}

/// The panel's state: asking first, an install running, an install that failed with no Bun to fall back on (whichever way the
/// switch is), or the quiet line once Mods are on and a Bun is there. Nothing otherwise.
pub fn panel_state(view: Option<&View>, confirming: bool, enabled: bool) -> Option<Panel> {
    let view = view?;
    if confirming {
        return Some(Panel::Confirm);
    }
    if job_running(view.job.as_ref()) {
        return Some(Panel::Progress);
    }
    if let Some(job) = &view.job
        && job.phase == "failed"
        && job.reason.as_deref() != Some("cancelled")
        && view.bun.is_none()
    {
        return Some(Panel::Failed);
    }
    (enabled && view.bun.is_some()).then_some(Panel::Ready)
}

/// What the page keeps of the runtime (`useModsRuntimeStore` and the panel's own refs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModsRuntime {
    pub view: Option<View>,
    /// The panel is asking whether to install; the switch stays off meanwhile.
    pub confirming: bool,
    /// The server's refusal of the last button, or why it couldn't be reached.
    pub action_error: Option<String>,
    /// An install that was already running when the page first read the view "kept going while you were away".
    pub came_back: bool,
    seen_first: bool,
    /// The machine the view was asked of, so a switch of machine asks again.
    pub loaded_for: Option<String>,
    /// Counts reads and buttons, so an answer to a machine since left is dropped.
    pub request: u64,
    /// A press on the switch is waiting for the first read of the view.
    pub asking: bool,
}

impl ModsRuntime {
    /// The panel's own watch on the view: the first one decides `came_back`.
    fn take(&mut self, view: View) {
        if !self.seen_first {
            self.seen_first = true;
            self.came_back = job_running(view.job.as_ref());
        }
        self.view = Some(view);
    }

    /// `applyEvent`'s merge: progress changes the job in place.
    pub fn merge_job(&mut self, job: Job) {
        if let Some(view) = self.view.as_mut() {
            view.job = Some(job);
        }
    }

    /// Whether the view says Bun is there to run mods on (installed, or the user's own path is set).
    pub fn has_bun(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.bun.is_some() || v.configured_path.is_some())
    }
}

/// Which button the panel's request is.
#[derive(Clone, Debug)]
pub enum Action {
    Install,
    Use(String),
    Cancel,
}

impl Action {
    fn path(&self) -> String {
        match self {
            Action::Install => format!("{RUNTIME}/install"),
            Action::Use(_) => format!("{RUNTIME}/use"),
            Action::Cancel => format!("{RUNTIME}/cancel"),
        }
    }
}

/// The server's reason for refusing: its `error` field, else the status.
fn refusal(status: u16, body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::trim).filter(|e| !e.is_empty()).map(str::to_string))
        .unwrap_or_else(|| format!("Fleet answered {status}."))
}

/// One request: the view the server answers, or the words to show.
fn send(base: &str, method: &str, path: &str, body: Option<&Value>) -> Result<View, String> {
    let (status, reply) = crate::fleet::http::text_with(method, base, path, body).map_err(|_| "Fleet couldn't be reached.".to_string())?;
    if !(200..300).contains(&status) {
        return Err(refusal(status, &reply));
    }
    serde_json::from_str::<Value>(&reply).ok().and_then(|v| parse_view(&v)).ok_or_else(|| "Fleet couldn't be reached.".to_string())
}

impl State {
    /// Whether the Mods panel is on screen: Settings on Plugins, or Mods' own page.
    fn mods_panel_open(&self) -> bool {
        self.rail == crate::Rail::Settings
            && (matches!(&self.settings.page, Page::Plugins) || matches!(&self.settings.page, Page::Plugin(id) if id == "mods"))
    }

    /// Called from `poll`: while the Mods panel can be on screen, asks the machine Settings shows when it hasn't (the plugin
    /// row's `load`); leaving forgets it, so coming back reads again and an install that went on meanwhile "kept going".
    pub fn keep_mods_runtime(&mut self, _tx: &mut Tx) -> bool {
        if !self.mods_panel_open() {
            let had = self.settings.mods_runtime != ModsRuntime::default();
            self.settings.mods_runtime = ModsRuntime::default();
            return had;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.mods_runtime.loaded_for.as_deref() == Some(key.as_str()) {
            return false;
        }
        self.settings.mods_runtime =
            ModsRuntime { loaded_for: Some(key.clone()), request: self.settings.mods_runtime.request + 1, ..Default::default() };
        self.read_mods_runtime(&key, false);
        true
    }

    /// `load()`: the view again. A failed read keeps what is shown. With `then_press`, the switch was pressed before the view
    /// was known: it is decided once the view has come.
    fn read_mods_runtime(&mut self, key: &str, then_press: bool) {
        let Some(link) = self.link_for_machine(key).filter(|_| self.settings_target().is_some()) else {
            self.settings.mods_runtime.asking = false;
            return;
        };
        let request = self.settings.mods_runtime.request;
        link.off_thread(
            |base| send(base, "GET", RUNTIME, None),
            move |s: &mut State, _tx: &mut Tx, got: Result<View, String>| {
                if s.settings.mods_runtime.request != request {
                    return;
                }
                if let Ok(view) = got {
                    s.settings.mods_runtime.take(view);
                }
                if then_press {
                    s.settings.mods_runtime.asking = false;
                    s.press_mods_on();
                }
            },
        );
    }

    /// The Mods switch (`modsOwnSwitch.set`): while the panel asks, a press cancels; turning on with no Bun asks first and
    /// writes nothing (a press before the view is known reads it, then asks); everything else writes the preference.
    pub fn mods_switch(&mut self, on: bool) {
        if self.settings.mods_runtime.confirming {
            self.settings.mods_runtime.confirming = false;
            return;
        }
        if on {
            self.press_mods_on();
        } else {
            self.save_setting(MODS_KEY, "false");
        }
    }

    fn press_mods_on(&mut self) {
        if self.settings.mods_runtime.asking {
            return;
        }
        if self.settings.mods_runtime.view.is_none() && self.link.is_some() {
            let key = self.settings.choice.key(&self.machines).to_string();
            self.settings.mods_runtime.asking = true;
            self.read_mods_runtime(&key, true);
            return;
        }
        if self.settings.mods_runtime.view.is_some() && !self.settings.mods_runtime.has_bun() {
            self.settings.mods_runtime.confirming = true;
            return;
        }
        self.save_setting(MODS_KEY, "true");
    }

    /// A button of the panel: the request goes to the machine shown; the answer is the new view or the words to show; then the
    /// preferences are read again, because the server may have turned the switch on or off.
    pub fn mods_runtime_action(&mut self, action: Action) {
        let page = &mut self.settings.mods_runtime;
        if !matches!(action, Action::Cancel) {
            page.came_back = false;
            page.confirming = false;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else { return };
        let request = self.settings.mods_runtime.request;
        let body = match &action {
            Action::Use(path) => Some(json!({ "path": path })),
            _ => None,
        };
        let path = action.path();
        link.off_thread(
            move |base| send(base, "POST", &path, body.as_ref()),
            move |s: &mut State, _tx: &mut Tx, got: Result<View, String>| {
                if s.settings.mods_runtime.request != request {
                    return;
                }
                match got {
                    Ok(view) => {
                        s.settings.mods_runtime.action_error = None;
                        s.settings.mods_runtime.view = Some(view);
                    }
                    Err(e) => s.settings.mods_runtime.action_error = Some(e),
                }
                s.load_preferences();
            },
        );
    }

    /// `mods.runtime` and `mods.changed` on the `sessions` topic. Progress changes the job in place; any other phase reads the
    /// view again; the server turning the switch off after a failed install makes the preferences be read again.
    pub fn on_mods_event(&mut self, topic: &Topic, event: &Event) {
        if *topic != Topic::Sessions || !self.mods_panel_open() {
            return;
        }
        match event {
            Event::Other(Kind::ModsRuntime, payload) => {
                let job = payload.get("job").and_then(parse_job);
                let downloading = job.as_ref().is_some_and(|j| j.phase == "downloading");
                if let Some(job) = job {
                    self.settings.mods_runtime.merge_job(job);
                }
                if !downloading {
                    let key = self.settings.choice.key(&self.machines).to_string();
                    self.read_mods_runtime(&key, false);
                }
            }
            Event::Other(Kind::ModsChanged, payload) if payload.get("reason").and_then(Value::as_str) == Some("switch") => {
                self.load_preferences();
            }
            _ => {}
        }
    }

    /// "Started 3 minutes ago" for an install already going when the page opened.
    pub fn mods_started_words(&self, job: &Job) -> String {
        let at = job.started_at.as_deref().and_then(crate::fleet::work::parse_iso_ms);
        let now = self.now_ms.unwrap_or_else(now_ms);
        at.map(|at| super::harnesses::relative(at, now, crate::conv::time::local_offset_secs())).unwrap_or_else(|| "a moment ago".into())
    }
}
