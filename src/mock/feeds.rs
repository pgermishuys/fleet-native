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

/// `rocket`'s branches, remotes and default base.
pub fn detail() -> Detail {
    let branch = |name: &str, current: bool, remote: bool| Branch {
        name: name.into(),
        short_hash: "3f9a2c1".into(),
        message: "Tidy the launch checks".into(),
        author: "Ada".into(),
        date: "2026-11-02".into(),
        is_current: current,
        is_remote: remote,
    };
    Detail {
        name: "rocket".into(),
        path: ROCKET.into(),
        branch: "fix-login".into(),
        default_base: "main".into(),
        default_branch: "main".into(),
        branches: vec![branch("main", false, false), branch("fix-login", true, false), branch("origin/main", false, true)],
        remotes: vec![Remote {
            name: "origin".into(),
            url: "https://github.com/ada/rocket.git".into(),
            github: Some(("ada".into(), "rocket".into())),
        }],
    }
}

pub fn worktrees() -> Vec<Worktree> {
    vec![Worktree {
        path: "/home/ada/src/rocket-worktrees/fix-login".into(),
        branch: Some("fix-login".into()),
        commit_hash: "3f9a2c1d".into(),
    }]
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
