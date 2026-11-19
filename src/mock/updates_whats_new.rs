//! Scenes of Settings → Updates & about › What's new, and of the card and chip Fleet posts after an update. The releases are
//! invented (no real change, person or repository); nothing is read from GitHub.
use crate::State;
use crate::fleet::updates::after_update::{updated_notice, whats_new_notice};
use crate::fleet::updates::notes::{ReleaseNote, whats_new_summary};

const PR: &str = "https://github.com/example/harbor/pull";

fn change(title: &str, n: u32) -> String {
    format!("* {title} by @maker in {PR}/{n}\n")
}

fn release(version: &str, day: &str, body: String) -> ReleaseNote {
    ReleaseNote {
        version: version.into(),
        published_at: Some(format!("2026-{day}T08:00:00Z")),
        body,
        url: format!("https://github.com/example/harbor/releases/tag/v{version}"),
    }
}

/// Seven releases, newest first: the newest has a hand-written intro, the middle ones GitHub's generated lists with fixes and
/// behind-the-scenes changes counted.
pub fn releases() -> Vec<ReleaseNote> {
    let list = |lines: Vec<String>| format!("## What's Changed\n{}\n**Full Changelog**: {PR}/../compare/v0.1.0...v0.2.0\n", lines.concat());
    vec![
        release(
            "0.73.0",
            "11-16",
            format!(
                "## Updates, in one place\n\nSettings now shows every version and what changed in it, so an update never arrives as a surprise.\n\n{}",
                list(vec![
                    change("feat(settings): What's new on the Updates page", 412),
                    change("feat(machines): update every machine one at a time", 409),
                    change("fix(claude-code): keep what the agent left running", 411),
                    change("test(updates): hold the stranger port", 410),
                ])
            ),
        ),
        release(
            "0.72.0",
            "11-12",
            list(vec![
                change("feat(files): open the files a reply names in a tab", 398),
                change("fix(files): show images as a picture, not in the editor", 401),
                change("fix(sessions): dragging sessions works again", 402),
                change("refactor(client): split the session list", 400),
                change("docs: describe the new keys", 403),
            ]),
        ),
        release(
            "0.71.0",
            "11-08",
            list(vec![change("Pin sessions above the projects", 386), change("fix(api): answer a missing file with a 404", 388)]),
        ),
        release("0.70.0", "11-03", list(vec![change("feat(github): show a pull request's checks", 377)])),
        release("0.69.0", "10-28", list(vec![change("fix(ui): the review's first ten fixes", 370)])),
        release("0.68.0", "10-21", list(vec![change("feat(opencode2): sessions survive a restart", 361)])),
        release("0.67.0", "10-14", list(vec![change("Remember each machine's last version", 352)])),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// 0.73.0 is downloaded and waits; 0.72.0 runs: the first two rows open, five older ones folded.
    Waiting,
    /// A link named 0.71.0: its row is open and flashing.
    Arrived,
    /// GitHub couldn't be reached, but notes were saved.
    Stale,
    /// No notes at all, and the reason.
    Empty,
    /// The notes are still coming.
    Loading,
}

impl State {
    pub fn show_whats_new(&mut self, dark: bool, case: Case) {
        self.show_updates(dark, super::updates_page::Case::Staged);
        let store = self.updates.releases.entry("home".into()).or_default();
        store.loaded = case != Case::Loading;
        store.loading = case == Case::Loading;
        store.fetched_at = Some("2026-11-16T09:00:00Z".into());
        match case {
            Case::Waiting | Case::Arrived => store.releases = releases(),
            Case::Stale => {
                store.releases = releases();
                store.error = Some("Couldn't reach GitHub".into());
            }
            Case::Empty => store.error = Some("Couldn't reach GitHub".into()),
            Case::Loading => {}
        }
        if case == Case::Arrived {
            self.updates.whats_new.opened.insert("0.71.0".into(), true);
            self.updates.whats_new.flash = Some(("0.71.0".into(), self.clock - 0.3));
        }
        // Fleet's page is long; the scene lands on this group as a link to it does.
        self.open_settings("updates", Some("whats-new"));
    }

    /// The What's new card an update posts, open above the composer (`since` for an update that skipped a version).
    pub fn show_whats_new_card(&mut self, dark: bool, since: Option<&str>) {
        self.dark = Some(dark);
        self.now_ms = crate::fleet::work::parse_iso_ms("2026-11-17T12:00:00Z");
        let previous = since.unwrap_or("0.72.0");
        let summary = whats_new_summary(&releases(), previous, "0.73.0", 3).expect("notes for 0.73.0");
        self.notices.post(whats_new_notice(&summary, previous, "0.73.0", "Nov 16"), 0.0);
        self.notices.open_next();
    }

    /// Only the quiet chip: the notes for the new version weren't there.
    pub fn show_updated_chip(&mut self, dark: bool) {
        self.dark = Some(dark);
        self.notices.post(updated_notice("0.73.0"), 0.0);
    }
}
