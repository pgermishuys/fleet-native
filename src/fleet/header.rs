//! What the session header says, as data (`SessionDetailHeader.vue`, `SessionActionToolbar.vue`, the capability logic of
//! `routes/sessions.$id.tsx`, `SessionAnalyticsPopover.vue`'s formats). Pure: no Rune, no I/O. Piece B draws it.
//!
//! `HeaderModel::build` takes what the page holds (the row of the list, or the detail read from the session's own
//! machine) and gives every word and flag the header draws. The machine is the one the open session runs on, so a
//! session on another machine shows that machine's data and that machine's "not answering".
use super::groups::{LineageKind, Listing, lineage_of, lineage_origin_of};
use super::row_status::{display_title, format_date};
use super::rows::{Capabilities, Retrying, Row};
use crate::composer::choice::{ModelOption, model_display_name};
use crate::composer::format::{format_count, format_money};
use crate::composer::status::HarnessInfo;
use crate::conv::working::{describe_retry, retry_attempt_label};
use serde_json::Value;

/// `sessionStatusIndicator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    Working,
    Disconnected,
    NotAnswering,
    Retry,
    Waiting,
    Idle,
}

impl Indicator {
    /// The `data-status` Fleet's screen-reader span carries.
    pub fn word(self) -> &'static str {
        match self {
            Indicator::Working => "working",
            Indicator::Disconnected => "disconnected",
            Indicator::NotAnswering => "not-answering",
            Indicator::Retry => "retry",
            Indicator::Waiting => "waiting",
            Indicator::Idle => "idle",
        }
    }
}

/// `glyphStatus`: the status the row's glyph is drawn for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Active,
    WaitingInput,
    Disconnected,
    Completed,
    Stopped,
    Error,
    Idle,
}

impl Glyph {
    pub fn word(self) -> &'static str {
        match self {
            Glyph::Active => "active",
            Glyph::WaitingInput => "waiting_input",
            Glyph::Disconnected => "disconnected",
            Glyph::Completed => "completed",
            Glyph::Stopped => "stopped",
            Glyph::Error => "error",
            Glyph::Idle => "idle",
        }
    }
}

/// `normalizeLifecycleStatus`, with the page's default: a session that says nothing is `running`.
pub fn effective_lifecycle(word: &str) -> &'static str {
    match word {
        "completed" | "complete" => "completed",
        "stopped" => "stopped",
        "error" => "error",
        "disconnected" => "disconnected",
        _ => "running",
    }
}

fn normalize_activity(word: &str) -> Option<&'static str> {
    Some(match word {
        "active" | "busy" => "busy",
        "delegating" => "delegating",
        "retry" => "retry",
        "waiting_input" => "waiting_input",
        "idle" => "idle",
        _ => return None,
    })
}

/// `isActiveActivityStatus`.
pub fn is_active_activity(word: &str) -> bool {
    matches!(word, "busy" | "delegating" | "retry")
}

/// The page's `effectiveActivityStatus`: busy while a prompt is sent and not yet echoed or the row is active (a retrying
/// row counts as busy here), else the row's, else idle.
pub fn effective_activity(row_activity: &str, prompt_in_flight: bool) -> &'static str {
    if prompt_in_flight || is_active_activity(row_activity) || row_activity == "active" {
        return "busy";
    }
    normalize_activity(row_activity).unwrap_or("idle")
}

/// `headerActivityStatus`: a retrying row keeps `retry` for the header, which names the retry.
pub fn header_activity(row_activity: &str, effective: &'static str) -> &'static str {
    if row_activity == "retry" { "retry" } else { effective }
}

pub fn indicator(lifecycle: &str, activity: &str, machine_answering: bool) -> Indicator {
    if lifecycle == "disconnected" {
        return Indicator::Disconnected;
    }
    if !machine_answering && matches!(activity, "busy" | "delegating" | "retry") {
        return Indicator::NotAnswering;
    }
    match activity {
        "retry" => Indicator::Retry,
        "waiting_input" => Indicator::Waiting,
        "busy" | "delegating" => Indicator::Working,
        _ => Indicator::Idle,
    }
}

/// `sessionStatusLabel`.
pub fn status_label(indicator: Indicator, retry: &Retrying) -> String {
    match indicator {
        Indicator::Working => "Working".into(),
        Indicator::Disconnected => "Disconnected".into(),
        Indicator::NotAnswering => "Not answering".into(),
        Indicator::Retry => retry_attempt_label(retry).map_or("Retrying\u{2026}".into(), |a| format!("Retrying ({a})\u{2026}")),
        Indicator::Waiting => "Needs input".into(),
        Indicator::Idle => "Idle".into(),
    }
}

pub fn glyph_status(indicator: Indicator, lifecycle: &str) -> Glyph {
    match indicator {
        Indicator::Working | Indicator::Retry => Glyph::Active,
        Indicator::Waiting => Glyph::WaitingInput,
        _ => match lifecycle {
            "disconnected" => Glyph::Disconnected,
            "completed" => Glyph::Completed,
            "stopped" => Glyph::Stopped,
            "error" => Glyph::Error,
            _ => Glyph::Idle,
        },
    }
}

/// The path cut at its last separator, so the head gives way first and the folder's own name stays (`directoryParts`).
pub fn directory_parts(directory: &str) -> (String, String) {
    match directory.rfind(['/', '\\']) {
        Some(i) if i > 0 => (directory[..i].to_string(), directory[i..].to_string()),
        _ => (String::new(), directory.to_string()),
    }
}

/// What the page lets the header offer (`canAbort` and the rest, with the route's fallbacks).
#[derive(Clone, Debug, PartialEq)]
pub struct Actions {
    pub can_abort: bool,
    pub can_archive: bool,
    pub can_restore: bool,
    pub can_fork: bool,
    /// Why Fork is off; `None` when it is on, and for an archived session (which has no Fork at all).
    pub fork_disabled_reason: Option<String>,
    pub can_delete: bool,
}

impl Actions {
    pub fn resolve(caps: &Capabilities, archived: bool, lifecycle: &str, activity: &str) -> Actions {
        let fallback_abort = lifecycle == "running" && is_active_activity(activity);
        let can_fork = caps.can_fork.unwrap_or(true);
        Actions {
            can_abort: caps.can_abort.unwrap_or(fallback_abort),
            // Capabilities are resolved when the list loads, so the retention state wins once it changes here.
            can_archive: !archived && caps.can_archive.unwrap_or(!archived),
            can_restore: archived,
            can_fork,
            fork_disabled_reason: if archived || can_fork {
                None
            } else {
                Some(caps.fork_disabled_reason.clone().unwrap_or_else(|| "This session can't be forked.".into()))
            },
            can_delete: caps.can_delete.unwrap_or(true),
        }
    }

    /// The Fork button's tooltip (on a wrapper in Fleet, since a disabled button shows none): what it does, or why it can't.
    pub fn fork_tip(&self) -> String {
        if self.can_fork {
            "Fork: a new session with a copy of this conversation".into()
        } else {
            self.fork_disabled_reason.clone().unwrap_or_default()
        }
    }

    /// The button's `aria-label`.
    pub fn fork_label(&self) -> String {
        if self.can_fork { "Fork".into() } else { format!("Fork: {}", self.fork_disabled_reason.clone().unwrap_or_default()) }
    }

    /// Whether the Fork button is drawn: not for an archived session; off, with its reason, when it can't fork.
    pub fn show_fork(&self, archived: bool) -> bool {
        !archived && (self.can_fork || self.fork_disabled_reason.is_some())
    }
}

/// The chip of the machine the open session runs on (rows 2.1 and 17.44), shown when the user has more than one.
#[derive(Clone, Debug, PartialEq)]
pub struct MachineChip {
    pub name: String,
    pub answering: bool,
}

impl MachineChip {
    /// "mini", or "mini · not answering".
    pub fn label(&self) -> String {
        if self.answering { self.name.clone() } else { format!("{} \u{b7} not answering", self.name) }
    }

    pub fn tooltip(&self) -> String {
        if self.answering { format!("Runs on {}", self.name) } else { format!("{} isn't answering", self.name) }
    }
}

/// The removed-checkout chip's state (`showCheckoutChip` and its three templates).
#[derive(Clone, Debug, PartialEq)]
pub enum Checkout {
    /// "Putting the checkout back…"
    Restoring,
    /// "Checkout removed" and Restore now.
    Removed,
    /// "Checkout restored. Dependencies and .env aren't back." for a few seconds.
    Restored,
}

impl Checkout {
    pub fn text(&self) -> &'static str {
        match self {
            Checkout::Restoring => "Putting the checkout back\u{2026}",
            Checkout::Removed => "Checkout removed",
            Checkout::Restored => "Checkout restored. Dependencies and .env aren't back.",
        }
    }
}

/// `checkoutRemovedLabel`: the chip's tooltip, with the date when it is known.
pub fn checkout_removed_sentence(removed_at: Option<&str>, branch: &str, now: i64, offset_secs: i64) -> String {
    let date = removed_at.and_then(super::work::parse_iso_ms).map(|at| format_date(at, now, offset_secs)).unwrap_or_default();
    let when = if date.is_empty() { String::new() } else { format!(" on {date}") };
    let branch = if branch.is_empty() { String::new() } else { format!(" {branch}") };
    format!(
        "Fleet removed this session's checkout{when} to save space. The branch{branch} and this conversation are kept; your next message puts it back."
    )
}

/// The checkout chip for a row: a restore under way here or elsewhere, a removed checkout, or the note after a restore.
pub fn checkout_state(row: &Row, restoring_here: bool, restored_note: bool) -> Option<Checkout> {
    if restoring_here || row.checkout_restoring {
        Some(Checkout::Restoring)
    } else if row.checkout_removed_at.is_some() {
        Some(Checkout::Removed)
    } else if restored_note {
        Some(Checkout::Restored)
    } else {
        None
    }
}

/// The analytics popover's two values (`SessionAnalyticsPopover.vue`): "—" for a number the server doesn't have.
pub fn usage_strings(tokens: Option<u64>, cost: Option<f64>) -> (String, String) {
    let or_dash = |s: String| if s.is_empty() { "\u{2014}".to_string() } else { s };
    (or_dash(format_count(tokens.map(|t| t as f64))), or_dash(format_money(cost)))
}

/// The open session's own record, read from `GET /api/sessions/{id}` on the machine it runs on (`useSessionDetail`). The
/// same fields as a list item, so a session the list doesn't hold (another machine's, or one opened by link) still has
/// a header.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detail {
    pub title: String,
    pub project_name: Option<String>,
    pub branch: String,
    pub row: Row,
    pub listing: Listing,
}

impl Detail {
    pub fn from_wire(v: &Value) -> Option<Detail> {
        if !v.is_object() {
            return None;
        }
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let mut row = Row::from_list_item(v);
        // The detail names its lifecycle `lifecycleStatus` (and `status`), not `sessionStatus`.
        if row.lifecycle.is_empty() {
            row.lifecycle = text("lifecycleStatus").or_else(|| text("status")).unwrap_or_default();
        }
        Some(Detail {
            title: text("title").unwrap_or_default(),
            project_name: text("projectName").filter(|n| !n.is_empty()),
            branch: text("branch").unwrap_or_default(),
            row,
            listing: Listing::from_row(v),
        })
    }
}

/// What `HeaderModel::build` reads.
pub struct Inputs<'a> {
    pub title: &'a str,
    pub project_name: Option<&'a str>,
    /// The row of the list, else the detail's.
    pub row: &'a Row,
    pub listing: &'a Listing,
    pub branch: &'a str,
    /// A prompt was sent and its echo hasn't come back (the page counts it as busy).
    pub prompt_in_flight: bool,
    /// The machine the open session runs on, when the user has more than one; `answering` is its state.
    pub machine: Option<MachineChip>,
    pub harnesses: &'a [HarnessInfo],
    pub models: &'a [ModelOption],
    pub now: i64,
    pub offset_secs: i64,
    pub restoring_here: bool,
    pub restored_note: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderModel {
    pub title: String,
    pub archived: bool,
    /// `effective_lifecycle` of the row: the stopped banner reads it.
    pub lifecycle: &'static str,
    pub indicator: Indicator,
    pub status_label: String,
    pub glyph: Glyph,
    /// "Retrying · attempt 3 of 10 · in 12 s · API overloaded (529)", only while the indicator is `retry`.
    pub retry_note: Option<String>,
    pub machine: Option<MachineChip>,
    pub project: Option<String>,
    pub harness_label: Option<String>,
    pub profile: Option<String>,
    pub model_label: Option<String>,
    pub branch: Option<String>,
    pub directory: String,
    pub directory_head: String,
    pub directory_leaf: String,
    pub tags: Vec<String>,
    /// Who started it: the parent's id and how, unless the user moved the session out of it.
    pub lineage: Option<(String, LineageKind)>,
    pub checkout: Option<Checkout>,
    pub checkout_sentence: String,
    pub actions: Actions,
    pub tokens_text: String,
    pub cost_text: String,
}

impl HeaderModel {
    pub fn build(i: &Inputs) -> HeaderModel {
        let row = i.row;
        let lifecycle = effective_lifecycle(&row.lifecycle);
        let effective = effective_activity(&row.activity, i.prompt_in_flight);
        let activity = header_activity(&row.activity, effective);
        let answering = i.machine.as_ref().is_none_or(|m| m.answering);
        let indicator = indicator(lifecycle, activity, answering);
        let archived = row.archived;
        let harness_label = row
            .harness_type
            .as_deref()
            .map(|t| i.harnesses.iter().find(|h| h.kind == t).and_then(|h| h.display_name.clone()).unwrap_or_else(|| t.to_string()));
        let model_id = row.selected_model.as_ref().map(|m| m.model_id.as_str()).or(row.last_model_id.as_deref());
        let model = model_display_name(model_id, i.models);
        let (head, leaf) = directory_parts(&row.directory);
        let (tokens_text, cost_text) = usage_strings(row.total_tokens, row.total_cost);
        HeaderModel {
            title: display_title(None, i.title),
            archived,
            lifecycle,
            indicator,
            status_label: status_label(indicator, &row.retrying),
            glyph: glyph_status(indicator, lifecycle),
            retry_note: (indicator == Indicator::Retry).then(|| describe_retry(&row.retrying, i.now)),
            machine: i.machine.clone(),
            project: i.project_name.map(str::trim).filter(|p| !p.is_empty()).map(str::to_string),
            harness_label,
            profile: row.harness_profile_name.clone(),
            model_label: (!model.is_empty()).then_some(model),
            branch: Some(i.branch).filter(|b| !b.is_empty()).map(str::to_string),
            directory: row.directory.clone(),
            directory_head: head,
            directory_leaf: leaf,
            tags: row.tags.clone(),
            lineage: lineage_of(i.listing),
            checkout: checkout_state(row, i.restoring_here, i.restored_note),
            checkout_sentence: checkout_removed_sentence(row.checkout_removed_at.as_deref(), i.branch, i.now, i.offset_secs),
            actions: Actions::resolve(&row.capabilities, archived, lifecycle, effective),
            tokens_text,
            cost_text,
        }
    }
}

impl crate::State {
    /// Reads the open session's own record from the machine it runs on, off the UI thread, and keeps it for the header.
    pub fn load_header_detail(&mut self, session: &str) {
        let parent = self.store.session_by_id(&self.projects, session).and_then(|s| lineage_origin_of(&s.listing));
        if let Some((parent, _)) = parent {
            self.load_parent_title(&parent);
        }
        // The chips and the pill read the session's links (`ensureLoaded` when the session changes).
        self.ensure_links_loaded(session);
        let Some(link) = self.link_for(session) else { return };
        let id = session.to_string();
        let path = format!("/api/sessions/{}", crate::conv::image_sniff::encode_component(session));
        link.off_thread(
            move |base| super::http::get(base, &path),
            move |s, _, result| {
                // A session the machine can't give (gone, not answering) leaves the header on its row.
                if let Some(detail) = result.ok().as_ref().and_then(Detail::from_wire) {
                    let parent = lineage_of(&detail.listing).map(|(p, _)| p);
                    s.header_details.insert(id, detail);
                    if let Some(parent) = parent {
                        s.load_parent_title(&parent);
                    }
                }
            },
        );
    }

    /// The machine chip of the session's machine, when the user has more than one machine.
    pub fn header_machine(&self, session: &str) -> Option<MachineChip> {
        if !self.machines.has_machines() {
            return None;
        }
        // The session's own machine; without a link (the scenes) the lookups still say where it was listed.
        let key = self
            .store
            .machine_of(session)
            .or_else(|| self.machines.session_machines.get(session).map(String::as_str))
            .unwrap_or(super::settings::HOME_MACHINE_KEY)
            .to_string();
        let entry = self.machines.entries().into_iter().find(|e| e.key == key)?;
        Some(MachineChip { answering: self.machines.is_answering(&key), name: entry.name })
    }
}

impl crate::State {
    /// The header of the open session. The row of the list wins, as the page's does; the session's own record fills what
    /// the list doesn't carry (its profile, a branch the list had none for).
    pub fn header_model(&self) -> HeaderModel {
        let session = self.session();
        let detail = self.header_details.get(&session.id);
        let mut row = session.row.clone();
        if row.harness_profile_name.is_none() {
            row.harness_profile_name = detail.and_then(|d| d.row.harness_profile_name.clone());
        }
        // The model that answered last comes from the open conversation first, as the composer's "Default" does.
        row.last_model_id = self.last_model_id();
        let branch = if session.branch.is_empty() { detail.map_or("", |d| d.branch.as_str()) } else { session.branch.as_str() };
        let no_models = Vec::new();
        let models = self.pickers.catalog_of(&session.id).map_or(&no_models, |c| &c.models);
        HeaderModel::build(&Inputs {
            title: &session.title,
            project_name: Some(self.project().name.as_str()),
            row: &row,
            listing: &session.listing,
            branch,
            prompt_in_flight: self.optimistic.is_some(),
            machine: self.header_machine(&session.id),
            harnesses: &self.pickers.harnesses,
            models,
            now: crate::view::message::now_ms(self),
            offset_secs: crate::conv::time::local_offset_secs(),
            restoring_here: self.head.restoring_here(&session.id),
            restored_note: self.head.restored_note(&session.id, self.clock),
        })
    }
}

impl crate::State {
    /// A chip or the pill was pressed (`show`): open the right panel and ask the Context tab to flash `target`, a link id or
    /// "origin". Fleet also introduces and activates the tab; until component 10 owns the tab strip, the panel's two tabs are the plan and this one.
    pub fn show_in_context(&mut self, target: &str) {
        self.show_context_tab();
        let id = self.session().id.clone();
        self.smart_links.request_focus(&id, target);
    }
}
