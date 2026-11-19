//! After an update (`use-update-notices.ts`, `noteRunningVersion`): this device keeps the Fleet version it last ran, and when
//! it finds a newer one it says what's new, once. With the notes for that version it posts the What's new card (the changes most
//! worth a look, every version an update skipped over); without them (offline, or not published) only a quiet "Updated to …" chip.
//!
//! Native has one Fleet to ask, the one it is connected to, so there is one source, "server" (Fleet's browser client does the
//! same; its "app" source is the Electron shell's own version, which native has no semver for: a build number). The words
//! and the notices are plain functions; [`State::keep_after_update`] runs them as the status and the notes arrive.
use super::compare_versions;
use super::notes::{ItemKind as Kind, WHATS_NEW_ITEMS, WhatsNewSummary, whats_new_summary};
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::fleet::notices::{Action, ActionTone, IconRef, Item, ItemKind, Notice};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::updates::{Status, is_newer_version};
use crate::icons::SPARKLES;
use std::cmp::Ordering;

/// The source this device keeps a last version for (`weave:last-version:server`).
pub const SOURCE: &str = "server";
/// The card stays this long: there is a list to read (`WHATS_NEW_HOLD_MS`).
pub const HOLD_SECS: f64 = 20.0;
/// "Updated to …" stays this long in the status bar when there are no notes (`UPDATED_CHIP_MS`).
pub const UPDATED_CHIP_SECS: f64 = 12.0;
/// What the card's primary button and the chip tell their source: open What's new at `<version>[:<since>]`. The button's also
/// settles the card.
pub const OPEN_CHIP: &str = "fleet-update:open-whats-new:";
pub const OPEN_CARD: &str = "fleet-update:see-all-changes:";

/// "0.49" for 0.49.0; a patch release keeps its patch (0.47.1) (`shortVersion`).
pub fn short_version(version: &str) -> String {
    let v = version.strip_prefix('v').unwrap_or(version);
    match v.strip_suffix(".0") {
        Some(rest) if rest.split('.').count() == 2 && rest.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) => {
            rest.into()
        }
        _ => v.into(),
    }
}

/// "0.48.0 and 0.49.0", or "0.45.0 to 0.49.0" for more; releases come newest first (`versionList`).
pub fn version_list(versions: &[&str]) -> String {
    let (newest, oldest) = (versions[0], versions[versions.len() - 1]);
    if versions.len() == 2 { format!("{oldest} and {newest}") } else { format!("{oldest} to {newest}") }
}

/// What the What's new card says: its title, the line under it, and what it leaves out (`whatsNewCopy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copy {
    pub title: String,
    pub body: String,
    pub more: Option<String>,
}

/// `day` is the newest release's date ("12 Oct"), empty when it has none.
pub fn whats_new_copy(summary: &WhatsNewSummary, previous: &str, current: &str, day: &str) -> Copy {
    let versions: Vec<&str> = summary.releases.iter().map(|r| r.version.as_str()).collect();
    let left = summary.more;
    if versions.len() <= 1 {
        let more = match (left, summary.more_are_fixes) {
            (0, _) => None,
            (n, true) => Some(format!("+{n} more {}", if n == 1 { "fix" } else { "fixes" })),
            (n, false) => Some(format!("+{n} more")),
        };
        let tail = if day.is_empty() { String::new() } else { format!(" \u{b7} {day}") };
        return Copy {
            title: format!("What's new in Fleet {}", short_version(current)),
            body: format!("Updated from {previous}{tail}"),
            more,
        };
    }
    let list = version_list(&versions);
    Copy {
        title: format!("What's new since Fleet {previous}"),
        body: format!("{} updates \u{b7} {list}", versions.len()),
        more: (left > 0).then(|| format!("+{left} more in {list}")),
    }
}

/// The id of a What's new card (`whats-new:<source>:<version>`).
pub fn card_id(version: &str) -> String {
    format!("whats-new:{SOURCE}:{version}")
}

/// What opening What's new at this update names: the version, and `since` when it skipped over some (`open`).
fn open_run(prefix: &str, summary: &WhatsNewSummary, previous: &str, version: &str) -> String {
    if summary.releases.len() > 1 { format!("{prefix}{version}:{previous}") } else { format!("{prefix}{version}") }
}

/// The What's new card with its chip (`postWhatsNew`).
pub fn whats_new_notice(summary: &WhatsNewSummary, previous: &str, version: &str, day: &str) -> Notice {
    let id = card_id(version);
    let copy = whats_new_copy(summary, previous, version, day);
    let kind = |k: Kind| match k {
        Kind::Highlight => ItemKind::Highlight,
        Kind::New => ItemKind::New,
        Kind::Fixed => ItemKind::Fixed,
    };
    let items = summary.items.iter().map(|i| Item { text: i.text.clone(), label: i.scope.clone(), kind: Some(kind(i.kind)) }).collect();
    let action = |label: &str, tone: ActionTone, run: String| Action { label: label.into(), icon: None, tone, run };
    Notice {
        body: Some(copy.body),
        icon: Some(IconRef(&SPARKLES)),
        items,
        more: copy.more,
        chip: Some(format!("What's new in {}", short_version(version))),
        hold_secs: Some(HOLD_SECS),
        on_chip_click: Some(open_run(OPEN_CHIP, summary, previous, version)),
        actions: vec![
            action("See all changes", ActionTone::Primary, open_run(OPEN_CARD, summary, previous, version)),
            action("Close", ActionTone::Quiet, format!("fleet-update:dismiss:{id}")),
        ],
        ..Notice::new(id, copy.title)
    }
}

/// No notes to show: only a quiet "Updated to …" that goes away (`postUpdated`).
pub fn updated_notice(version: &str) -> Notice {
    Notice {
        on_chip_click: Some(format!("{OPEN_CHIP}{version}")),
        ..Notice::new(format!("updated:{SOURCE}:{version}"), format!("Updated to Fleet {version}. See what's new."))
            .chip(format!("Updated to {version}"))
            .quiet()
            .expires(UPDATED_CHIP_SECS)
    }
}

/// `noteRunningVersion`'s decision: the version this device ran before, when what runs now is newer. The caller keeps the
/// new version either way.
pub fn updated_from(last: Option<&str>, now: &str) -> Option<String> {
    let previous = last.filter(|p| !p.is_empty())?;
    is_newer_version(Some(now), Some(previous)).then(|| previous.to_string())
}

/// Splits what a button told: `<version>` or `<version>:<since>`.
pub fn parse_open(rest: &str) -> (&str, Option<&str>) {
    match rest.split_once(':') {
        Some((version, since)) => (version, Some(since)),
        None => (rest, None),
    }
}

/// An update this device found and has not yet said anything about.
#[derive(Debug)]
struct Pending {
    previous: String,
    version: String,
    /// The notes were read once more because they lacked this version (`notesFor` loads again when it is missing).
    reread: bool,
}

#[derive(Debug, Default)]
pub struct AfterUpdate {
    /// The running version has been noted this run.
    noted: bool,
    pending: Option<Pending>,
}

impl State {
    /// This run's version has been noted (for a test to wait on).
    pub fn after_update_noted(&self) -> bool {
        self.after_update.noted
    }

    /// Notes the version this Fleet runs once per run, and once the notes are in says what's new, or only that it updated. Called
    /// whenever the notices are kept; every step waits for an answer from the Fleet, and a Fleet that is managed by an app
    /// says nothing (`status !== "managed"`).
    pub fn keep_after_update(&mut self) {
        if self.link.is_none() {
            return;
        }
        if !self.after_update.noted {
            let clock = self.clock;
            let store = self.updates.status.entry(HOME_MACHINE_KEY.to_string()).or_default();
            let status = store.status.as_ref().map(|s| (s.status, s.current_version.clone()));
            match status {
                None => {
                    if store.due(clock) {
                        self.load_update_status(HOME_MACHINE_KEY, clock);
                    }
                    return;
                }
                Some((Status::Managed, _)) => self.after_update.noted = true,
                Some((_, version)) if version.is_empty() => return,
                Some((_, version)) => self.note_running_version(&version),
            }
        }
        self.keep_pending_update();
    }

    fn note_running_version(&mut self, version: &str) {
        self.after_update.noted = true;
        let previous = updated_from(self.sidebar.last_run_version(SOURCE), version);
        if self.sidebar.save_last_run_version(SOURCE, version) {
            self.save_settings();
        }
        if let Some(previous) = previous {
            self.after_update.pending = Some(Pending { previous, version: version.to_string(), reread: false });
        }
    }

    /// Waits for the notes, then posts the card or the chip.
    fn keep_pending_update(&mut self) {
        let Some(pending) = &self.after_update.pending else { return };
        let version = pending.version.clone();
        let store = self.updates.releases.entry(HOME_MACHINE_KEY.to_string()).or_default();
        if store.loading {
            return;
        }
        let has = store.releases.iter().any(|r| compare_versions(&r.version, &version) == Ordering::Equal);
        let reread = pending.reread;
        if !has && !(store.loaded && reread) {
            if let Some(p) = self.after_update.pending.as_mut() {
                p.reread = store.loaded;
            }
            self.load_releases(HOME_MACHINE_KEY);
            return;
        }
        let Some(pending) = self.after_update.pending.take() else { return };
        let releases = &self.updates.releases[HOME_MACHINE_KEY].releases;
        let notice = match whats_new_summary(releases, &pending.previous, &pending.version, WHATS_NEW_ITEMS) {
            Some(summary) => {
                let published = summary.releases.first().and_then(|r| r.published_at.as_deref());
                let day = super::whats_new::day(published, false, crate::view::message::now_ms(self), local_offset_secs());
                whats_new_notice(&summary, &pending.previous, &pending.version, &day)
            }
            None => updated_notice(&pending.version),
        };
        self.notices.post(notice, self.clock);
    }
}
