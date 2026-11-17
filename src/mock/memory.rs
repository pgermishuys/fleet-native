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
