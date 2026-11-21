//! Your work's page (`pages/GitHubBrowserPage.vue`, rows 4.x and 5.x of docs/parity/21-github.md): what it lists and in which bucket,
//! the words under its title, the countdown on the device code, and what it asks for when it opens. Pure but for `keep_github_home`;
//! the page that draws it is `view::home`.
use super::items::{ItemSummary, NeedsYou, Work, item_resource_id, needs_you, summary_from_link, your_pull_requests};
use super::sessions::sessions_for;
use super::{ID, state, state_ref};
use crate::fleet::new_session::machines::format_relative;
use crate::fleet::smart_links::Enrichment;
use crate::{Rail, State, Tx};
use std::collections::HashSet;
use std::time::Duration;

/// The four lists of the page, in the order it draws them. An item is in one only: Needs you first, then In Fleet, then the rest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Buckets {
    pub needs: Vec<NeedsYou>,
    pub in_fleet: Vec<ItemSummary>,
    pub pulls: Vec<ItemSummary>,
    pub assigned: Vec<ItemSummary>,
}

impl Buckets {
    pub fn is_empty(&self) -> bool {
        self.needs.len() + self.in_fleet.len() + self.pulls.len() + self.assigned.len() == 0
    }
}

/// `needs`, `inFleet`, `yourPulls` and `assigned` of the page. In Fleet is read from the sessions' own header links, open ones whose
/// enrichment has resolved, with a live (unarchived) session, so it asks GitHub nothing; newest update first.
pub fn buckets(s: &State, work: &Work) -> Buckets {
    let needs = needs_you(work);
    let listed: HashSet<String> = needs.iter().map(|n| item_resource_id(&n.item).to_lowercase()).collect();
    let mut seen = HashSet::new();
    let mut in_fleet: Vec<ItemSummary> = Vec::new();
    for link in s.smart_links.header_by_session.values().flatten() {
        let key = link.resource_id.to_lowercase();
        if link.is_terminal || link.enrichment != Enrichment::Resolved || seen.contains(&key) || listed.contains(&key) {
            continue;
        }
        let Some(item) = summary_from_link(link) else { continue };
        if sessions_for(s, &link.resource_id).is_empty() {
            continue;
        }
        seen.insert(key);
        in_fleet.push(item);
    }
    in_fleet.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let elsewhere: HashSet<String> = in_fleet.iter().map(|i| item_resource_id(i).to_lowercase()).collect();
    let fresh = |item: &ItemSummary| {
        let key = item_resource_id(item).to_lowercase();
        !listed.contains(&key) && !elsewhere.contains(&key)
    };
    Buckets {
        needs,
        in_fleet,
        pulls: your_pull_requests(work).into_iter().filter(fresh).collect(),
        assigned: work.assigned.iter().filter(|i| fresh(i)).cloned().collect(),
    }
}

/// `scope`: "Everything you're involved in" with nothing followed, else how many repositories.
pub fn scope_words(followed: usize) -> String {
    match followed {
        0 => "Everything you're involved in".into(),
        1 => "1 repository".into(),
        n => format!("{n} repositories"),
    }
}

/// The line under the title: the scope, then " · updated {when}" once the work has loaded. `age_secs` is how long ago it did.
pub fn header_line(followed: usize, age_secs: Option<f64>) -> String {
    let scope = scope_words(followed);
    match age_secs {
        Some(age) => format!("{scope} · updated {}", format_relative(0, (age.max(0.0) * 1000.0) as i64, 0)),
        None => scope,
    }
}

/// `expiresIn`: the minutes the code still works for, rounded (0 once it is nearly out, when the words leave the bracket out).
pub fn expires_in_minutes(expires_at: f64, now: f64) -> u64 {
    ((expires_at - now) / 60.0).round().max(0.0) as u64
}

/// The text the waiting box says under the code.
pub fn waiting_words(minutes: u64) -> String {
    if minutes > 0 {
        format!("Waiting for you to approve it (the code works for {minutes} more minutes)…")
    } else {
        "Waiting for you to approve it…".into()
    }
}

/// The words over the button after a try that failed, `None` when the last try didn't fail.
pub fn failure_words(device: &super::auth::Device) -> Option<String> {
    use super::auth::Device;
    match device {
        Device::Expired => Some("The code expired. Start again for a new one.".into()),
        Device::Denied => Some("GitHub says the request was denied.".into()),
        Device::Error(message) => Some(message.clone()),
        _ => None,
    }
}

impl State {
    /// Your work is the page on screen: GitHub's rail, at `/github`.
    pub fn github_home_shown(&self) -> bool {
        self.active_rail() == Some(Rail::Plugin(ID)) && state_ref(&self.places).at == super::Where::Home
    }

    /// `onMounted` of the page: once per visit it reads the followed repositories (the line under its title), the sessions' links (In
    /// Fleet) and, connected, your work. Ticks the "updated …" words. True when something drawn changed.
    pub(super) fn keep_github_home(&mut self, tx: &mut Tx) -> bool {
        let shown = self.github_home_shown();
        let was = state_ref(&self.places).home_asked;
        let mut changed = false;
        if shown != was {
            state(&mut self.places).home_asked = shown;
            changed = true;
            if shown {
                self.github_bookmarks_use();
                super::sessions::ensure_loaded(self);
                if super::auth_ref(&self.places).connected {
                    self.github_work_load();
                }
            } else {
                state(&mut self.places).home_age = None;
            }
        }
        if shown {
            let g = state_ref(&self.places);
            let age = g.work.loaded_at.map(|at| self.clock - at);
            let words = age.map(|a| header_line(0, Some(a)));
            if words != g.home_age {
                state(&mut self.places).home_age = words;
                changed = true;
            }
            if age.is_some() {
                tx.wake_after(Duration::from_secs(5));
            }
        }
        changed
    }
}
