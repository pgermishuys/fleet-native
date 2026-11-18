//! The autocomplete popup's state (docs/parity/05-composer.md, piece K): the engine of `composer::triggers` fed with the
//! lists the finder holds, the model the popup draws, and the keys and clicks that reach it. Ports the wiring of
//! `Composer.vue` around `useAutocomplete`. The popup is drawn by `view::autocomplete`.
use super::finder::{Scope, Step, parse_files, parse_sessions};
use super::http;
use crate::State;
use crate::composer::refs::{Picks, SessionItem, skill_list_hint};
use crate::composer::status::{Mode, mode_of};
use crate::composer::triggers::{Autocomplete, Ctx, Edit, Item, Key, Lists};
use rune::{KeyEvent, Tx};
use std::collections::HashMap;

/// `/btw`, Fleet's own command, listed first for a harness that can fork a session.
const SIDE_QUESTION: &str = "Ask a side question without disturbing the session";

/// What the popup draws (the props of `AutocompletePopup.vue`).
#[derive(Debug, Clone, PartialEq)]
pub struct Popup {
    pub items: Vec<Item>,
    pub loading: bool,
    pub error: Option<String>,
    /// The highlighted row's value.
    pub selected: Option<String>,
    /// What is typed after the trigger, marked in session and skill labels.
    pub query: String,
    pub skill_hint: String,
    pub session_note: &'static str,
    pub empty_text: Option<String>,
    /// What the files' group label says beside it (`in {folder}` on the New session page).
    pub file_hint: Option<String>,
}

#[derive(Default)]
pub struct Auto {
    engine: Autocomplete,
    /// The files and sessions picked into each draft, by draft owner (piece L keeps them with the draft).
    picks: HashMap<String, Picks>,
    pub popup: Option<Popup>,
    /// The text, caret and owner the popup was last worked out for.
    seen: Option<(String, usize, Option<String>)>,
    /// Something it reads changed since: a key, an answer, a scene.
    dirty: bool,
    /// A picked row's caret is set again on the next tick (`nextTick(() => setSelectionRange(...))`).
    caret_again: Option<(String, usize)>,
}

impl Auto {
    pub fn changed(&mut self) {
        self.dirty = true;
    }

    /// The picks of one draft.
    pub fn picks_of(&self, owner: &str) -> Option<&Picks> {
        self.picks.get(owner)
    }

    pub fn picks_mut(&mut self, owner: &str) -> &mut Picks {
        self.picks.entry(owner.to_string()).or_default()
    }

    /// Gives a draft the picks kept with it, unless it has some already (a session typed in since the app started).
    pub fn seed_picks(&mut self, owner: &str, picks: Picks) {
        let held = self.picks.entry(owner.to_string()).or_default();
        if *held == Picks::default() {
            *held = picks;
        }
    }
}

/// An invented session's `updated`, from how long ago it was last used.
fn mock_updated(now: i64, ago_secs: Option<i64>) -> f64 {
    (now - ago_secs.unwrap_or(0) * 1000) as f64
}

impl State {
    /// Whether the text area's text is a shell command or the popup has nothing to say about it: a `!` draft is sent as
    /// typed, with no `@` references or `/` commands in it.
    fn autocomplete_text(&self) -> &str {
        let caps = self.caps();
        if mode_of(&self.draft.text, &caps, self.side_open) == Mode::Shell { "" } else { &self.draft.text }
    }

    /// The sessions the `@` list can offer: the list's own, with the fetched ones (archived, sub-agents) behind them. What the
    /// list holds wins, since its status is live; it keeps no `updated`, so a fetched copy gives that.
    pub(crate) fn mentionable_sessions(&self) -> Vec<SessionItem> {
        // One reading of the clock for the whole list: the rows are ranked by `updated`, and two invented sessions with the same
        // age would otherwise differ by the millisecond that passed between their reads, which put them in either order.
        let now = self.now_ms.unwrap_or_else(super::link::now_ms);
        let mut fetched: Vec<SessionItem> =
            if self.link.is_some() { self.finder.sessions.sessions.clone() } else { crate::mock::finder::other_sessions(now) };
        let mut out = Vec::new();
        for (p, project) in self.projects.iter().enumerate() {
            for (i, session) in project.sessions.iter().enumerate() {
                let id = if session.id.is_empty() { format!("mock.{p}.{i}") } else { session.id.clone() };
                let at = fetched.iter().position(|f| f.id == id);
                let known = at.map(|at| fetched.remove(at));
                let (status, activity) = match (session.row.lifecycle.as_str(), session.status) {
                    ("", crate::mock::Status::Working) => ("active".to_string(), Some("busy".to_string())),
                    ("", crate::mock::Status::NeedsInput) => ("waiting_input".to_string(), Some("waiting_input".to_string())),
                    ("", crate::mock::Status::Idle) => ("idle".to_string(), None),
                    (life, _) => (life.to_string(), Some(session.row.activity.clone()).filter(|a| !a.is_empty())),
                };
                out.push(SessionItem {
                    id,
                    title: session.title.clone(),
                    status,
                    activity,
                    harness_type: session.row.harness_type.clone().unwrap_or_else(|| "opencode".into()),
                    project_name: Some(project.name.clone()),
                    workspace_display_name: None,
                    workspace_directory: project.path.clone(),
                    parent_session_id: None,
                    archived: session.row.archived,
                    updated: if session.row.updated > 0 {
                        session.row.updated as f64
                    } else {
                        known.map_or_else(|| mock_updated(now, session.ago_secs), |k| k.updated)
                    },
                });
            }
        }
        out.extend(fetched);
        out
    }

    /// What the popup lists from, as the finder has it now. Files are the one list that follows the query, so they are
    /// filled in by the caller once it knows it.
    fn autocomplete_lists(&self) -> Lists {
        let owner = self.draft_owner();
        let mut lists = Lists::for_draft(owner.clone());
        let row = &self.session().row;
        lists.harness_type = row.harness_type.clone();
        lists.harness_names = self.pickers.harnesses.iter().filter_map(|h| h.display_name.clone().map(|n| (h.kind.clone(), n))).collect();
        if self.caps().supports_side {
            lists.fleet_commands = vec![crate::composer::refs::Command {
                name: "btw".into(),
                description: Some(SIDE_QUESTION.into()),
                kind: Some("command".into()),
                user_only: false,
            }];
        }
        let live = self.link.is_some();
        let commands = owner.as_ref().and_then(|o| self.finder.commands.get(o));
        match commands {
            Some(c) => {
                lists.set_commands(c.items.clone());
                lists.commands_loading = c.loading;
                lists.commands_error = c.error.clone();
            }
            None if live => lists.commands_loading = true,
            None => lists.set_commands(crate::mock::finder::commands()),
        }
        match owner.as_ref().and_then(|o| self.finder.agents.get(o)) {
            Some(a) => {
                lists.agents = a.items.clone();
                lists.agents_loading = a.loading;
                lists.agents_error = a.error.clone();
            }
            None if live => lists.agents_loading = true,
            None => lists.agents = crate::mock::finder::agents(),
        }
        lists.files_loading = self.finder.files.loading;
        lists.files_error = self.finder.files.error.clone();
        lists.sessions = self.mentionable_sessions();
        lists
    }

    /// Runs `f` with the engine and a context for the open session's draft at its caret.
    fn with_autocomplete<R>(&mut self, f: impl FnOnce(&mut Autocomplete, &mut Ctx) -> R) -> Option<R> {
        let owner = self.draft_owner()?;
        let mut lists = self.autocomplete_lists();
        let caret = self.draft.range().1;
        // The file list follows the query: the one this draft's caret is typing, if the `@` list is open.
        let query = {
            let mut scratch = Picks::default();
            let ctx = Ctx { text: self.autocomplete_text(), caret, lists: &lists, picks: &mut scratch };
            self.auto.engine.file_query(&ctx).map(str::to_string)
        };
        lists.files = match &query {
            Some(q) if self.link.is_some() => self.finder.files.files(q),
            Some(q) => crate::mock::finder::find_files(q),
            None => Vec::new(),
        };
        let text = if self.autocomplete_text().is_empty() { "" } else { self.draft.text.as_str() };
        let picks = self.auto.picks.entry(owner).or_default();
        let mut ctx = Ctx { text, caret, lists: &lists, picks };
        Some(f(&mut self.auto.engine, &mut ctx))
    }

    /// The session the file search looks in: the open session's, once the server has one (an invented session has no
    /// directory to search, so its list is the mock one).
    fn file_scope(&self) -> Option<Scope> {
        self.link.as_ref()?;
        self.open_id().map(Scope::Session)
    }

    /// Applies what a pick or a key does to the text: the whole new text goes in as one edit (so undo takes it back), the
    /// caret is set at once and again on the next tick.
    fn apply_autocomplete(&mut self, edit: Edit, tx: &mut Tx) {
        let end = self.draft.text.len();
        self.draft.set_selection(0, end);
        self.draft.insert(&edit.text);
        self.draft.set_caret(edit.caret);
        self.auto.caret_again = Some((self.draft.text.clone(), edit.caret));
        self.auto.dirty = true;
        tx.focus("composer", false);
        tx.wake_after(std::time::Duration::from_millis(16));
    }

    /// A click on a row (`onSelect`), or on a folder's chevron (`onOpenFolder`): the text area keeps its focus.
    pub fn autocomplete_pick(&mut self, value: &str, open_folder: bool, tx: &mut Tx) {
        if self.on_new_session_page() {
            return self.new_autocomplete_pick(value, open_folder, tx);
        }
        let edit = self.with_autocomplete(|engine, ctx| {
            let disabled = engine.items(ctx).iter().any(|i| i.value == value && i.disabled_reason.is_some());
            if disabled {
                None
            } else if open_folder {
                engine.on_open_folder(ctx, value)
            } else {
                engine.on_select(ctx, value)
            }
        });
        if let Some(Some(edit)) = edit {
            self.apply_autocomplete(edit, tx);
        } else {
            tx.focus("composer", false);
        }
    }

    /// A key in the text area, before the editor sees it (`onKeyDown`). True when the popup took it: the arrows, and Enter,
    /// Tab and Esc while it is open (Enter doesn't send, Esc doesn't interrupt).
    pub fn autocomplete_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        // Fleet's popup doesn't look at `isComposing`; native doesn't pick while an input method is composing (row 5.14).
        if self.draft.is_composing() || self.composer_disabled() {
            return false;
        }
        let key = match ev.key {
            rune::Key::Down => Key::Down,
            rune::Key::Up => Key::Up,
            rune::Key::Enter => Key::Enter,
            rune::Key::Tab if !ev.mods.shift => Key::Tab,
            rune::Key::Escape => Key::Escape,
            _ => Key::Other,
        };
        let Some(out) = self.with_autocomplete(|engine, ctx| engine.on_key(ctx, key)) else { return false };
        self.auto.dirty = true;
        if let Some(edit) = out.edit {
            self.apply_autocomplete(edit, tx);
        }
        // `scrollIntoView({block: "nearest"})` on the highlighted row (row 5.11).
        if matches!(key, Key::Down | Key::Up)
            && let Some(i) = self.with_autocomplete(|engine, ctx| engine.selected_index(ctx))
        {
            tx.scroll_into_view_in("autocomplete-scroll", format!("autocomplete-row-{i}"));
        }
        if out.consumed || out.swallowed {
            return true;
        }
        // Esc in the text area interrupts the session unless a popup took it first (row 1.14). The field would otherwise
        // keep the key for itself, so it is asked here.
        if key == Key::Escape && self.can_interrupt() {
            crate::interrupt(self);
            return true;
        }
        false
    }

    /// The text, the caret or a list changed (called every poll): works out the popup, asks the server for what the open
    /// list needs, and lets a held Enter pick. True when the popup changed and must redraw.
    pub fn keep_autocomplete(&mut self, now: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if let Some((text, caret)) = self.auto.caret_again.take()
            && self.draft.text == text
        {
            self.draft.set_caret(caret);
            changed = true;
        }
        let owner = self.draft_owner();
        let seen = (self.draft.text.clone(), self.draft.range().1, owner.clone());
        let waiting = self.auto.engine.has_pending() || self.auto.popup.as_ref().is_some_and(|p| p.loading);
        if !self.auto.dirty && self.auto.seen.as_ref() == Some(&seen) && !waiting {
            return changed;
        }
        self.auto.dirty = false;
        self.auto.seen = Some(seen);
        let scope = self.file_scope();
        let Some((popup, query, mention_open)) = self.with_autocomplete(|engine, ctx| {
            let query = engine.file_query(ctx).map(str::to_string);
            let mention_open = engine.is_mention_open(ctx);
            let popup = engine.is_open(ctx).then(|| {
                let skill_open = engine.is_skill_open(ctx);
                let q = engine.query(ctx).to_string();
                let name = ctx.lists.harness_type.as_deref().and_then(|t| ctx.lists.harness_names.get(t).cloned());
                Popup {
                    items: engine.items(ctx),
                    loading: engine.is_loading(ctx),
                    error: engine.error(ctx),
                    selected: engine.selected_value(ctx),
                    skill_hint: skill_list_hint(ctx.lists.harness_type.as_deref(), name.as_deref().unwrap_or("The agent")),
                    session_note: "",
                    empty_text: skill_open.then(|| format!("No skill named “{q}”. It stays as text.")),
                    query: q,
                    file_hint: None,
                }
            });
            (popup, query, mention_open)
        }) else {
            changed |= self.auto.popup.take().is_some();
            return changed;
        };
        let mut popup = popup;
        if let Some(p) = &mut popup {
            p.session_note = if self.fleet_tools() {
                "The agent gets a link to the session, not a copy, and reads what it needs."
            } else {
                "The agent gets the session's title and a recap of where it stands, not a copy."
            };
        }
        self.ask_for_lists(scope, query.as_deref(), mention_open, now, tx);
        // A held Enter or Tab picks the first match once the answer is in.
        let held = self.with_autocomplete(|engine, ctx| engine.poll(ctx)).flatten();
        if let Some(edit) = held {
            self.apply_autocomplete(edit, tx);
            return true;
        }
        if popup != self.auto.popup {
            self.auto.popup = popup;
            changed = true;
        }
        changed
    }

    /// Whether the open session's harness can read another session through Fleet's tools.
    fn fleet_tools(&self) -> bool {
        let kind = self.session().row.harness_type.as_deref();
        self.pickers.harnesses.iter().any(|h| Some(h.kind.as_str()) == kind && h.supports_fleet_tools)
    }

    /// The requests the open list needs: the file search (after its wait), and the sessions when the `@` list opens.
    fn ask_for_lists(&mut self, scope: Option<Scope>, query: Option<&str>, mention_open: bool, now: f64, tx: &mut Tx) {
        match self.finder.files.want(scope.as_ref(), query, now) {
            Step::Idle => {}
            Step::Wait(seconds) => tx.wake_after(std::time::Duration::from_secs_f64(seconds + 0.002)),
            Step::Send { generation, query } => {
                let link = match &scope {
                    Some(Scope::Session(id)) => self.link_for(id),
                    _ => self.link.clone(),
                };
                if let (Some(link), Some(scope)) = (link, scope) {
                    let asked = query.clone();
                    link.off_thread(
                        move |base| {
                            let answer = match &scope {
                                Scope::Session(id) => http::find_files(base, id, &asked),
                                Scope::Directory(path) => http::find_in_directory(base, path, &asked),
                            };
                            match answer {
                                Ok((status, body)) if (200..300).contains(&status) => Ok(parse_files(&body)),
                                Ok((status, body)) => Err(super::finder::failure(status, &body)),
                                Err(why) => Err(why),
                            }
                        },
                        move |s, _, result| {
                            s.finder.files.answered(generation, &query, result);
                            s.auto.dirty = true;
                        },
                    );
                }
            }
        }
        if self.link.is_some() && self.finder.sessions.should_fetch(mention_open, now) {
            let at = now;
            if let Some(link) = &self.link {
                link.off_thread(
                    |base| http::sessions_for_mentions(base).map(|body| parse_sessions(&body)),
                    move |s, _, result| {
                        s.finder.sessions.arrived(result, s.clock.max(at));
                        s.auto.dirty = true;
                    },
                );
            }
        } else if self.is_mock() {
            let _ = self.finder.sessions.should_fetch(mention_open, now);
        }
    }

    /// Asks for the session's commands and skills (when it opens, after a reconnect, and when the harness says its catalog
    /// changed). What was there stays up meanwhile.
    pub fn load_commands(&self, session: &str) {
        let Some(link) = self.link_for(session) else { return };
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::commands(base, &asked),
            move |s, _, result| {
                s.finder.on_commands(&id, result);
                s.auto.dirty = true;
            },
        );
    }

    /// The text of the age on a session's row in the popup.
    pub fn mention_age(&self, updated: f64) -> String {
        super::row_status::format_relative(
            updated as i64,
            self.now_ms.unwrap_or_else(super::link::now_ms),
            crate::conv::time::local_offset_secs(),
        )
    }
}
