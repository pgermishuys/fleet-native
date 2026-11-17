//! Invented locations for Settings → Folders when the app has no Fleet to ask (and for its scenes and tests): one root from
//! the environment, one added, and one added whose folder is gone. Nothing here is from a real machine.
use crate::fleet::settings::locations::{Browse, Entry, Root};

pub fn roots() -> Vec<Root> {
    let root = |id: Option<&str>, path: &str, env, exists| Root { id: id.map(str::to_string), path: path.into(), env, exists };
    vec![
        root(None, "/work", true, true),
        root(Some("loc-1"), "/srv/projects", false, true),
        root(Some("loc-2"), "/srv/old-checkouts", false, false),
    ]
}

/// The folders under `path` in an invented tree; none for the machine's roots.
pub fn browse(path: Option<&str>) -> Browse {
    let entry = |parent: &str, name: &str, git| Entry { name: name.into(), path: format!("{parent}/{name}"), git };
    let (current, entries) = match path {
        None => (
            None,
            vec![
                Entry { name: "work".into(), path: "/work".into(), git: false },
                Entry { name: "srv".into(), path: "/srv".into(), git: false },
            ],
        ),
        Some("/work") => {
            (Some("/work"), vec![entry("/work", "lantern", true), entry("/work", "harbor-api", true), entry("/work", "notes", false)])
        }
        Some("/srv") => (Some("/srv"), vec![entry("/srv", "projects", false), entry("/srv", "old-checkouts", false)]),
        Some(other) => (Some(other), vec![]),
    };
    Browse { current: current.map(str::to_string), entries, roots: vec!["/work".into(), "/srv".into()], ..Browse::default() }
}
