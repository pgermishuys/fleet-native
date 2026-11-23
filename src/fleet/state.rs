//! How what the connection learns reaches the state. Every method here runs on the UI thread, from
//! an `AppHandle::update` closure, and does no I/O.
use super::events::{Event, Kind};
use super::history::{self, Page};
use super::http;
use super::link::Connection;
use super::model::{Msg, StreamState};
use super::prompts::{self, Confirm};
use super::reducer;
use super::rows::RowPatch;
use super::sessions;
use super::store;
use super::view;
use super::work::to_items;
use crate::State;
use crate::conv::background;
use crate::conv::compaction::fold_compaction_summaries;
use crate::conv::fold::FoldOptions;
use crate::conv::fold_plan::FoldPlan;
use crate::conv::message_list::ListContext;
use crate::mock::{Project, Status};
use crate::view::conversation::{Message, Older};
use rune::Tx;

/// Where an event came from, for `State::subscribe` (`onGlobalEvent` is `Sessions`, `onDomainEvent`
/// is `Session`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Topic {
    Sessions,
    Session(String),
}

type Listener = Box<dyn FnMut(&Topic, &Event) + Send>;

/// Who wants events the reducer doesn't apply. Components 3-22 attach here, so none of them
/// touches `link.rs`.
#[derive(Default)]
pub struct Subscribers(Vec<(Option<Kind>, Listener)>);

impl State {
    /// Calls `listener` with every event that isn't the open stream's own to apply: on the `sessions`
    /// topic, and on the open session's. `only` limits it to one type. Runs on the UI thread, so it
    /// must not block. (A component that needs another session's topic asks the link for it; the
    /// Files canvas will, in component 11.)
    pub fn subscribe(&mut self, only: Option<Kind>, listener: impl FnMut(&Topic, &Event) + Send + 'static) {
        self.subscribers.0.push((only, Box::new(listener)));
    }

    /// Hands events to the subscribers. Events the reducer applies go too, so a listener can follow
    /// them without a second path; `Cursor` marks are internal and skipped.
    pub fn dispatch(&mut self, topic: &Topic, events: &[Event]) {
        for event in events.iter().filter(|e| !matches!(e, Event::Cursor(_))) {
            let kind = event.kind();
            self.on_catalog_event(event);
            self.on_feeds_event(event);
            self.on_usage_event(event);
            self.on_smart_link_event(topic, event);
            self.on_memory_event(topic, event);
            self.on_dream_event(topic, event);
            self.on_mods_event(topic, event);
            self.on_mods_sites_event(topic, event);
            self.on_mod_renders_event(topic, event);
            self.on_mod_panes_event(topic, event);
            self.on_mod_drafts_event(topic, event);
            self.on_update_event(topic, event);
            self.on_list_work_event(event);
            self.on_queue_event(event);
            self.on_ask_event(topic, event);
            self.on_notification_event(event);
            self.on_rules_event(event);
            self.on_server_canvas_event(event);
            self.on_terminal_event(topic, event);
            self.on_app_event(event);
            self.on_progress_event(topic, event);
            self.on_workflow_event(event);
            self.on_diffs_event(topic, event);
            self.on_file_live_event(topic, event);
            self.on_file_browser_event(topic, event);
            for (only, listener) in &mut self.subscribers.0 {
                if only.is_none() || *only == kind {
                    listener(topic, event);
                }
            }
        }
    }

    /// The id of the open session on the server; `None` for the invented ones.
    pub fn open_id(&self) -> Option<String> {
        match &self.route {
            super::store::Route::Session(id) if !id.is_empty() && !super::store::is_invented(id) => Some(id.clone()),
            _ => None,
        }
    }

    /// Opens a session in the list and, on a server, asks it for the conversation.
    pub fn open_session(&mut self, project: usize, session: usize, tx: &mut Tx) {
        self.set_open((project, session));
        if let Some(p) = self.projects.get_mut(project) {
            p.open = true;
        }
        self.after_open(tx);
    }

    /// Opens a session listed under another machine, in place: it is kept in `elsewhere` (newest 20) with its machine, which
    /// is where every call about it goes from now on (`handleSessionSelect`, `rememberSessions`). A session the machine
    /// doesn't list (a machine that's gone) opens nothing.
    pub fn open_machine_session(&mut self, machine: &str, id: &str, tx: &mut Tx) {
        let Some(row) = self.machines.others.get(machine).and_then(|m| m.sessions.iter().find(|s| s.id == id)).map(|s| s.row_copy()) else {
            return;
        };
        self.open_machine_row(machine, row, tx);
    }

    /// `open_machine_session` for a row the caller holds: the home page's own list of another machine (`handleSessionSelect`).
    pub fn open_machine_row(&mut self, machine: &str, row: crate::mock::Session, tx: &mut Tx) {
        let id = row.id.clone();
        self.machines.remember_sessions(machine, std::slice::from_ref(&id));
        self.store.upsert_elsewhere(machine, row);
        self.go(store::Route::Session(id));
        self.after_open(tx);
    }

    /// What opening any session does once the route names it.
    pub(crate) fn after_open(&mut self, tx: &mut Tx) {
        // The right panel comes back the way this session left it, else the way it was last left anywhere.
        if let Some(c) = self.open_id().and_then(|id| self.sidebar.remembered_right(&id)) {
            self.content_open = !c;
        }
        // The text area shows the new session's draft at once, not a frame later.
        self.keep_drafts(self.clock, tx);
        if let Some(id) = self.open_id() {
            // The machine of the open session is the one you're on (`currentOpened`).
            let key = self.machine_key_of(&id).to_string();
            self.machines.set_current(&key);
        }
        if let Some(id) = self.open_id()
            && self.link.is_some()
            && self.stream_of.as_deref() != Some(id.as_str())
        {
            self.keep_current_stream();
            // What was kept shows at once, with the status the list has now; the snapshot that follows replaces it.
            let shown = self.kept.get(&id).cloned().map(|mut kept| {
                let activity = if self.session().status == Status::Working { "busy" } else { "idle" };
                reducer::with_activity_status(&mut kept, activity);
                kept
            });
            self.stream = shown;
            self.stream_of = Some(id.clone());
            self.loading = true;
            self.open_on_link(&id);
            self.sync_open();
            self.load_catalog(&id);
            self.load_commands(&id);
            self.load_queue(&id);
            self.load_asks(&id);
            self.load_rules(&id);
            self.load_server_canvases(&id);
            self.load_terminals(&id);
            self.ensure_progress_loaded(&id, false);
            self.diffs_open(&id);
            self.load_retry(&id);
            self.load_interruption(&id);
            self.load_browser(&id);
            self.load_header_detail(&id);
        }
        self.reset_scroll();
        self.follow_end(tx);
    }

    /// Keeps the conversation being left, if it came from a snapshot (`keepCurrentStream`).
    pub(crate) fn keep_current_stream(&mut self) {
        let (id, stream) = (self.stream_of.take(), self.stream.take());
        if let (Some(id), Some(mut stream), false) = (id, stream, self.loading) {
            // A page in flight is dropped when you leave; coming back starts from a plain button.
            stream.is_loading_older = false;
            stream.load_older_failed = false;
            self.kept.keep(&id, stream);
        }
    }

    /// The connection came back: every conversation shown is out of date until its new snapshot.
    pub fn on_reconnected(&mut self) {
        self.on_reconnected_for(super::settings::HOME_MACHINE_KEY);
    }

    /// A hub came back: home's reloads the pickers too; another machine's reloads only what its open session shows.
    pub fn on_reconnected_for(&mut self, key: &str) {
        if key == super::settings::HOME_MACHINE_KEY {
            self.load_pickers();
            self.load_smart_links();
            self.load_list_work();
        }
        let open = self.open_id().filter(|id| self.target_of(id).is_some_and(|t| t.key == key));
        let open_here = open.is_some();
        if let Some(id) = open {
            self.load_commands(&id);
            self.load_queue(&id);
            self.load_asks(&id);
            self.load_rules(&id);
            self.load_server_canvases(&id);
            self.load_terminals(&id);
            self.reload_apps(&id);
            self.ensure_progress_loaded(&id, true);
            self.refetch_diffs();
            self.load_retry(&id);
            self.load_interruption(&id);
            self.load_browser(&id);
            self.load_header_detail(&id);
            // Only a session whose Agents tab has been shown holds a history.
            if self.agents_history.contains_key(&id) {
                self.load_agents_history(&id);
            }
        }
        if self.stream.is_some() && (open_here || (key == super::settings::HOME_MACHINE_KEY && self.open_id().is_none())) {
            self.loading = true;
        }
    }

    /// Hides or shows the session list (Ctrl/Cmd+B) and remembers it.
    pub fn set_sidebar_open(&mut self, open: bool) {
        self.sidebar_open = open;
        self.sidebar.set_panel_collapsed(!open);
        self.save_settings();
    }

    /// Opens or closes the right panel and remembers it for the open session (`setRightPanelCollapsed`).
    pub fn set_content_open(&mut self, open: bool) {
        self.content_open = open;
        self.diffs_wanted_changed();
        let id = self.open_id();
        self.sidebar.set_right_collapsed(id.as_deref(), !open);
        self.save_settings();
    }

    /// A project header was pressed: fold or unfold its group and remember it (`handleToggleProject`).
    pub fn toggle_project(&mut self, p: usize) {
        let Some(project) = self.projects.get_mut(p) else { return };
        project.open = !project.open;
        let key = fold_key(project);
        self.sidebar.set_group_collapsed(&key, !project.open);
        self.save_settings();
    }

    /// The settings file was read (off the UI thread): the list and the panels take what it says.
    pub fn adopt_settings(&mut self, dir: std::path::PathBuf, stored: super::settings::Sidebar) {
        self.notices.adopt_shown(stored.notices_shown());
        self.sidebar.adopt(dir, stored);
        self.apply_appearance();
        self.seed_feeds_from_settings();
        self.sidebar_open = !self.sidebar.panel_collapsed();
        self.right.widened = self.sidebar.canvas_widened();
        self.terminals.restore(self.sidebar.terminal_open().to_vec(), self.sidebar.terminal_height());
        for project in &mut self.projects {
            if self.sidebar.is_group_collapsed(&fold_key(project)) {
                project.open = false;
            }
        }
        if let Some(c) = self.open_id().and_then(|id| self.sidebar.remembered_right(&id)) {
            self.content_open = !c;
        }
    }

    /// Opens the next waiting notice's card at a quiet moment, expires notices, and keeps the shown ids in the settings
    /// file. Until it is quiet, a waiting notice looks again every half second (`WAIT_POLL_MS`). True when something on
    /// screen changed.
    pub(crate) fn keep_notices(&mut self, now: f64, tx: &mut crate::Tx) -> bool {
        use super::notices::WAIT_POLL_SECS;
        use std::time::Duration;
        self.keep_memory_undo();
        self.keep_dream_notices();
        self.keep_dream_buttons();
        self.keep_skill_undo();
        self.keep_restart_buttons();
        self.keep_restart_notice();
        self.keep_update_buttons();
        self.keep_update_notices();
        let mut changed = self.notices.expire(now);
        if self.notices.open().is_none() && self.notices.next_waiting().is_some() {
            if self.quiet.is_quiet(now) {
                self.notices.open_next();
                changed = true;
            } else {
                tx.wake_after(Duration::from_secs_f64(WAIT_POLL_SECS));
            }
        }
        if let Some(at) = self.notices.next_expiry() {
            tx.wake_after(Duration::from_secs_f64((at - now).max(0.0)));
        }
        // The open card settles by itself after its hold, unless it is hovered, has focus inside it, or is pinned.
        let focus_inside = tx.focused().is_some_and(|k| k.starts_with("notice-"));
        changed |= self.notices.keep_hold(now, focus_inside);
        if let Some(left) = self.notices.hold_left(now) {
            tx.wake_after(Duration::from_secs_f64(left.max(0.0)));
        }
        // The countdown bar drains smoothly while it runs.
        if self.notices.countdown(now).is_some() && !self.notices.held() {
            tx.wake_after(Duration::from_millis(33));
            changed = true;
        }
        // The card rises in and folds into its chip over a few frames of the clock.
        if self.notices.keep_motion(now) {
            tx.wake_after(Duration::from_millis(16));
            changed = true;
        }
        // A chip fades in and pulses its ring as it arrives, so the clock keeps drawing until it is still.
        if self.notices.keep_chips(now) {
            tx.wake_after(Duration::from_millis(33));
            changed = true;
        }
        if let Some(ids) = self.notices.take_shown_save() {
            self.sidebar.save_notices_shown(&ids);
            self.save_settings();
        }
        changed
    }

    /// Writes the settings file on a thread, if it has a folder and something changed.
    pub(crate) fn save_settings(&mut self) {
        if let Some((dir, json)) = self.sidebar.take_save() {
            super::settings::spawn_save(dir, json);
        }
    }

    /// A session was created from the New session page: remember the folder, workspace and choice on `machine` (a machine key).
    pub fn remember_new_session(
        &mut self,
        machine: &str,
        folder: &super::new_session::request::Folder,
        workspace: &super::new_session::request::Workspace,
        choice: Option<(&str, &crate::composer::choice::AgentModelChoice)>,
    ) {
        self.sidebar.update_new_session_defaults(machine, |d| d.remember(folder, workspace, choice));
        self.save_settings();
    }

    /// Where a new folder or clone went, so the next one goes there too.
    pub fn remember_new_folder_root(&mut self, machine: &str, root: &str) {
        self.sidebar.update_new_session_defaults(machine, |d| d.remember_new_folder_root(root));
        self.save_settings();
    }

    /// Whether the list holds any row.
    pub fn has_rows(&self) -> bool {
        self.projects.iter().any(|p| !p.sessions.is_empty())
    }

    /// What the panel says went wrong: the sessions' failure, else the projects' (`errorMessage`).
    pub fn list_error(&self) -> Option<String> {
        super::panel::error_message(self.store.error.as_deref(), self.project_load.error.as_deref())
    }

    /// The first load is still going: nothing held and no answer yet. Before the hub has connected nothing has been asked,
    /// and the window is still waiting for its first list, so that counts too.
    pub fn list_is_loading(&self) -> bool {
        let waiting = self.link.is_some() && !self.store.list_loaded && self.store.error.is_none() && !self.is_refused();
        self.store.is_loading || self.project_load.is_loading || waiting || self.searching
    }

    /// "Try again": reads the sessions and the projects again, together.
    pub fn retry_list(&mut self) {
        if let Some(link) = &self.link {
            link.refetch_list();
        }
    }

    /// A list request failed. Only the newest request's failure counts; the rows stay (`useSessions`).
    pub fn on_list_failed(&mut self, request: u64, why: String) {
        // The panel shows the failure (a block or a banner); Fleet has no status-bar line for it.
        self.store.finish(request, Err(why));
    }

    /// The sessions arrived with the projects read beside them: groups them under the projects (the last good answer when
    /// the read failed) and shows the result, unless a newer request has overtaken this one or nothing changed.
    pub fn on_sessions(
        &mut self,
        request: u64,
        listed: Vec<crate::mock::Session>,
        projects: Result<Vec<super::groups::ProjectInfo>, String>,
        tx: &mut Tx,
    ) {
        if !self.store.finish(request, Ok(())) {
            return;
        }
        let before = self.project_load.projects.clone();
        if let Some(request) = self.project_load.begin() {
            self.project_load.finish(request, projects);
        }
        let same = !store::changed(&self.projects, &self.store.shown, &listed) && before == self.project_load.projects;
        if same {
            // Nothing the list draws changed, so nothing is rebuilt; the ages still move on.
            for s in self.projects.iter_mut().flat_map(|p| p.sessions.iter_mut()) {
                if let Some(n) = listed.iter().find(|n| n.id == s.id) {
                    (s.row.updated, s.row.created) = (n.row.updated, n.row.created);
                }
            }
            return;
        }
        self.store.shown = listed.iter().map(|s| s.id.clone()).collect();
        let grouped = sessions::group_sessions(listed, &self.project_load.projects);
        self.on_list(grouped, tx);
    }

    /// The session list arrived, or changed. Keeps which projects are folded and which session is open.
    pub fn on_list(&mut self, mut projects: Vec<Project>, tx: &mut Tx) {
        // Which groups are folded is remembered by project id (`isProjectExpanded`), not by what the last list had.
        for project in &mut projects {
            project.open = !self.sidebar.is_group_collapsed(&fold_key(project));
        }
        // The page isn't a session in the list: a refresh leaves it where it is.
        if self.on_new_session_page() {
            self.projects = projects;
            return;
        }
        // Invented sessions being replaced by a first list (a scene, a headless run) still open the first one, as they did; a
        // normal run has none, and lands on the dashboard.
        let was_invented = matches!(&self.route, store::Route::Session(id) if store::is_invented(id));
        // The open session's conversation isn't in the list; carry it over.
        let old_messages = self.session_mut().map(|s| std::mem::take(&mut s.messages)).unwrap_or_default();
        self.projects = projects;
        // The route names the session, so a reorder, a filter or a pin leaves it open wherever it went.
        let opened_by_id = self.opening.holds(&self.route);
        match self.session_mut() {
            Some(session) => session.messages = old_messages,
            // A session opened by its id that the list doesn't hold (yet, or at all) stays open: its own read decides.
            None if opened_by_id => {}
            None => {
                self.route = store::Route::None;
                self.stream = None;
                self.stream_of = None;
                if !self.has_rows() {
                    // Nothing to open: the page that starts the first one, as Fleet's "New session" would (docs/parity/07, Decisions).
                    self.open_new_session_page(None);
                } else if !self.projects.is_empty() && was_invented {
                    self.open_session(0, 0, tx);
                } else if !self.projects.is_empty() {
                    // A start with sessions lands on the dashboard with nothing selected, not on the first session (`/`).
                    self.go(store::Route::Home);
                }
            }
        }
        self.sync_open();
    }

    /// A `sessions` event that changes rows (`use-session-activity-updates.ts` and its siblings).
    pub fn on_row_patch(&mut self, patch: RowPatch) {
        let open = self.open_id();
        patch.apply(&mut self.projects);
        if let RowPatch::Activity { session, .. } = &patch
            && open.as_deref() == Some(session.as_str())
            && self.session().status == Status::Working
            && self.working_since < 0.0
        {
            self.working_since = self.clock;
        }
    }

    /// The open session's snapshot arrived.
    pub fn on_snapshot(&mut self, id: &str, state: StreamState, tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        if state.session_status.working() {
            self.working_since = self.clock;
        }
        self.prompts.on_messages(id, &state.messages);
        // A skill call in the conversation needs the list of built-in skills to know whether it offers Improve.
        let calls_skill = state.messages.iter().flat_map(|m| &m.parts).any(
            |p| matches!(p, super::model::PartData::Tool { tool, .. } if crate::conv::tools::get_tool(tool).category == crate::conv::tools::Category::Skill),
        );
        if calls_skill {
            self.skills_ensure_loaded();
        }
        self.stream = Some(state);
        self.stream_of = Some(id.to_string());
        // A snapshot after a reconnect leaves a reader where they are; only the first one of a session starts at the end.
        let first = std::mem::replace(&mut self.loading, false);
        self.sync_open();
        if first {
            // Asking for older history is decided from where the box is once this conversation is laid out in it, never
            // from a reading of the box before (empty, or the session left): which of those `poll` saw last depended on
            // whether a frame was drawn while the snapshot was on its way. The first reading of the new layout counts,
            // even if it is the same as the last.
            self.scroll_layout += 1;
            self.scroll_seen = None;
            self.follow_end(tx);
        }
    }

    /// "Load older messages" (`loadOlder`). Ignored while the snapshot is loading (a kept copy's cursor may be
    /// stale), while a page is already on its way, or when there is nothing older to ask for.
    pub fn load_older(&mut self) {
        let Some(id) = self.open_id() else { return };
        let Some(link) = self.link_for(&id) else { return };
        let Some(stream) = self.stream.as_mut() else { return };
        let Some(cursor) = stream.cursor.clone() else { return };
        if self.loading || !stream.has_more || stream.is_loading_older {
            return;
        }
        stream.is_loading_older = true;
        stream.load_older_failed = false;
        link.load_older(&id, &cursor);
    }

    /// The answer to `load_older`, or why there is none. A page whose cursor a newer snapshot replaced no longer
    /// fits and is dropped (the button comes back as it was); a failure keeps the cursor, so the next try asks
    /// for the same page; an answer for a session no longer open is dropped.
    pub fn on_history_page(&mut self, id: &str, cursor: &str, page: Result<Page, String>) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        let Some(stream) = self.stream.as_mut() else { return };
        match page {
            Ok(page) if stream.cursor.as_deref() == Some(cursor) => {
                stream.messages = history::prepend_page(std::mem::take(&mut stream.messages), &page.messages);
                stream.cursor = page.cursor;
                stream.has_more = page.has_more;
                stream.is_loading_older = false;
                stream.load_older_failed = false;
                self.sync_open();
            }
            other => {
                stream.load_older_failed = other.is_err();
                stream.is_loading_older = false;
            }
        }
    }

    /// The server took a prompt: the receipt confirms it, as the committed event does (whichever is first).
    pub fn on_prompt_receipt(&mut self, session: &str, correlation_id: &str, receipt: &serde_json::Value, _tx: &mut Tx) {
        let confirm = Confirm {
            correlation_id: Some(receipt.get("correlationId").and_then(|c| c.as_str()).unwrap_or(correlation_id).to_string()),
            event_id: receipt.get("eventId").and_then(|e| e.as_i64()),
            server_message_id: None,
        };
        self.prompts.confirm(session, &confirm);
        self.attach.delivered(&confirm.correlation_id.clone().unwrap_or_default());
        self.held.delivered(&confirm.correlation_id.clone().unwrap_or_default());
        self.composer_error = None;
        self.notice = None;
        self.sync_open();
    }

    /// The server refused a prompt, or it never got there: it leaves the conversation and its text goes
    /// back to the composer, unless something new was typed meanwhile.
    pub fn on_prompt_refused(&mut self, session: &str, correlation_id: &str, why: String, tx: &mut Tx) {
        self.put_back_images(correlation_id, tx);
        let lines = self.held.refused(correlation_id);
        if let Some(prompt) = self.prompts.remove(session, correlation_id)
            && self.open_id().as_deref() == Some(session)
        {
            // What came back is what was typed: the terminal lines are chips again, not blocks in the text. It goes in front of
            // whatever was typed meanwhile.
            let refused = lines.map_or(prompt.body, |(_, typed)| typed);
            self.draft = rune::TextEdit::new(super::prompts::draft_after_refusal(&refused, &self.draft.text));
        }
        // The error box above the composer (row 1.10), not the status bar.
        self.composer_error = Some(why);
        self.optimistic = None;
        self.sync_open();
    }

    /// The composer's Send against a server: the prompt joins the conversation as pending, and goes.
    pub fn send_to_server(&mut self, session: &str, text: &str) {
        self.send_to_server_as(session, text, false);
    }

    /// The same, into the running turn when `steer` (`delivery: "steer"`).
    pub fn send_to_server_as(&mut self, session: &str, text: &str, steer: bool) {
        self.send_prompt_with(session, text, text, steer, false);
    }

    /// The same; `with_images` takes the draft's pictures along. A prompt of pictures alone has no text to show as pending.
    pub fn send_prompt_with(&mut self, session: &str, text: &str, typed: &str, steer: bool, with_images: bool) {
        if self.link.is_none() {
            return;
        }
        let shown = self.prompts.add(session, text, super::link::now_ms(), self.clock);
        let Some(correlation_id) = shown.or_else(|| with_images.then(super::prompts::new_correlation_id)) else { return };
        let id = self.prompts.of(session).iter().find(|p| p.correlation_id == correlation_id).map(|p| p.id.clone());
        let mut pick = self.prompt_pick(id);
        pick.steer = steer;
        if with_images {
            pick.attachments = self.attach.take_for_send(session, &correlation_id);
            self.forget_previews(&pick.attachments);
        }
        // The terminal lines are in the text already; they wait to come back if the server refuses it.
        if text != typed {
            self.held.take_terminals_for_send(session, &correlation_id, typed);
        }
        self.add_references(&mut pick, text);
        self.note_pick_on_row(&pick);
        self.record_recent_model(&pick);
        if let Some(link) = self.link_for(session) {
            link.send_prompt(session, &correlation_id, &super::prompts::tidy_prompt(text), pick);
        }
        self.sync_open();
    }

    /// A frame's worth of the open session's events.
    pub fn on_events(&mut self, id: &str, events: Vec<Event>, tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        self.dispatch(&Topic::Session(id.to_string()), &events);
        self.focus_new_ask(tx);
        self.on_stream_events(id, events, tx);
    }

    /// Events the open stream applies without telling the subscribers: the ones that came on the
    /// `sessions` topic (already dispatched there), and the others after `dispatch`.
    pub fn on_stream_events(&mut self, id: &str, events: Vec<Event>, _tx: &mut Tx) {
        if self.open_id().as_deref() != Some(id) || events.is_empty() {
            return;
        }
        for event in &events {
            if let Event::PromptCommitted { life, correlation_id, event_id } = event {
                let confirm = Confirm {
                    correlation_id: correlation_id.clone(),
                    event_id: *event_id,
                    server_message_id: life.info.get("id").and_then(|i| i.as_str()).map(str::to_string),
                };
                self.prompts.confirm(id, &confirm);
            }
        }
        let mut changed = false;
        for event in &events {
            changed |= self.apply_browser_event(id, event);
        }
        let Some(state) = self.stream.as_mut() else {
            if changed {
                self.sync_open();
            }
            return;
        };
        let was_working = state.session_status.working();
        for event in &events {
            reducer::apply(state, event);
        }
        if !was_working && state.session_status.working() {
            self.working_since = self.clock;
        }
        self.prompts.on_messages(id, &state.messages);
        // No scroll to the end here: the conversation follows the end by itself while the person is
        // there, and a person who scrolled up to read is left where they are.
        self.sync_open();
    }

    /// Redraws the open session from the reducer's state. Only the messages that changed are
    /// rebuilt: a streaming reply touches the last one, and copying all two thousand every frame
    /// is what a long conversation can't afford. The last message is always rebuilt, so an edit
    /// in place there is never missed; an edit in place to an earlier one that keeps its size is
    /// (the reducer doesn't make those).
    pub(crate) fn sync_open(&mut self) {
        let Some(stream) = &self.stream else {
            self.fold_plan = Default::default();
            return;
        };
        // Which background work ended and how: a notice arriving later changes the cards of calls long since drawn.
        let finished = background::finished_of(&stream.messages, &stream.running_work, &self.work.older);
        let finished_changed = finished != self.work.finished;
        self.work.finished = finished;
        let finished = &self.work.finished;
        let folded = fold_compaction_summaries(&stream.messages);
        let shown: Vec<&Msg> = stream.messages.iter().filter(|m| !folded.hidden.contains(&m.id)).collect();
        let summary_of = |m: &Msg| folded.summaries.get(&m.id).map(String::as_str);
        let at = self.open_at();
        let sent = self.session_at(at).map(|s| s.id.clone()).unwrap_or_default();
        // The prompts the conversation doesn't hold yet, after the delivered messages (`mergeMessagesByTimestamp`).
        let optimistic: Vec<Message> =
            self.prompts.of(&sent).iter().filter(|prompt| !shown.iter().any(|m| m.id == prompt.id)).map(prompts::optimistic_view).collect();
        // Not `session_mut()`: that would borrow all of `self` while `stream` is still read.
        let Some(session) = (if at.0 == store::ELSEWHERE {
            self.store.elsewhere_at_mut(at.1)
        } else {
            self.projects.get_mut(at.0).and_then(|p| p.sessions.get_mut(at.1))
        }) else {
            return;
        };
        let n = shown.len();
        let same_shape = !finished_changed
            && session.messages.len() == n + optimistic.len()
            && session.messages.iter().zip(&shown).all(|(a, b)| a.id == b.id)
            && session.messages[n..].iter().zip(&optimistic).all(|(a, b)| a.id == b.id);
        if same_shape {
            let last = n.saturating_sub(1);
            for (i, (old, new)) in session.messages.iter_mut().zip(&shown).enumerate() {
                if i == last || old.version() != view::version(new, summary_of(new)) {
                    *old = view::message(new, summary_of(new), finished);
                }
            }
            // A few rows at most, and one may have just turned to needing a retry.
            session.messages.truncate(n);
            session.messages.extend(optimistic);
        } else {
            session.messages =
                prompts::merge_by_timestamp(shown.into_iter().map(|m| view::message(m, summary_of(m), finished)).collect(), optimistic);
        }
        // The row shows the derived status: a sub-agent's question is the amber diamond, every other
        // kind of work the working glyph.
        let status = sessions::stream_status_of(stream.session_status);
        // The session's own question reaches the list as `waiting_input` while its stream still says busy
        // (the turn hasn't ended): the list is right, as it is in Fleet's rows.
        let asked = super::rows::glyph_of(&session.row.lifecycle, &session.row.activity) == Status::NeedsInput;
        if status == Status::Working && asked {
            session.status = Status::NeedsInput;
        } else if status != Status::Idle {
            session.status = status;
        } else if session.status == Status::Working {
            // Not NeedsInput: that may be the session's own question, from the list.
            session.status = Status::Idle;
            session.touch(super::link::now_ms());
        }
        self.refold();
        self.sync_questions();
        self.load_older_work();
        self.sync_mod_rows();
    }

    /// Asks the server for all of the session's work, once, when a notice says work failed and the stream holds no item
    /// for its call: work Fleet stopped reads stopped, not failed (`use-background-work.ts`).
    fn load_older_work(&mut self) {
        let (Some(stream), Some(id)) = (&self.stream, self.open_id()) else { return };
        let Some(link) = self.link_for(&id) else { return };
        if self.work.older_for.as_deref() == Some(id.as_str())
            || !background::needs_older_work(&stream.messages, &stream.running_work, &self.work.older)
        {
            return;
        }
        self.work.older_for = Some(id.clone());
        let asked = id.clone();
        link.off_thread(move |base| http::work_all(base, &asked), move |s, _, result| s.on_older_work(&id, result));
    }

    /// The answer of `load_older_work`; one for a session no longer open is dropped.
    pub fn on_older_work(&mut self, id: &str, result: Result<(u16, serde_json::Value), String>) {
        if self.open_id().as_deref() != Some(id) {
            return;
        }
        if let Ok((status, body)) = result
            && (200..300).contains(&status)
        {
            self.work.older = to_items(Some(&body));
            self.sync_open();
        }
    }

    /// Works out which messages the stream draws and which fold (`foldTurns`), from the reducer's messages. Runs when the
    /// messages change and when a turn is opened or closed; without a conversation from a server everything draws.
    pub fn refold(&mut self) {
        let Some(stream) = &self.stream else {
            self.fold_plan = Default::default();
            return;
        };
        if self.watched_of != self.stream_of {
            self.watched.on_session_change();
            self.watched_of = self.stream_of.clone();
        }
        let running = stream.session_status.working();
        self.watched.on_working(running);
        let sent = self.prompts.of(&self.open_id().unwrap_or_default());
        let context = ListContext {
            delegations: &stream.delegations,
            finished: &self.work.finished,
            running: &stream.running_work,
            older: &self.work.older,
            models: &[],
            thinking: self.thinking,
        };
        let directory = self.session().row.directory.clone();
        let directory = (!directory.is_empty()).then_some(directory);
        let options = FoldOptions { running, keep_last: self.watched.watched(), directory: directory.as_deref(), open: &self.turns_open };
        let stats = self.conversation_file_stats();
        let snapshots = self.turn_stats.snapshots_of(&self.stream_of.clone().unwrap_or_default());
        let mut plan = FoldPlan::build_with_stats(&stream.messages, sent, &context, &options, stats.as_ref(), Some(snapshots));
        plan.place(self.session().messages.iter().map(|m| m.id.as_str()));
        let whole_file = plan.whole_file_write;
        self.fold_plan = plan;
        self.keep_turn_stats(whole_file, running);
    }

    /// The fold row's click.
    pub fn toggle_turn(&mut self, turn_id: &str) {
        crate::conv::fold::toggle_turn(&mut self.turns_open, turn_id);
        self.refold();
    }

    /// A scene: the open session's row as it reads once the reducer has derived its status from a
    /// snapshot with one sub-agent, working (`waiting` false) or stopped on a question.
    pub fn show_sub_agent(&mut self, waiting: bool) {
        let child = if waiting { "waiting_input" } else { "busy" };
        let snapshot = serde_json::json!({
            "messages": [],
            "activityStatus": "idle",
            "delegations": [{"delegationId": "d1", "childSessionId": "c1", "title": "Check the tests", "status": "running", "childActivityStatus": child}],
        });
        let status = sessions::stream_status_of(reducer::create_state(&snapshot).session_status);
        if let Some(session) = self.session_mut() {
            session.status = status;
        }
        if status == Status::Working {
            self.working_since = self.clock;
        }
    }

    /// A scene: the harness was unavailable, so the snapshot is the cached one (`isPartial`).
    pub fn show_partial(&mut self) {
        // No conversation yet, so the banner at the top of the scroll is in view.
        self.set_open((usize::MAX, 0));
        self.stream = Some(reducer::create_state(&serde_json::json!({"messages": [], "isPartial": true})));
    }

    pub(crate) fn session_mut(&mut self) -> Option<&mut crate::mock::Session> {
        let (p, i) = self.open_at();
        if p == store::ELSEWHERE {
            return self.store.elsewhere_at_mut(i);
        }
        self.projects.get_mut(p)?.sessions.get_mut(i)
    }

    fn session_at(&self, at: (usize, usize)) -> Option<&crate::mock::Session> {
        if at.0 == store::ELSEWHERE {
            return self.store.elsewhere_at(at.1);
        }
        self.projects.get(at.0)?.sessions.get(at.1)
    }

    /// The open conversation is the harness's cached data, not live (`isPartial`).
    pub fn is_partial(&self) -> bool {
        self.stream.as_ref().is_some_and(|s| s.is_partial)
    }

    /// Fleet's recap of the open session, when it wrote one (`RecapLine.vue`).
    pub fn recap(&self) -> Option<&str> {
        self.stream.as_ref()?.recap.as_deref()
    }

    /// What the top of the conversation offers: nothing, or the button, its busy line, or its retry wording.
    pub fn older(&self) -> Option<Older> {
        let stream = self.stream.as_ref().filter(|s| s.has_more)?;
        Some(match (stream.is_loading_older, stream.load_older_failed) {
            (true, _) => Older::Loading,
            (false, true) => Older::Failed,
            (false, false) => Older::Ask,
        })
    }

    /// A scene: a long conversation whose snapshot says there is more before it (`hasMore`), in the state asked for.
    pub fn show_older(&mut self, older: Older) {
        // No conversation under it, so the line at the top of the scroll is in view.
        self.set_open((usize::MAX, 0));
        let stream = self.stream.get_or_insert_with(|| reducer::create_state(&serde_json::json!({"messages": []})));
        {
            stream.has_more = true;
            stream.cursor = Some("scene-cursor".into());
            stream.is_loading_older = older == Older::Loading;
            stream.load_older_failed = older == Older::Failed;
        }
    }

    /// The Fleet turned the connection away for want of a token.
    pub fn is_refused(&self) -> bool {
        self.connection == Connection::Refused
    }

    /// What the status bar says about the connection; `None` when all is well or there's no server.
    pub fn connection_note(&self) -> Option<String> {
        match (&self.link, &self.connection) {
            (None, _) | (_, Connection::Online) => self.notice.clone(),
            // Name the address, in Fleet's own wording for a server it can't reach ("Can't reach …. Trying again…",
            // `UnreachableBanner.vue`). The reason (`why`) is for the log: "Connection refused (os error 61)" helps no one.
            (Some(link), Connection::Connecting) => Some(format!("Connecting to {}…", link.base)),
            (Some(link), Connection::Refused) => Some(super::auth::refused_note(&link.base)),
            (Some(link), Connection::Down(_)) => Some(format!("Can't reach Fleet at {}. Trying again…", link.base)),
        }
    }
}

/// The key a project's group is folded under: this machine's, by project id (the Pinned group's is `pinned`).
fn fold_key(project: &Project) -> String {
    super::settings::project_group_key(super::settings::HOME_MACHINE_KEY, &project.id)
}
