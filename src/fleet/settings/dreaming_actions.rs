//! The Dreaming panel's calls and what they change (`DreamingPanel.vue`: `load`, `dreamNow`, `undo`, `settleFlag`, the poll).
//! Every call goes to the machine Settings shows, like the rest of the Memory page, and none blocks: a thread sends and the
//! answer comes back to the UI thread. With no Fleet to ask the panel shows the invented dream in `mock::dreaming`, and the
//! buttons change only what is drawn.
use super::dreaming::{self, Dream, POLL_SECS};
use super::memory::{self, Request};
use crate::fleet::link::Link;
use crate::fleet::settings_nav::Page;
use crate::{State, Tx};
use rune::TextEdit;
use std::time::Duration;

/// What a flag's button does to its note before the flag is marked dealt with.
enum Act {
    Forget(String),
    Save(String, String),
}

fn ask_dream(base: &str, req: &Request) -> Result<Dream, String> {
    let body = memory::send(base, req)?;
    dreaming::parse_dream(&body).ok_or_else(|| req.fallback.clone())
}

fn words(error: String) -> String {
    if error.is_empty() { "Something went wrong.".into() } else { error }
}

impl State {
    /// The link to the machine Settings shows, or none when there is no Fleet to ask.
    pub fn dreaming_link(&self) -> Option<Link> {
        let key = self.settings.choice.key(&self.machines).to_string();
        self.link_for_machine(&key).filter(|_| self.settings_target().is_some())
    }

    /// The panel shows with Memory on and a repository chosen (`isDreamingEnabled && repository`), once the page has its notes.
    pub fn shows_dreaming(&self) -> bool {
        let page = &self.settings.memory;
        page.enabled() && page.notes.is_some() && page.repository.is_some()
    }

    /// Harnesses of the machine Settings shows that can be dreamed on: `(type, display name)`.
    pub fn dreaming_harnesses(&self) -> Vec<(String, String)> {
        let key = self.settings.choice.key(&self.machines).to_string();
        self.harness_choice(&key).enabled().into_iter().map(|h| (h.kind.clone(), h.display_name.clone())).collect()
    }

    /// The harness "Dream now" uses (`harnessType`).
    pub fn dreaming_harness_type(&self) -> String {
        let enabled: Vec<String> = self.dreaming_harnesses().into_iter().map(|(kind, _)| kind).collect();
        let nightly = self.settings.memory.dreaming.nightly_harness.as_deref();
        dreaming::harness_choice(self.sidebar.dreaming_harness(), nightly, &enabled)
    }

    /// Called from `poll`: asks for the repository's last dream when the panel appears or the repository changes, asks again
    /// every three seconds while a dream is reading or running, and cuts a flag's text at its limit.
    pub fn keep_dreaming(&mut self, now: f64, tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Memory {
            return false;
        }
        if !self.shows_dreaming() {
            let had = self.settings.memory.dreaming != Default::default();
            self.settings.memory.dreaming = Default::default();
            return had;
        }
        let mut changed = false;
        let page = &mut self.settings.memory;
        let at = (page.request, page.repository.clone());
        let d = &mut page.dreaming;
        if d.edit.text.chars().count() > dreaming::MAX_FLAG_CHARS {
            d.edit.replace_text(d.edit.text.chars().take(dreaming::MAX_FLAG_CHARS).collect::<String>());
            changed = true;
        }
        if d.asked.as_ref() != Some(&at) {
            d.asked = Some(at);
            d.set_dream(None);
            d.reading = false;
            if self.dreaming_link().is_some() {
                self.read_dreams();
            } else {
                self.settings.memory.dreaming.dream = Some(crate::mock::dreaming::last_night());
            }
            return true;
        }
        if d.busy() {
            let due = *d.next_poll.get_or_insert(now + POLL_SECS);
            let start = now >= due && !d.reading;
            if start {
                d.next_poll = Some(now + POLL_SECS);
            }
            let wait = d.next_poll.map_or(POLL_SECS, |at| (at - now).clamp(0.05, POLL_SECS));
            if start {
                self.read_dreams();
                changed = true;
            }
            tx.wake_after(Duration::from_secs_f64(wait));
        }
        changed
    }

    /// `load`: the repository's dreams, newest first; only the first is shown. An answer for a repository no longer chosen,
    /// or for a machine no longer shown, is dropped.
    fn read_dreams(&mut self) {
        let Some(link) = self.dreaming_link() else { return };
        let page = &mut self.settings.memory;
        let Some(repository) = page.repository.clone() else { return };
        let request = page.request;
        page.dreaming.reading = true;
        let call = dreaming::list_dreams(&repository);
        let wanted = repository.clone();
        link.off_thread(
            move |base| {
                let body = memory::send(base, &call)?;
                dreaming::parse_dreams(&body).ok_or_else(|| call.fallback.clone())
            },
            move |s: &mut State, _tx: &mut Tx, got: Result<Vec<Dream>, String>| {
                let page = &mut s.settings.memory;
                if page.request != request || page.repository.as_deref() != Some(wanted.as_str()) {
                    return;
                }
                let d = &mut page.dreaming;
                d.reading = false;
                match got {
                    Ok(dreams) => {
                        let stopped = d.set_dream(dreams.into_iter().next());
                        // Only this read's own error clears; a failed Dream now or Undo keeps its note.
                        if d.error == d.load_error {
                            d.error = None;
                        }
                        d.load_error = None;
                        if stopped {
                            s.memory_read_notes_again();
                        }
                    }
                    Err(e) => {
                        let e = words(e);
                        d.load_error = Some(e.clone());
                        d.error = Some(e);
                    }
                }
            },
        );
    }

    /// "Dream now" (`dreamNow`): starts a dream on the harness shown. None while one is reading or running, or being started.
    pub fn dream_now(&mut self) {
        let page = &self.settings.memory;
        let d = &page.dreaming;
        let Some(repository) = page.repository.clone() else { return };
        if d.starting || d.busy() {
            return;
        }
        let Some(link) = self.dreaming_link() else { return };
        let harness = self.dreaming_harness_type();
        let request = page.request;
        let d = &mut self.settings.memory.dreaming;
        d.starting = true;
        d.error = None;
        let call = dreaming::start_dream(&repository, Some(&harness));
        link.off_thread(
            move |base| ask_dream(base, &call),
            move |s: &mut State, _tx: &mut Tx, got: Result<Dream, String>| {
                if s.settings.memory.request != request {
                    return;
                }
                let d = &mut s.settings.memory.dreaming;
                d.starting = false;
                match got {
                    Ok(dream) => {
                        d.set_dream(Some(dream));
                    }
                    Err(e) => d.error = Some(words(e)),
                }
            },
        );
    }

    /// Undo on a change (`undo`): the change is put back, the dream comes back updated and the notes are read again.
    pub fn dreaming_undo(&mut self, change: &str) {
        self.settle_change(change, None, true);
    }

    /// Keep, Forget and Save all settle a flag: Fleet marks it dealt with, so it doesn't show again (`settleFlag`).
    pub fn dreaming_keep(&mut self, change: &str) {
        self.settle_change(change, None, false);
    }

    pub fn dreaming_forget(&mut self, change: &str) {
        let Some(note) = self.flag_note(change) else { return };
        self.settle_change(change, Some(Act::Forget(note)), false);
    }

    pub fn dreaming_save(&mut self, change: &str) {
        let Some(note) = self.flag_note(change) else { return };
        let text = self.settings.memory.dreaming.edit.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.settle_change(change, Some(Act::Save(note, text)), false);
    }

    /// The note a flag points at; a flag with none can't be edited or forgotten.
    fn flag_note(&self, change: &str) -> Option<String> {
        let dream = self.settings.memory.dreaming.dream.as_ref()?;
        dream.changes.iter().find(|c| c.id == change)?.note_id.clone()
    }

    fn settle_change(&mut self, change: &str, act: Option<Act>, undo: bool) {
        let d = &mut self.settings.memory.dreaming;
        let Some(dream_id) = d.dream.as_ref().map(|d| d.id.clone()) else { return };
        if d.undoing.is_some() {
            return;
        }
        d.undoing = Some(change.to_string());
        d.error = None;
        let Some(link) = self.dreaming_link() else {
            // No Fleet: the change is only marked, since the invented page is not a store.
            let d = &mut self.settings.memory.dreaming;
            d.undoing = None;
            d.editing = None;
            if let Some(c) = d.dream.as_mut().and_then(|dr| dr.changes.iter_mut().find(|c| c.id == change)) {
                c.undone = true;
            }
            return;
        };
        let request = self.settings.memory.request;
        let calls = match &act {
            Some(Act::Forget(note)) => vec![memory::forget_note(note)],
            Some(Act::Save(note, text)) => vec![memory::update_note(note, text)],
            None => vec![],
        };
        let undo_call = dreaming::undo_change(&dream_id, change);
        let touched_notes = undo || act.is_some();
        link.off_thread(
            move |base| {
                for call in &calls {
                    memory::send(base, call)?;
                }
                ask_dream(base, &undo_call)
            },
            move |s: &mut State, _tx: &mut Tx, got: Result<Dream, String>| {
                if s.settings.memory.request != request {
                    return;
                }
                let d = &mut s.settings.memory.dreaming;
                d.undoing = None;
                match got {
                    Ok(dream) => {
                        let stopped = d.set_dream(Some(dream));
                        d.editing = None;
                        if touched_notes || stopped {
                            s.memory_read_notes_again();
                        }
                    }
                    Err(e) => d.error = Some(words(e)),
                }
            },
        );
    }

    /// Edit on a flag: opens its text area with the note's words, if the flag points at a note.
    pub fn dreaming_edit(&mut self, change: &str) {
        let d = &mut self.settings.memory.dreaming;
        let Some(c) = d.dream.as_ref().and_then(|dr| dr.changes.iter().find(|c| c.id == change)) else { return };
        if c.note_id.is_none() {
            return;
        }
        let words = c.text.clone().unwrap_or_default();
        d.editing = Some(change.to_string());
        d.edit = TextEdit::new(words);
    }

    pub fn dreaming_cancel_edit(&mut self) {
        self.settings.memory.dreaming.editing = None;
    }

    pub fn dreaming_toggle_why(&mut self, change: &str) {
        let open = &mut self.settings.memory.dreaming.open;
        if !open.remove(change) {
            open.insert(change.to_string());
        }
    }

    pub fn dreaming_toggle_skipped(&mut self) {
        let d = &mut self.settings.memory.dreaming;
        d.show_skipped = !d.show_skipped;
    }

    /// Picking a harness keeps it for the install, not for the machine (`weave:dreaming-harness`).
    pub fn dreaming_pick_harness(&mut self, kind: &str) {
        self.sidebar.save_dreaming_harness(kind);
        self.save_settings();
    }

    /// A link to a session a dream read or started: it opens on the machine this app is on (`/sessions/{id}`).
    pub fn open_dreaming_session(&mut self, id: &str, tx: &mut Tx) {
        self.rail = crate::Rail::Sessions;
        let at = self.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|s| s.id == id).map(|i| (p, i)));
        match at {
            Some((p, i)) => self.open_session(p, i, tx),
            None => {
                self.go(crate::fleet::store::Route::Session(id.to_string()));
                self.after_open(tx);
            }
        }
    }
}
