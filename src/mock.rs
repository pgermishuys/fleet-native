//! Invented projects, sessions and conversations. Nothing here comes from a real Fleet.
use crate::view::conversation::{Message, Part, ToolState};

pub mod agents;
pub mod asks;
pub mod browser;
pub mod context;
pub mod context_tab;
pub mod export;
pub mod failure;
pub mod feeds;
pub mod find;
pub mod finder;
pub mod fold;
pub mod frame;
pub mod groups;
pub mod header;
pub mod images;
pub mod links;
pub mod long;
pub mod machines;
pub mod pages;
pub mod parts;
pub mod questions;
pub mod ring;
pub mod rows;
pub mod rules;
pub mod tools;
pub mod update;
pub mod work;
pub mod working;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// A turn is running: the four-dot glyph, and no time label.
    Working,
    /// Waiting on the user: the amber diamond and a "Needs input" pill.
    NeedsInput,
    /// Nothing live: an empty glyph slot and how long ago it was last used.
    Idle,
}

pub struct Session {
    /// The server's id; empty for the invented sessions.
    pub id: String,
    pub title: String,
    pub status: Status,
    /// How long ago an invented session was last used, in seconds (the server's sessions carry `row.updated`).
    pub ago_secs: Option<i64>,
    pub branch: String,
    pub messages: Vec<Message>,
    pub tokens: u32,
    /// What the server's list row carries beyond the glyph; empty for the invented sessions.
    pub row: crate::fleet::rows::Row,
    /// Where the row belongs: its project, pin and lineage ids.
    pub listing: crate::fleet::groups::Listing,
}

impl Session {
    /// The row alone, with no conversation: what another machine's list holds is kept as opened here.
    pub fn row_copy(&self) -> Session {
        Session {
            id: self.id.clone(),
            title: self.title.clone(),
            status: self.status,
            ago_secs: self.ago_secs,
            branch: self.branch.clone(),
            messages: Vec::new(),
            tokens: self.tokens,
            row: self.row.clone(),
            listing: self.listing.clone(),
        }
    }

    /// The session was just used: its age starts again (and an invented session stops being N seconds old).
    pub fn touch(&mut self, now_ms: i64) {
        self.row.updated = now_ms;
        self.ago_secs = None;
    }
}

impl crate::fleet::groups::Listed for Session {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    /// The server's `sessionStatus`; the invented sessions have none, so it is what their glyph means.
    fn status_word(&self) -> &str {
        match (self.row.lifecycle.as_str(), self.status) {
            ("", Status::Working) => "active",
            ("", Status::NeedsInput) => "waiting_input",
            ("", Status::Idle) => "idle",
            (word, _) => word,
        }
    }
    fn listing(&self) -> &crate::fleet::groups::Listing {
        &self.listing
    }
}

pub struct Project {
    /// The server's project id; empty for the group of sessions with no project (`ungrouped`) is `"ungrouped"`.
    pub id: String,
    pub kind: crate::fleet::groups::ProjectKind,
    /// `position` from `GET /api/projects`; the groups order by it.
    pub position: i64,
    pub name: String,
    pub path: String,
    pub open: bool,
    pub sessions: Vec<Session>,
}

fn session(title: &'static str, status: Status, age: &'static str, branch: &'static str, messages: Vec<Message>) -> Session {
    let tokens = 1_840 + 3_217 * messages.len() as u32;
    Session {
        id: String::new(),
        title: title.into(),
        status,
        ago_secs: ago_secs(age),
        branch: branch.into(),
        messages,
        tokens,
        row: Default::default(),
        listing: Default::default(),
    }
}

/// "now", "12m", "3h", "2d" as seconds ago; an empty word is a session with no time.
fn ago_secs(age: &str) -> Option<i64> {
    let n = age.trim_end_matches(|c: char| c.is_alphabetic()).parse::<i64>().unwrap_or(0);
    match age.chars().last()? {
        'm' => Some(n * 60),
        'h' => Some(n * 3600),
        'd' => Some(n * 86_400),
        _ => Some(0),
    }
}

/// What the session slot shows when there is nothing to open (a server with no sessions yet).
pub fn blank_session() -> Session {
    session("No session open", Status::Idle, "", "", Vec::new())
}

/// A short stand-in conversation for the sessions that aren't written out in full.
fn brief(ask: &'static str, answer: &'static str) -> Vec<Message> {
    vec![Message::user("u1", ask), Message::assistant("a1", vec![Part::text("a1-t", answer)])]
}

pub use feeds::feeds;

pub fn projects() -> Vec<Project> {
    let mut projects = vec![
        Project {
            id: "mock-orchard".into(),
            kind: Default::default(),
            position: 1,
            name: "Orchard".into(),
            path: "~/code/orchard".into(),
            open: true,
            sessions: vec![
                session(
                    "Move the image cache to disk",
                    Status::Working,
                    "now",
                    "cache-on-disk",
                    vec![
                        Message::user(
                            "u1",
                            "The image cache lives in memory and we lose it on every restart. Can you move it to disk, and keep the 200 MB limit?",
                        ),
                        Message::assistant(
                            "a1",
                            vec![
                                Part::text(
                                    "a1-t1",
                                    "I'll read how the cache is used first, then swap the store underneath it so callers don't change.",
                                ),
                                Part::tool("a1-r1", "Read", "src/cache/image_cache.ts", ToolState::Plain),
                                Part::tool("a1-r2", "Read", "src/cache/eviction.ts", ToolState::Plain),
                                Part::text(
                                    "a1-t2",
                                    "The cache is one map behind get and put, and eviction already works from a size total. So the change is small: a disk store with the same two calls, an index file for sizes and last use, and eviction that deletes files instead of dropping entries.",
                                ),
                                Part::text("a1-t3", "I'll write the store, move the tests over to a temporary folder, and run them."),
                                Part::tool("a1-e1", "Edit", "src/cache/disk_store.ts", ToolState::Diff(86, 0)),
                                Part::tool("a1-e2", "Edit", "src/cache/image_cache.ts", ToolState::Diff(14, 31)),
                                Part::tool("a1-b1", "Bash", "bun test src/cache", ToolState::Took(4.2)),
                            ],
                        ),
                        Message::user("u2", "Also make sure a half-written file can't be served after a crash."),
                        Message::assistant("a2", vec![Part::running_tool("a2-b1", "Bash", "bun test src/cache --rerun-each 20")]),
                    ],
                ),
                session(
                    "Which plan does the checkout page show?",
                    Status::NeedsInput,
                    "",
                    "main",
                    vec![
                        Message::user("u1", "The checkout page shows the yearly plan first. Should it?"),
                        Message::assistant(
                            "a1",
                            vec![
                                Part::text(
                                    "a1-t1",
                                    "It shows whichever plan comes first in plans.json, and yearly was added at the top last month. Nothing in the code picks one on purpose.",
                                ),
                                Part::text(
                                    "a1-t2",
                                    "I can make monthly the default, or keep yearly first and mark it as the default explicitly. Which do you want?",
                                ),
                            ],
                        ),
                    ],
                ),
                session(
                    "Fix the flaky checkout test",
                    Status::Idle,
                    "12m",
                    "fix-checkout-flake",
                    brief(
                        "checkout.spec fails about one run in ten. Find out why.",
                        "The test reads the total before the discount request returns. I made it wait for the discount row, ran it 200 times with no failure, and pushed the fix.",
                    ),
                ),
                session(
                    "Upgrade the date picker",
                    Status::Idle,
                    "3h",
                    "date-picker-4",
                    brief(
                        "Move us to version 4 of the date picker.",
                        "Done. Two props were renamed and the week now starts from the locale, so I removed our own week-start setting. All 48 tests pass.",
                    ),
                ),
                session(
                    "Tidy the release notes",
                    Status::Idle,
                    "2d",
                    "main",
                    brief(
                        "Group the release notes by area and drop the internal items.",
                        "Grouped into Shop, Accounts and Fixes. I left out nine items that only touched tests or build scripts.",
                    ),
                ),
            ],
        },
        Project {
            id: "mock-lighthouse".into(),
            kind: Default::default(),
            position: 2,
            name: "Lighthouse".into(),
            path: "~/code/lighthouse".into(),
            open: true,
            sessions: vec![
                session(
                    "Why is the build slow on Windows?",
                    Status::Idle,
                    "1h",
                    "main",
                    brief(
                        "The Windows build takes 14 minutes and Linux takes 5. Why?",
                        "Nine of the fourteen minutes are the antivirus scanning node_modules. Excluding the build folder in the runner's settings brings it to about six minutes.",
                    ),
                ),
                session(
                    "Sketch the onboarding copy",
                    Status::Idle,
                    "5h",
                    "onboarding",
                    brief(
                        "Write three short screens of onboarding copy.",
                        "Three screens: what Lighthouse watches, how to add your first site, and where alerts go. Each is one heading and one sentence.",
                    ),
                ),
                session(
                    "Add a dark map style",
                    Status::Idle,
                    "4d",
                    "dark-map",
                    brief(
                        "The map is blinding at night. Add a dark style.",
                        "Added a dark style that follows the app's theme. Roads and labels keep the same contrast as the light one.",
                    ),
                ),
            ],
        },
        Project {
            id: "mock-field-notes".into(),
            kind: Default::default(),
            position: 3,
            name: "Field notes".into(),
            path: "~/notes/field".into(),
            open: false,
            sessions: vec![session(
                "Summarise last week's notes",
                Status::Idle,
                "6d",
                "main",
                brief("Summarise last week's notes in five lines.", "Five lines, one per day you wrote."),
            )],
        },
        // Where the slice A measurements scroll. Folded, so the list looks as it did.
        Project {
            id: "mock-benchmarks".into(),
            kind: Default::default(),
            position: 4,
            name: "Benchmarks".into(),
            path: "~/code/benchmarks".into(),
            open: false,
            sessions: vec![session(long::TITLE, Status::Idle, "1d", "main", long::conversation())],
        },
    ];
    // Each invented session knows its project the way a server row does, so the filter matches on it.
    for (p, project) in projects.iter_mut().enumerate() {
        for (i, session) in project.sessions.iter_mut().enumerate() {
            // The invented sessions get ids of their own so the open one is chosen by id like a server's; the prefix
            // keeps them from ever being sent to a server (`store::is_invented`).
            session.id = crate::fleet::store::invented_id(p, i);
            session.listing.project_id = Some(project.id.clone());
            session.listing.project_name = Some(project.name.clone());
        }
    }
    projects
}

/// What the agent in this prototype answers to anything you send.
pub const CANNED_REPLY: &str =
    "This is a layout prototype, so no agent is listening. The message you sent is drawn by Rune the same way Fleet's client draws it.";

/// Invented usage for the scene with bars on the headings: Claude at 60%, Codex at 97% (resets 2:05 PM today) and
/// Copilot used up (resets in a few hours).
pub fn usage_limits(now: i64) -> crate::fleet::usage::UsageLimits {
    use crate::fleet::usage::{ProviderUsage, Status, UsageLimits, Window};
    let provider = |id: &str, name: &str, utilization: f64, resets_in_min: i64, status: Status| ProviderUsage {
        provider: id.into(),
        name: name.into(),
        windows: vec![Window {
            window: "five_hour".into(),
            utilization: Some(utilization),
            resets_at: Some(now + resets_in_min * 60_000),
            window_seconds: Some(18_000.0),
            status,
        }],
        ..ProviderUsage::default()
    };
    UsageLimits {
        enabled: true,
        providers: vec![
            provider("claude", "Claude", 0.6, 180, Status::Allowed),
            provider("codex", "Codex", 0.97, 125, Status::Allowed),
            provider("copilot", "GitHub Copilot", 0.2, 240, Status::Rejected),
        ],
    }
}

/// The agents and models the invented sessions offer (the pickers' lists when no server is given): two providers, one
/// model with no effort levels, and three with levels so the toggle can be shown.
pub fn catalog() -> crate::fleet::catalog::SessionCatalog {
    use crate::composer::choice::{AgentOption, ModelOption, ModelReference};
    let agent = |name: &str, description: &str, model: Option<(&str, &str)>| AgentOption {
        id: name.into(),
        name: name.into(),
        description: description.into(),
        model: model.map(|(p, m)| ModelReference::new(p, m)),
    };
    let model = |provider: (&str, &str), id: &str, name: &str, variants: &[&str]| {
        let mut m = ModelOption::new(provider.0, provider.1, id, name);
        m.variants = variants.iter().map(|v| v.to_string()).collect();
        m
    };
    let anthropic = ("anthropic", "Anthropic");
    let orchard = ("orchard", "Orchard");
    crate::fleet::catalog::SessionCatalog::loaded(
        vec![
            agent("build", "Edits files and runs commands", Some(("anthropic", "sonnet-5-5"))),
            agent("plan", "Reads the code and proposes a plan", Some(("anthropic", "opus-5-5"))),
            agent("general", "Answers questions about the project", None),
        ],
        vec![
            model(anthropic, "sonnet-5-5", "Sonnet 5.5", &["low", "medium", "high"]),
            model(anthropic, "opus-5-5", "Opus 5.5", &["low", "medium", "high", "xhigh"]),
            model(anthropic, "haiku-5-5", "Haiku 5.5", &[]),
            model(orchard, "pear-2", "Pear 2", &["low", "high"]),
            model(orchard, "quince-1", "Quince 1", &[]),
        ],
    )
}
