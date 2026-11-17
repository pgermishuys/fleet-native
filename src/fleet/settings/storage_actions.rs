//! Settings → Folders → Storage's calls and what they change (`StorageSection.vue`). Reads, size walks and a clean go to the
//! machine Settings shows (`State::settings_target`); nothing here blocks: a thread sends, the answer comes back to the UI thread.
//! With no Fleet the card works on invented checkouts and a clean only changes that list: it can't reach a real folder.
use super::storage::{self, Listing, Report, SIZE_CONCURRENCY, Size, StoragePage};
use crate::fleet::settings_nav::Page;
use crate::{State, Tx};

impl State {
    /// Called from `poll`: while Folders is open, reads the checkouts when the card hasn't, or when the switch moved to another
    /// machine; leaving the page forgets the card (the cache of sizes stays), so coming back reads again (`onMounted(load)`).
    /// Sizes start as soon as the counts are in: Rune can't tell that the card has scrolled into view (PORT.md, the decision).
    pub fn keep_storage(&mut self, _tx: &mut Tx) -> bool {
        if self.rail != crate::Rail::Settings || self.settings.page != Page::Folders {
            let had = self.settings.storage.loaded_for.is_some();
            // The run number carries on, so an answer to a walk from an earlier visit is still known to be stale.
            self.settings.storage = StoragePage { run: self.settings.storage.run + 1, ..StoragePage::default() };
            return had;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.storage.loaded_for.as_deref() == Some(key.as_str()) {
            return false;
        }
        self.settings.storage = StoragePage { loaded_for: Some(key), run: self.settings.storage.run, ..StoragePage::default() };
        self.read_storage();
        true
    }

    /// `load()`: counts first, then sizes; a newer read makes older answers stale.
    fn read_storage(&mut self) {
        let key = self.settings.choice.key(&self.machines).to_string();
        let page = &mut self.settings.storage;
        page.run += 1;
        let run = page.run;
        page.loading = page.checkouts.is_empty();
        page.error = None;
        page.queue.clear();
        page.in_flight = 0;
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            // No Fleet: the invented checkouts, already measured.
            let page = &mut self.settings.storage;
            page.loading = false;
            page.adopt(storage::mock_listing(), crate::mock::storage::sizes());
            return;
        };
        link.off_thread(storage::load, move |s: &mut State, tx: &mut Tx, got: Result<Listing, String>| {
            if s.settings.storage.run != run {
                return;
            }
            let now = s.clock;
            let recent = s.settings.storage_sizes.recent(&key, now);
            let page = &mut s.settings.storage;
            page.loading = false;
            match got {
                Ok(listing) => page.adopt(listing, recent),
                Err(why) => page.error = Some(why),
            }
            s.pump_storage_sizes(tx);
        });
    }

    /// Keeps up to three folders being measured, the next from the queue (`SIZE_CONCURRENCY`).
    fn pump_storage_sizes(&mut self, _tx: &mut Tx) {
        let key = self.settings.choice.key(&self.machines).to_string();
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else { return };
        let run = self.settings.storage.run;
        while self.settings.storage.in_flight < SIZE_CONCURRENCY {
            let Some(id) = self.settings.storage.queue.pop_front() else { return };
            self.settings.storage.in_flight += 1;
            let (asked, machine) = (id.clone(), key.clone());
            link.off_thread(
                move |base| storage::measure(base, &asked),
                move |s: &mut State, tx: &mut Tx, got: Option<Size>| {
                    let now = s.clock;
                    // A measured size is worth keeping even when its page has moved on.
                    if let Some(size) = got {
                        s.settings.storage_sizes.remember(&machine, &id, size, now);
                    }
                    let page = &mut s.settings.storage;
                    if page.run != run {
                        return;
                    }
                    page.in_flight = page.in_flight.saturating_sub(1);
                    if let Some(size) = got {
                        page.sizes.insert(id, size);
                    }
                    s.pump_storage_sizes(tx);
                },
            );
        }
    }

    /// Preview…: opens the panel and clears any earlier report.
    pub fn preview_storage(&mut self) {
        let page = &mut self.settings.storage;
        if page.removable().is_empty() {
            return;
        }
        page.previewing = true;
        page.report = None;
    }

    pub fn cancel_storage_preview(&mut self) {
        if !self.settings.storage.cleaning {
            self.settings.storage.previewing = false;
        }
    }

    /// Remove N checkouts: the removable ones go, on the machine shown only.
    pub fn clean_storage(&mut self) {
        let page = &mut self.settings.storage;
        let ids: Vec<String> = page.removable().iter().map(|c| c.workspace_id.clone()).collect();
        if page.cleaning || ids.is_empty() {
            return;
        }
        page.cleaning = true;
        page.error = None;
        let run = page.run;
        let key = self.settings.choice.key(&self.machines).to_string();
        let Some(link) = self.link_for_machine(&key).filter(|_| self.settings_target().is_some()) else {
            // No Fleet: the invented list loses them, and nothing on disk is touched.
            let page = &mut self.settings.storage;
            let removed: Vec<_> = page.checkouts.iter().filter(|c| ids.contains(&c.workspace_id)).cloned().collect();
            page.cleaning = false;
            self.finish_clean(Report { removed, kept: Vec::new() }, false);
            return;
        };
        link.off_thread(
            move |base| storage::clean(base, &ids),
            move |s: &mut State, tx: &mut Tx, got: Result<Report, String>| {
                if s.settings.storage.run != run {
                    return;
                }
                s.settings.storage.cleaning = false;
                match got {
                    Ok(report) => {
                        s.finish_clean(report, true);
                        s.pump_storage_sizes(tx);
                    }
                    Err(why) => s.settings.storage.error = Some(why),
                }
            },
        );
    }

    /// The report: what the run removed, then what it kept and what the preview had already kept; then the card reads again.
    fn finish_clean(&mut self, run: Report, reread: bool) {
        let page = &mut self.settings.storage;
        page.report_sizes = page.sizes.clone();
        let again: Vec<String> = run.kept.iter().map(|c| c.workspace_id.clone()).collect();
        let mut kept = run.kept;
        kept.extend(page.kept().into_iter().filter(|c| !again.contains(&c.workspace_id)).cloned());
        page.previewing = false;
        if reread {
            page.report = Some(Report { removed: run.removed, kept });
            self.read_storage();
        } else {
            page.checkouts.retain(|c| !run.removed.iter().any(|r| r.workspace_id == c.workspace_id));
            page.report = Some(Report { removed: run.removed, kept });
        }
    }
}
