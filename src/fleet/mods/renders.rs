//! Asking the server to draw tool rows (port of `lib/mods/renders.ts`, the part that is not Vue): which tools a session's
//! mods hook, when a row asks, what it is sent, how many asks run at once, and what an answer does to the views.
//!
//! This is the bookkeeping, with no thread and no request: every method says what the caller must now fetch (`Effect`), and
//! `renders_state.rs` does it on a thread and hands the answer back. A row of a hooked tool asks `/mods/render` when it
//! appears and when its status changes, never as its output streams; rows of one call share one request; at most four run
//! at a time. Nothing is asked while the machine's Mods switch is off.
use super::points::{ModTreeView, ToolRowSite, ToolRowView, Views, tool_row_view_id};
use super::types::Author;
use crate::conv::tools::{Category, get_tool};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

/// The contract's limits on what a mod is sent.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_OUTPUT_CHARS: usize = 262_144;
pub const MAX_IN_FLIGHT: usize = 4;

pub const SITES: [ToolRowSite; 2] = [ToolRowSite::ToolUse, ToolRowSite::ToolResult];

pub fn site_name(site: ToolRowSite) -> &'static str {
    match site {
        ToolRowSite::ToolUse => "ToolUse",
        ToolRowSite::ToolResult => "ToolResult",
    }
}

pub fn site_of(name: &str) -> Option<ToolRowSite> {
    SITES.into_iter().find(|s| site_name(*s) == name)
}

/// A site a session's mods hook: `tools` are canonical names, `None` hooks every tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteHook {
    pub site: ToolRowSite,
    pub tools: Option<Vec<String>>,
}

/// A pane a mod declares in `mod.json`, listed in the session's `+` menu; `off` when its mod is turned off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredPane {
    pub mod_name: String,
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub draft: bool,
    pub off: bool,
}

/// What a session's `/mods/sites` said.
#[derive(Clone, Debug, PartialEq)]
pub enum Loaded {
    Hooks {
        hooks: Vec<SiteHook>,
        panes: Vec<DeclaredPane>,
    },
    /// The switch is off, or the server answered no (a 404 is Mods off too): no hooks and no panes.
    Off,
    /// The request itself failed: no hooks, and the panes stay as they were.
    Failed,
}

/// `GET /mods/sites`' body: the hooks with their tool names mapped to Fleet's canonical ones, and the declared panes.
pub fn parse_sites(body: &Value) -> Loaded {
    let hooks = body
        .get("sites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|h| {
            let site = site_of(h.get("site")?.as_str()?);
            // A site this client doesn't draw (the band, a chip) keeps its place in the answer, and is not ours to ask.
            let tools =
                h.get("tools").and_then(Value::as_array).map(|t| t.iter().filter_map(Value::as_str).map(|n| get_tool(n).name).collect());
            site.map(|site| SiteHook { site, tools })
        })
        .collect();
    let panes = body
        .get("panes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let text = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
            Some(DeclaredPane {
                mod_name: text("mod")?,
                id: text("id")?,
                title: text("title")?,
                icon: text("icon"),
                draft: p.get("draft").and_then(Value::as_bool).unwrap_or(false),
                off: p.get("off").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect();
    Loaded::Hooks { hooks, panes }
}

/// The part of a row that decides when to ask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowHead {
    pub session_id: String,
    pub call_id: String,
    /// The name the harness sent.
    pub raw_tool: String,
    /// Fleet's status text ("Completed", "Stopped") or the harness's ("completed").
    pub status: String,
}

/// The part that is sent, read only when asking.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowRead {
    pub input: Value,
    pub output: String,
    pub title: String,
    pub duration_ms: Option<f64>,
}

pub fn canonical_status(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "stopped" | "cancelled" => "cancelled",
        "background" | "running" => "running",
        "completed" => "completed",
        "error" => "error",
        _ => "pending",
    }
}

fn category_name(c: Category) -> &'static str {
    match c {
        Category::Read => "read",
        Category::Search => "search",
        Category::Edit => "edit",
        Category::Shell => "shell",
        Category::Web => "web",
        Category::Subagent => "subagent",
        Category::Question => "question",
        Category::Plan => "plan",
        Category::Skill => "skill",
        Category::Fleet => "fleet",
        Category::Other => "other",
    }
}

/// The props a mod is sent for a row, within the contract's limits (UTF-16 units for the output, as Fleet counts them).
pub fn render_props(head: &RowHead, read: &RowRead) -> Value {
    let tool = get_tool(&head.raw_tool);
    let input = if read.input.is_null() { json!({}) } else { read.input.clone() };
    let input_truncated = input.to_string().len() > MAX_INPUT_BYTES;
    let units: Vec<u16> = read.output.encode_utf16().collect();
    let output_truncated = units.len() > MAX_OUTPUT_CHARS;
    let output = if output_truncated { String::from_utf16_lossy(&units[units.len() - MAX_OUTPUT_CHARS..]) } else { read.output.clone() };
    let mut props = json!({
        "tool": tool.name,
        "rawTool": head.raw_tool,
        "category": category_name(tool.category),
        "status": canonical_status(&head.status),
        "title": read.title,
        "input": if input_truncated { json!({}) } else { input },
        "inputTruncated": input_truncated,
        "output": output,
        "outputTruncated": output_truncated,
    });
    if let Some(ms) = read.duration_ms {
        props["durationMs"] = json!(ms);
    }
    props
}

/// A render the caller runs: the request is `POST /api/sessions/{session}/mods/render` with `body`.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderJob {
    pub machine: String,
    pub session_id: String,
    pub key: String,
    pub body: Value,
}

/// What an answer to `/mods/render` said.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub tree: Value,
    pub mods: Vec<Author>,
}

pub fn parse_answer(body: &Value) -> Option<Answer> {
    let mods = body
        .get("mods")?
        .as_array()?
        .iter()
        .filter_map(|m| {
            Some(Author { name: m.get("name")?.as_str()?.to_string(), draft: m.get("draft").and_then(Value::as_bool).unwrap_or(false) })
        })
        .collect();
    Some(Answer { tree: body.get("tree").cloned().unwrap_or(Value::Null), mods })
}

/// What the caller must do after a call.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Read the session's hooks (reading the machine's switch first when it has not been read).
    LoadHooks {
        machine: String,
        session_id: String,
    },
    Render(RenderJob),
}

/// The row's read, done at most once however many sites ask.
struct Lazy<F: FnOnce() -> RowRead> {
    f: Option<F>,
    value: Option<RowRead>,
}

impl<F: FnOnce() -> RowRead> Lazy<F> {
    fn get(&mut self) -> &RowRead {
        if self.value.is_none() {
            self.value = self.f.take().map(|f| f());
        }
        self.value.get_or_insert_with(RowRead::default)
    }
}

enum Slot {
    Waiting,
    Done(Option<Answer>),
}

/// Every session's hooks, declared panes, generations and answers; the tool row views are in `views`.
#[derive(Default)]
pub struct Renders {
    /// A session's hooks as the server last said; `None` while unknown or when it has none (Mods off). A session in here has
    /// been asked once: it is not asked again until the switch or the mods change.
    hooks: HashMap<String, Option<Vec<SiteHook>>>,
    panes: HashMap<String, Vec<DeclaredPane>>,
    /// Bumped to make a session's rows (or one call's) ask again.
    session_gen: HashMap<String, u64>,
    call_gen: HashMap<String, u64>,
    /// The key each owner last asked for, and the answers by key.
    wanted: HashMap<String, String>,
    answers: HashMap<String, Slot>,
    /// The canonical tool name of the call each key asked about (a view carries it).
    tools: HashMap<String, String>,
    owners_by_session: HashMap<String, HashSet<String>>,
    /// The sessions watched on each machine, so a machine's switch and events reload only its own.
    sessions_by_machine: HashMap<String, Vec<String>>,
    queue: VecDeque<RenderJob>,
    in_flight: usize,
    /// Reads of the sites or the switch that are on their way.
    loading: usize,
    pub views: Views<ToolRowView>,
    /// The Mods switch of each machine as last known (on or off), to see it move.
    pub switch_seen: HashMap<String, Option<bool>>,
    /// The switch as last read from each machine (a machine whose switch couldn't be read has none here).
    pub switches: HashMap<String, super::super::settings::mods_kept::Switch>,
    /// The rows the app shows, when something other than the open session's stream says so (a test, a scene).
    pub fixed_rows: Option<Vec<(RowHead, RowRead)>>,
}

fn owner_of(site: ToolRowSite, session: &str, call: &str) -> String {
    format!("mod-row:{}:{session}:{call}", site_name(site))
}

impl Renders {
    /// Forgets everything.
    pub fn reset(&mut self) {
        *self = Renders::default();
    }

    /// Nothing is on its way: no render running or queued, no read of the sites or the switch.
    pub fn idle(&self) -> bool {
        self.in_flight == 0 && self.queue.is_empty() && self.loading == 0
    }

    pub fn begin_load(&mut self) {
        self.loading += 1;
    }

    pub fn end_load(&mut self) {
        self.loading = self.loading.saturating_sub(1);
    }

    /// The panes the session's mods declare.
    pub fn declared_panes(&self, session: &str) -> &[DeclaredPane] {
        self.panes.get(session).map_or(&[], Vec::as_slice)
    }

    /// Holds the panes a session's mods declare, as if `/mods/sites` had said them (a scene).
    pub fn set_declared_panes(&mut self, session: &str, panes: Vec<DeclaredPane>) {
        self.panes.insert(session.to_string(), panes);
    }

    pub fn hooks_of(&self, session: &str) -> Option<&Vec<SiteHook>> {
        self.hooks.get(session).and_then(Option::as_ref)
    }

    pub fn knows(&self, session: &str) -> bool {
        self.hooks.contains_key(session)
    }

    /// Whether a row's tool could be asked for: its hooks are not yet known, or a site hooks it. A cheap test to run before
    /// building a row.
    pub fn may_ask(&self, session: &str, tool: &str) -> bool {
        match self.hooks.get(session) {
            None => true,
            Some(None) => false,
            Some(Some(hooks)) => hooks.iter().any(|h| h.tools.as_ref().is_none_or(|t| t.iter().any(|n| n == tool))),
        }
    }

    pub fn watched(&self, machine: &str) -> Vec<String> {
        self.sessions_by_machine.get(machine).cloned().unwrap_or_default()
    }

    pub fn watched_machines(&self) -> Vec<String> {
        self.sessions_by_machine.keys().cloned().collect()
    }

    /// `watchModSession`: remember the session on its machine, and read its hooks once.
    pub fn watch(&mut self, machine: &str, session: &str) -> Vec<Effect> {
        // The right panel passes "" while no session is open: nothing to watch.
        if session.is_empty() {
            return vec![];
        }
        self.remember(machine, session);
        self.first_load(machine, session)
    }

    fn remember(&mut self, machine: &str, session: &str) {
        let list = self.sessions_by_machine.entry(machine.to_string()).or_default();
        if !list.iter().any(|s| s == session) {
            list.push(session.to_string());
        }
    }

    fn first_load(&mut self, machine: &str, session: &str) -> Vec<Effect> {
        if self.hooks.contains_key(session) {
            return vec![];
        }
        self.hooks.insert(session.to_string(), None);
        vec![Effect::LoadHooks { machine: machine.to_string(), session_id: session.to_string() }]
    }

    /// `loadHooks` done: keep the hooks, forget the drawings when there are none, and make the session's rows ask again.
    pub fn hooks_loaded(&mut self, session: &str, loaded: Loaded) {
        match loaded {
            Loaded::Hooks { hooks, panes } => {
                self.hooks.insert(session.to_string(), Some(hooks));
                self.panes.insert(session.to_string(), panes);
            }
            Loaded::Off => {
                self.panes.insert(session.to_string(), vec![]);
                self.hooks.insert(session.to_string(), None);
            }
            Loaded::Failed => {
                self.hooks.insert(session.to_string(), None);
            }
        }
        if self.hooks.get(session).is_some_and(Option::is_none) {
            self.forget_views(session);
        }
        // A new version may draw differently.
        *self.session_gen.entry(session.to_string()).or_default() += 1;
    }

    fn forget_views(&mut self, session: &str) {
        for owner in self.owners_by_session.get(session).cloned().unwrap_or_default() {
            self.views.remove_by_owner(&owner);
            self.wanted.remove(&owner);
        }
    }

    /// The sites of every session watched on the machine, again (`reloadAll`).
    pub fn reload_all(&mut self, machine: &str) -> Vec<Effect> {
        self.watched(machine).into_iter().map(|session_id| Effect::LoadHooks { machine: machine.to_string(), session_id }).collect()
    }

    /// `mod.ui`: make the session's rows (or one call's) ask again. With no session: every session known.
    pub fn invalidate(&mut self, session: Option<&str>, request_id: Option<&str>) {
        let ids: Vec<String> = match session {
            Some(id) => vec![id.to_string()],
            None => self.hooks.keys().cloned().collect(),
        };
        for id in ids {
            match request_id {
                Some(call) => *self.call_gen.entry(format!("{id}:{call}")).or_default() += 1,
                None => *self.session_gen.entry(id).or_default() += 1,
            }
        }
    }

    /// One row, as the rows' effect runs it (`sync`). Returns what to fetch.
    pub fn sync(&mut self, machine: &str, head: &RowHead, read: impl FnOnce() -> RowRead) -> Vec<Effect> {
        let (session, call) = (head.session_id.as_str(), head.call_id.as_str());
        if session.is_empty() || call.is_empty() {
            return vec![];
        }
        self.remember(machine, session);
        if !self.hooks.contains_key(session) {
            return self.first_load(machine, session);
        }
        let tool = get_tool(&head.raw_tool).name;
        let mut effects = vec![];
        let mut read = Lazy { f: Some(read), value: None };
        for site in SITES {
            let hooked = self
                .hooks_of(session)
                .is_some_and(|hooks| hooks.iter().any(|h| h.site == site && h.tools.as_ref().is_none_or(|t| t.contains(&tool))));
            let owner = owner_of(site, session, call);
            if hooked {
                effects.extend(self.request(machine, head, site, &tool, &mut read));
            } else if self.wanted.remove(&owner).is_some() {
                self.views.remove_by_owner(&owner);
            }
        }
        effects
    }

    fn request(
        &mut self,
        machine: &str,
        head: &RowHead,
        site: ToolRowSite,
        tool: &str,
        read: &mut Lazy<impl FnOnce() -> RowRead>,
    ) -> Vec<Effect> {
        let (session, call) = (head.session_id.as_str(), head.call_id.as_str());
        let status = canonical_status(&head.status);
        let owner = owner_of(site, session, call);
        let key = [
            session,
            call,
            site_name(site),
            status,
            &self.session_gen.get(session).copied().unwrap_or(0).to_string(),
            &self.call_gen.get(&format!("{session}:{call}")).copied().unwrap_or(0).to_string(),
        ]
        .join("\0");
        if self.wanted.get(&owner) == Some(&key) {
            return vec![];
        }
        self.wanted.insert(owner.clone(), key.clone());
        self.owners_by_session.entry(session.to_string()).or_default().insert(owner.clone());
        match self.answers.get(&key) {
            // Another row of the call asked already and is waiting, or has the answer: it is applied below or on its arrival.
            Some(Slot::Waiting) => vec![],
            Some(Slot::Done(answer)) => {
                let answer = answer.clone();
                self.apply(&owner, site, session, call, tool, answer);
                vec![]
            }
            None => {
                self.answers.insert(key.clone(), Slot::Waiting);
                self.tools.insert(key.clone(), tool.to_string());
                let body = json!({ "site": site_name(site), "requestId": call, "props": render_props(head, read.get()) });
                self.schedule(RenderJob { machine: machine.to_string(), session_id: session.to_string(), key, body })
            }
        }
    }

    fn schedule(&mut self, job: RenderJob) -> Vec<Effect> {
        if self.in_flight < MAX_IN_FLIGHT {
            self.in_flight += 1;
            vec![Effect::Render(job)]
        } else {
            self.queue.push_back(job);
            vec![]
        }
    }

    /// A render ended (its answer, or `None` when it failed or was refused): applies it to the rows still wanting that key,
    /// and starts the next queued one.
    pub fn answered(&mut self, key: &str, answer: Option<Answer>) -> Vec<Effect> {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.answers.insert(key.to_string(), Slot::Done(answer.clone()));
        let tool = self.tools.get(key).cloned().unwrap_or_default();
        let mut parts = key.split('\0');
        let (session, call, site) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        if let (Some(site), Some(answer)) = (site_of(site), answer) {
            let owner = owner_of(site, session, call);
            // A newer ask (another status, a `mod.ui`) has taken this one's place.
            if self.wanted.get(&owner).map(String::as_str) == Some(key) {
                self.apply(&owner, site, session, call, &tool, Some(answer));
            }
        }
        match self.queue.pop_front() {
            Some(next) => {
                self.in_flight += 1;
                vec![Effect::Render(next)]
            }
            None => vec![],
        }
    }

    fn apply(&mut self, owner: &str, site: ToolRowSite, session: &str, call: &str, tool: &str, answer: Option<Answer>) {
        self.views.remove_by_owner(owner);
        let Some(answer) = answer else { return };
        // No mod draws: Fleet's own row. (A mod that draws nothing comes back with its name and a null tree.)
        if answer.tree.is_null() && answer.mods.is_empty() {
            return;
        }
        // `on_action` stays unset: a used control goes to `State::mod_action`, which knows the session's machine.
        let view = ModTreeView::new(answer.tree, answer.mods);
        self.views.contribute(
            owner,
            vec![ToolRowView { site, session_id: session.to_string(), call_id: call.to_string(), tool: tool.to_string(), view }],
        );
    }

    pub fn view(&self, site: ToolRowSite, session: &str, call: &str) -> Option<std::sync::Arc<ToolRowView>> {
        self.views.get(&tool_row_view_id(site, session, call))
    }
}
