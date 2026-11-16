//! The Context tab's data and state (`SessionContextCanvas.vue`, `SmartLinkRow.vue`, rows 12.1 to 12.11 of
//! docs/parity/08-session-header-and-context.md): which section each link goes in, what a row's detail line says, the
//! workspace list, the attach form, the Refresh button and the flash a header chip asks for. The words and the grouping are
//! plain functions; `State` methods at the end do the loads and requests on threads. The views are `view::context_tab` and
//! `view::smart_link_row`.
use super::smart_links::{
    Enrichment, OriginRecord, Relationship, SmartLink, format_checked_ago, has_title, is_pull_request, link_number, summarize_checks,
};
use crate::State;
use rune::{TextEdit, Tx};

/// How long Refresh keeps its spinner at least, so the press reads as an action (the results come as pushed updates).
pub const REFRESH_SPIN_SECS: f64 = 1.2;
/// The "GitHub checked …" line re-reads the clock this often.
pub const FRESHNESS_TICK_SECS: f64 = 15.0;
/// `context-flash`: the ring fades out over this long.
pub const FLASH_SECS: f64 = 1.2;

/// What the content panel shows. Component 10 owns the tab strip; until it is built the panel holds the plan and this tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelTab {
    #[default]
    Plan,
    Context,
}

/// Where a session started when an automation did it: the row's origin gives the name, the origin records the detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationOrigin {
    pub name: String,
    pub summary: Option<String>,
    pub created_at: Option<String>,
}

/// `automationOrigin`: only a `start-session` record of an automation counts.
pub fn automation_origin(origin: Option<&super::rows::Origin>, records: &[OriginRecord]) -> Option<AutomationOrigin> {
    let origin = origin.filter(|o| o.source_type == "automation")?;
    let record = records.iter().find(|r| r.source_type == "automation" && r.action_id == "start-session");
    Some(AutomationOrigin {
        name: origin.title.clone().unwrap_or_else(|| "Automation".to_string()),
        summary: record.and_then(|r| r.summary.clone()).filter(|s| !s.is_empty()),
        created_at: record.map(|r| r.created_at.clone()).filter(|s| !s.is_empty()),
    })
}

/// "Automation · <summary> · <time ago>": the parts that are known, joined as Fleet's template does.
pub fn automation_detail(a: &AutomationOrigin, now: i64, offset_secs: i64) -> String {
    let mut text = "Automation".to_string();
    if let Some(summary) = &a.summary {
        text.push_str(" \u{b7} ");
        text.push_str(summary);
    }
    if let Some(ago) = format_checked_ago(a.created_at.as_deref(), now, offset_secs) {
        text.push_str(" \u{b7} ");
        text.push_str(&ago);
    }
    text
}

/// The links of one session, grouped as the tab shows them (`byRelationship`).
#[derive(Debug, Default)]
pub struct Groups<'a> {
    pub origin: Option<&'a SmartLink>,
    pub own: Vec<&'a SmartLink>,
    pub pinned: Vec<&'a SmartLink>,
    pub mentioned: Vec<&'a SmartLink>,
}

impl<'a> Groups<'a> {
    pub fn of(links: &[&'a SmartLink]) -> Groups<'a> {
        let by = |r: Relationship| links.iter().copied().filter(move |l| l.relationship == r);
        Groups {
            origin: by(Relationship::Origin).next(),
            own: by(Relationship::Own).collect(),
            pinned: by(Relationship::Pinned).collect(),
            mentioned: by(Relationship::Mentioned).collect(),
        }
    }
}

/// "This session's pull request" or "...requests".
pub fn own_heading(count: usize) -> &'static str {
    if count == 1 { "This session's pull request" } else { "This session's pull requests" }
}

/// A row's detail line (`SmartLinkRow` `detail`): the note, what the link is (once GitHub gave it a title), then how GitHub
/// answered.
pub fn row_detail(link: &SmartLink, note: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(note) = note.filter(|n| !n.is_empty()) {
        parts.push(note.to_string());
    }
    let number = link_number(link).map_or("null".to_string(), |n| n.to_string());
    // Until GitHub answers, the title already reads "Issue #42".
    if has_title(link) {
        parts.push(format!("{} #{number}", if is_pull_request(link) { "Pull request" } else { "Issue" }));
    }
    match link.enrichment {
        Enrichment::Resolved => {
            if !link.status_label.is_empty() {
                parts.push(link.status_label.clone());
            }
            let failing = summarize_checks(link).failing;
            if is_pull_request(link) && !link.is_terminal && failing > 0 {
                parts.push(format!("{failing} failing"));
            }
        }
        Enrichment::Pending => parts.push("Checking GitHub\u{2026}".into()),
        Enrichment::NotConnected => parts.push("GitHub not connected".into()),
        Enrichment::NotFound => parts.push("Not found on GitHub".into()),
        Enrichment::Error => parts.push("Couldn't check GitHub".into()),
    }
    parts.join(" \u{b7} ")
}

/// `workspace`: Branch, Isolation (unless the session uses the folder as it is) and Directory, only those that exist.
pub fn workspace_rows(branch: Option<&str>, isolation: Option<&str>, directory: &str) -> Vec<(&'static str, String)> {
    let mut rows = Vec::new();
    if let Some(branch) = branch.filter(|b| !b.is_empty()) {
        rows.push(("Branch", branch.to_string()));
    }
    if let Some(isolation) = isolation.filter(|i| !i.is_empty() && *i != "existing") {
        rows.push(("Isolation", isolation.to_string()));
    }
    if !directory.is_empty() {
        rows.push(("Directory", directory.to_string()));
    }
    rows
}

/// What the tab shows in place of (or under) the sections, in Fleet's order: loading, the error, the empty note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Feedback {
    None,
    Loading,
    Error(String),
    Empty,
}

/// `firstLoad`, `loadError` and `isEmpty`: until the links arrive the session isn't empty, its links are loading; a failure
/// says so instead of "Nothing attached yet"; held links hide both.
pub fn feedback(has_links: bool, has_automation: bool, loading: bool, error: Option<&str>) -> Feedback {
    if has_links {
        return Feedback::None;
    }
    if loading {
        return Feedback::Loading;
    }
    if let Some(why) = error {
        return Feedback::Error(why.to_string());
    }
    if has_automation { Feedback::None } else { Feedback::Empty }
}

/// The attach form, the Refresh button and the flash: everything the tab keeps between frames.
pub struct ContextTab {
    pub tab: PanelTab,
    pub attaching: bool,
    pub url: TextEdit,
    pub attach_error: Option<String>,
    pub attach_busy: bool,
    /// Refresh was pressed at this time; its spinner lasts until [`REFRESH_SPIN_SECS`] after.
    pub refreshing_since: Option<f64>,
    /// The request being shown and when it started, and the last nonce acted on (a repeat request flashes again).
    pub flash: Option<(String, f64)>,
    pub seen_nonce: u64,
    /// The session whose links and origin records this tab has asked for.
    pub loaded_for: Option<String>,
    /// The clock the freshness line was last computed from.
    pub freshness_at: f64,
}

impl Default for ContextTab {
    fn default() -> Self {
        ContextTab {
            tab: PanelTab::Plan,
            attaching: false,
            url: TextEdit::new(String::new()),
            attach_error: None,
            attach_busy: false,
            refreshing_since: None,
            flash: None,
            seen_nonce: 0,
            loaded_for: None,
            freshness_at: 0.0,
        }
    }
}

impl ContextTab {
    pub fn refreshing(&self, clock: f64) -> bool {
        self.refreshing_since.is_some_and(|at| clock - at < REFRESH_SPIN_SECS)
    }

    /// `startAttach`.
    pub fn start_attach(&mut self) {
        self.attaching = true;
        self.attach_error = None;
    }

    /// `cancelAttach`.
    pub fn cancel_attach(&mut self) {
        self.attaching = false;
        self.url = TextEdit::new(String::new());
        self.attach_error = None;
    }

    /// Attach is enabled when the field has something besides spaces and no request is out.
    pub fn can_attach(&self) -> bool {
        !self.attach_busy && !self.url.text.trim().is_empty()
    }

    /// How strong the ring of a flashing target is now, from the accent at 55% fading out; `None` once it has faded.
    pub fn flash_ring(&self, target: &str, clock: f64) -> Option<f32> {
        let (flashing, start) = self.flash.as_ref()?;
        let since = clock - start;
        (flashing == target && (0.0..FLASH_SECS).contains(&since)).then(|| 0.55 * (1.0 - (since / FLASH_SECS) as f32))
    }
}

impl State {
    /// Whether the Context tab is on screen: the panel is open on it.
    pub fn context_visible(&self) -> bool {
        self.content_open && self.context.tab == PanelTab::Context
    }

    /// Opens the panel on the Context tab.
    pub fn show_context_tab(&mut self) {
        self.context.tab = PanelTab::Context;
        self.set_content_open(true);
    }

    pub fn set_panel_tab(&mut self, tab: PanelTab) {
        self.context.tab = tab;
    }

    /// On open (`watch(sessionId, immediate)`): the session's links and origin records are asked for once per session shown.
    /// A failed load is asked for again by Try again, not here. Called every frame; it does nothing unless the session changed.
    pub fn keep_context(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.context_visible() {
            return false;
        }
        let id = self.session().id.clone();
        let mut changed = false;
        if self.context.loaded_for.as_deref() != Some(id.as_str()) {
            self.context.loaded_for = Some(id.clone());
            self.context.cancel_attach();
            self.context.attach_busy = false;
            self.ensure_links_loaded(&id);
            self.load_origin_records(&id);
            changed = true;
        }
        // A header chip's request: flash its target and bring it into view, once per request.
        if let Some(request) = self.smart_links.focus.clone()
            && request.nonce != self.context.seen_nonce
            && request.session_id == id
        {
            self.context.seen_nonce = request.nonce;
            let key = target_key(&request.target);
            tx.scroll_into_view_in("context-scroll", key.clone());
            self.context.flash = Some((request.target, now));
            changed = true;
        }
        if self.context.flash.as_ref().is_some_and(|(_, at)| now - at < FLASH_SECS) {
            tx.wake_after(std::time::Duration::from_millis(33));
            changed = true;
        } else if self.context.flash.take().is_some() {
            changed = true;
        }
        if let Some(at) = self.context.refreshing_since {
            if now - at < REFRESH_SPIN_SECS {
                tx.wake_after(std::time::Duration::from_millis(100));
            } else {
                self.context.refreshing_since = None;
                self.context.freshness_at = now;
            }
            changed = true;
        }
        if now - self.context.freshness_at >= FRESHNESS_TICK_SECS {
            self.context.freshness_at = now;
            changed = true;
        }
        tx.wake_after(std::time::Duration::from_secs_f64(FRESHNESS_TICK_SECS));
        changed
    }

    pub fn start_attach(&mut self) {
        self.context.start_attach();
    }

    /// `connectGitHub`: Settings, until component 16 has the GitHub plugin's page to go to.
    pub fn connect_github(&mut self) {
        self.rail = crate::Rail::Settings;
    }

    /// Try again after a failed load (`retryLoad`).
    pub fn retry_links(&mut self) {
        let id = self.session().id.clone();
        self.ensure_links_loaded(&id);
    }

    /// `submitAttach`: sends the URL; success clears and closes the form, a refusal keeps it and says why.
    pub fn submit_attach(&mut self) {
        if !self.context.can_attach() {
            return;
        }
        let url = self.context.url.text.trim().to_string();
        let id = self.session().id.clone();
        self.context.attach_busy = true;
        self.add_link(&id, &url, move |s, _, error| {
            s.context.attach_busy = false;
            s.context.attach_error = error;
            if s.context.attach_error.is_none() {
                s.context.cancel_attach();
            }
        });
    }

    /// `refresh`: asks for another check and keeps the spinner for a moment.
    pub fn refresh_context(&mut self) {
        if self.context.refreshing(self.clock) {
            return;
        }
        let id = self.session().id.clone();
        self.context.refreshing_since = Some(self.clock);
        self.refresh_links(&id, |_, _, _| {});
    }

    /// The tab's links for the open session, already filtered to the visible ones.
    pub fn context_links(&self) -> Vec<&SmartLink> {
        self.smart_links.visible_links(&self.session().id)
    }
}

/// The key of the element a request's target names: a link's row, or the origin section.
pub fn target_key(target: &str) -> String {
    if target == "origin" { "context-origin".to_string() } else { format!("context-link-{target}") }
}
