//! Invented data for what the New session page reads (component 7, piece C): repositories under two locations, one
//! repository's branches and worktrees, the harnesses, a profile, the agents and models, the naming. With no server the page
//! draws these, so scenes and tests come out the same every time.
use crate::composer::choice::{AgentOption, ModelOption};
use crate::fleet::new_session::feed_state::{CatalogFeed, CatalogKey, Feeds, HarnessFeed, RepoFeed};
use crate::fleet::new_session::feeds::*;
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses, Load};
use crate::fleet::settings::HOME_MACHINE_KEY;

pub const ROCKET: &str = "/home/ada/src/rocket";

fn repository(name: &str, root: &str) -> Repository {
    Repository { name: name.into(), path: format!("{root}/{name}"), parent_root: root.into() }
}

/// The repositories the invented machine found.
pub fn scan() -> Scan {
    Scan {
        repositories: vec![
            repository("rocket", "/home/ada/src"),
            repository("comet", "/home/ada/src"),
            repository("Aurora-docs", "/home/ada/src"),
            repository("ledger", "/home/ada/work"),
        ],
        scanned_at: Some(1_763_000_000_000),
    }
}

/// `rocket`'s branches, remotes and default base, in the order the server sends them (most recent commit first, a local
/// branch before its `origin/` twin). The hashes are those of the scratch repository the pictures' Fleet client shows.
pub fn detail() -> Detail {
    detail_in("/home/ada/src")
}

/// The same repository under another folder, for scenes that put the native app and Fleet's own client side by side.
pub fn detail_in(root: &str) -> Detail {
    let branch = |name: &str, message: &str, hash: &str, current: bool| Branch {
        name: name.into(),
        short_hash: hash.into(),
        message: message.into(),
        author: String::new(),
        date: "2026-11-02T09:00:00Z".into(),
        is_current: current,
        is_remote: name.starts_with("origin/"),
    };
    Detail {
        name: "rocket".into(),
        path: format!("{root}/rocket"),
        branch: "fix-login".into(),
        default_base: "origin/main".into(),
        default_branch: "main".into(),
        branches: vec![
            branch("fix-login", "Keep the redirect after sign-in", "3f01000", true),
            branch("spike-cache", "Cache the stage plan", "383debe", false),
            branch("origin/spike-cache", "Cache the stage plan", "383debe", false),
            branch("release/2.0", "Bump the fuel gauge to 2.0", "1eecfcc", false),
            branch("origin/release/2.0", "Bump the fuel gauge to 2.0", "1eecfcc", false),
            branch("main", "Tidy the launch checks", "f6b0df7", false),
            branch("origin/main", "Tidy the launch checks", "f6b0df7", false),
        ],
        remotes: vec![Remote {
            name: "origin".into(),
            url: "https://github.com/ada/rocket.git".into(),
            github: Some(("ada".into(), "rocket".into())),
        }],
    }
}

pub fn worktrees() -> Vec<Worktree> {
    worktrees_in("/home/ada/src")
}

/// `git worktree list` for the repository, as the server sends it: a detached one, one beside the repository, two on branches.
pub fn worktrees_in(root: &str) -> Vec<Worktree> {
    let parent = root.rsplit_once('/').map_or("", |(p, _)| p);
    let tree = |path: String, branch: Option<&str>, hash: &str| Worktree { path, branch: branch.map(Into::into), commit_hash: hash.into() };
    vec![
        tree(format!("{parent}/elsewhere/rocket-detached"), None, "f6b0df792767"),
        tree(format!("{root}/rocket-worktrees/release-2-0"), Some("release/2.0"), "1eecfccb9349"),
        tree(format!("{root}/rocket-worktrees/spike-cache"), Some("spike-cache"), "383debe87c6b"),
    ]
}

pub fn harnesses() -> Vec<HarnessEntry> {
    let mut opencode = HarnessEntry::ready("opencode2");
    opencode.display_name = "OpenCode 2".into();
    opencode.is_default = true;
    opencode.supports_profiles = true;
    let mut claude = HarnessEntry::ready("claude-code");
    claude.display_name = "Claude Code".into();
    vec![opencode, claude]
}

pub fn catalog() -> Catalog {
    let agent =
        |name: &str, description: &str| AgentOption { id: name.into(), name: name.into(), description: description.into(), model: None };
    Catalog {
        supported: true,
        agents: vec![agent("build", "Writes code and runs the tools"), agent("plan", "Reads and proposes, changes nothing")],
        models: vec![
            ModelOption::new("anthropic", "Anthropic", "sonnet", "Sonnet"),
            ModelOption::new("anthropic", "Anthropic", "opus", "Opus"),
        ],
        default_agent: Some("build".into()),
        default_model: None,
    }
}

/// Everything read for the page, for the machine called home.
pub fn feeds() -> Feeds {
    let mut f = Feeds::default();
    f.repos.insert(HOME_MACHINE_KEY.into(), RepoFeed::seeded(scan()));
    let mut harness_feed = HarnessFeed::default();
    harness_feed.harnesses = Harnesses { all: harnesses(), load: Load::Done, preferred: None };
    f.harnesses.insert(HOME_MACHINE_KEY.into(), harness_feed);
    let seq = f.detail.begin(Some(ROCKET), false);
    f.detail.finish(seq.unwrap_or_default(), Ok(detail()));
    let seq = f.worktrees.begin(Some(ROCKET), true);
    f.worktrees.finish(seq.unwrap_or_default(), Ok(worktrees()));
    f.profiles.insert(
        (HOME_MACHINE_KEY.into(), "opencode2".into()),
        vec![Profile {
            id: "work".into(),
            name: "Work".into(),
            content: "{\n  \"model\": \"anthropic/sonnet\"\n}".into(),
            is_default: true,
            ..Profile::default()
        }],
    );
    let key = CatalogKey { harness: "opencode2".into(), directory: Some(ROCKET.into()), profile: None };
    let mut cat = CatalogFeed::default();
    cat.select(Some(key.clone()), 0);
    cat.finish(1, 0, Ok(catalog()));
    f.catalogs.insert(HOME_MACHINE_KEY.into(), cat);
    f.roots = vec!["/home/ada/src".into(), "/home/ada/work".into()];
    f.defaults = Some(DirectoryDefaults { first_branch: "main".into(), home: Some("/home/ada".into()) });
    f
}
