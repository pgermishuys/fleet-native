//! Invented memory for Settings → Memory when the app has no Fleet to ask (and for its scenes and tests): a fresh machine
//! with the switch off, three repositories and a handful of notes. Nothing here is from a real machine.
use crate::fleet::settings::memory::{List, Note, Notes, Overview, Repository};

pub const REPOSITORY: &str = "/work/lantern";

pub fn overview() -> Overview {
    let repo = |path: &str, name: &str, count| Repository { path: path.into(), name: name.into(), count };
    Overview {
        enabled: false,
        repositories: vec![
            repo(REPOSITORY, "lantern", 2),
            repo("/work/harbor-api", "harbor-api", 1),
            repo("/work/tidepool", "tidepool", 0),
        ],
        machine_count: 2,
        max_repository_notes: 40,
        max_machine_notes: 15,
        search_enabled: false,
        max_always_repository_notes: None,
        max_always_machine_notes: None,
        max_search_repository_notes: None,
        max_search_machine_notes: None,
    }
}

fn note(id: &str, list: List, text: &str, kind: &str) -> Note {
    Note {
        id: id.into(),
        list,
        text: text.into(),
        kind: kind.into(),
        repository: (list == List::Repository).then(|| REPOSITORY.to_string()),
        session_id: None,
        session_title: None,
        created: "2026-10-02T09:12:00Z".into(),
        updated: "2026-10-02T09:12:00Z".into(),
        lifetime: None,
        days_left: None,
        expired: false,
        relearned: 0,
        how: None,
        in_search: false,
        pinned: false,
        found: 0,
        last_found: None,
    }
}

pub fn notes() -> Notes {
    let mut slow =
        note("aaaa0002", List::Repository, "The integration suite needs the fixtures server running first: `just fixtures`.", "learned");
    slow.session_title = Some("Speed up the nightly suite".into());
    slow.lifetime = Some(30);
    slow.days_left = Some(21);
    Notes {
        repository: Some(REPOSITORY.into()),
        repository_notes: vec![note("aaaa0001", List::Repository, "Rebase onto main; don't merge main in.", "from-you"), slow],
        machine_notes: vec![note("bbbb0001", List::Machine, "/tmp fills up here: put build output under ~/.cache.", "added")],
        tokens: 280,
        expired_notes: vec![],
    }
}

// ─── Notes for the lists' scenes (Component 16, piece Q) ────────────────────

fn learned(id: &str, list: List, text: &str, title: &str, how: Option<&str>, lifetime: Option<i64>, left: Option<i64>) -> Note {
    let mut n = note(id, list, text, "learned");
    n.session_title = Some(title.into());
    n.how = how.map(str::to_string);
    n.lifetime = lifetime;
    n.days_left = left;
    n
}

/// Both lists with every kind of note: from you, added, learned and tested, learned and untested, kept for good, and one
/// that is nearly out of days; and two expired.
pub fn rich_notes() -> Notes {
    let mut tested = learned(
        "aaaa0002",
        List::Repository,
        "The integration suite needs the fixtures server running first: `just fixtures`.",
        "Speed up the nightly suite",
        Some("ran the suite twice, once with the server down"),
        Some(7),
        Some(5),
    );
    tested.updated = "2026-10-14T16:40:00Z".into();
    let mut untested = learned(
        "aaaa0003",
        List::Repository,
        "Snapshots live next to the test file, not in a shared folder.",
        "Fix the flaky snapshot test",
        None,
        Some(7),
        Some(0),
    );
    untested.updated = "2026-10-17T10:05:00Z".into();
    let mut kept = learned(
        "aaaa0004",
        List::Repository,
        "Releases are cut from the `release/` branches only.",
        "Prepare the 2.4 release",
        Some("read the release script"),
        None,
        None,
    );
    kept.relearned = 1;
    let mut from_you = note("aaaa0001", List::Repository, "Rebase onto main; don't merge main in.", "from-you");
    from_you.session_title = Some("Tidy the branch".into());
    from_you.updated = "2026-10-09T08:20:00Z".into();
    let mut machine_learned = learned(
        "bbbb0002",
        List::Machine,
        "The disk is nearly full: delete build folders when a task is done.",
        "Free some space",
        Some("df showed 96% used"),
        Some(7),
        Some(1),
    );
    machine_learned.updated = "2026-10-15T19:30:00Z".into();
    let expired = |id: &str, list: List, text: &str| {
        let mut n = learned(id, list, text, "An older session", None, Some(7), Some(0));
        n.expired = true;
        n.relearned = 1;
        n
    };
    Notes {
        repository: Some(REPOSITORY.into()),
        repository_notes: vec![from_you, tested, untested, kept],
        machine_notes: vec![
            note("bbbb0001", List::Machine, "/tmp fills up here: put build output under ~/.cache.", "added"),
            machine_learned,
        ],
        tokens: 410,
        expired_notes: vec![
            expired("cccc0001", List::Repository, "The linter needs the `--fix` flag in CI."),
            expired("cccc0002", List::Machine, "Port 8080 is taken by another service."),
        ],
    }
}

/// `rich_notes` with "Search older notes" on: the last learned note waits to be found, and one is pinned.
pub fn search_notes() -> (Overview, Notes) {
    let mut overview = overview();
    overview.enabled = true;
    overview.search_enabled = true;
    overview.max_always_repository_notes = Some(12);
    overview.max_always_machine_notes = Some(5);
    overview.max_search_repository_notes = Some(60);
    overview.max_search_machine_notes = Some(25);
    let mut notes = rich_notes();
    notes.expired_notes.clear();
    notes.repository_notes[0].pinned = true;
    notes.repository_notes[2].in_search = true;
    notes.repository_notes[2].found = 3;
    notes.repository_notes[2].last_found = Some("2026-10-18T09:00:00Z".into());
    notes.machine_notes[1].in_search = true;
    notes.machine_notes[1].found = 1;
    notes.machine_notes[1].last_found = Some("2026-10-16T13:00:00Z".into());
    (overview, notes)
}

/// A note with long unbroken text and a long `how`, to see them wrap and cut.
pub fn long_notes() -> Notes {
    let mut notes = rich_notes();
    notes.repository_notes[1].text = "The deploy token is read from DEPLOY_TOKEN_FOR_THE_STAGING_ENVIRONMENT_AND_THE_PREVIEW_ENVIRONMENT_NEVER_FROM_THE_FILE_ON_DISK_ALTHOUGH_THE_OLD_SCRIPT_DID".into();
    notes.repository_notes[1].how = Some(
        "ran the deploy script against staging three times with the token set, unset and set to an empty string, then read how the script chooses between them"
            .into(),
    );
    notes
}

/// What a scene shows of the lists: the page open on invented data with the changes the picture is about.
#[derive(Clone, Copy)]
pub enum Variant {
    Notes,
    Long,
    SearchOn,
    ExpiredOpen,
    Adding,
    Editing,
    Filtered,
    ConfirmClear,
    Empty,
}

pub fn scene(s: &mut crate::State, dark: bool, variant: Variant) {
    use crate::fleet::settings::memory::MemoryPage;
    s.dark = Some(dark);
    s.now_ms = crate::fleet::work::parse_iso_ms("2026-10-20T12:00:00Z");
    s.open_settings("memory", None);
    let key = s.settings.choice.key(&s.machines).to_string();
    let mut o = overview();
    o.enabled = true;
    let mut page = MemoryPage {
        overview: Some(o),
        notes: Some(rich_notes()),
        repository: Some(REPOSITORY.into()),
        loaded_for: Some(key),
        ..Default::default()
    };
    let ui = &mut s.settings.memory_ui;
    *ui = Default::default();
    match variant {
        Variant::Notes => {}
        Variant::Long => page.notes = Some(long_notes()),
        Variant::SearchOn => {
            let (o, n) = search_notes();
            (page.overview, page.notes) = (Some(o), Some(n));
        }
        Variant::ExpiredOpen => ui.show_expired = true,
        Variant::Adding => {
            ui.adding = Some(List::Repository);
            ui.draft = rune::TextEdit::new("Run `just lint` before pushing");
        }
        Variant::Editing => {
            ui.editing = Some("aaaa0003".into());
            ui.edit = rune::TextEdit::new("Snapshots live next to the test file, not in a shared folder.");
        }
        Variant::Filtered => ui.search = rune::TextEdit::new("snapshot"),
        Variant::ConfirmClear => ui.confirming_clear = true,
        Variant::Empty => {
            let mut n = rich_notes();
            (n.repository_notes, n.machine_notes, n.expired_notes, n.tokens) = (vec![], vec![], vec![], 0);
            page.notes = Some(n);
        }
    }
    s.settings.memory = page;
}
