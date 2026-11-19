//! Fleet's own sample mods (`ModsShowcase.vue`'s `GROUPS`): what each does, as its `mod.json` says it, and where to see it once
//! it is added. The list is hard-coded in the client, so it is here too.

pub struct Sample {
    pub name: &'static str,
    pub description: &'static str,
    pub try_it: &'static str,
}

pub struct Group {
    pub title: &'static str,
    pub samples: &'static [Sample],
}

const fn s(name: &'static str, description: &'static str, try_it: &'static str) -> Sample {
    Sample { name, description, try_it }
}

pub static GROUPS: &[Group] = &[
    Group {
        title: "Tool rows",
        samples: &[
            s("test-chips", "Draws test runs as passed, failed and skipped counts", "Run tests in any session."),
            s("build-summary", "Draws build output as error and warning counts", "Run a build in any session."),
            s("git-glance", "Draws git status and log as counts", "Run git status in any session."),
            s("http-check", "Draws curl responses as a status pill and size", "Run curl -i in any session."),
            s("lint-summary", "Draws ESLint and ruff runs as error and warning counts", "Run ESLint or ruff in any session."),
            s("edit-size", "Draws file writes and patches as lines added and removed", "Edit a file in any session."),
            s("search-hits", "Draws grep and glob results as match and file counts", "Run a grep in any session."),
        ],
    },
    Group {
        title: "Prompts, turns and permission asks",
        samples: &[
            s(
                "branch-context",
                "Tells the model which git branch the session is on when you ask about branches, PRs, pushes or merges",
                "Ask about the branch, a PR or a push in any session.",
            ),
            s(
                "tests-after-edits",
                "When a turn changed files, queues \"Run the tests\" as the next message",
                "Have an agent change a file.",
            ),
            s(
                "push-guard",
                "Refuses the agent's git push while the session is on main, and tells it to push a branch instead",
                "Set Settings \u{2192} What agents can do to Ask, then have an agent push from main.",
            ),
        ],
    },
    Group {
        title: "Buttons and panes",
        samples: &[
            s(
                "press-counter",
                "Counts presses of a button in an opened shell row",
                "Open a shell command's row in any session and press Count a press.",
            ),
            s(
                "release-board",
                "A Release board pane in the + menu, and a button in a shell row that opens it and closes it again",
                "Pick Release board under From mods in a session's + menu.",
            ),
        ],
    },
];
