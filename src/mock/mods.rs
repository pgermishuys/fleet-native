//! Invented kept mods for Settings → Skills & tools → Mods when the app has no Fleet to ask (and for its scenes and tests).
//! Nothing here is from a real machine.
use crate::fleet::settings::mods_kept::{KeptMod, Off, Version, View};

fn version(number: i64, created_at: &str, from: Option<(&str, &str)>) -> Version {
    Version { number, created_at: created_at.into(), session_id: from.map(|f| f.0.into()), session_title: from.map(|f| f.1.into()) }
}

/// One mod that runs (on its second version, from a session), one the user turned off, one that turned itself off after three
/// failures.
pub fn kept() -> View {
    View {
        safe_mode: false,
        mods: vec![
            KeptMod {
                name: "test-chips".into(),
                description: Some("Draws test runs as passed, failed and skipped counts".into()),
                active: Some(2),
                active_version: Some("0.2.0".into()),
                off: None,
                versions: vec![
                    version(1, "2026-10-08T09:00:00Z", Some(("s-1", "Make test output readable"))),
                    version(2, "2026-10-10T08:30:00Z", Some(("s-2", "Make test output readable"))),
                ],
            },
            KeptMod {
                name: "context-gauge".into(),
                description: Some("A gauge of how much of the context is used".into()),
                active: Some(1),
                active_version: Some("0.1.0".into()),
                off: Some(Off { by: "user".into(), error: None }),
                versions: vec![version(1, "2026-10-09T09:00:00Z", None)],
            },
            KeptMod {
                name: "branch-badge".into(),
                description: Some("A badge for the branch the session is on".into()),
                active: Some(3),
                active_version: Some("0.3.0".into()),
                off: Some(Off { by: "strikes".into(), error: Some("Cannot read properties of undefined (reading 'branch')".into()) }),
                versions: vec![
                    version(1, "2026-10-06T09:00:00Z", Some(("s-3", "Show the branch in the header"))),
                    version(2, "2026-10-07T09:00:00Z", Some(("s-3", "Show the branch in the header"))),
                    version(3, "2026-10-09T10:00:00Z", Some(("s-4", "Show the branch in the header"))),
                ],
            },
        ],
    }
}
