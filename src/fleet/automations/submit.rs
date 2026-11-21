//! The composer's Send and Save, and what it keeps up to date while the page is open (`handleCreate`, `handleSave`,
//! `resetEditState` and the repository watches of `AutomationDetailPanel.vue`; `submit` of `AutomationComposer.vue`; rows
//! 1.8, 1.9, 4.11, 7.11, 7.12, 7.14, 9.3). Each request goes to the machine the view shows, on a thread; with no server
//! (scenes, the app with no Fleet) a create or save changes the invented list, so the page can be tried.
use super::actions::words_of;
use super::composer::{ComposerState, Context, composer_time_zone};
use super::model::{Automation, AutomationRequest};
use super::nav::ViewMode;
use super::schedule::describe_date;
use crate::{State, Tx};
use std::sync::OnceLock;

const CREATE_FAILED: &str = "Couldn't create the automation.";
const SAVE_FAILED: &str = "Couldn't save the automation.";
pub const SAVED: &str = "Saved.";

/// What the composer's upkeep remembers between frames.
#[derive(Debug, Default)]
pub struct Upkeep {
    /// The machine whose repositories were asked for since the page opened.
    asked: Option<String>,
    /// The repository paths last handed to [`super::nav::Nav::repositories_changed`].
    seen: Vec<String>,
    /// The draft was given New session's folder once the repositories were known (`applyInitialFolder`).
    pub(super) draft_folder_done: bool,
    /// The event types the machine's Fleet offers (`fetchEventCatalog`, read once with the repositories); None until it
    /// answers, and after a failure, when the When menu offers the built-in list.
    pub(super) events: Option<Vec<String>>,
}

/// The zone schedules are saved in, read once: finding it reads a link on disk, which a view must not do every frame.
pub fn time_zone() -> &'static str {
    static ZONE: OnceLock<String> = OnceLock::new();
    ZONE.get_or_init(composer_time_zone)
}

/// The local clock's offset at an instant, as the schedule words read it.
pub(crate) fn local(ms: i64) -> i64 {
    crate::conv::time::local_offset_at(ms)
}

impl State {
    /// Runs `f` with the composer's state (the open automation's changes, or the draft) and what it reads from outside.
    pub fn with_composer<R>(&self, editing: bool, f: impl FnOnce(&ComposerState, &Context) -> R) -> R {
        let open = if editing { self.open_automation() } else { None };
        let ctx = Context::new(open.as_ref(), time_zone(), crate::view::message::now_ms(self), &local);
        f(self.automations_nav.composer(editing), &ctx)
    }

    /// The composer is the open automation's (edit mode) rather than the draft's.
    pub fn composer_editing(&self) -> bool {
        self.automations_nav.mode == ViewMode::Edit
    }

    /// Enter in the box, or Send (`submit`): nothing unless it can be sent; with no folder chosen nothing is sent (piece K
    /// opens the folder menu there); else create or save.
    pub fn automation_submit(&mut self, tx: &mut Tx) {
        let editing = self.composer_editing();
        let busy = self.automations_nav.submitting;
        let Some(request) = self.with_composer(editing, |c, ctx| c.can_submit(ctx, busy).then(|| c.request(ctx)).flatten()) else {
            return;
        };
        // Like a new session: where it runs is a choice, not a silent default.
        if self.automations_nav.composer(editing).folder.is_none() {
            return;
        }
        if editing { self.automation_save(request, tx) } else { self.automation_create_now(request, tx) }
    }

    /// `handleCreate`: the draft goes, the new automation opens, and the note says when it first runs; a refusal keeps the
    /// text and says why over the composer.
    fn automation_create_now(&mut self, request: AutomationRequest, tx: &mut Tx) {
        let machine = self.automations_machine();
        self.automations_nav.form_error = None;
        if self.link_for_machine(&machine).is_none() {
            let store = self.automations.of_mut(&machine);
            let created = invented(&request, &format!("a-new-{}", store.list.len() + 1), None);
            store.list.push(created.clone());
            self.automation_created(created);
            return;
        }
        self.automations_nav.submitting = true;
        self.automation_create(
            &machine,
            &request,
            tx,
            Box::new(|s, _, result| {
                s.automations_nav.submitting = false;
                match result {
                    Ok(created) => s.automation_created(created),
                    Err(e) => s.automations_nav.form_error = Some(words_of(e, CREATE_FAILED)),
                }
            }),
        );
    }

    fn automation_created(&mut self, created: Automation) {
        self.automations_nav.reset_draft();
        self.automations_nav.set_active(&created.id);
        // Opening it clears the page's notes, so the note comes after.
        let first = created.next_run_at.as_deref().and_then(crate::fleet::work::parse_iso_ms).map(|ms| describe_date(ms, &local));
        let words = match first {
            Some(when) => format!("Created and on. First run {when}."),
            None => "Created and on.".to_string(),
        };
        self.automation_say(false, words);
    }

    /// `handleSave`: the changes become the automation as saved, and "Saved."; a refusal says why over the composer.
    fn automation_save(&mut self, request: AutomationRequest, tx: &mut Tx) {
        let Some(a) = self.open_automation() else { return };
        let machine = self.automations_machine();
        self.automations_nav.form_error = None;
        if self.link_for_machine(&machine).is_none() {
            if let Some(mine) = self.automations.of_mut(&machine).list.iter_mut().find(|x| x.id == a.id) {
                *mine = invented(&request, &a.id, Some(&a));
            }
            self.automation_saved();
            return;
        }
        self.automations_nav.submitting = true;
        let id = a.id.clone();
        self.automation_update(
            &machine,
            &a.id,
            &request,
            tx,
            Box::new(move |s, _, result| {
                s.automations_nav.submitting = false;
                if !s.still_open(&id) {
                    return;
                }
                match result {
                    Ok(()) => s.automation_saved(),
                    Err(e) => s.automations_nav.form_error = Some(words_of(e, SAVE_FAILED)),
                }
            }),
        );
    }

    /// The list was read again before the answer came back, so the edit state is made again from the saved automation.
    fn automation_saved(&mut self) {
        self.automations_nav.edit_for = None;
        self.reset_edit_state();
        self.automation_say(false, SAVED);
    }

    /// `resetEditState`: the open automation's composer is the automation as saved (as the list has it now).
    pub(crate) fn reset_edit_state(&mut self) {
        let Some(a) = self.open_automation() else { return };
        let repositories = self.composer_repositories();
        let nav = &mut self.automations_nav;
        nav.edit = ComposerState::state_for(&a, &repositories);
        nav.edit_box = rune::TextEdit::new(&a.prompt);
        nav.edit_for = Some(a.id);
    }

    /// The repository paths of the machine the view shows, as far as they are known.
    fn composer_repositories(&self) -> Vec<String> {
        self.feeds.repos(&self.automations_machine()).list.iter().map(|r| r.path.clone()).collect()
    }

    /// The composer's upkeep, each frame while the page is open: the edit state made from the open automation when it opens
    /// or first appears in the list (rows 1.8, 1.9); the machine's repositories asked for once, a folder read before they
    /// came upgraded (7.14), and the draft started in New session's folder once they're known (9.3). True when something
    /// changed.
    pub(crate) fn keep_automation_composer(&mut self, tx: &mut Tx) -> bool {
        let mode = self.automations_nav.mode;
        if mode == ViewMode::List {
            return false;
        }
        let mut changed = false;
        if mode == ViewMode::Edit
            && let Some(id) = self.automations_nav.open_id().map(str::to_string)
            && self.automations_nav.edit_for.as_deref() != Some(id.as_str())
            && self.open_automation().is_some()
        {
            self.reset_edit_state();
            changed = true;
        }
        let machine = self.automations_machine();
        if self.feed_base(&machine).is_none() {
            return changed;
        }
        if self.automations_nav.composer.asked.as_deref() != Some(machine.as_str()) {
            self.automations_nav.composer = Upkeep { draft_folder_done: false, ..Upkeep::default() };
            self.automations_nav.composer.asked = Some(machine.clone());
            self.read_repositories(&machine, false);
            let asked = machine.clone();
            self.automation_event_catalog(
                &machine,
                tx,
                Box::new(move |s, _, result| {
                    // The built-in list stays when it fails; a machine switched away from since keeps its answer out.
                    if let Ok(list) = result
                        && s.automations_nav.composer.asked.as_deref() == Some(asked.as_str())
                    {
                        s.automations_nav.composer.events = Some(list);
                    }
                }),
            );
            return true;
        }
        let feed = self.feeds.repos(&machine);
        let paths: Vec<String> = feed.list.iter().map(|r| r.path.clone()).collect();
        if paths != self.automations_nav.composer.seen {
            self.automations_nav.repositories_changed(&paths);
            self.automations_nav.composer.seen = paths.clone();
            changed = true;
        }
        let ready = !feed.loading && (feed.scanned_at.is_some() || feed.error.is_some());
        if mode == ViewMode::Create && ready && !self.automations_nav.composer.draft_folder_done {
            self.automations_nav.composer.draft_folder_done = true;
            let initial = self.sidebar.new_session_defaults(&machine).initial_folder(&paths);
            // This app talks to a machine's own Fleet, never a cloud one, so a plain folder is a fine start.
            self.automations_nav.draft.apply_initial_folder(false, true, initial, false);
            changed = true;
        }
        changed
    }

    /// The plan line's ask: "Just once" makes the "on Friday" a one-off, "Every Friday instead" undoes it.
    pub fn automation_plan_answer(&mut self) {
        let editing = self.composer_editing();
        let Some(action) = self.with_composer(editing, |c, ctx| c.plan(ctx).ask.map(|a| a.action)) else { return };
        let once = action == super::words::AskAction::JustOnce;
        let nav = &mut self.automations_nav;
        if editing { nav.edit.set_force_once(once) } else { nav.draft.set_force_once(once) }
    }
}

/// An automation made from a request with no server to ask: the invented list's create and save.
fn invented(request: &AutomationRequest, id: &str, was: Option<&Automation>) -> Automation {
    let mut v = request.to_json();
    v["id"] = serde_json::json!(id);
    v["isEnabled"] = serde_json::json!(was.is_none_or(|a| a.is_enabled));
    let mut made = Automation::from_json(&v);
    if let Some(was) = was {
        made.created_at = was.created_at.clone();
        made.next_run_at = was.next_run_at.clone();
        made.last_run = was.last_run.clone();
    }
    made
}
