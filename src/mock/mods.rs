//! Invented kept mods for Settings → Skills & tools → Mods when the app has no Fleet to ask (and for its scenes and tests).
//! Nothing here is from a real machine.
use crate::fleet::settings::mods_kept::{KeptMod, Off, Switch, Version, View};

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

/// Invented drafts of the open session, one for each case a card shows: on with a request and a note, on, turned off by the user,
/// turned off by three failures, still being written, standing in for a kept v2, with a check problem, and with a request that has
/// no note.
pub fn drafts(session: &str) -> Vec<crate::fleet::mods::drafts::ModDraft> {
    use crate::fleet::mods::drafts::{KeepRequest, ModDraft, Problem};
    let draft = |name: &str, description: Option<&str>| ModDraft {
        session_id: session.into(),
        name: name.into(),
        description: description.map(Into::into),
        version: description.map(|_| "0.1.0".into()),
        ..Default::default()
    };
    let asked = |note: Option<&str>| Some(KeepRequest { note: note.map(Into::into), at: "2026-10-10T10:00:00Z".into() });
    vec![
        ModDraft {
            keep_request: asked(Some("Counts are easier to scan than the raw output.")),
            kept: Some(2),
            ..draft("test-chips", Some("Draws test runs as passed, failed and skipped counts"))
        },
        draft("context-gauge", Some("A gauge of how much of the context is used")),
        ModDraft { off: Some(Off { by: "user".into(), error: None }), ..draft("tool-timer", Some("Shows how long each tool call took")) },
        ModDraft {
            off: Some(Off { by: "strikes".into(), error: Some("Cannot read properties of undefined (reading 'branch')".into()) }),
            ..draft("branch-badge", Some("A badge for the branch the session is on"))
        },
        draft("deploy-notes", None),
        ModDraft {
            problem: Some(Problem { code: "register".into(), message: "Call on() inside register".into(), line: Some(12) }),
            ..draft("run-summary", Some("A summary line under each run"))
        },
        ModDraft { keep_request: asked(None), ..draft("file-chips", Some("Chips for the files a reply names")) },
    ]
}

impl crate::State {
    /// The open session with some of those drafts under its last reply, and Mods on.
    pub fn show_mod_drafts(&mut self, dark: bool, more: bool) {
        self.show_chips(super::context::Case::Chips, dark);
        let session = self.session().id.clone();
        let machine = self.machine_key_of(&session).to_string();
        self.mod_drafts.switches.insert(machine, Switch { on: true, safe_mode: false });
        let pick = if more {
            ["context-gauge", "tool-timer", "deploy-notes", "file-chips"]
        } else {
            ["test-chips", "branch-badge", "run-summary", ""]
        };
        self.mod_drafts.set(&session, drafts(&session).into_iter().filter(|d| pick.contains(&d.name.as_str())).collect());
    }
}
