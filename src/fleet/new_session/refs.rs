//! What the New session page's message box adds to component 5's `@` list and images (rows A8.1-A8.13, A9.1-A9.6, A7.9): the same
//! engine (`composer::triggers`), popup model and held pictures, driven by the box's own text instead of an open session's draft.
//! The page has no session, so there is no `/` list, no `$`, no agents; the files come from the Folder chip's folder, on the
//! machine the page is for, and a pick belongs to the draft's row.
use super::places::{ItemPickers, PickerItem, find_hash_trigger, remove_trigger, repository_for_preset};
use super::request::{Folder, Image, Workspace};
use super::send::MESSAGE_KEY;
use crate::State;
use crate::composer::attach::HeldImage;
use crate::composer::draftrefs::{Known, RefKey, RefSpan, picked_reference_ranges, reference_key_action, reference_spans};
use crate::composer::refs::Picks;
use crate::composer::triggers::{Autocomplete, Ctx, Edit, Key, Lists};
use crate::fleet::autocomplete::Popup;
use crate::fleet::finder::{Scope, Step, parse_files, parse_sessions};
use crate::fleet::http;
use rune::{KeyEvent, Tx};
use std::ops::Range;
use std::time::Duration;

/// What the box keeps for its `@` list, its picks and the GitHub place.
#[derive(Default)]
pub struct NewRefs {
    engine: Autocomplete,
    /// The list the page draws above the frame (`None` while it is shut).
    pub popup: Option<Popup>,
    seen: Option<(String, usize, String)>,
    /// A picked row's caret is set again on the next tick (`nextTick(() => setSelectionRange(...))`).
    caret_again: Option<(String, usize)>,
    /// Picks the new folder doesn't have, drawn as plain text until they are picked again (`unfoundTokens`).
    pub unfound: Vec<String>,
    /// The paths the folder change turned back into text, for the line under the box (`folderNotice`).
    pub notice: Option<Vec<String>>,
    /// The Folder chip's folder as of the last look, to see it change (`watch(filesDirectory)`).
    folder_seen: Option<Option<String>>,
    /// Numbers a folder lookup, so a stale one is dropped.
    lookup: u64,
    /// The GitHub preset is attached and its repository not yet looked for among the scanned ones.
    preset_folder_pending: bool,
    /// What hosts registered for the `#` place; empty until component 21.
    pub places: ItemPickers,
}

/// The name of a folder as the notice and the list's group hint say it (`folderName`).
fn folder_name(path: &str) -> String {
    path.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

impl State {
    /// The Folder chip's folder, where files are searched; none for no folder (`filesDirectory`).
    pub fn new_files_directory(&self) -> Option<String> {
        match self.new_draft.draft()?.folder.as_ref()? {
            Folder::Repository(path) | Folder::Directory(path) => Some(path.clone()),
            Folder::None => None,
        }
    }

    /// The group hint of the files (`in {folderName}`), none for a chat with no folder.
    pub fn new_file_hint(&self) -> Option<String> {
        self.new_files_directory().map(|d| format!("in {}", folder_name(&d)))
    }

    /// `supportsImages`: whether the harness the page will use passes pictures on (A9.1).
    pub fn new_page_supports_images(&self) -> bool {
        let kind = self.resolved_harness();
        self.harnesses_here().all.iter().any(|h| h.kind == kind && h.supports_images)
    }

    fn new_page_fleet_tools(&self) -> bool {
        let kind = self.resolved_harness();
        self.harnesses_here().all.iter().any(|h| h.kind == kind && h.supports_fleet_tools)
    }

    /// The draft's row key: whose picks and pictures these are.
    pub(crate) fn new_row_key(&self) -> Option<String> {
        self.new_draft.draft().map(|d| d.row_key.clone())
    }

    /// The pictures held for the first message, as `Image`s for the request.
    pub(crate) fn new_session_images(&self) -> Vec<Image> {
        self.new_row_key()
            .map(|k| {
                self.attach
                    .of(&k)
                    .iter()
                    .map(|i| Image { mime: i.mime.clone(), filename: Some(i.filename.clone()), data: i.data.clone() })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The sessions picked and still in `message`, as the request's `sessionReferences` (A8.6).
    pub(crate) fn new_session_references(&self, message: &str) -> Vec<(String, String)> {
        let Some(key) = self.new_row_key() else { return Vec::new() };
        let picks = self.auto.picks_of(&key);
        picks.map(|p| p.session_references_in(message).into_iter().map(|r| (r.token, r.session_id)).collect()).unwrap_or_default()
    }

    /// After a send was taken: the picks and the pictures go (A8.13, A9.6), and the folder line with them.
    pub(crate) fn forget_new_refs(&mut self) {
        let Some(key) = self.new_row_key() else { return };
        *self.auto.picks_mut(&key) = Picks::default();
        let held: Vec<HeldImage> = self.attach.clear(&key);
        self.forget_previews(&held);
        let refs = &mut self.new_box.refs;
        refs.unfound.clear();
        refs.notice = None;
    }

    /// The lists the page's `@` reads: the sessions and the files in the folder, never commands, skills or agents.
    fn new_lists(&self, key: &str, query: Option<&str>) -> Lists {
        let mut lists = Lists::for_draft(None);
        lists.draft_key = Some(key.to_string());
        lists.harness_type = Some(self.resolved_harness());
        lists.harness_names = self.harnesses_here().all.iter().map(|h| (h.kind.clone(), h.display_name.clone())).collect();
        lists.files_loading = self.finder.files.loading;
        lists.files_error = self.finder.files.error.clone();
        lists.sessions = self.mentionable_sessions();
        if let (Some(q), Some(_)) = (query, self.new_files_directory()) {
            lists.files = if self.link_for_machine(&self.target_key()).is_some() {
                self.finder.files.files(q)
            } else {
                crate::mock::finder::find_files(q)
            };
        }
        lists
    }

    /// Runs `f` with the engine and a context for the box's text at its caret.
    fn with_new_mentions<R>(&mut self, f: impl FnOnce(&mut Autocomplete, &mut Ctx) -> R) -> Option<R> {
        let key = self.new_row_key().filter(|_| self.on_new_session_page())?;
        let caret = self.new_box.edit.range().1;
        let text = self.new_box.edit.text.clone();
        let mut scratch = Picks::default();
        let mut probe = Lists::for_draft(None);
        probe.draft_key = Some(key.clone());
        let query = {
            let ctx = Ctx { text: &text, caret, lists: &probe, picks: &mut scratch };
            self.new_box.refs.engine.file_query(&ctx).map(str::to_string)
        };
        let lists = self.new_lists(&key, query.as_deref());
        let picks = self.auto.picks_mut(&key);
        let mut ctx = Ctx { text: &text, caret, lists: &lists, picks };
        Some(f(&mut self.new_box.refs.engine, &mut ctx))
    }

    /// The edit a pick or a key made goes into the box as one edit (so undo takes it back).
    fn apply_new_edit(&mut self, edit: Edit, tx: &mut Tx) {
        let b = &mut self.new_box;
        let end = b.edit.text.len();
        b.edit.set_selection(0, end);
        b.edit.insert(&edit.text);
        b.edit.set_caret(edit.caret);
        b.refs.caret_again = Some((b.edit.text.clone(), edit.caret));
        tx.focus(MESSAGE_KEY, false);
        tx.wake_after(Duration::from_millis(16));
    }

    /// A click on a row, or on a folder's chevron (`onSelect`, `onOpenFolder`).
    pub fn new_autocomplete_pick(&mut self, value: &str, open_folder: bool, tx: &mut Tx) {
        let edit = self.with_new_mentions(|engine, ctx| {
            let disabled = engine.items(ctx).iter().any(|i| i.value == value && i.disabled_reason.is_some());
            if disabled {
                None
            } else if open_folder {
                engine.on_open_folder(ctx, value)
            } else {
                engine.on_select(ctx, value)
            }
        });
        match edit.flatten() {
            Some(edit) => self.apply_new_edit(edit, tx),
            None => tx.focus(MESSAGE_KEY, false),
        }
    }

    /// A key in the box before the field sees it: the arrows, Enter, Tab and Esc while the list is open (Enter doesn't send,
    /// Esc doesn't leave). True when the list took it (A5.8-A5.12's `@` keys).
    pub fn new_autocomplete_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if self.new_box.edit.is_composing() || self.new_session_starting() {
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
        let Some(out) = self.with_new_mentions(|engine, ctx| engine.on_key(ctx, key)) else { return false };
        if let Some(edit) = out.edit {
            self.apply_new_edit(edit, tx);
        }
        if matches!(key, Key::Down | Key::Up)
            && let Some(i) = self.with_new_mentions(|engine, ctx| engine.selected_index(ctx))
        {
            tx.scroll_into_view_in("autocomplete-scroll", format!("autocomplete-row-{i}"));
        }
        out.consumed || out.swallowed
    }

    fn new_picked_ranges(&self) -> Vec<Range<usize>> {
        let Some(picks) = self.new_row_key().and_then(|k| self.auto.picks_of(&k).cloned()) else { return Vec::new() };
        let sessions = picks.remembered_session_tokens();
        let files: Vec<&str> = picks.files().iter().map(String::as_str).collect();
        picked_reference_ranges(&self.new_box.edit.text, &sessions, &files)
    }

    /// Backspace and Delete next to a pick take all of it, the arrows step over it (`handleReferenceKey`). True when the key was
    /// dealt with (A21.59).
    pub fn new_reference_key(&mut self, ev: &KeyEvent) -> bool {
        let key = match ev.key {
            rune::Key::Backspace => RefKey::Backspace,
            rune::Key::Delete => RefKey::Delete,
            rune::Key::Left => RefKey::Left,
            rune::Key::Right => RefKey::Right,
            _ => return false,
        };
        let is_delete = matches!(key, RefKey::Backspace | RefKey::Delete);
        if ev.mods.cmd || (!is_delete && (ev.mods.shift || ev.mods.alt || ev.mods.ctrl)) {
            return false;
        }
        let (anchor, focus) = self.new_box.edit.range();
        if anchor != focus {
            return false;
        }
        let Some(action) = reference_key_action(key, focus, &self.new_picked_ranges()) else { return false };
        self.new_box.edit.set_selection(action.selection.0, action.selection.1);
        action.prevent_default
    }

    /// The picks in the box, to tint (`useDraftReferences` with the unfound ones plain).
    pub fn new_references(&self) -> Vec<RefSpan> {
        let Some(picks) = self.new_row_key().and_then(|k| self.auto.picks_of(&k).cloned()) else { return Vec::new() };
        let sessions = picks.remembered_session_tokens();
        let files: Vec<&str> = picks.files().iter().map(String::as_str).collect();
        let plain: Vec<&str> = self.new_box.refs.unfound.iter().map(String::as_str).collect();
        let caret = self.new_box.edit.range().1;
        reference_spans(&self.new_box.edit.text, Some(caret), &Known { sessions: &sessions, files: &files, skills: &[], plain: &plain })
    }

    /// The paths the folder change turned back into text, as the line under the box says it (A8.11).
    pub fn new_folder_notice(&self) -> Option<(Vec<String>, String)> {
        let paths = self.new_box.refs.notice.clone()?;
        let folder = self.new_files_directory().map_or_else(|| "a chat with no folder".to_string(), |d| folder_name(&d));
        Some((paths, folder))
    }

    /// The popup the page draws, when the box has one open.
    pub fn new_popup(&self) -> Option<&Popup> {
        self.new_box.refs.popup.as_ref().filter(|_| self.on_new_session_page())
    }

    /// Everything the box's `@` needs each tick: the list worked out, what it asks the server for, the caret kept outside a
    /// pick, the picks the text lost forgotten, a folder change looked up, a GitHub item handed to the page. True when the
    /// page must be drawn again.
    pub fn keep_new_mentions(&mut self, now: f64, tx: &mut Tx) -> bool {
        if !self.on_new_session_page() {
            return std::mem::take(&mut self.new_box.refs.popup).is_some();
        }
        let mut changed = self.take_initial_source();
        let Some(key) = self.new_row_key() else { return changed };
        if let Some((text, caret)) = self.new_box.refs.caret_again.take()
            && self.new_box.edit.text == text
        {
            self.new_box.edit.set_caret(caret);
            changed = true;
        }
        // A caret left inside a pick (by a click) moves to its nearer edge.
        let (anchor, focus) = self.new_box.edit.range();
        if anchor == focus {
            let moved = crate::composer::draftrefs::caret_outside_references(focus, &self.new_picked_ranges());
            if moved != focus {
                self.new_box.edit.set_caret(moved);
            }
        }
        // Picks deleted from the text are forgotten; so are the plain ones that went.
        let text = self.new_box.edit.text.clone();
        self.auto.picks_mut(&key).forget_files_not_in(&text);
        let before = self.new_box.refs.unfound.len();
        self.new_box.refs.unfound.retain(|t| crate::composer::refs::text_has_token(&text, t));
        changed |= before != self.new_box.refs.unfound.len();
        changed |= self.keep_folder_picks(&key, tx);
        changed | self.keep_new_popup(&key, now, tx)
    }

    fn keep_new_popup(&mut self, key: &str, now: f64, tx: &mut Tx) -> bool {
        let seen = (self.new_box.edit.text.clone(), self.new_box.edit.range().1, key.to_string());
        let was_open = self.new_box.refs.popup.is_some();
        self.new_box.refs.seen = Some(seen);
        let scope = self.new_files_directory().map(Scope::Directory);
        let hint = self.new_file_hint();
        let note = if self.new_page_fleet_tools() {
            "The agent gets a link to the session, not a copy, and reads what it needs."
        } else {
            "The agent gets the session's title and a recap of where it stands, not a copy."
        };
        let Some((popup, query, mention_open)) = self.with_new_mentions(|engine, ctx| {
            let query = engine.file_query(ctx).map(str::to_string);
            let mention_open = engine.is_mention_open(ctx);
            let popup = engine.is_open(ctx).then(|| Popup {
                items: engine.items(ctx),
                loading: engine.is_loading(ctx),
                error: engine.error(ctx),
                selected: engine.selected_value(ctx),
                skill_hint: String::new(),
                session_note: "",
                empty_text: None,
                query: engine.query(ctx).to_string(),
                file_hint: None,
            });
            (popup, query, mention_open)
        }) else {
            return std::mem::take(&mut self.new_box.refs.popup).is_some();
        };
        let mut popup = popup;
        if let Some(p) = &mut popup {
            p.session_note = note;
            p.file_hint = hint;
        }
        self.ask_new_lists(scope, query.as_deref(), mention_open, now, tx);
        let held = self.with_new_mentions(|engine, ctx| engine.poll(ctx)).flatten();
        if let Some(edit) = held {
            self.apply_new_edit(edit, tx);
            return true;
        }
        if popup != self.new_box.refs.popup {
            self.new_box.refs.popup = popup;
            return true;
        }
        // The list is open: look again next frame (an answer may have come).
        if was_open {
            tx.wake_after(Duration::from_millis(30));
        }
        false
    }

    /// The requests the open list needs, to the machine the page is for: the file search after its wait, and the sessions
    /// once when the list opens (A8.4, A8.5).
    fn ask_new_lists(&mut self, scope: Option<Scope>, query: Option<&str>, mention_open: bool, now: f64, tx: &mut Tx) {
        let link = self.link_for_machine(&self.target_key());
        match self.finder.files.want(scope.as_ref(), query, now) {
            Step::Idle => {}
            Step::Wait(seconds) => tx.wake_after(Duration::from_secs_f64(seconds + 0.002)),
            Step::Send { generation, query } => {
                if let (Some(link), Some(Scope::Directory(path))) = (link.clone(), scope) {
                    let asked = query.clone();
                    link.off_thread(
                        move |base| match http::find_in_directory(base, &path, &asked) {
                            Ok((status, body)) if (200..300).contains(&status) => Ok(parse_files(&body)),
                            Ok((status, body)) => Err(crate::fleet::finder::failure(status, &body)),
                            Err(why) => Err(why),
                        },
                        move |s, _, result| s.finder.files.answered(generation, &query, result),
                    );
                }
            }
        }
        if link.is_some() && self.finder.sessions.should_fetch(mention_open, now) {
            if let Some(link) = link {
                link.off_thread(
                    |base| http::sessions_for_mentions(base).map(|body| parse_sessions(&body)),
                    move |s, _, result| s.finder.sessions.arrived(result, s.clock.max(now)),
                );
            }
        } else if self.link.is_none() {
            let _ = self.finder.sessions.should_fetch(mention_open, now);
        }
    }

    /// `watch(filesDirectory)`: a file picked in one folder may not be in the next. Each is looked up there; the ones that aren't
    /// go back to plain text, with a line saying so. Nothing leaves the draft, and sessions stay (A8.9).
    fn keep_folder_picks(&mut self, key: &str, tx: &mut Tx) -> bool {
        let next = self.new_files_directory();
        let Some(previous) = self.new_box.refs.folder_seen.clone() else {
            self.new_box.refs.folder_seen = Some(next);
            return false;
        };
        if previous == next {
            return false;
        }
        self.new_box.refs.folder_seen = Some(next.clone());
        self.new_box.refs.lookup += 1;
        let had_notice = self.new_box.refs.notice.take().is_some();
        let picks: Vec<String> = self.auto.picks_of(key).map(|p| p.files().to_vec()).unwrap_or_default();
        if picks.is_empty() {
            return had_notice;
        }
        let Some(dir) = next else {
            self.lose_picks(picks);
            return true;
        };
        let generation = self.new_box.refs.lookup;
        match self.link_for_machine(&self.target_key()) {
            Some(link) => link.off_thread(
                move |base| {
                    picks
                        .into_iter()
                        .filter(|token| {
                            let path = token.trim_start_matches('@');
                            let asked = path.trim_end_matches('/');
                            // A lookup that fails counts as found: the pick stays.
                            match http::find_in_directory(base, &dir, asked) {
                                Ok((status, body)) if (200..300).contains(&status) => !parse_files(&body).iter().any(|f| f == path),
                                _ => false,
                            }
                        })
                        .collect::<Vec<String>>()
                },
                move |s, tx: &mut Tx, missing| {
                    if s.new_box.refs.lookup == generation {
                        s.lose_picks(missing);
                        tx.wake_after(Duration::from_millis(1));
                    }
                },
            ),
            None => {
                let missing = picks
                    .into_iter()
                    .filter(|t| {
                        let path = t.trim_start_matches('@');
                        !crate::mock::finder::find_files(path.trim_end_matches('/')).iter().any(|f| f == path)
                    })
                    .collect();
                self.lose_picks(missing);
            }
        }
        let _ = tx;
        true
    }

    /// `forgetFileReferences`, `unfoundTokens` and the line under the box.
    fn lose_picks(&mut self, missing: Vec<String>) {
        if missing.is_empty() {
            return;
        }
        let Some(key) = self.new_row_key() else { return };
        let tokens: Vec<&str> = missing.iter().map(String::as_str).collect();
        self.auto.picks_mut(&key).forget_files(Some(&tokens));
        let refs = &mut self.new_box.refs;
        for token in &missing {
            if !refs.unfound.contains(token) {
                refs.unfound.push(token.clone());
            }
        }
        refs.notice = Some(missing.iter().map(|t| t.trim_start_matches('@').to_string()).collect());
    }

    // ── The GitHub place (A6, A7, A19) ──────────────────────────────────────

    /// Registers a host's `#` list (component 21 calls this once, at start-up).
    pub fn register_item_picker(&mut self, picker: std::sync::Arc<dyn super::places::ItemPicker>) {
        self.new_box.refs.places.register(picker);
    }

    /// A `#query` the caret is typing, when some host can answer it (A6.1; inert with nothing registered).
    pub fn new_hash_trigger(&self) -> Option<super::places::HashTrigger> {
        if self.new_box.refs.places.is_empty() || self.new_session_starting() {
            return None;
        }
        find_hash_trigger(&self.new_box.edit.text, self.new_box.edit.range().1)
    }

    /// The items the host offers for the `#` the caret is typing in the Folder chip's folder.
    pub fn new_hash_items(&self) -> Vec<PickerItem> {
        match (self.new_hash_trigger(), self.new_files_directory()) {
            (Some(t), Some(dir)) => self.new_box.refs.places.search(&dir, &t.query),
            _ => Vec::new(),
        }
    }

    /// `pickGitHubItem`: the `#query` leaves the message, the item attaches, the workspace is a new worktree, the branch name clears.
    pub fn pick_item(&mut self, item: &PickerItem, tx: &mut Tx) {
        if let Some(trigger) = self.new_hash_trigger() {
            let (text, caret) = remove_trigger(&self.new_box.edit.text, &trigger);
            self.apply_new_edit(Edit { text, caret }, tx);
        }
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.github_preset = Some(item.preset.clone());
            draft.workspace = Workspace::New;
            draft.branch_name.clear();
        }
        self.new_box.refs.preset_folder_pending = false;
    }

    /// The preset's chip was removed: the box takes the keyboard (A7.6).
    pub fn remove_github_preset(&mut self, tx: &mut Tx) {
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.github_preset = None;
        }
        self.new_box.refs.preset_folder_pending = false;
        tx.focus(MESSAGE_KEY, false);
    }

    /// An issue handed to the page ("start session") is taken: it attaches, and its repository is looked for among the scanned
    /// ones once they are there (`watch(newSessionInitialSource)`, `applyInitialFolder`, A7.9). True when something changed.
    fn take_initial_source(&mut self) -> bool {
        let mut changed = false;
        if self.new_draft.draft().is_some()
            && let Some(preset) = self.new_draft.initial_source().cloned()
        {
            if let Some(draft) = self.new_draft.draft_mut() {
                draft.github_preset = Some(preset);
                draft.has_chosen_folder = false;
            }
            self.new_draft.set_initial_source(None);
            self.new_box.refs.preset_folder_pending = true;
            changed = true;
        }
        if self.new_box.refs.preset_folder_pending {
            let feed = self.feeds.repos(&self.target_key());
            let preset = self.new_draft.draft().and_then(|d| d.github_preset.clone());
            let chosen = self.new_draft.draft().is_some_and(|d| d.has_chosen_folder);
            if let Some(preset) = preset.filter(|_| !chosen && (!feed.loading || !feed.list.is_empty())) {
                self.new_box.refs.preset_folder_pending = false;
                if let Some(repo) = repository_for_preset(&preset, &feed.list) {
                    self.set_folder(Folder::Repository(repo.path.clone()), false);
                }
                changed = true;
            } else if chosen || self.new_draft.draft().is_none_or(|d| d.github_preset.is_none()) {
                self.new_box.refs.preset_folder_pending = false;
            }
        }
        changed
    }
}

/// What a picture of the box's GitHub place and folder line shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefsScene {
    /// An issue attached from a fake host, the page's folder and worktree following it.
    Preset,
    /// Two picks the new folder doesn't have, turned back into text.
    Notice,
}

impl State {
    pub(crate) fn show_new_session_refs(&mut self, scene: RefsScene) {
        self.open_new_session_page(None);
        self.set_folder(Folder::Repository(crate::mock::feeds::ROCKET.into()), true);
        match scene {
            RefsScene::Preset => {
                if let Some(d) = self.new_draft.draft_mut() {
                    d.github_preset = Some(super::request::GitHubPreset {
                        source_type: "github-issue".into(),
                        owner: "acme".into(),
                        repo: "rocket".into(),
                        number: 42,
                        title: "Login redirect loops after sign-in".into(),
                        body: None,
                        html_url: "https://github.com/acme/rocket/issues/42".into(),
                        repo_full_name: "acme/rocket".into(),
                        suggested_branch: Some("fix/issue-42".into()),
                    });
                    d.message = "Start with the redirect in the sign-in handler".into();
                }
            }
            RefsScene::Notice => {
                if let Some(d) = self.new_draft.draft_mut() {
                    d.message = "Finish @src/billing/retry-policy.ts and @docs/billing/ today".into();
                }
                self.new_box.refs.unfound = vec!["@src/billing/retry-policy.ts".into(), "@docs/billing/".into()];
                self.new_box.refs.notice = Some(vec!["src/billing/retry-policy.ts".into(), "docs/billing/".into()]);
            }
        }
        self.sync_new_message();
    }
}
