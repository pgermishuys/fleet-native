//! The morning dream notice (`composables/use-dream-notices.ts`): with Memory on, one card tells the user once that a night's
//! dream updated a repository's notes, and "See what changed" opens Settings → Memory on that repository.
//!
//! [`notice`] decides whether a dream posts and what it says; it is plain Rust. [`State::on_dream_event`] posts the card for a
//! `memory.dreamed` event, [`State::keep_dream_notices`] asks the live machine once for the nights it missed (`catchUp`), and
//! [`State::keep_dream_buttons`] acts on the button. Both ask the **live** machine, never the one Settings shows.
use super::events::{Event, Kind};
use super::notices::{Action, ActionTone, IconRef, Notice};
use super::settings::dream_nightly::{self, parse_nightly};
use super::settings::dreaming::{Dream, Status, parse_dream};
use super::settings::memory;
use super::state::Topic;
use crate::State;
use crate::icons::MOON;

/// What the button tells its source, followed by the dream's id.
pub const OPEN_PREFIX: &str = "dream-open:";

pub fn notice_id(dream: &Dream) -> String {
    format!("dream-{}", dream.id)
}

/// The card for a night's dream, or none (`show`): the dream is a night's (it has a `runId`, "Dream now" has none), it is done,
/// and it applied a change (a confirmed note counts) or left a flag open.
pub fn notice(dream: &Dream) -> Option<Notice> {
    let (changed, confirmed, flagged) = (dream.changed(), dream.confirmed(), dream.open_flags().len());
    if dream.run_id.is_none() || dream.status != Status::Done || changed + confirmed + flagged == 0 {
        return None;
    }
    let name = &dream.repository_name;
    let title = if changed > 0 {
        format!("Fleet updated {name}'s notes")
    } else if confirmed > 0 {
        format!("Fleet checked {name}'s notes")
    } else {
        format!("Some of {name}'s notes look out of date")
    };
    let label = if changed + confirmed > 0 { "See what changed" } else { "Review" };
    Some(Notice {
        body: Some(dream.describe()),
        icon: Some(IconRef(&MOON)),
        actions: vec![Action {
            label: label.into(),
            icon: None,
            tone: ActionTone::Primary,
            run: format!("{OPEN_PREFIX}{}\n{}", dream.id, dream.repository),
        }],
        ..Notice::new(notice_id(dream), title)
    })
}

/// What the app keeps of the notices: whether last night's have been asked for since Memory went on.
#[derive(Debug, Default)]
pub struct DreamNotices {
    caught_up: bool,
}

impl State {
    /// `memory.dreamed` on the `sessions` topic. Ignored while Memory is off (the subscription is "stopped" then).
    pub fn on_dream_event(&mut self, topic: &Topic, event: &Event) {
        let Event::Other(Kind::MemoryDreamed, payload) = event else { return };
        if *topic != Topic::Sessions || !self.dreaming_enabled() {
            return;
        }
        if let Some(notice) = parse_dream(payload).as_ref().and_then(notice) {
            self.notices.post(notice, self.clock);
        }
    }

    /// With Memory on, asks the live machine once for the nights that finished in the last day; with it off, forgets that it
    /// did, so turning it on asks again. A failed ask says nothing (Settings → Memory shows the error if it matters).
    pub fn keep_dream_notices(&mut self) {
        let on = self.link.is_some() && self.dreaming_enabled();
        if !on {
            self.dream_notices.caught_up = false;
            return;
        }
        if self.dream_notices.caught_up {
            return;
        }
        self.dream_notices.caught_up = true;
        let Some(link) = self.link.clone() else { return };
        link.off_thread(
            |base| {
                let call = dream_nightly::get_nightly();
                parse_nightly(&memory::send(base, &call).ok()?).map(|n| n.recent)
            },
            |s: &mut State, _, recent: Option<Vec<Dream>>| {
                if !s.dreaming_enabled() {
                    return;
                }
                for notice in recent.iter().flatten().filter_map(notice) {
                    s.notices.post(notice, s.clock);
                }
            },
        );
    }

    /// Whether last night's dreams have been asked for since Memory went on.
    pub fn dream_notices_asked(&self) -> bool {
        self.dream_notices.caught_up
    }

    /// "See what changed" and "Review": Settings → Memory on the dream's repository, and the card settles.
    pub fn keep_dream_buttons(&mut self) {
        for run in self.notices.take_heard_with(OPEN_PREFIX) {
            let Some((id, repository)) = run[OPEN_PREFIX.len()..].split_once('\n') else { continue };
            self.open_memory(repository);
            let id = format!("dream-{id}");
            if self.notices.open().is_some_and(|n| n.id == id) {
                self.notices.settle();
            } else {
                self.notices.remove(&id);
            }
        }
    }
}
